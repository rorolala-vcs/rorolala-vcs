using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>Where a drag lands in a view: the directory it goes into, and the row that stands for it.</summary>
/// <param name="Into">The directory the dragged entries go into.</param>
/// <param name="Row">
/// What says which row to light — the entry's own key — or nothing where no row stands for the place.
/// </param>
/// <remarks>
/// The row is a key rather than a path because what a row stands for is not always a file: a listing of a plugin's
/// place has rows whose key is the provider's own, and the marking is the view's either way.
/// </remarks>
internal readonly record struct Land(string Into, object? Row);

/// <summary>
/// The drop protocol over one view: whether a drag may land, and what letting go does.
/// </summary>
/// <remarks>
/// One answer for every view that takes files rather than one per view, because the protocol is one thing: a drag
/// is asked whether it may land, the row it would land on is lit by the view, and letting go hands the work to the
/// file operations. What differs between a listing and a tree is only where their rows are and which of them may
/// be landed on, and each view answers that for itself (<see cref="Listing{TItem}.Landing"/>).
/// </remarks>
internal sealed class Drops
{
    /// <summary>The host, for what cannot be done.</summary>
    private readonly IPluginHost _host;

    /// <summary>The location a read is said through, since every location reads its directory again.</summary>
    private readonly Browser _browser;

    /// <summary>Takes the light off the row a drag would have landed on.</summary>
    private readonly Action<object?> _mark;

    /// <summary>Sets the protocol up over one view.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">The location a read is to be said through.</param>
    /// <param name="mark">Takes the light off the row a drag would have landed on.</param>
    public Drops(IPluginHost host, Browser browser, Action<object?> mark)
    {
        _host = host;
        _browser = browser;
        _mark = mark;
    }

    /// <summary>
    /// What letting go where a drag would land would do.
    /// </summary>
    /// <remarks>
    /// Asked rather than decided by the caller, because it is the same answer for every view that takes files: a
    /// drag with <c>Ctrl</c> held copies and one without moves, which is what the platform is told and what the
    /// source reads back.
    /// </remarks>
    /// <param name="e">The drag.</param>
    /// <returns>What letting go here would do.</returns>
    public static DragDropEffects Over(DragEventArgs e) =>
        e.KeyModifiers.HasFlag(KeyModifiers.Control) ? DragDropEffects.Copy : DragDropEffects.Move;

    /// <summary>Takes a drag off the view: nothing is lit any more.</summary>
    public void Off() => _mark(null);

    /// <summary>
    /// Lets a drag go: what it brought goes into the directory it was over.
    /// </summary>
    /// <remarks>
    /// A drag this program started is answered here too, so a drag between two of its docks never leaves it, and
    /// the source is told by <see cref="Drag.Answered"/> not to remove the originals again.
    /// </remarks>
    /// <param name="land">Where it was let go, or nothing where it was let go nowhere.</param>
    /// <param name="e">The drop.</param>
    public async void Dropped(Land? land, DragEventArgs e)
    {
        _mark(null);

        if (land is not { } where)
        {
            return;
        }

        var into = where.Into;

        // Told before the work is waited for: what the platform is owed an answer to cannot wait for a question
        // the file operations may have to put to a person.
        Drag.Answered = true;
        e.Handled = true;

        var copy = e.KeyModifiers.HasFlag(KeyModifiers.Control) || e.DragEffects == DragDropEffects.Copy;

        // Into the directory it is already in is a move that would only rename it, or a copy that makes a
        // second one. Neither is what letting go inside one directory means, so neither is offered.
        var paths = Paths(e.DataTransfer)
            .Where(path => !string.Equals(ParentOf(path), into, StringComparison.Ordinal))
            .ToArray();

        if (paths.Length == 0)
        {
            return;
        }

        if (copy)
        {
            await FileOps.Copy(paths, into, _host.Log.Error);
        }
        else
        {
            await FileOps.Move(paths, into, _host.Log.Error);
        }

        Again(_browser);
    }

    /// <summary>
    /// Says the files may have changed, to be read again not before this event is over.
    /// </summary>
    /// <remarks>
    /// Every location's directory and not only one's, because the operation that has just finished may have
    /// changed a directory another dock is the one showing: a move is answered by the dock it was dropped on,
    /// and the entries may have been dragged out of another dock — which, a dock being able to be out of step,
    /// may be looking at a directory of its own. Deferring is the other half of it: reading a directory again
    /// rebuilds the views showing it, and one of them may be the view answering the event.
    /// </remarks>
    /// <param name="through">A location to say it through, since every location reads its directory again.</param>
    private static void Again(Browser through) => Dispatcher.UIThread.Post(through.Touch);

    /// <summary>The paths a drag carries, whether it brought them as files or as text.</summary>
    /// <param name="data">What the drag carries.</param>
    private static IReadOnlyList<string> Paths(IDataTransfer data)
    {
        var files = data.TryGetFiles();
        var fromFiles = files?
            .Select(file => file.TryGetLocalPath())
            .Where(path => path is { Length: > 0 })
            .Select(path => FileOps.Bare(path!))
            .ToArray() ?? [];

        if (fromFiles.Length > 0)
        {
            return fromFiles;
        }

        var text = data.TryGetText();

        return text is null
            ? []
            : text.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(FileOps.Bare)
                .Where(path => File.Exists(path) || Directory.Exists(path))
                .ToArray();
    }

    /// <summary>The directory an entry sits in, for telling a move that would go nowhere.</summary>
    /// <param name="path">The path to ask about.</param>
    private static string ParentOf(string path) => Path.GetDirectoryName(FileOps.Bare(path)) ?? string.Empty;
}

/// <summary>
/// The other half of the protocol: handing entries to the platform as a drag.
/// </summary>
/// <remarks>
/// One answer for every view that can be dragged from, for the reason the drop side is one answer: what a drag
/// carries and what finishing it owes do not depend on how the rows are drawn.
/// </remarks>
internal static class Drag
{
    /// <summary>
    /// How far the pointer moves with the button held before it drags or frames rather than clicks.
    /// </summary>
    /// <remarks>
    /// Both gestures begin at the same press, so both need the same slack before they may begin: a press that has
    /// moved a pixel is still a click in the hand, and one that has moved this far is not.
    /// </remarks>
    public const double Slip = 4;

    /// <summary>
    /// The format the object a source made travels under, within this program.
    /// </summary>
    /// <remarks>
    /// In-process on purpose: what a logical place takes is an object of the source's own making, which no
    /// platform can serialize and no other program has a word for. A drag that leaves this program still carries
    /// its files and their paths, and this is the extra thing this program's own views read to decide whether a
    /// place can take it — which is why the platform is asked to keep it out of its own negotiation.
    /// </remarks>
    public static readonly DataFormat<LogicalDrag> Payload =
        DataFormat.CreateInProcessFormat<LogicalDrag>("rorolala.logical-drag");

    /// <summary>
    /// Whether a drop of this program's own answered the drag in flight.
    /// </summary>
    /// <remarks>
    /// It is what tells a moved-out drag from a moved-in one: a drop answered by one of this program's views has
    /// already dealt with what was carried, so the source must not take the originals away as well. Held for the
    /// whole program rather than by a view, because the drop is answered by the view it landed on and the removal
    /// is the source's — which is a view of its own, and may be another dock's (Section 7.7).
    /// </remarks>
    public static bool Answered { get; set; }

    /// <summary>
    /// Hands entries over as a drag, and finishes what that leaves unfinished.
    /// </summary>
    /// <remarks>
    /// The files are offered themselves as well as their paths written out, so that a file manager receives them
    /// as files and a text field as text. What the platform does with the drag is its own business: this hands
    /// over the data and waits to be told what became of it. A move another program made is a
    /// move this program has to finish, the other program having only copied the files it was handed; a move one
    /// of this program's own docks answered has already moved them, which is what <see cref="Answered"/> records —
    /// taking them away again would delete what was just carried.
    /// </remarks>
    /// <param name="through">A location to say a read through, since every location reads its directory again.</param>
    /// <param name="from">The press the drag begins at, which is what the platform is handed.</param>
    /// <param name="carrying">What to carry, in the order the view shows it.</param>
    /// <param name="lead">The entry the drag took hold of, whose picture the card carries.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public static async void Away(
        Browser through,
        PointerPressedEventArgs from,
        IReadOnlyList<Entry> carrying,
        Entry lead,
        Action<string> failed
    )
    {
        if (TopLevel.GetTopLevel(from.Source as Visual)?.StorageProvider is not { } storage)
        {
            return;
        }

        using var transfer = new DataTransfer();

        foreach (var entry in carrying)
        {
            var item = entry.Kind == EntryKind.Directory
                ? (IStorageItem?)await storage.TryGetFolderFromPathAsync(entry.Path)
                : await storage.TryGetFileFromPathAsync(entry.Path);

            if (item is not null)
            {
                transfer.Add(DataTransferItem.CreateFile(item));
            }
        }

        // The paths are what this program's own object says, and what a logical place is offered: the contract rubs
        // the type out at the boundary and the source hands over what it made, and what a file drag says is where
        // the files are — the same words the text carries, since a place that takes one takes a file's name.
        var said = string.Join(Environment.NewLine, carrying.Select(entry => entry.Path));

        transfer.Add(DataTransferItem.CreateText(said));
        transfer.Add(DataTransferItem.Create(Payload, new LogicalDrag(said)));

        Drag.Answered = false;
        Ghost.Carrying = lead;

        try
        {
            var effect = await DragDrop.DoDragDropAsync(from, transfer, DragDropEffects.Move | DragDropEffects.Copy);

            if (effect == DragDropEffects.Move && !Drag.Answered)
            {
                await FileOps.Remove([.. carrying], failed);

                // Said here and not left to the drop, because the drop that moved them was another program's: the
                // entries were removed by this one, and the listing that held them has to be read again.
                Dispatcher.UIThread.Post(through.Touch);
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }

        Ghost.Carrying = null;
    }
}

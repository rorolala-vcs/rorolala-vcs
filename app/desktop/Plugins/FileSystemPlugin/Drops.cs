using Avalonia.Input;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>Where a drag lands in a view: the directory it goes into, and the row that stands for it.</summary>
/// <param name="Into">The directory the dragged entries go into.</param>
/// <param name="Row">The path of the row to light, or nothing where no row stands for the place.</param>
internal readonly record struct Land(string Into, string Row);

/// <summary>
/// The drop protocol over one view: whether a drag may land, what is lit while it is over, and what letting go
/// does.
/// </summary>
/// <remarks>
/// One answer for every view rather than one per view, because the protocol is one thing: a drag is asked
/// whether it may land, the row it would land on is lit, and letting go hands the work to the agent. What
/// differs between a listing and a tree is only where their rows are and which of them may be landed on, and each
/// view answers that for itself.
/// </remarks>
internal sealed class Drops
{
    /// <summary>
    /// Whether a drop of this program's own answered the drag in flight.
    /// </summary>
    /// <remarks>
    /// It is what tells a moved-out drag from a moved-in one: a drop answered here has already moved the files,
    /// so the source must not take the originals away as well. Held for the whole program rather than by a view,
    /// because the drop is answered by the view it landed on and the removal is the source's — which is a view of
    /// its own, and may be another dock's (Section 7.7).
    /// </remarks>
    public static bool Answered { get; set; }

    /// <summary>The host, for what cannot be done.</summary>
    private readonly IPluginHost _host;

    /// <summary>The location a read is said through, since every location reads its directory again.</summary>
    private readonly Browser _browser;

    /// <summary>Where a drag at a position would land in this view, or nothing where it may not land here.</summary>
    private readonly Func<DragEventArgs, Land?> _landing;

    /// <summary>Lights the row a drag would land on, or takes the light off.</summary>
    private readonly Action<string?> _mark;

    /// <summary>Sets the protocol up over one view.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">The location a read is to be said through.</param>
    /// <param name="landing">Where a drag at a position would land, or nothing where it may not land here.</param>
    /// <param name="mark">Lights the row a drag would land on, or takes the light off.</param>
    public Drops(
        IPluginHost host,
        Browser browser,
        Func<DragEventArgs, Land?> landing,
        Action<string?> mark
    )
    {
        _host = host;
        _browser = browser;
        _landing = landing;
        _mark = mark;
    }

    /// <summary>
    /// Takes a drag over the view: lights what it would land on, and answers whether it may land.
    /// </summary>
    /// <param name="e">The drag.</param>
    /// <returns>What letting go here would do.</returns>
    public DragDropEffects Over(DragEventArgs e)
    {
        if (_landing(e) is not { } land)
        {
            _mark(null);

            return DragDropEffects.None;
        }

        _mark(land.Row);

        return Copying(e) ? DragDropEffects.Copy : DragDropEffects.Move;
    }

    /// <summary>Takes a drag off the view: nothing is lit any more.</summary>
    public void Off() => _mark(null);

    /// <summary>
    /// Lets a drag go: what it brought goes into the directory it was over.
    /// </summary>
    /// <remarks>
    /// A drag this program started is answered here too, so a drag between two of its docks never leaves it, and
    /// the source is told by <see cref="Answered"/> not to remove the originals again.
    /// </remarks>
    /// <param name="e">The drop.</param>
    public async void Dropped(DragEventArgs e)
    {
        _mark(null);

        if (_landing(e) is not { } land)
        {
            return;
        }

        var into = land.Into;

        // Told before the work is waited for: what the platform is owed an answer to cannot wait for a question
        // the agent may have to put to a person.
        Answered = true;
        e.Handled = true;

        var copy = Copying(e) || e.DragEffects == DragDropEffects.Copy;

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

        Later();
    }

    /// <summary>
    /// Says the files may have changed, to be read again not before this event is over.
    /// </summary>
    /// <remarks>
    /// Every location's directory and not only one's, because the operation that has just finished may have
    /// changed a directory this dock is not the one showing: a move is answered by the dock it was dropped on,
    /// and the entries may have been dragged out of another dock — which, a dock being able to be out of step,
    /// may be looking at a directory of its own. Deferring is the other half of it: reading a directory again
    /// rebuilds the views showing it, and one of them may be the view answering the event.
    /// </remarks>
    private void Later() => Dispatcher.UIThread.Post(_browser.Touch);

    /// <summary>Whether the drag is one to copy rather than to move, which is what holding a control asks for.</summary>
    /// <param name="e">The drag.</param>
    private static bool Copying(DragEventArgs e) => e.KeyModifiers.HasFlag(KeyModifiers.Control);

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
                .Where(path => File.Exists(path) || System.IO.Directory.Exists(path))
                .ToArray();
    }

    /// <summary>The directory an entry sits in, for telling a move that would go nowhere.</summary>
    /// <param name="path">The path to ask about.</param>
    private static string ParentOf(string path) => Path.GetDirectoryName(FileOps.Bare(path)) ?? string.Empty;
}

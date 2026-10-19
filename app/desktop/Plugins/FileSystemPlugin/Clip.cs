using System.Diagnostics.CodeAnalysis;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Input.Platform;
using Avalonia.Platform.Storage;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// What a copy or a cut has put within reach of a paste, and which of it a paste is to move.
/// </summary>
/// <remarks>
/// The paths go on the <em>system</em> clipboard rather than into a list of this plugin's own, so that the
/// browser is one program among several: a copy made here is a copy another file manager can paste, and a
/// copy taken there is one this can paste. What the system clipboard cannot say is whether a copy is a cut,
/// so that one thing <em>is</em> this plugin's — the set of paths that are on their way out — and it is what
/// fades those entries until the paste that moves them.
/// <para>
/// The paths are also kept here, because a paste must work even where the system clipboard does not carry
/// files back: a platform whose clipboard is text-only, or one that will not hand its own selection back to
/// the program that set it, would otherwise make a copy and a paste silently do nothing. The system
/// clipboard is read first, so that a copy taken elsewhere is the one pasted; this is the fallback that
/// keeps a copy and a paste a pair inside the program whatever the platform does.
/// </para>
/// <para>
/// One of these belongs to the plugin, not to a dock: two directory docks are two views of one location, and
/// a copy made in one is a copy the other can paste.
/// </para>
/// </remarks>
internal sealed class Clip
{
    /// <summary>Where what a paste is about to do is said.</summary>
    private readonly ILog _log;

    /// <summary>Makes a clipboard over the log a paste says what it found in.</summary>
    /// <param name="log">Where a paste says what it read and where it is putting it.</param>
    public Clip(ILog log) => _log = log;

    /// <summary>The paths a cut has offered to a paste, until that paste happens.</summary>
    private readonly HashSet<string> _moving = new(StringComparer.Ordinal);

    /// <summary>The paths the last copy or cut offered, for a paste where the system clipboard gives none.</summary>
    private readonly List<string> _held = [];

    /// <summary>Raised when what is cut changes, so that the views can redraw faded and unfaded entries.</summary>
    public event Action? Changed;

    /// <summary>Whether an entry is one a cut has offered to a paste.</summary>
    /// <param name="path">The entry's path.</param>
    public bool IsCut(string path) => _moving.Contains(path);

    /// <summary>
    /// How faded an entry is drawn while it is cut, so that it reads as on its way out.
    /// </summary>
    /// <remarks>
    /// A constant rather than a number written where it is used, because a listing and a tree both draw entries, and
    /// two answers to how faded a cut one is would be two answers to one question. Read as a constant rather than
    /// through an instance because a view of a place has a clipboard of its own (see <see cref="LogicalClip"/>) and
    /// no file clipboard to ask.
    /// </remarks>
    public const double FadedAmount = 0.45;

    /// <summary>
    /// The same answer through an instance, for the views that hold a file clipboard.
    /// </summary>
    /// <remarks>
    /// A control's own <c>Clip</c> — the geometry one draws with — is what the name <c>Clip</c> means there, so a
    /// view cannot reach the constant by the type's name: it reads it through the clipboard instance it holds,
    /// which is why this stays a member of one.
    /// </remarks>
    [SuppressMessage(
        "Performance",
        "CA1822:Mark members as static",
        Justification = "a control's own `Clip` is the geometry property of that name, so a view cannot spell `Clip.FadedAmount`; the view reads it through the clipboard instance it holds, which is why it stays on one"
    )]
    public double Faded => FadedAmount;

    /// <summary>Puts entries on the clipboard to be copied, and takes back any earlier cut.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What to copy.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public void Copy(Control from, IReadOnlyList<Entry> entries, Action<string> failed)
    {
        Offer(from, entries, cut: false, failed);
    }

    /// <summary>Puts entries on the clipboard to be moved, and remembers which they are.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What to cut.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public void Cut(Control from, IReadOnlyList<Entry> entries, Action<string> failed)
    {
        Offer(from, entries, cut: true, failed);
    }

    /// <summary>
    /// Copies or moves what is on the clipboard into a directory.
    /// </summary>
    /// <remarks>
    /// An entry that this plugin cut is moved; everything else is copied. That is how a cut is told from a
    /// copy across processes, where the system clipboard carries only the paths: the paths are this plugin's
    /// to compare, and a paste of any of them ends the cut for all of them, which is what a paste of a cut
    /// means.
    /// </remarks>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="into">The directory to put them in.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <param name="done">Run once something was pasted, so that a view can read the directory again.</param>
    public async void Paste(Control from, string into, Action<string> failed, Action done)
    {
        try
        {
            // Read in a try of its own, because a clipboard another program is holding open is not a paste that failed:
            // what this plugin offered is still known, and the two are one offer (see `_held`). A read that throws is
            // therefore answered as giving nothing, rather than as ending the paste before the fallback is reached.
            var paths = await OfferedBySystem(from);

            // The system clipboard first, so that a copy taken in another program is pasted; this plugin's
            // own record only where it gave nothing, so that a copy and a paste made here still pair up.
            if (paths.Count == 0)
            {
                paths = [.. _held];
            }

            if (paths.Count == 0)
            {
                // Said, and the offer is left as it stands rather than cleared: nothing was pasted, and a cut that
                // is still on a clipboard this program cannot read is still a cut.
                _log.Info("paste: nothing readable on the clipboard");

                return;
            }

            var pasted = false;

            // A **cut** into the directory an entry already sits in is nothing to do, and one that was asked
            // for anyway must not rename it out from under the user. A copy there is a different thing: it
            // means "one more of this here", so it stays in and the run is left to put it beside the first.
            var moving = paths
                .Where(path => IsCut(path) && !string.Equals(Holding(path), into, StringComparison.Ordinal))
                .ToArray();
            var copying = paths.Where(path => !IsCut(path)).ToArray();

            // Said before anything is done with them, so that a paste that does nothing reads as a paste that
            // had nothing to do rather than as a key that was never pressed.
            _log.Info($"paste into `{into}`: {moving.Length} to move, {copying.Length} to copy");

            // One call each, so that a question about a taken name is asked once for the batch rather than
            // once per item.
            if (moving.Length > 0)
            {
                pasted |= await FileOps.Move(moving, into, failed);
            }

            if (copying.Length > 0)
            {
                pasted |= await FileOps.Copy(copying, into, failed);
            }

            Offered([], cut: false);

            if (pasted)
            {
                done();
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }
    }

    /// <summary>
    /// The paths the system clipboard holds, or nothing when it cannot be read.
    /// </summary>
    /// <remarks>
    /// Kept apart from <see cref="Paths"/> so that a read that throws is one answer among others rather than the end of
    /// the paste: the platform's clipboard is another program's to hold open, and one that refuses for a moment is not a
    /// copy this program cannot paste. What could not be read is said in the log, because a paste that falls back on the
    /// plugin's own offer and a paste with nothing to paste are otherwise the same from the outside.
    /// </remarks>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <returns>The paths, or nothing when the clipboard gave none.</returns>
    private async Task<IReadOnlyList<string>> OfferedBySystem(Control from)
    {
        try
        {
            return TopLevel.GetTopLevel(from)?.Clipboard is { } clipboard
                ? await Paths(clipboard)
                : [];
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Info($"paste: the clipboard could not be read, so what this program offered is pasted instead: {error.Message}");

            return [];
        }
    }

    /// <summary>
    /// Records what a copy or a cut offered, hands it to the system clipboard, and redraws the cut state.
    /// </summary>
    /// <remarks>
    /// Recorded before the clipboard is written, so that a clipboard that refuses the files still leaves a
    /// copy and a paste that pair up inside the program, and so that a cut is faded the moment it is made
    /// rather than when the system answers.
    /// </remarks>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What was offered.</param>
    /// <param name="cut">Whether they are to be moved rather than copied.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private void Offer(Control from, IReadOnlyList<Entry> entries, bool cut, Action<string> failed)
    {
        Offered(entries, cut);
        Hand(from, entries, failed);
    }

    /// <summary>Remembers what is on offer, and what of it is to move.</summary>
    /// <param name="entries">What was offered.</param>
    /// <param name="cut">Whether they are to be moved rather than copied.</param>
    private void Offered(IReadOnlyList<Entry> entries, bool cut)
    {
        _held.Clear();
        _moving.Clear();

        foreach (var entry in entries)
        {
            _held.Add(entry.Path);

            if (cut)
            {
                _moving.Add(entry.Path);
            }
        }

        Changed?.Invoke();
    }

    /// <summary>
    /// Hands entries to the system clipboard as files and as their paths written out.
    /// </summary>
    /// <remarks>
    /// Both, because the two answer different readers: a file manager takes the files, and a text field or a
    /// script takes the text. A path that the system will not turn into a file — one that has gone since the
    /// listing was read — is left out of the files rather than stopping the copy, since the text still names
    /// it and the rest of the copy is still worth making.
    /// </remarks>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What to hand over.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static async void Hand(Control from, IReadOnlyList<Entry> entries, Action<string> failed)
    {
        try
        {
            var top = TopLevel.GetTopLevel(from);
            var clipboard = top?.Clipboard;
            var storage = top?.StorageProvider;

            if (clipboard is null || storage is null)
            {
                return;
            }

            using var transfer = new DataTransfer();

            foreach (var entry in entries)
            {
                var item = entry.Kind == EntryKind.Directory
                    ? (IStorageItem?)await storage.TryGetFolderFromPathAsync(entry.Path)
                    : await storage.TryGetFileFromPathAsync(entry.Path);

                if (item is not null)
                {
                    transfer.Add(DataTransferItem.CreateFile(item));
                }
            }

            transfer.Add(DataTransferItem.CreateText(string.Join(Environment.NewLine, entries.Select(entry => entry.Path))));

            await clipboard.SetDataAsync(transfer);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }
    }

    /// <summary>
    /// The paths on the clipboard, whether they were left there as files or as text.
    /// </summary>
    /// <remarks>
    /// Files first, since that is what a file manager puts there; text second, so that paths copied out of a
    /// terminal paste as paths and not as nothing. Only text that names something is taken, so that pasting a
    /// sentence does not make a run of empty names.
    /// </remarks>
    /// <param name="clipboard">The clipboard to read.</param>
    private static async Task<IReadOnlyList<string>> Paths(IClipboard clipboard)
    {
        var files = await clipboard.TryGetFilesAsync();
        var fromFiles = files?
            .Select(file => file.TryGetLocalPath())
            .Where(path => path is { Length: > 0 })
            .Select(path => FileOps.Bare(path!))
            .ToArray() ?? [];

        if (fromFiles.Length > 0)
        {
            return fromFiles;
        }

        var text = await clipboard.TryGetTextAsync();

        return text is null
            ? []
            : text.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(FileOps.Bare)
                .Where(path => File.Exists(path) || Directory.Exists(path))
                .ToArray();
    }

    /// <summary>The directory a pasted path sits in, or nothing where its path has no directory.</summary>
    /// <param name="path">The path to ask about.</param>
    private static string Holding(string path) => Path.GetDirectoryName(FileOps.Bare(path)) ?? string.Empty;
}

/// <summary>
/// What a place's entries were copied or cut to, within this program.
/// </summary>
/// <remarks>
/// A place's entries are not files, so what a copy of them is cannot be the system clipboard: there is nothing a file
/// manager could do with one, and no path to hand a program. What it is instead is what the same views read a drag
/// under, kept until a paste takes it — a cut fades the rows it holds for the same reason a cut file does, so that it
/// reads as on its way out.
/// <para>
/// One of these belongs to the plugin rather than to a dock, for the same reason the file clipboard does: a cut made in
/// one dock is one another dock can paste.
/// </para>
/// </remarks>
internal sealed class LogicalClip
{
    /// <summary>What was copied or cut, as the sources made them.</summary>
    private readonly List<object> _kept = [];

    /// <summary>Whether what is kept is a cut, which a paste takes away.</summary>
    private bool _cut;

    /// <summary>Raised when what is on offer changes, which is when the rows of a place are drawn again.</summary>
    public event Action? Changed;

    /// <summary>
    /// Whether something kept here is on its way out, which is what a row is faded for.
    /// </summary>
    /// <param name="data">What a row carries.</param>
    /// <returns>Whether it was cut.</returns>
    public bool IsCut(object data)
    {
        ArgumentNullException.ThrowIfNull(data);

        return _cut && _kept.Contains(data);
    }

    /// <summary>Puts what a place holds on offer, to be copied or moved by a paste.</summary>
    /// <param name="items">What each chosen entry carries.</param>
    /// <param name="cut">Whether they are to be moved rather than copied.</param>
    public void Put(IReadOnlyList<object> items, bool cut)
    {
        ArgumentNullException.ThrowIfNull(items);

        _kept.Clear();
        _kept.AddRange(items);
        _cut = cut;

        Changed?.Invoke();
    }

    /// <summary>What is on offer, if anything is, without ending the offer.</summary>
    /// <remarks>
    /// Read before anything is done with it, because whether a paste means something is the place's answer: a paste
    /// into somewhere that refuses it is a paste that did nothing, and a cut that was taken away by it would be lost
    /// with no word — the rows would come back unfaded and the entries would be nowhere.
    /// </remarks>
    /// <returns>What was kept, or nothing when nothing is.</returns>
    public IReadOnlyList<object>? Peek() => _kept.Count == 0 ? null : [.. _kept];

    /// <summary>Ends a cut once a paste has made something of it, and leaves a copy as it is.</summary>
    /// <remarks>
    /// A copy stays on offer because a copy is what may be pasted again; a cut is gone because the paste that took it
    /// is the move of what it named, and one faded row that has already moved would say it is still on its way out.
    /// </remarks>
    public void Took()
    {
        if (!_cut)
        {
            return;
        }

        _kept.Clear();
        _cut = false;

        Changed?.Invoke();
    }
}


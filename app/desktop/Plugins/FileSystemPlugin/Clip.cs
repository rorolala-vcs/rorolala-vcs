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
    /// A property rather than a constant of the class, because a control's own <c>Clip</c> — the geometry one
    /// draws with — is what the name <c>Clip</c> means there, so a view cannot reach a constant of this one by
    /// name. Kept here rather than by a view because a listing and a tree both draw entries, and two answers to
    /// how faded a cut one is would be two answers to one question.
    /// </remarks>
    public double Faded => 0.45;

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
            var clipboard = TopLevel.GetTopLevel(from)?.Clipboard;
            var paths = clipboard is null ? [] : await Paths(clipboard);

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
            // means "one more of this here", so it stays in and the agent is left to put it beside the first.
            var moving = paths
                .Where(path => IsCut(path) && !string.Equals(Holding(path), into, StringComparison.Ordinal))
                .ToArray();
            var copying = paths.Where(path => !IsCut(path)).ToArray();

            // Said before anything is done with them, so that a paste that does nothing reads as a paste that
            // had nothing to do rather than as a key that was never pressed (Section 7.7).
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

            var transfer = new DataTransfer();

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

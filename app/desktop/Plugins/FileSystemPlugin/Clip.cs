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
/// The paths go on the <em>system</em> clipboard rather than into a list of this plugin's own, so that
/// the browser is one program among several: a copy made here is a copy another file manager can paste,
/// and a copy taken there is one this can paste. What the system clipboard cannot say is whether a copy
/// is a cut, so that one thing <em>is</em> this plugin's — the set of paths that are on their way out —
/// and it is what fades those entries until the paste that moves them.
/// <para>
/// One of these belongs to the plugin, not to a dock: two directory docks are two views of one
/// location, and a copy made in one is a copy the other can paste.
/// </para>
/// </remarks>
internal sealed class Clip
{
    /// <summary>The paths a cut has offered to a paste, until that paste happens.</summary>
    private readonly HashSet<string> _moving = new(StringComparer.Ordinal);

    /// <summary>Raised when what is cut changes, so that the views can redraw faded and unfaded entries.</summary>
    public event Action? Changed;

    /// <summary>Whether an entry is one a cut has offered to a paste.</summary>
    /// <param name="path">The entry's path.</param>
    public bool IsCut(string path) => _moving.Contains(path);

    /// <summary>Puts entries on the clipboard to be copied, and takes back any earlier cut.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What to copy.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public void Copy(Control from, IReadOnlyList<Entry> entries, Action<string> failed)
    {
        Uncut();
        Offer(from, entries, failed);
    }

    /// <summary>Puts entries on the clipboard to be moved, and remembers which they are.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What to cut.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public void Cut(Control from, IReadOnlyList<Entry> entries, Action<string> failed)
    {
        Uncut();

        foreach (var entry in entries)
        {
            _moving.Add(entry.Path);
        }

        Changed?.Invoke();
        Offer(from, entries, failed);
    }

    /// <summary>
    /// Copies or moves what is on the clipboard into a directory.
    /// </summary>
    /// <remarks>
    /// An entry that this plugin cut is moved; everything else is copied. That is how a cut is told from
    /// a copy across processes, where the system clipboard carries only the paths: the paths are this
    /// plugin's to compare, and a paste of any of them ends the cut for all of them, which is what a
    /// paste of a cut means.
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

            if (clipboard is null)
            {
                return;
            }

            var paths = await Paths(clipboard);
            var pasted = false;

            foreach (var path in paths)
            {
                // A paste into the directory an entry already sits in is nothing to do, and one that
                // was asked for anyway must not rename it out from under the user.
                if (string.Equals(Holding(path), into, StringComparison.Ordinal))
                {
                    continue;
                }

                if (IsCut(path))
                {
                    FileOps.Move(path, into, failed);
                }
                else
                {
                    FileOps.Copy(path, into, failed);
                }

                pasted = true;
            }

            Uncut();

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

    /// <summary>Forgets what was cut, so that nothing is faded and nothing moves.</summary>
    private void Uncut()
    {
        if (_moving.Count > 0)
        {
            _moving.Clear();
            Changed?.Invoke();
        }
    }

    /// <summary>
    /// Hands entries to the system clipboard as files and as their paths written out.
    /// </summary>
    /// <remarks>
    /// Both, because the two answer different readers: a file manager takes the files, and a text field
    /// or a script takes the text. A path that the system will not turn into a file — one that has gone
    /// since the listing was read — is left out of the files rather than stopping the copy, since the
    /// text still names it and the rest of the copy is still worth making.
    /// </remarks>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What to hand over.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static async void Offer(Control from, IReadOnlyList<Entry> entries, Action<string> failed)
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
    /// Files first, since that is what a file manager puts there; text second, so that paths copied out
    /// of a terminal paste as paths and not as nothing. Only text that names something is taken, so that
    /// pasting a sentence does not make a run of empty names.
    /// </remarks>
    /// <param name="clipboard">The clipboard to read.</param>
    private static async Task<IReadOnlyList<string>> Paths(IClipboard clipboard)
    {
        var files = await clipboard.TryGetFilesAsync();
        var fromFiles = files?
            .Select(file => file.TryGetLocalPath())
            .Where(path => path is { Length: > 0 })
            .Select(path => path!)
            .ToArray() ?? [];

        if (fromFiles.Length > 0)
        {
            return fromFiles;
        }

        var text = await clipboard.TryGetTextAsync();

        return text is null
            ? []
            : text.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Where(path => File.Exists(path) || Directory.Exists(path))
                .ToArray();
    }

    /// <summary>The directory a pasted path sits in, or nothing where its path has no directory.</summary>
    /// <param name="path">The path to ask about.</param>
    private static string Holding(string path) => Path.GetDirectoryName(path) ?? string.Empty;
}

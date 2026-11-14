using System.Diagnostics;
using Avalonia.Controls;
using Avalonia.Input.Platform;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// What the browser does to an entry: opens it, reveals where it is, or names it.
/// </summary>
/// <remarks>
/// Opening here is the operating system's. A file is opened only after the host's open pipeline has
/// allowed it, which is the browser's step rather than this one: what this class does is the launch
/// itself, and the answer says whether the system took it.
/// </remarks>
internal static class Openers
{
    /// <summary>
    /// Opens an entry: a directory is browsed to, a file is handed to the system.
    /// </summary>
    /// <param name="entry">The entry to open.</param>
    /// <param name="browser">The browser showing it, for a directory.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether the system took the entry, which a directory always does.</returns>
    public static bool Open(Entry entry, Browser browser, Action<string> failed)
    {
        if (entry.Kind == EntryKind.Directory)
        {
            browser.Go(entry.Path);

            return true;
        }

        return Launch(entry.Path, failed);
    }

    /// <summary>Reveals an entry in the system's own file manager.</summary>
    /// <param name="entry">The entry to reveal.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public static void Reveal(Entry entry, Action<string> failed)
    {
        var directory =
            entry.Kind == EntryKind.Directory
                ? entry.Path
                : Path.GetDirectoryName(entry.Path) ?? entry.Path;

        _ = Launch(directory, failed);
    }

    /// <summary>Puts an entry's path on the clipboard.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="text">What to copy.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public static async void Copy(Control from, string text, Action<string> failed)
    {
        try
        {
            var clipboard = TopLevel.GetTopLevel(from)?.Clipboard;

            if (clipboard is not null)
            {
                await clipboard.SetTextAsync(text);
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }
    }

    /// <summary>
    /// Hands a path to the system, which knows what application it belongs to.
    /// </summary>
    /// <param name="path">What to hand over.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether the system took it.</returns>
    private static bool Launch(string path, Action<string> failed)
    {
        try
        {
            _ = Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });

            return true;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);

            return false;
        }
    }
}

using System.Diagnostics;
using Avalonia.Controls;
using Avalonia.Input.Platform;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// What the browser does to an entry: opens it, reveals where it is, or names it.
/// </summary>
/// <remarks>
/// Opening here is the operating system's, not the host's pipeline. Section 8 puts the open on the
/// host, but the contract gives a plugin no way to ask for one — <c>IPluginHost</c> has a registry
/// for hooks and nothing to raise a request through. Until that is settled in the contract, the
/// browser opens directly, which is visible behaviour and not a thing to leave unsaid.
/// </remarks>
internal static class Openers
{
    /// <summary>
    /// Opens an entry: a directory is browsed to, a file is handed to the system.
    /// </summary>
    /// <param name="entry">The entry to open.</param>
    /// <param name="browser">The browser showing it, for a directory.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public static void Open(Entry entry, Browser browser, Action<string> failed)
    {
        if (entry.Kind == EntryKind.Directory)
        {
            browser.Go(entry.Path);

            return;
        }

        Launch(entry.Path, failed);
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

        Launch(directory, failed);
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
    private static void Launch(string path, Action<string> failed)
    {
        try
        {
            Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }
    }
}

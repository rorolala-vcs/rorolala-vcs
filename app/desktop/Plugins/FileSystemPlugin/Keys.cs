using Avalonia.Input;

namespace FileSystemPlugin;

/// <summary>
/// The keys the File System answers itself, beyond what the toolkit's own controls do with them.
/// </summary>
internal static class Keys
{
    /// <summary>
    /// Whether this key is a user asking for the filesystem to be looked at again, and takes it if it is.
    /// </summary>
    /// <remarks>
    /// <c>F5</c> is what a file browser is refreshed with, and it is a dock's own key rather than the shell's:
    /// the shell has nothing of the filesystem to read, and what it would refresh is a different question. It is
    /// taken at the top of the dock and not by the listing, so that it is reached wherever in the dock the
    /// keyboard is — the address, the zoom, or an entry (Section 7.7).
    /// </remarks>
    /// <param name="e">The key.</param>
    /// <param name="browser">A location of the plugin, which is what a read is said through.</param>
    /// <returns>Whether the key was taken.</returns>
    public static bool Again(KeyEventArgs e, Browser browser)
    {
        if (e.Key != Key.F5)
        {
            return false;
        }

        e.Handled = true;
        browser.Touch();

        return true;
    }
}

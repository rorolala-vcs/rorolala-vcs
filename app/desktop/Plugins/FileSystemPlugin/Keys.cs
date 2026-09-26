using Avalonia.Controls;
using Avalonia.Input;

namespace FileSystemPlugin;

/// <summary>What one view's clipboard keys do: the three things a view knows and the keys cannot.</summary>
/// <param name="Copy">Puts what is chosen on the clipboard.</param>
/// <param name="Cut">Puts what is chosen on the clipboard to be moved.</param>
/// <param name="Paste">Puts what is on the clipboard where this view is looking.</param>
internal readonly record struct Clipboard(Action Copy, Action Cut, Action Paste);

/// <summary>
/// The keys the File System answers itself, beyond what the toolkit's own controls do with them.
/// </summary>
internal static class Keys
{
    /// <summary>
    /// Whether this key is one the clipboard answers, and does what it means over one view's choice.
    /// </summary>
    /// <remarks>
    /// Taken at the top of a dock and not by the listing, so that it is answered wherever the keyboard is inside
    /// the dock — the zoom, the toolbar, or an entry — rather than only after an entry was clicked. A field is
    /// left alone, because in the address <c>Ctrl+C</c> copies the words written in it (Section 7.7).
    /// </remarks>
    /// <param name="e">The key.</param>
    /// <param name="clipboard">What this view's copy, cut and paste are.</param>
    /// <returns>Whether the key was taken.</returns>
    public static bool Clipboard(KeyEventArgs e, Clipboard clipboard)
    {
        if (e.Source is TextBox ||
            !(e.KeyModifiers.HasFlag(KeyModifiers.Control) || e.KeyModifiers.HasFlag(KeyModifiers.Meta)))
        {
            return false;
        }

        switch (e.Key)
        {
            case Key.C:
                clipboard.Copy();
                break;
            case Key.X:
                clipboard.Cut();
                break;
            case Key.V:
                clipboard.Paste();
                break;
            default:
                return false;
        }

        e.Handled = true;

        return true;
    }

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

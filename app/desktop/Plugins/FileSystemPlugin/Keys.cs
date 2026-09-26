using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;

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
    /// <para>
    /// Every press is said out loud, whether or not it is taken. What a dock answers is answered at the top of
    /// itself, so a key that was pressed while the keyboard was outside the dock is a key nothing here ever saw —
    /// and without a word for the presses that <em>did</em> arrive, a key that did nothing and a key that was
    /// never seen read exactly the same from the outside.
    /// </para>
    /// </remarks>
    /// <param name="e">The key.</param>
    /// <param name="clipboard">What this view's copy, cut and paste are.</param>
    /// <param name="log">Where the press is said.</param>
    /// <returns>Whether the key was taken.</returns>
    public static bool Clipboard(KeyEventArgs e, Clipboard clipboard, ILog log)
    {
        if (!(e.KeyModifiers.HasFlag(KeyModifiers.Control) || e.KeyModifiers.HasFlag(KeyModifiers.Meta)))
        {
            return false;
        }

        if (e.Source is TextBox)
        {
            // Said as well, because it is one of the answers to a key that seemed to do nothing: the key arrived
            // and was left to the field it was pressed in.
            log.Info($"{Name(e.Key)}: left to the field it was pressed in");

            return false;
        }

        switch (e.Key)
        {
            case Key.C:
                log.Info("Ctrl+C: copying what is chosen");
                clipboard.Copy();
                break;
            case Key.X:
                log.Info("Ctrl+X: cutting what is chosen");
                clipboard.Cut();
                break;
            case Key.V:
                log.Info("Ctrl+V: pasting into the directory being looked at");
                clipboard.Paste();
                break;
            default:
                return false;
        }

        e.Handled = true;

        return true;
    }

    /// <summary>The element the keyboard is on in a control's window, or nothing where it is nowhere.</summary>
    /// <param name="anywhere">A control, which is how its window is reached.</param>
    /// <returns>What the keyboard is on.</returns>
    public static IInputElement? On(Control anywhere) =>
        TopLevel.GetTopLevel(anywhere)?.FocusManager?.GetFocusedElement();

    /// <summary>Whether the keyboard is on a control, at it or anywhere under it.</summary>
    /// <remarks>
    /// What a dock asks before it swaps the view being read for another one: a view replaced from under the
    /// keyboard takes the keyboard with it, because the control that had it is no longer in the tree — and a dock
    /// answers its keys at the top of itself, so a keyboard that is nowhere is a dock whose keys do nothing
    /// (Section 7.7).
    /// </remarks>
    /// <param name="on">What the keyboard is on, or nothing.</param>
    /// <param name="view">The control to ask about.</param>
    /// <returns>Whether they are the same control, or one holds the other.</returns>
    public static bool Holds(IInputElement? on, Visual view)
    {
        for (var at = on as Visual; at is not null; at = at.GetVisualParent())
        {
            if (ReferenceEquals(at, view))
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>A key as a person writes it, for saying which one was pressed.</summary>
    /// <param name="key">The key.</param>
    private static string Name(Key key) =>
        key switch
        {
            Key.C => "Ctrl+C",
            Key.X => "Ctrl+X",
            Key.V => "Ctrl+V",
            _ => $"Ctrl+{key}",
        };

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
    /// <param name="log">Where the press is said.</param>
    /// <returns>Whether the key was taken.</returns>
    public static bool Again(KeyEventArgs e, Browser browser, ILog log)
    {
        if (e.Key != Key.F5)
        {
            return false;
        }

        // Said out loud like the clipboard's keys, and for the same reason: a key that was pressed while the
        // keyboard was outside the dock is one nothing here ever saw.
        log.Info("F5: reading every directory again");

        e.Handled = true;
        browser.Touch();

        return true;
    }
}

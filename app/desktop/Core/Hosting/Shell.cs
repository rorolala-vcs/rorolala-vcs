using Avalonia.Controls;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The window the shell's own commands act on, named once it exists.
/// </summary>
/// <remarks>
/// The host's own menu items are registered before the window is made, so they cannot close over it.
/// They read it here instead, which is nothing until the window is up and never nothing afterwards
/// while there is a window to name. It is the set of windows that writes it, since that is what hears
/// a window close: the name follows the window the user is in, and moves to one still up when that one
/// goes away rather than being left on a window that is gone.
/// </remarks>
internal sealed class Shell
{
    /// <summary>The window that is up and was last made or used, or nothing when no window is up.</summary>
    public Window? Window { get; set; }
}

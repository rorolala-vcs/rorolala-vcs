using Avalonia.Controls;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The window the shell's own commands act on, named once it exists.
/// </summary>
/// <remarks>
/// The host's own menu items are registered before the window is made, so they cannot close over it.
/// They read it here instead, which is nothing until the window is up and never nothing afterwards.
/// </remarks>
internal sealed class Shell
{
    /// <summary>The main window, once it is up.</summary>
    public Window? Window { get; set; }
}

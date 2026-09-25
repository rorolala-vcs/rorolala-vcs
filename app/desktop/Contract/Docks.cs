using Avalonia.Controls;

namespace RorolalaDesktop.Contract;

/// <summary>Whether a dock is a toggle or a factory of instances.</summary>
public enum DockOpenMode
{
    /// <summary>
    /// At most one instance exists. Activating the dock shows or hides it.
    /// </summary>
    Toggle,

    /// <summary>
    /// Every activation creates a new instance. This is what "copyable" means.
    /// </summary>
    New,
}

/// <summary>Where a dock view is placed in the dock area.</summary>
public enum DockPlacement
{
    /// <summary>The top region.</summary>
    Top,

    /// <summary>The left region.</summary>
    Left,

    /// <summary>The right region.</summary>
    Right,

    /// <summary>The bottom region.</summary>
    Bottom,

    /// <summary>The central region.</summary>
    Center,

    /// <summary>A window of its own.</summary>
    Float,
}

/// <summary>One dock command shown in a dock's header.</summary>
/// <param name="LabelKey">An i18n key for the command's label.</param>
/// <param name="Command">What the command does.</param>
/// <param name="IconKey">An optional key naming the command's icon.</param>
public sealed record DockHeaderCommand(string LabelKey, Action Command, string? IconKey = null);

/// <summary>
/// One dock's view, as the host holds it.
/// </summary>
/// <remarks>
/// A plugin may build the view out of Avalonia controls directly, which is the deliberate trade
/// that keeps the contract small: the host inserts <see cref="View"/> into its own visual tree, so
/// the control must come from the host's Avalonia copy (Section 3.4).
/// </remarks>
public interface IDockView
{
    /// <summary>The control the dock shows.</summary>
    Control View { get; }

    /// <summary>Commands shown in the dock's own header.</summary>
    IReadOnlyList<DockHeaderCommand> HeaderCommands { get; }
}

/// <summary>
/// What a plugin says about a dock it registers.
/// </summary>
/// <param name="Owner">The registering plugin, or the kernel for a core dock.</param>
/// <param name="DockNameId">
/// A globally unique stable id, by convention <c>&lt;plugin-id&gt;.&lt;dock&gt;</c>. It is what layout
/// persistence and the <c>Window</c> menu address the dock by, so it must not change once shipped.
/// </param>
/// <param name="DisplayNameKey">An i18n key for the dock's title.</param>
/// <param name="OpenMode">Whether the dock toggles or is created anew each time.</param>
/// <param name="DefaultPlacement">Where the dock is placed when none is asked for.</param>
/// <param name="Create">The factory making a view for a requested placement.</param>
public sealed record DockRegistration(
    PluginId Owner,
    string DockNameId,
    string DisplayNameKey,
    DockOpenMode OpenMode,
    DockPlacement DefaultPlacement,
    Func<DockPlacement, IDockView> Create
);

/// <summary>Where a plugin registers docks.</summary>
public interface IDockRegistry
{
    /// <summary>
    /// Registers a dock.
    /// </summary>
    /// <param name="registration">What the plugin says about the dock.</param>
    void Register(DockRegistration registration);
}

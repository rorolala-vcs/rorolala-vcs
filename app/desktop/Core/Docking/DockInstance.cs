using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Docking;

/// <summary>
/// One live dock: the registration it was made from, its view, and where it is.
/// </summary>
/// <remarks>
/// The runtime handle a live dock has is this object; it is never written to configuration. What is
/// written is the stable <see cref="DockNameId"/> plus the ordinal, which is what lets the set of
/// open docks survive a restart even when one name has several instances.
/// </remarks>
internal sealed class DockInstance
{
    /// <summary>What the dock was registered as.</summary>
    public required DockRegistration Registration { get; init; }

    /// <summary>The view the dock shows.</summary>
    public required IDockView View { get; init; }

    /// <summary>Which instance of this name this is, counting from zero.</summary>
    public required int Ordinal { get; init; }

    /// <summary>The dock's title, resolved from its display-name key.</summary>
    public required string Title { get; init; }

    /// <summary>Where the dock currently sits.</summary>
    public DockPlacement Placement { get; set; }

    /// <summary>
    /// Whether the dock is shown.
    /// </summary>
    /// <remarks>
    /// A toggle dock that is hidden still exists, so showing it again is showing the same instance
    /// rather than making a new one; hiding it is not closing it.
    /// </remarks>
    public bool IsOpen { get; set; } = true;

    /// <summary>
    /// What the dock remembered about itself, which the layout keeps.
    /// </summary>
    /// <remarks>
    /// Held here rather than in the view, because a view is the plugin's and this is the host's to
    /// write down: what the view keeps, it keeps through this.
    /// </remarks>
    public Dictionary<string, string> Meta { get; } = new(StringComparer.Ordinal);

    /// <summary>The dock's stable name id.</summary>
    public string DockNameId => Registration.DockNameId;

    /// <summary>Whether the dock toggles or is created anew.</summary>
    public DockOpenMode OpenMode => Registration.OpenMode;
}

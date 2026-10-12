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

    /// <summary>
    /// What the dock was opened with.
    /// </summary>
    /// <remarks>
    /// Kept as it was asked for rather than unpacked into fields here, because it is the same value the
    /// view is handed and a second reading of it would be a second answer to what the dock was told.
    /// </remarks>
    public required DockRequest Request { get; init; }

    /// <summary>
    /// The window this dock is in, or nothing for the main one.
    /// </summary>
    /// <remarks>
    /// A key rather than a window, because a dock outlives any window that is closed and opened again, and
    /// because what is written down is what can be read back: a handle is this run's and the key is the
    /// program's. It is set by <see cref="DockManager.Move"/> before anything is told the dock moved, so
    /// that the window it is leaving and the one it is arriving in never both claim it.
    /// </remarks>
    public string? Window { get; set; }

    /// <summary>
    /// Whether this dock was opened in the window it is in, rather than moved into it.
    /// </summary>
    /// <remarks>
    /// What a window closing means for the dock it holds. A dock that was moved into a window of its own came
    /// out of the main window, and the main window is where it goes back to; a dock that was opened in one —
    /// a directory asked for in a window of its own, which is what a dock asked for anew in its own window is
    /// — was never in the main window, so closing that window is the end of it rather than a return. Without
    /// the difference, closing a window that was opened onto one directory leaves a dock of that directory in
    /// the main window that nobody asked for.
    /// <para>
    /// Written where the window is written: it is set as the instance is made in a window, and cleared as the
    /// instance is moved, so a dock that has been moved reads as moved.
    /// </para>
    /// </remarks>
    public bool OpenedHere { get; set; }

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

using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Docking;

/// <summary>
/// Which dock in the program the user is working in.
/// </summary>
/// <remarks>
/// One record for the whole program rather than one per window, because a dock is one dock wherever it is shown:
/// a dock moved into a window of its own is the dock being worked in before and after, and two windows answering
/// for themselves would each be right about a different dock. What the area of each window does with it is draw
/// its mark around the dock when it is one of its own, and nothing when it is not.
/// <para>
/// The dock is held rather than named, so that a close or a move can be told apart from another dock of the same
/// name: a window of a browser closing is not a second browser taking the focus, and the record has to say.
/// </para>
/// </remarks>
internal sealed class DockFocus : IDockFocus
{
    /// <summary>The dock being worked in, or nothing before one has been.</summary>
    private DockInstance? _dock;

    /// <inheritdoc />
    public FocusedDock? Current =>
        _dock is { } dock ? new FocusedDock(dock.DockNameId, dock.Ordinal) : null;

    /// <inheritdoc />
    public event Action? Changed;

    /// <summary>The dock itself, which is what the host answers its own questions from.</summary>
    internal DockInstance? Instance => _dock;

    /// <summary>Makes a dock the one being worked in, or says there is none.</summary>
    /// <remarks>
    /// Saying the same dock twice says nothing: what a reader of the record wants is where the user went, and a
    /// rebuild of the same dock is not a move. Nothing here checks whether the dock is still open — the manager
    /// is what knows the docks, and it is the manager that passes a remaining one on when the focused one goes.
    /// </remarks>
    /// <param name="dock">The dock, or nothing for none.</param>
    internal void Set(DockInstance? dock)
    {
        if (ReferenceEquals(_dock, dock))
        {
            return;
        }

        _dock = dock;
        Changed?.Invoke();
    }
}

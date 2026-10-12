using Avalonia;
using Avalonia.Input;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Docking;

/// <summary>
/// A dock carried by the pointer: the drag a header begins, and what a drop reads back.
/// </summary>
/// <remarks>
/// A drag is the toolkit's rather than this host's, because a dock has to be able to leave the window it was
/// picked up in: pulling one out of a region and letting it go over another window, or over nothing at all,
/// is one gesture, and a window that kept the pointer to itself could not see where it ended.
/// <para>
/// What travels is the dock's stable name id and its ordinal — the same pair the layout file addresses an
/// instance by — so what a drop lands is the instance that was picked up rather than whichever instance of
/// that name happens to be first. The format is the application's own rather than text, so that a drop
/// target that is not this host's — a file listing inside a dock, which reads text as paths — neither reads
/// the payload nor answers the drag: a drag nothing takes is what tells the host the dock was let go of
/// where there is no region for it.
/// </para>
/// <para>
/// The press, the slack it is given, and the hand-over are one thing rather than three, so they are one
/// object: a header, and the name of the window a dock of one's own is drawn in, both begin the same drag.
/// </para>
/// </remarks>
internal sealed class DockDrag
{
    /// <summary>How far the pointer moves with the button held before a press becomes a drag.</summary>
    /// <remarks>
    /// A press that has moved a pixel is still a click in the hand, and one that has moved this far is not.
    /// The same slack a listing gives one of its entries, since what the two gestures are told apart by is
    /// the hand rather than the surface.
    /// </remarks>
    private const double Slip = 4;

    /// <summary>The format a dock travels in, which nothing outside this host reads.</summary>
    private static readonly DataFormat<string> Dock = DataFormat.CreateStringApplicationFormat(
        "rorolala.dock"
    );

    /// <summary>
    /// What stands between the dock's name id and its ordinal.
    /// </summary>
    /// <remarks>
    /// A character a name id cannot hold, since a name id is an identifier and is written as
    /// <c>&lt;plugin-id&gt;.&lt;dock&gt;</c>: what is carried has to come apart again unambiguously, and a
    /// separator that could occur in a name would make two docks share one payload.
    /// </remarks>
    private const char Between = '\u001f';

    /// <summary>Where a dock that has been picked up is handed over.</summary>
    private readonly DockWindows _windows;

    /// <summary>The dock the pointer took hold of.</summary>
    private readonly DockInstance _instance;

    /// <summary>The window it was picked up in, or nothing for the main one.</summary>
    private readonly string? _window;

    /// <summary>The press the drag would begin at, or nothing once it has.</summary>
    private PointerPressedEventArgs? _press;

    /// <summary>Where the pointer was when that press landed.</summary>
    private Point _from;

    /// <summary>Begins a drag of one dock, taken hold of in one window.</summary>
    /// <param name="windows">Where the dock is handed over.</param>
    /// <param name="instance">The dock the pointer took hold of.</param>
    /// <param name="window">The window it was picked up in, or nothing for the main one.</param>
    public DockDrag(DockWindows windows, DockInstance instance, string? window)
    {
        _windows = windows;
        _instance = instance;
        _window = window;
    }

    /// <summary>The dock the pointer took hold of.</summary>
    public DockInstance Instance => _instance;

    /// <summary>Remembers a press that may become a drag.</summary>
    /// <remarks>
    /// A press is not a drag: what separates the two is how far the pointer goes before it is let go of,
    /// which is why nothing is done here but writing down where it started. Which button asked for the
    /// gesture is settled when the pointer moves rather than here, for the reason it is there.
    /// </remarks>
    /// <param name="press">The press.</param>
    /// <param name="at">What the position is measured against.</param>
    public void Take(PointerPressedEventArgs press, Visual at)
    {
        _press = press;
        _from = press.GetPosition(at);
    }

    /// <summary>
    /// Follows the pointer, and hands the dock over once it has gone far enough.
    /// </summary>
    /// <remarks>
    /// The dock is handed over once and the press is forgotten with it: what follows the hand-over is the
    /// platform's drag and not this gesture, and a second hand-over of the same press would be a second drag
    /// of one dock. A press that never moves far enough is a click, which is what the header it landed on
    /// answers with.
    /// </remarks>
    /// <param name="moved">The move.</param>
    /// <param name="at">What the position is measured against.</param>
    public void Follow(PointerEventArgs moved, Visual at)
    {
        if (_press is not { } press)
        {
            return;
        }

        if (!moved.GetCurrentPoint(at).Properties.IsLeftButtonPressed)
        {
            return;
        }

        var away = moved.GetPosition(at) - _from;

        if (Math.Abs(away.X) < Slip && Math.Abs(away.Y) < Slip)
        {
            return;
        }

        _press = null;
        _windows.PickUp(_instance, press, _window);
    }

    /// <summary>What a dock travels as: its stable name id, and which instance of it this is.</summary>
    /// <param name="instance">The dock to carry.</param>
    /// <returns>What the platform is handed.</returns>
    public static DataTransfer Carrying(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        var transfer = new DataTransfer();

        transfer.Add(DataTransferItem.Create(Dock, $"{instance.DockNameId}{Between}{instance.Ordinal}"));

        return transfer;
    }

    /// <summary>
    /// The dock a drag is carrying, or nothing when it is not one of this host's.
    /// </summary>
    /// <remarks>
    /// A payload nothing answers to is nothing to land, which is not a failure: the drag may be another
    /// program's, or may name a dock that was closed while it was in flight.
    /// </remarks>
    /// <param name="docks">The docks to look the instance up among.</param>
    /// <param name="data">What the drag carries.</param>
    /// <returns>The dock, or nothing.</returns>
    public static DockInstance? Carried(DockManager docks, IDataTransfer? data)
    {
        ArgumentNullException.ThrowIfNull(docks);

        if (data?.TryGetValue(Dock) is not { } payload)
        {
            return null;
        }

        var parts = payload.Split(Between);

        return parts.Length == 2 && int.TryParse(parts[1], out var ordinal)
            ? docks.InstanceOf(parts[0], ordinal)
            : null;
    }

    /// <summary>Hands a dock to the platform, and waits to be told what became of it.</summary>
    /// <param name="from">The press the drag begins at, which is what the platform is handed.</param>
    /// <param name="instance">The dock to carry.</param>
    /// <returns>What the drag came to, which is nothing where no target took it.</returns>
    public static async Task<DragDropEffects> Away(PointerPressedEventArgs from, DockInstance instance)
    {
        using var transfer = Carrying(instance);

        return await DragDrop.DoDragDropAsync(from, transfer, DragDropEffects.Move);
    }
}

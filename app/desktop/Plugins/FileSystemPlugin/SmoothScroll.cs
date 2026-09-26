using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.VisualTree;

namespace FileSystemPlugin;

/// <summary>
/// Makes the wheel glide rather than jump, in one list.
/// </summary>
/// <remarks>
/// The toolkit scrolls by whole steps however fast the wheel is turned: a list as tall as a directory
/// moves a row at a time and stops between them, which a fast wheel reads as a stutter and a long
/// directory makes tiring. What is done here is to take the wheel on the way down, before the list's
/// own scroller has seen it, and move the offset over a short glide instead, so that the same wheel
/// becomes one steady motion that lands where it was aimed.
/// <para>
/// Only the wheel is smoothed. A scrollbar dragged by hand, a row brought into view by the keyboard,
/// and a page asked for by the toolkit all move the offset directly, because in every one of those the
/// user is already driving and a glide trailing behind the pointer or the keyboard is lag rather than
/// smoothness.
/// </para>
/// <para>
/// The list holds this through the handler it registers, so nothing has to keep it: the subscription is
/// the lifetime.
/// </para>
/// </remarks>
internal sealed class SmoothScroll
{
    /// <summary>How far one notch of the wheel moves, in pixels.</summary>
    /// <remarks>
    /// Pixels rather than whole rows, which is the whole point: a step that is a row is the jump this
    /// exists to remove. It is a little more than the toolkit's own fifty so that a notch still covers
    /// ground, and small enough that a glide has somewhere to go.
    /// </remarks>
    private const double Step = 64.0;

    /// <summary>How long a glide takes, which is long enough to read as motion and short enough to feel immediate.</summary>
    private static readonly TimeSpan Glide = TimeSpan.FromMilliseconds(150);

    /// <summary>The list whose wheel is smoothed.</summary>
    private readonly Control _list;

    /// <summary>
    /// Where the list is going.
    /// </summary>
    /// <remarks>
    /// A further turn of the wheel moves this rather than replacing it, so that turning the wheel faster
    /// than the glide arrives still covers the distance turned instead of stopping at the last notch.
    /// </remarks>
    private double _target;

    /// <summary>Where the glide began.</summary>
    private double _from;

    /// <summary>When the glide began, or nothing while the next frame is to be its first.</summary>
    private TimeSpan? _start;

    /// <summary>Whether a glide is in flight.</summary>
    private bool _gliding;

    /// <summary>Attaches to one list.</summary>
    /// <param name="list">The list whose wheel is smoothed.</param>
    public SmoothScroll(Control list)
    {
        _list = list;

        // On the way down, because the list's own scroller is a descendant of it and would otherwise have
        // already jumped: a tunnel handler is the only place that runs before it.
        list.AddHandler(InputElement.PointerWheelChangedEvent, Wheeled, RoutingStrategies.Tunnel);
    }

    /// <summary>Turns a wheel notch into a glide towards where it was aimed.</summary>
    /// <param name="sender">The list, which is ignored.</param>
    /// <param name="e">The wheel, which is taken over.</param>
    private void Wheeled(object? sender, PointerWheelEventArgs e)
    {
        if (Viewer() is not { } viewer)
        {
            return;
        }

        var span = viewer.Extent.Height - viewer.Viewport.Height;

        // Nothing here to scroll is the wheel's business elsewhere: left unhandled, it goes on to
        // whatever holds the list, which is what every other control does with a wheel it cannot use.
        if (span <= 0 || e.Delta.Y == 0)
        {
            return;
        }

        _target = Math.Clamp((_gliding ? _target : viewer.Offset.Y) - (e.Delta.Y * Step), 0, span);
        _from = viewer.Offset.Y;
        _start = null;

        e.Handled = true;

        if (!_gliding)
        {
            _gliding = true;
            Request();
        }
    }

    /// <summary>Asks for the next frame of the glide.</summary>
    private void Request() => TopLevel.GetTopLevel(_list)?.RequestAnimationFrame(Tick);

    /// <summary>Moves the offset a little further towards the target, and keeps going until it arrives.</summary>
    /// <param name="now">When this frame is.</param>
    private void Tick(TimeSpan now)
    {
        if (Viewer() is not { } viewer)
        {
            // The list is gone, which is a glide with nothing left to move.
            _gliding = false;
            return;
        }

        _start ??= now;

        var done = Math.Clamp((now - _start.Value).TotalMilliseconds / Glide.TotalMilliseconds, 0, 1);

        // Eased out rather than linear: the wheel has already given the motion its speed, so the glide
        // should start fast and settle rather than start at rest.
        var eased = 1 - Math.Pow(1 - done, 3);

        viewer.Offset = new Vector(viewer.Offset.X, _from + ((_target - _from) * eased));

        if (done < 1)
        {
            Request();
        }
        else
        {
            _gliding = false;
        }
    }

    /// <summary>The scroller inside the list, which its template makes and so is looked for rather than held.</summary>
    private ScrollViewer? Viewer() =>
        _list.GetVisualDescendants().OfType<ScrollViewer>().FirstOrDefault();
}

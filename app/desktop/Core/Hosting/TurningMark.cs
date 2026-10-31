using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// A ring that turns while something is being waited for.
/// </summary>
/// <remarks>
/// Drawn rather than made of pictures: the colours in force are the ones it turns in, and a set of frames for
/// every colour a person may choose would be a set of files for a ring.
/// <para>
/// The ring is one fixed arc and what turns is the whole mark, around its own centre, rather than an arc drawn
/// again at a new angle each frame. The two say the same thing when they are right, and the rotation cannot be
/// wrong the way a redrawn arc can: the ends of an arc and its centre are not independent, and an arc asked for
/// with the wrong of the two ways round — the long way when it was told short — is drawn on the other circle
/// through the same ends, which orbits the centre it was meant to turn around. A short arc, and a rotation of
/// the mark, leave that nothing to get wrong.
/// </para>
/// <para>
/// It is not the toolkit's own mark: what a program is drawn with is its own palette, and a mark the base theme
/// draws would be drawn in the theme's colours rather than in the ones a user chose.
/// </para>
/// <para>
/// It turns while it is in a window and stops when it is not: a mark nobody can see that goes on asking for
/// frames is work a closed window is still doing.
/// </para>
/// </remarks>
internal sealed class TurningMark : Control
{
    /// <summary>How long one whole turn takes.</summary>
    private static readonly TimeSpan Turn = TimeSpan.FromMilliseconds(900);

    /// <summary>How often it is moved on, which is as fine as an eye reads a turn.</summary>
    private static readonly TimeSpan Frame = TimeSpan.FromMilliseconds(15);

    /// <summary>
    /// How much of the ring is drawn, in radians.
    /// </summary>
    /// <remarks>
    /// Short of half a turn, so that the arc between its two ends is the only short one there is and nothing
    /// has to decide which of the two was meant.
    /// </remarks>
    private const double Sweep = Math.PI * 0.75;

    /// <summary>Where the arc starts, in radians: the top of the ring.</summary>
    private const double Start = -Math.PI / 2;

    /// <summary>What it is drawn in.</summary>
    private readonly IBrush _ink;

    /// <summary>How far the ring's line is from its centre, which leaves the line inside the mark's own size.</summary>
    private readonly double _radius;

    /// <summary>How thick the ring's line is.</summary>
    private readonly double _thickness;

    /// <summary>The clock the turn is kept on.</summary>
    private readonly DispatcherTimer _clock = new() { Interval = Frame };

    /// <summary>The turn itself, which is what changes as the clock ticks.</summary>
    private readonly RotateTransform _turning = new();

    /// <summary>Where the turn has got to, in degrees.</summary>
    private double _turned;

    /// <summary>Makes a ring.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="diameter">How wide the mark is, line included.</param>
    public TurningMark(IBrush ink, double diameter)
    {
        ArgumentNullException.ThrowIfNull(ink);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(diameter);

        _ink = ink;
        _thickness = Math.Max(2, diameter / 7);
        _radius = (diameter / 2) - (_thickness / 2) - 1;

        Width = diameter;
        Height = diameter;

        // Named in full because the alignment the mark is put at and the type the alignment is are the same
        // word, and this is the one place the two meet.
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center;
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center;

        // The mark turns around the middle of the room it was given rather than around a corner of it, which is
        // the one statement of where its centre is: the drawing itself is never told to move.
        RenderTransformOrigin = RelativePoint.Center;
        RenderTransform = _turning;

        _clock.Tick += (_, _) =>
        {
            _turned = (_turned + (360 * (Frame.TotalMilliseconds / Turn.TotalMilliseconds))) % 360;
            _turning.Angle = _turned;
        };
    }

    /// <inheritdoc />
    public override void Render(DrawingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        // A mark with no room is not a mark: a window being laid out gives it nothing to draw in for as long as
        // it has no size, and a ring of no radius would be a dot rather than nothing.
        if (_radius <= 0)
        {
            return;
        }

        var centre = new Point(Width / 2, Height / 2);
        var ring = new StreamGeometry();

        using (var at = ring.Open())
        {
            at.BeginFigure(On(centre, Start), isFilled: false);

            at.ArcTo(
                On(centre, Start + Sweep),
                new Size(_radius, _radius),
                rotationAngle: 0,
                isLargeArc: false,
                SweepDirection.Clockwise
            );
        }

        context.DrawGeometry(null, new Pen(_ink, _thickness), ring);
    }

    /// <inheritdoc />
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _clock.Start();

        base.OnAttachedToVisualTree(e);
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _clock.Stop();

        base.OnDetachedFromVisualTree(e);
    }

    /// <summary>A point on the ring, around its centre.</summary>
    /// <param name="centre">The centre of the mark.</param>
    /// <param name="angle">Where on the ring, in radians from the right-hand side.</param>
    /// <returns>The point.</returns>
    private Point On(Point centre, double angle) =>
        new(centre.X + (_radius * Math.Cos(angle)), centre.Y + (_radius * Math.Sin(angle)));
}

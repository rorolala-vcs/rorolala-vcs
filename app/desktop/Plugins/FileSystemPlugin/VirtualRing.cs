using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The unclosed ring drawn over a virtual entry's icon: turning while nothing is known, filling as it is.
/// </summary>
/// <remarks>
/// Drawn rather than made of pictures, for the reason the host's own turning mark is drawn: what it is drawn
/// in is the look in force, and a set of frames for every colour a person may choose would be a set of files.
/// Its shape is the one in <c>app/desktop/Core/Hosting/TurningMark.cs</c>; a plugin cannot reference the
/// host's assembly, so the shape is written again here rather than the type being shared.
/// <para>
/// A pending ring turns as a whole rather than being drawn again at a new angle: a short arc and a rotation
/// leave nothing to get wrong, where an arc asked for the long way round is drawn on the other circle through
/// the same ends and orbits the wrong centre. A ring with a proportion is drawn again instead, since its arc
/// is what changes and its ends are its answer.
/// </para>
/// <para>
/// It listens to the entry it was made for and stops when it leaves the tree: a ring nobody can see that goes
/// on asking for frames is work a closed dock is still doing.
/// </para>
/// </remarks>
internal sealed class VirtualRing : Control
{
    /// <summary>How long one whole turn takes.</summary>
    private static readonly TimeSpan Turn = TimeSpan.FromMilliseconds(1200);

    /// <summary>How often it is moved on, which is as fine as an eye reads a turn.</summary>
    private static readonly TimeSpan Frame = TimeSpan.FromMilliseconds(30);

    /// <summary>Where the arc starts, in radians: the top of the ring.</summary>
    private const double Start = -Math.PI / 2;

    /// <summary>How much of the ring a pending entry draws, as a share of a whole turn.</summary>
    private const double Opening = 0.75;

    /// <summary>The least an arc is drawn at, so a proportion of nothing is still a mark rather than none.</summary>
    private const double LeastSweep = 0.02;

    /// <summary>What the ring is drawn in.</summary>
    public static readonly StyledProperty<IBrush?> ArcBrushProperty =
        AvaloniaProperty.Register<VirtualRing, IBrush?>(nameof(ArcBrush));

    /// <summary>How wide and tall the ring is, which is the icon it lies over.</summary>
    private readonly double _size;

    /// <summary>The entry the ring is for, which is what it asks how far along it is.</summary>
    private readonly VirtualEntry _entry;

    /// <summary>The clock the turn is kept on.</summary>
    private readonly DispatcherTimer _clock = new() { Interval = Frame };

    /// <summary>The turn itself, which is what changes as the clock ticks.</summary>
    private readonly RotateTransform _turning = new();

    /// <summary>Where the turn has got to, in degrees.</summary>
    private double _turned;

    /// <summary>Whether the entry is being listened to, which is only while the ring is in a tree.</summary>
    private bool _listening;

    /// <summary>Draws the ring in the look's own colours, and has the look reach it when it changes.</summary>
    static VirtualRing() => AffectsRender<VirtualRing>(ArcBrushProperty);

    /// <summary>Makes a ring over one entry's icon.</summary>
    /// <param name="entry">The entry the ring is for.</param>
    /// <param name="size">How wide and tall the icon is.</param>
    public VirtualRing(VirtualEntry entry, double size)
    {
        ArgumentNullException.ThrowIfNull(entry);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(size);

        _entry = entry;
        _size = size;

        Width = size;
        Height = size;
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center;
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center;

        // The turn is drawn around the middle of the room the ring was given, which is said once here and
        // never again: the drawing itself is not told to move.
        RenderTransformOrigin = RelativePoint.Center;
        RenderTransform = _turning;

        // Named rather than written as a colour, so a theme the user chooses is the ring's own: the ink is a
        // resource of the look, and a value read here would be the colour of the theme it was read under.
        this[!ArcBrushProperty] = new DynamicResourceExtension(PaletteKeys.Primary);

        _clock.Tick += (_, _) =>
        {
            _turned = (_turned + (360 * (Frame.TotalMilliseconds / Turn.TotalMilliseconds))) % 360;
            _turning.Angle = _turned;
        };
    }

    /// <summary>What the ring is drawn in.</summary>
    public IBrush? ArcBrush
    {
        get => GetValue(ArcBrushProperty);
        set => SetValue(ArcBrushProperty, value);
    }

    /// <summary>How thick the line is at an icon of a size, which leaves the line inside the mark.</summary>
    /// <param name="size">How wide and tall the icon is.</param>
    /// <returns>How many pixels wide the line is.</returns>
    internal static double Stroke(double size) => Math.Max(2, size * 0.08);

    /// <summary>How far the line's middle is from the ring's centre, at an icon of a size.</summary>
    /// <param name="size">How wide and tall the icon is.</param>
    /// <returns>How many pixels from the centre.</returns>
    internal static double Radius(double size) => Math.Max(1, (size - Stroke(size)) / 2 - 1);

    /// <summary>How much of a whole turn the arc covers.</summary>
    /// <remarks>
    /// A pending entry covers three quarters and leaves the other quarter as the gap that says it is not done,
    /// and a proportion covers itself, with a little left at nothing so the ring is still a mark. A proportion
    /// of all of it covers the whole circle rather than an arc whose ends meet.
    /// </remarks>
    /// <param name="state">What the entry says.</param>
    /// <param name="progress">How far along it is.</param>
    /// <returns>The share of a whole turn to draw.</returns>
    internal static double Sweep(VirtualEntryState state, float progress) =>
        state == VirtualEntryState.Pending
            ? Opening
            : Math.Max(LeastSweep, Math.Min(1, progress));

    /// <inheritdoc />
    public override void Render(DrawingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        var radius = Radius(_size);
        var stroke = Stroke(_size);

        if (radius <= 0 || stroke <= 0 || ArcBrush is not { } ink)
        {
            return;
        }

        var centre = new Point(_size / 2, _size / 2);
        var pen = new Pen(ink, stroke, lineCap: PenLineCap.Round);
        var sweep = Sweep(_entry.State, _entry.Progress);

        if (sweep >= 1)
        {
            context.DrawEllipse(null, pen, centre, radius, radius);

            return;
        }

        var ring = new StreamGeometry();

        using (var at = ring.Open())
        {
            at.BeginFigure(On(centre, radius, Start), isFilled: false);
            at.ArcTo(
                On(centre, radius, Start + (sweep * Math.PI * 2)),
                new Size(radius, radius),
                rotationAngle: 0,
                isLargeArc: sweep > 0.5,
                SweepDirection.Clockwise
            );
        }

        context.DrawGeometry(null, pen, ring);
    }

    /// <inheritdoc />
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _listening = true;
        _entry.PropertyChanged += Told;
        Spin(_entry.State == VirtualEntryState.Pending);

        base.OnAttachedToVisualTree(e);
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _listening = false;
        _entry.PropertyChanged -= Told;
        _clock.Stop();

        base.OnDetachedFromVisualTree(e);
    }

    /// <summary>Says that what the ring draws may be different.</summary>
    /// <param name="sender">The entry.</param>
    /// <param name="e">Which property changed.</param>
    private void Told(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is null or nameof(VirtualEntry.State))
        {
            Spin(_entry.State == VirtualEntryState.Pending);
        }

        // The entry is changed by the thread that knows about it, a bring's poll among them: a control is
        // drawn on the window's thread, so a change answered on another one is handed over rather than
        // reached from the wrong thread.
        if (Dispatcher.UIThread.CheckAccess())
        {
            InvalidateVisual();
        }
        else
        {
            Dispatcher.UIThread.Post(InvalidateVisual);
        }
    }

    /// <summary>Turns the ring, or stops it where it stands.</summary>
    /// <param name="on">Whether it is to turn.</param>
    private void Spin(bool on)
    {
        if (!_listening)
        {
            return;
        }

        if (on)
        {
            _clock.Start();

            return;
        }

        _clock.Stop();
        _turned = 0;
        _turning.Angle = 0;
    }

    /// <summary>A point on the ring, around its centre.</summary>
    /// <param name="centre">The centre of the ring.</param>
    /// <param name="radius">How far from the centre.</param>
    /// <param name="angle">Where on the ring, in radians from the right-hand side.</param>
    /// <returns>The point.</returns>
    private static Point On(Point centre, double radius, double angle) =>
        new(centre.X + (radius * Math.Cos(angle)), centre.Y + (radius * Math.Sin(angle)));
}

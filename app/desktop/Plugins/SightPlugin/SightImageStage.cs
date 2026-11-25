using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace SightPlugin;

/// <summary>
/// The picture a preview draws, which can be moved, enlarged and turned under the pointer.
/// </summary>
/// <remarks>
/// It behaves as the Version Graph's drawing does: the wheel enlarges around the pointer, a drag moves
/// the picture, and a double press enlarges a little at the place pressed. A change of scale, place or
/// turn is eased into view rather than snapped to, with the frame timer and the part the graph uses.
/// It adds what a picture needs rather than a graph does, a turn about its centre: holding R and
/// moving the pointer sideways turns it, Shift keeps the turn on ten-degree steps, and the angle is
/// said at the pointer while it turns.
/// <para>
/// The whole pane is the stage, so a drag or a wheel over its empty ground does the same as one over
/// the picture. The picture is never resampled by this control; the transform is handed to the
/// drawing, so the bitmap the cache holds is the one drawn at every scale.
/// </para>
/// </remarks>
internal sealed class SightImageStage : Control
{
    /// <summary>The smallest and largest enlargement, as a multiple of the fitted size.</summary>
    private const double MinZoom = 0.1;
    private const double MaxZoom = 16;

    /// <summary>How far one wheel notch enlarges.</summary>
    private const double ZoomStep = 1.15;

    /// <summary>How far a double press enlarges.</summary>
    private const double DoubleTapZoom = 1.6;

    /// <summary>How many degrees one pixel of sideways movement turns the picture.</summary>
    private const double DegreesPerPixel = 0.5;

    /// <summary>The step a held Shift keeps a turn on.</summary>
    private const double SnapDegrees = 10;

    /// <summary>How much of the way to the target a frame of the ease takes.</summary>
    private const double GlidePart = 0.22;

    /// <summary>The host, whose palette draws the ground and the angle at the pointer.</summary>
    private readonly IPluginHost _host;

    /// <summary>The picture drawn.</summary>
    private readonly Bitmap _picture;

    /// <summary>The picture's own width and height, in pixels.</summary>
    private readonly int _pixelWidth;

    /// <summary>The picture's own height, in pixels.</summary>
    private readonly int _pixelHeight;

    /// <summary>The frame timer that eases a change of scale, place or turn into view.</summary>
    private readonly DispatcherTimer _glide = new() { Interval = TimeSpan.FromMilliseconds(15) };

    /// <summary>How much larger than the fitted picture it is drawn.</summary>
    private double _zoom = 1;

    /// <summary>How far the picture's centre is moved from the pane's, in screen pixels.</summary>
    private Vector _pan;

    /// <summary>How far the picture is turned, in degrees.</summary>
    private double _rotation;

    /// <summary>Where the ease is taking the enlargement.</summary>
    private double _targetZoom = 1;

    /// <summary>Where the ease is taking the place.</summary>
    private Vector _targetPan;

    /// <summary>Where the ease is taking the turn.</summary>
    private double _targetRotation;

    /// <summary>Whether R is held, which turns the picture instead of moving it.</summary>
    private bool _turning;

    /// <summary>Whether a drag is moving the picture.</summary>
    private bool _moving;

    /// <summary>Where the pointer was when the drag began.</summary>
    private Point? _from;

    /// <summary>Where the pointer was on the last turn, so a turn is the distance since then.</summary>
    private Point? _turnedAt;

    /// <summary>Where the angle is said, or nothing while the picture is not being turned.</summary>
    private Point? _angleAt;

    /// <summary>Makes the stage for one picture.</summary>
    /// <param name="host">The host.</param>
    /// <param name="picture">The picture.</param>
    public SightImageStage(IPluginHost host, Bitmap picture)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(picture);

        _host = host;
        _picture = picture;
        _pixelWidth = picture.PixelSize.Width;
        _pixelHeight = picture.PixelSize.Height;
        Focusable = true;
        ClipToBounds = true;
        _glide.Tick += Glided;
    }

    /// <summary>How far the picture is turned, in degrees.</summary>
    public double RotationDegrees => _rotation;

    /// <summary>How much larger than the fitted picture it is drawn.</summary>
    public double Zoom => _zoom;

    /// <summary>Turns the picture by a step, as the tool dock's two arrows do.</summary>
    /// <param name="degrees">How far to turn it, clockwise.</param>
    public void TurnBy(double degrees) =>
        GlideTo(_targetZoom, _targetPan, _targetRotation + degrees);

    /// <summary>Puts the picture upright without moving or enlarging it.</summary>
    public void Upright() => GlideTo(_targetZoom, _targetPan, 0);

    /// <summary>Puts the picture back at its fitted size, in the middle, upright.</summary>
    public void ResetView() => GlideTo(1, default, 0);

    /// <inheritdoc />
    public override void Render(DrawingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        // The ground is drawn over the whole control, which is what makes a drag or a wheel over the
        // empty pane the same as one over the picture.
        context.FillRectangle(new SolidColorBrush(_host.Palette.Dark), new Rect(Bounds.Size));

        if (_pixelWidth <= 0 || _pixelHeight <= 0 || Bounds.Width <= 0 || Bounds.Height <= 0)
        {
            return;
        }

        using (context.PushTransform(Transform(_zoom, _pan, _rotation)))
        {
            context.DrawImage(_picture, new Rect(0, 0, _pixelWidth, _pixelHeight));
        }

        if (_turning && _angleAt is { } at)
        {
            DrawAngle(context, at);
        }
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        _glide.Stop();
        base.OnDetachedFromVisualTree(e);
    }

    /// <inheritdoc />
    protected override void OnPointerWheelChanged(PointerWheelEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        EnlargeAt(e.GetPosition(this), Math.Pow(ZoomStep, e.Delta.Y));
        e.Handled = true;
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        Focus();

        var point = e.GetCurrentPoint(this);

        if (!point.Properties.IsLeftButtonPressed)
        {
            return;
        }

        var at = e.GetPosition(this);

        if (e.ClickCount == 2)
        {
            EnlargeAt(at, DoubleTapZoom);
            e.Handled = true;

            return;
        }

        // A turn is read from the pointer's own travel while R is held, so a press in that state is
        // not a move of the picture.
        if (_turning)
        {
            _turnedAt = at;
            e.Handled = true;

            return;
        }

        // A drag is the pointer's and nothing else's, so an ease still on the way is stopped where it
        // stands and the picture follows the hand from there.
        StopGlide();
        _moving = true;
        _from = at;
        e.Pointer.Capture(this);
        e.Handled = true;
    }

    /// <inheritdoc />
    protected override void OnPointerMoved(PointerEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        var at = e.GetPosition(this);

        if (_turning)
        {
            Turn(at, e.KeyModifiers.HasFlag(KeyModifiers.Shift));
            e.Handled = true;

            return;
        }

        if (!_moving || _from is not { } from)
        {
            return;
        }

        _pan += at - from;
        _targetPan = _pan;
        _from = at;
        InvalidateVisual();
        e.Handled = true;
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        _moving = false;
        _from = null;
        e.Pointer.Capture(null);
    }

    /// <inheritdoc />
    protected override void OnKeyDown(KeyEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (e.Key == Key.R && !_turning)
        {
            // A turn is the pointer's as well, so an ease still on the way is stopped first.
            StopGlide();
            _turning = true;
            _turnedAt = null;
            _angleAt = null;
            e.Handled = true;

            return;
        }

        if (e.Key == Key.F && e.KeyModifiers == KeyModifiers.None)
        {
            ResetView();
            e.Handled = true;

            return;
        }

        base.OnKeyDown(e);
    }

    /// <inheritdoc />
    protected override void OnKeyUp(KeyEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (e.Key == Key.R)
        {
            _turning = false;
            _turnedAt = null;
            _angleAt = null;
            _targetRotation = _rotation;
            InvalidateVisual();
            e.Handled = true;

            return;
        }

        base.OnKeyUp(e);
    }

    /// <summary>Enlarges the picture, keeping the point under the pointer where it is.</summary>
    /// <remarks>
    /// The answer is read from where the view is going rather than from the frame in hand, so a wheel
    /// turned while an ease is on adds to the enlargement that ease is heading for.
    /// </remarks>
    /// <param name="at">The point under the pointer.</param>
    /// <param name="factor">How much to enlarge it.</param>
    private void EnlargeAt(Point at, double factor)
    {
        var next = Math.Clamp(_targetZoom * factor, MinZoom, MaxZoom);

        if (Math.Abs(next - _targetZoom) < double.Epsilon)
        {
            return;
        }

        var before = PicturePoint(at, _targetZoom, _targetPan, _targetRotation);
        var after = before.Transform(Transform(next, default, _targetRotation));

        // The pan is the displacement of the picture's centre, so it is set rather than added to: the
        // point that was under the pointer before the enlargement has to be put back under it now.
        GlideTo(next, at - after, _targetRotation);
    }

    /// <summary>Turns the picture by the pointer's sideways travel since the last reading.</summary>
    /// <param name="at">Where the pointer is.</param>
    /// <param name="snap">Whether the turn is kept on ten-degree steps.</param>
    private void Turn(Point at, bool snap)
    {
        if (_turnedAt is { } from)
        {
            _rotation += (at.X - from.X) * DegreesPerPixel;

            if (snap)
            {
                _rotation = Math.Round(_rotation / SnapDegrees) * SnapDegrees;
            }

            _targetRotation = _rotation;
            _angleAt = at;
            InvalidateVisual();
        }

        _turnedAt = at;
    }

    /// <summary>The point of the picture under a point of the pane.</summary>
    /// <param name="at">The point in the pane.</param>
    /// <param name="zoom">The enlargement the picture is read at.</param>
    /// <param name="pan">The place it is read at.</param>
    /// <param name="rotation">The turn it is read at.</param>
    /// <returns>The point in the picture, in its own pixels.</returns>
    private Point PicturePoint(Point at, double zoom, Vector pan, double rotation) =>
        Transform(zoom, pan, rotation).TryInvert(out var inverse)
            ? at.Transform(inverse)
            : at;

    /// <summary>The transform that draws the picture for one state.</summary>
    /// <param name="zoom">How much larger than the fitted picture it is drawn.</param>
    /// <param name="pan">How far its centre is moved, in screen pixels.</param>
    /// <param name="rotation">How far it is turned, in degrees.</param>
    /// <returns>The matrix from the picture's own pixels to the pane.</returns>
    private Matrix Transform(double zoom, Vector pan, double rotation)
    {
        var scale = FitScale * zoom;
        var radians = rotation * Math.PI / 180;
        var centre = new Point((Bounds.Width / 2) + pan.X, (Bounds.Height / 2) + pan.Y);

        return Matrix.CreateTranslation(-_pixelWidth / 2.0, -_pixelHeight / 2.0)
            * Matrix.CreateScale(scale, scale)
            * Matrix.CreateRotation(radians)
            * Matrix.CreateTranslation(centre.X, centre.Y);
    }

    /// <summary>How much the picture is scaled to fit the pane at zoom one.</summary>
    private double FitScale
    {
        get
        {
            if (Bounds.Width <= 0 || Bounds.Height <= 0 || _pixelWidth <= 0 || _pixelHeight <= 0)
            {
                return 0;
            }

            return Math.Min(Bounds.Width / _pixelWidth, Bounds.Height / _pixelHeight);
        }
    }

    /// <summary>Eases the view towards a state, starting the frame timer unless it stands nowhere yet.</summary>
    /// <remarks>
    /// A control with no size has nothing to ease: the stage is read before the dock is laid out, so the
    /// answer is taken at once rather than left for a frame that will never be drawn.
    /// </remarks>
    /// <param name="zoom">Where the enlargement is to be.</param>
    /// <param name="pan">Where the place is to be.</param>
    /// <param name="rotation">Where the turn is to be.</param>
    private void GlideTo(double zoom, Vector pan, double rotation)
    {
        _targetZoom = Math.Clamp(zoom, MinZoom, MaxZoom);
        _targetPan = pan;
        _targetRotation = rotation;

        if (Bounds.Width <= 0 || Bounds.Height <= 0)
        {
            _zoom = _targetZoom;
            _pan = _targetPan;
            _rotation = _targetRotation;
            InvalidateVisual();

            return;
        }

        if (!_glide.IsEnabled)
        {
            _glide.Start();
        }
    }

    /// <summary>Stops an ease where the view stands, so a drag or a turn is the pointer's alone.</summary>
    private void StopGlide()
    {
        _glide.Stop();
        _targetZoom = _zoom;
        _targetPan = _pan;
        _targetRotation = _rotation;
    }

    /// <summary>Moves the view a frame's part of the way to where the ease is taking it.</summary>
    /// <param name="sender">What raised the frame.</param>
    /// <param name="e">The frame's arguments.</param>
    private void Glided(object? sender, EventArgs e)
    {
        var zoom = _targetZoom - _zoom;
        var pan = _targetPan - _pan;
        var rotation = _targetRotation - _rotation;

        if (
            Math.Abs(zoom) < 0.0005
            && Math.Abs(pan.X) < 0.4
            && Math.Abs(pan.Y) < 0.4
            && Math.Abs(rotation) < 0.05
        )
        {
            _zoom = _targetZoom;
            _pan = _targetPan;
            _rotation = _targetRotation;
            _glide.Stop();
            InvalidateVisual();

            return;
        }

        _zoom += zoom * GlidePart;
        _pan += pan * GlidePart;
        _rotation += rotation * GlidePart;
        InvalidateVisual();
    }

    /// <summary>Draws the angle the picture stands at, beside the pointer, while it is being turned.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="at">Where the pointer is.</param>
    private void DrawAngle(DrawingContext context, Point at)
    {
        var palette = _host.Palette;
        var said = Math.Round(((_rotation % 360) + 360) % 360)
            .ToString("0", CultureInfo.InvariantCulture) + "°";
        var text = new FormattedText(
            said,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(FontFamily.Default),
            11,
            new SolidColorBrush(palette.Front)
        );
        var x = Math.Clamp(at.X + 14, 4, Math.Max(4, Bounds.Width - text.Width - 14));
        var y = Math.Clamp(at.Y + 14, 4, Math.Max(4, Bounds.Height - text.Height - 10));

        context.DrawRectangle(
            new SolidColorBrush(Color.FromArgb(220, palette.Dark.R, palette.Dark.G, palette.Dark.B)),
            new Pen(new SolidColorBrush(palette.Splitter), 1),
            new Rect(x - 6, y - 4, text.Width + 12, text.Height + 8),
            5,
            5
        );
        context.DrawText(text, new Point(x, y));
    }
}

using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop.CoreDocks;

/// <summary>
/// The plate a colour is chosen on: a bar for the hue, and a square for how much of it and how light.
/// </summary>
/// <remarks>
/// Six digits are not how everyone thinks of a colour, so the panel does not offer only those: the plate is
/// the space itself, with a ring standing where in it the colour is. What it says while the pointer is down is
/// the colour it is over — so that what is being chosen is seen before it is kept — and what it says when the
/// pointer comes up is that the choice is settled, which is when it is worth writing down.
/// <para>
/// Nothing here is a colour of the look's: a hue is every colour there is, a saturation is white to that hue,
/// and a value is nothing to black. What the look does own is the edge and the ring, which are read from the
/// theme, because they are drawn over a colour this program cannot know.
/// </para>
/// </remarks>
internal sealed class ColourPlate : UserControl
{
    /// <summary>How wide the plate is, and how tall its two parts are.</summary>
    private const double Wide = 220;
    private const double Tall = 130;
    private const double Bar = 16;

    /// <summary>How wide and tall a ring is, and so how far from where it stands it is drawn.</summary>
    private const double Ring = 14;

    /// <summary>Where around the wheel the colour stands, and how much of it and how light it is.</summary>
    private double _hue;
    private double _saturation;
    private double _value;

    /// <summary>Whether a press is being dragged, so that a move off the plate is still read.</summary>
    private bool _dragging;

    /// <summary>The two parts of the square: the hue, and the black over it.</summary>
    private readonly Border _tint;
    private readonly Border _shade;

    /// <summary>Where the colour stands in the square, and where the hue stands on the bar.</summary>
    private readonly Border _at;
    private readonly Border _along;

    /// <summary>Makes a plate standing where `current` stands.</summary>
    /// <param name="current">The colour to open on.</param>
    public ColourPlate(Color current)
    {
        (_hue, _saturation, _value) = Hsv.Of(current);

        _tint = new Border { Width = Wide, Height = Tall, Background = Tint() };
        _shade = new Border { Width = Wide, Height = Tall, Background = Shade() };
        _at = Ringed();
        _along = Ringed();

        var square = new Panel
        {
            Width = Wide,
            Height = Tall,
            Children = { _tint, _shade, Standing(_at, Wide, Tall) },
        };

        var gradient = new Border
        {
            Width = Wide,
            Height = Bar,
            CornerRadius = new CornerRadius(4),
            Background = Along(),
            BorderThickness = new Thickness(1),
        };
        gradient[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border");

        var bar = new Panel
        {
            Width = Wide,
            Height = Bar,
            Children = { gradient, Standing(_along, Wide, Bar) },
        };

        Content = new StackPanel { Spacing = 10, Children = { square, bar } };

        Dragged(square, (at, size) => (_saturation, _value) = At(at, size));
        Dragged(bar, (at, size) => _hue = at.X / size.Width * Hsv.Turn);

        Standing();
    }

    /// <summary>What the plate says while it is being dragged: the colour it is over.</summary>
    public event Action<Color>? Chosen;

    /// <summary>What it says when the pointer is let go, which is when the choice is worth keeping.</summary>
    public event Action? Settled;

    /// <summary>
    /// Where in a square a point stands, as how much of the hue and how light, each from nothing to one.
    /// </summary>
    /// <remarks>
    /// Left to right is more of the hue and top to bottom is less light, which is the way round every such
    /// plate is drawn: the brightest and purest colour is the top left corner, and black is the whole of the
    /// bottom edge.
    /// </remarks>
    /// <param name="at">Where the pointer is, from the top left corner of the square.</param>
    /// <param name="size">How wide and tall the square is.</param>
    /// <returns>How much of the hue, and how light, each from nothing to one.</returns>
    public static (double Saturation, double Value) At(Point at, Size size) =>
        (
            Math.Clamp(at.X / size.Width, 0, 1),
            Math.Clamp(1 - (at.Y / size.Height), 0, 1)
        );

    /// <summary>Reads a part of the plate while it is pressed and dragged.</summary>
    /// <remarks>
    /// The press is captured, so that a drag that leaves the plate is still read: letting go outside it would
    /// otherwise leave the colour standing where the pointer last was inside, which is not where it was let go.
    /// </remarks>
    /// <param name="part">The part to read.</param>
    /// <param name="read">What to do with where the pointer is in it, and how big it is.</param>
    private void Dragged(Control part, Action<Point, Size> read)
    {
        part.PointerPressed += (_, pressed) =>
        {
            _dragging = true;
            pressed.Pointer.Capture(part);
            Dragged(part, read, pressed.GetPosition(part));
        };
        part.PointerMoved += (_, moved) =>
        {
            if (_dragging)
            {
                Dragged(part, read, moved.GetPosition(part));
            }
        };
        part.PointerReleased += (_, released) =>
        {
            _dragging = false;
            released.Pointer.Capture(null);
            Settled?.Invoke();
        };
    }

    /// <summary>Reads one place on a part and says what colour the plate now stands for.</summary>
    /// <param name="part">The part the pointer is in.</param>
    /// <param name="read">What to do with where it is.</param>
    /// <param name="at">Where it is.</param>
    private void Dragged(Control part, Action<Point, Size> read, Point at)
    {
        read(at, part.Bounds.Size);
        Standing();
        Chosen?.Invoke(Hsv.To(_hue, _saturation, _value));
    }

    /// <summary>Says where the colour and the hue stand, and what hue the square is drawn in.</summary>
    private void Standing()
    {
        _tint.Background = Tint();

        var square = new Size(Wide, Tall);
        var (saturation, value) = (_saturation, _value);

        Canvas.SetLeft(_at, (saturation * square.Width) - (Ring / 2));
        Canvas.SetTop(_at, ((1 - value) * square.Height) - (Ring / 2));
        Canvas.SetLeft(_along, (_hue / Hsv.Turn * Wide) - (Ring / 2));
        Canvas.SetTop(_along, ((Bar - Ring) / 2));
    }

    /// <summary>The square's hue: white at one end and the pure hue at the other.</summary>
    /// <returns>The brush the hue is drawn in.</returns>
    private LinearGradientBrush Tint() =>
        new()
        {
            StartPoint = new RelativePoint(0, 0.5, RelativeUnit.Relative),
            EndPoint = new RelativePoint(1, 0.5, RelativeUnit.Relative),
            GradientStops =
            {
                new GradientStop(Colors.White, 0),
                new GradientStop(Hsv.To(_hue, 1, 1), 1),
            },
        };

    /// <summary>
    /// The square's darkness: nothing at the top and black at the bottom.
    /// </summary>
    /// <remarks>
    /// The clear end is a black that is not there rather than the toolkit's own transparent, which is
    /// <em>white</em> that is not there: what a plate fades into it would be washed towards white across the
    /// top of the square, which is where the purest colour is.
    /// </remarks>
    /// <returns>The brush the light is taken away with.</returns>
    private static LinearGradientBrush Shade() =>
        new()
        {
            StartPoint = new RelativePoint(0.5, 0, RelativeUnit.Relative),
            EndPoint = new RelativePoint(0.5, 1, RelativeUnit.Relative),
            GradientStops =
            {
                new GradientStop(Color.FromArgb(0, 0, 0, 0), 0),
                new GradientStop(Colors.Black, 1),
            },
        };

    /// <summary>The bar's own colours: the wheel, in the sixths it is drawn in.</summary>
    /// <returns>The brush the hue is chosen from.</returns>
    private static LinearGradientBrush Along()
    {
        var stops = new GradientStops();

        for (var sixth = 0; sixth <= 6; sixth++)
        {
            stops.Add(new GradientStop(Hsv.To(sixth * 60, 1, 1), sixth / 6.0));
        }

        return new LinearGradientBrush
        {
            StartPoint = new RelativePoint(0, 0.5, RelativeUnit.Relative),
            EndPoint = new RelativePoint(1, 0.5, RelativeUnit.Relative),
            GradientStops = stops,
        };
    }

    /// <summary>A ring for a place on the plate: white, with a dark ring inside it so that it shows on any colour.</summary>
    /// <returns>The ring.</returns>
    private static Border Ringed() =>
        new()
        {
            Width = Ring,
            Height = Ring,
            CornerRadius = new CornerRadius(Ring / 2),
            BorderThickness = new Thickness(2),
            BorderBrush = Brushes.White,
            Child = new Border
            {
                Margin = new Thickness(-1),
                BorderThickness = new Thickness(1),
                CornerRadius = new CornerRadius(Ring / 2),
                BorderBrush = new SolidColorBrush(Color.FromArgb(0x80, 0, 0, 0)),
            },
        };

    /// <summary>A ring put where it can be drawn anywhere in a part of a known size.</summary>
    /// <param name="ring">The ring to place.</param>
    /// <param name="width">How wide the part is.</param>
    /// <param name="height">How tall it is.</param>
    /// <returns>What to put in the part, which is where the ring is drawn.</returns>
    private static Canvas Standing(Border ring, double width, double height) =>
        new()
        {
            Width = width,
            Height = height,
            IsHitTestVisible = false,
            Children = { ring },
        };
}

using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Shapes;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;

namespace RorolalaFSAgent;

/// <summary>
/// The small part of the house look a conflict window needs.
/// </summary>
/// <remarks>
/// The Desktop's theme is a whole program's worth of rules; a dialog is a message, a box and four
/// buttons, so only what those are drawn with is reproduced here: square corners, one-pixel edges, flat
/// buttons with a neutral hover, and the primary spent on the one action the dialog exists for.
/// <para>
/// The primary is written into the base theme's own accent resources rather than applied control by
/// control, which is what makes a checked box and a focus edge follow it without a rule each — and the
/// ink on it is worked out from it, since the base theme writes white there and white cannot be read on
/// a light colour.
/// </para>
/// </remarks>
internal static class Look
{
    /// <summary>The size every word is set at.</summary>
    private const double BodySize = 13.0;

    /// <summary>The height of a row of a control: short enough to scan, tall enough to hit.</summary>
    private const double RowHeight = 28.0;

    /// <summary>How rounded anything is: not at all.</summary>
    private static readonly CornerRadius Square = new(0);

    /// <summary>The one edge every surface wears.</summary>
    private static readonly Thickness Edge = new(1);

    /// <summary>How long a colour takes to arrive.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(100);

    /// <summary>
    /// The typeface the program is set in, named through the font collection so that the face
    /// <c>WithInterFont</c> adds is the one used rather than a system font of the same name.
    /// </summary>
    private static readonly FontFamily Face = new("fonts:Inter#Inter");

    /// <summary>A hairline, for the edge of a control.</summary>
    private const string Line = "rorolala.fsagent.line";

    /// <summary>A band of chrome, or a surface under the pointer.</summary>
    private const string Tint = "rorolala.fsagent.tint";

    /// <summary>The same, one step stronger, for a surface being pressed.</summary>
    private const string DeeperTint = "rorolala.fsagent.tint.deeper";

    /// <summary>The hard shadow the one action wears.</summary>
    private const string Shadow = "rorolala.fsagent.shadow";

    /// <summary>
    /// The class the one action a dialog exists for wears.
    /// </summary>
    /// <remarks>
    /// The same word the Desktop's own look reads, so that which button is the action is said the same way
    /// in both programs rather than two ways that happen to agree.
    /// </remarks>
    private const string PrimaryAction = "primary";

    /// <summary>The styles a dialog is drawn with, in one primary.</summary>
    /// <param name="primary">The colour the brand and the one action are drawn in.</param>
    public static IReadOnlyList<IStyle> Styles(Color primary)
    {
        var ink = InkOn(primary);
        var held = Scaled(primary, 0.80);
        var bright = Lightened(primary, 0.18);

        return [Palette(primary, ink, held), .. Type(), .. Content(primary, ink, held, bright)];
    }

    /// <summary>
    /// The primary, written into the base theme's own accent resources, and the neutrals this dialog
    /// draws its edges and its hover in.
    /// </summary>
    /// <remarks>
    /// The entries are given once per variant, because a lookup reads the dictionary of the variant
    /// it is resolving for and never a default one. The neutrals are the variant's own ink held back,
    /// which is what makes a hover read as a hover on either ground.
    /// </remarks>
    private static Style Palette(Color primary, Color ink, Color held)
    {
        var style = new Style(selector => selector.OfType<Window>());

        style.Resources = new ResourceDictionary
        {
            ThemeDictionaries =
            {
                [ThemeVariant.Light] = Accents(primary, ink, held, Colors.Black),
                [ThemeVariant.Dark] = Accents(primary, ink, held, Colors.White),
            },
        };

        return style;
    }

    /// <summary>What one variant's palette is.</summary>
    /// <param name="primary">The primary family.</param>
    /// <param name="ink">What is written on the primary.</param>
    /// <param name="held">The primary a held surface takes.</param>
    /// <param name="ground">The variant's own ink, which the neutrals are that ink held back.</param>
    private static ResourceDictionary Accents(Color primary, Color ink, Color held, Color ground) =>
        new()
        {
            ["ThemeAccentColor"] = primary,
            ["ThemeAccentBrush"] = new SolidColorBrush(primary),
            ["HighlightForegroundColor"] = ink,
            ["HighlightForegroundBrush"] = new SolidColorBrush(ink),
            ["HighlightColor"] = primary,
            ["HighlightBrush"] = new SolidColorBrush(primary),
            ["HighlightColor2"] = held,
            ["HighlightBrush2"] = new SolidColorBrush(held),
            [Line] = new SolidColorBrush(WithAlpha(ground, 0x33)),
            [Tint] = new SolidColorBrush(WithAlpha(ground, 0x0F)),
            [DeeperTint] = new SolidColorBrush(WithAlpha(ground, 0x1F)),

            // A hard shadow rather than a soft one, and the same shape the Desktop draws: light from the
            // top left, so the shadow falls to the bottom right and finishes where it is.
            [Shadow] = new BoxShadows(
                new BoxShadow
                {
                    OffsetX = 2,
                    OffsetY = 2,
                    Blur = 0,
                    Spread = 0,
                    Color = Color.FromArgb(ground == Colors.Black ? (byte)0x40 : (byte)0x8C, 0, 0, 0),
                }
            ),
        };

    /// <summary>What the whole dialog is set in.</summary>
    private static Style[] Type() =>
        [
            // Text is not a control, so it does not take the rule below; both are needed.
            On(
                selector => selector.OfType<TextBlock>(),
                new Setter(TextBlock.FontFamilyProperty, Face),
                new Setter(TextBlock.FontSizeProperty, BodySize)
            ),
            On(
                selector => selector.OfType<TemplatedControl>(),
                new Setter(TemplatedControl.FontFamilyProperty, Face),
                new Setter(TemplatedControl.FontSizeProperty, BodySize)
            ),
        ];

    /// <summary>The controls the dialog is made of: buttons and the one checkbox.</summary>
    /// <param name="primary">The primary family.</param>
    /// <param name="ink">What is written on the primary.</param>
    /// <param name="held">The primary a held surface takes.</param>
    /// <param name="bright">The primary a surface under the pointer lifts to.</param>
    private static Style[] Content(Color primary, Color ink, Color held, Color bright) =>
        [
            // A button is flat, the same as everywhere else: an edge and a neutral hover, and no height
            // for pressing to change.
            On(
                selector => selector.OfType<Button>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 5)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<Button>().Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Tint)
            ),
            On(
                selector => selector.OfType<Button>().Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint)
            ),
            // The base theme repaints a hovered or held button's own part, which would take the flat edge
            // off it for as long as the pointer rested there.
            On(
                selector => selector.OfType<Button>().Class(":pointerover").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<Button>().Class(":pressed").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BackgroundProperty, DeeperTint),
                Brushed(ContentPresenter.BorderBrushProperty, Line)
            ),

            // The action the dialog exists for is the one surface the primary fills and the one thing in
            // it that is raised; the ink on it is the ink that can be read there rather than the white the
            // base theme would write.
            On(
                selector => selector.OfType<Button>().Class(PrimaryAction),
                Fixed(TemplatedControl.BackgroundProperty, primary),
                Fixed(TemplatedControl.ForegroundProperty, ink),
                Fixed(TemplatedControl.BorderBrushProperty, held)
            ),
            On(
                selector => selector.OfType<Button>().Class(PrimaryAction).Class(":pointerover"),
                Fixed(TemplatedControl.BackgroundProperty, bright),
                Fixed(TemplatedControl.BorderBrushProperty, held)
            ),
            On(
                selector => selector.OfType<Button>().Class(PrimaryAction).Class(":pressed"),
                Fixed(TemplatedControl.BackgroundProperty, held),
                Fixed(TemplatedControl.BorderBrushProperty, held)
            ),
            On(
                selector =>
                    selector.OfType<Button>().Class(PrimaryAction).Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BoxShadowProperty, Shadow)
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(PrimaryAction)
                        .Class(":pressed")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Fixed(ContentPresenter.BackgroundProperty, held),
                Fixed(ContentPresenter.BorderBrushProperty, held),
                new Setter(ContentPresenter.BoxShadowProperty, default(BoxShadows))
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(PrimaryAction)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Fixed(ContentPresenter.BorderBrushProperty, held)
            ),

            On(
                selector => selector.OfType<CheckBox>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(Layoutable.MinHeightProperty, RowHeight)
            ),
            // A checked box is filled with the primary and ticked in the ink that can be read on it,
            // where the base theme leaves the box empty and draws the tick in the primary.
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("border"),
                Fixed(Border.BackgroundProperty, primary),
                Fixed(Border.BorderBrushProperty, primary)
            ),
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("checkMark"),
                new Setter(Shape.FillProperty, new SolidColorBrush(ink))
            ),

            // Every part that changes colour on a state fades into it, which is all the motion there
            // is: a fade is what a flat surface can do without moving anything under the pointer.
            On(
                selector => selector.OfType<Button>().Template().Name("PART_ContentPresenter"),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
        ];

    /// <summary>A style over a selector, carrying the setters it was given.</summary>
    private static Style On(Func<Selector?, Selector> selector, params SetterBase[] setters)
    {
        var style = new Style(selector);

        foreach (var setter in setters)
        {
            style.Setters.Add(setter);
        }

        return style;
    }

    /// <summary>
    /// A setter whose value is one of the base theme's resources, resolved where the element is so
    /// that it is the colour of the variant in force there.
    /// </summary>
    private static Setter Brushed(AvaloniaProperty property, string resource) =>
        new(property, new DynamicResourceExtension(resource));

    /// <summary>A setter whose value is one of this theme's own colours.</summary>
    private static Setter Fixed(AvaloniaProperty property, Color colour) =>
        new(property, new SolidColorBrush(colour));

    /// <summary>The fade a surface takes a new colour with.</summary>
    private static Transitions Fading() =>
        new()
        {
            new BrushTransition
            {
                Property = TemplatedControl.BackgroundProperty,
                Duration = Fade,
            },
            new BrushTransition
            {
                Property = TemplatedControl.BorderBrushProperty,
                Duration = Fade,
            },
            new DoubleTransition { Property = Visual.OpacityProperty, Duration = Fade },
        };

    /// <summary>What is written on a surface filled with a colour: black or white, by contrast.</summary>
    private static Color InkOn(Color accent) =>
        Contrasts(Colors.Black, accent) >= Contrasts(Colors.White, accent)
            ? Colors.Black
            : Colors.White;

    /// <summary>One colour stepped towards black by a factor, for a held surface.</summary>
    private static Color Scaled(Color colour, double factor) =>
        Color.FromRgb(
            (byte)Math.Round(colour.R * factor),
            (byte)Math.Round(colour.G * factor),
            (byte)Math.Round(colour.B * factor)
        );

    /// <summary>One colour stepped towards white by a fraction of the way there, for a hovered surface.</summary>
    private static Color Lightened(Color colour, double amount) =>
        Color.FromRgb(
            (byte)Math.Round(colour.R + ((255 - colour.R) * amount)),
            (byte)Math.Round(colour.G + ((255 - colour.G) * amount)),
            (byte)Math.Round(colour.B + ((255 - colour.B) * amount))
        );

    /// <summary>The same colour, at another alpha.</summary>
    private static Color WithAlpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);

    /// <summary>How much one colour stands out from another.</summary>
    private static double Contrasts(Color one, Color other)
    {
        var (lighter, darker) = Luminance(one) > Luminance(other) ? (one, other) : (other, one);

        return (Luminance(lighter) + 0.05) / (Luminance(darker) + 0.05);
    }

    /// <summary>A colour's relative luminance, as the contrast ratio is defined over.</summary>
    private static double Luminance(Color colour) =>
        (0.2126 * Channel(colour.R)) + (0.7152 * Channel(colour.G)) + (0.0722 * Channel(colour.B));

    /// <summary>One channel, linearised from the value a display writes.</summary>
    private static double Channel(byte value)
    {
        var encoded = value / 255.0;

        return encoded <= 0.03928 ? encoded / 12.92 : Math.Pow((encoded + 0.055) / 1.055, 2.4);
    }
}

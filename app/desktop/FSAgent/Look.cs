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
/// buttons, so only what those are drawn with is reproduced here: the three grounds, one-pixel borders,
/// 5-pixel controls, the sunken hover, and the primary filled into the one action the dialog exists for.
/// <para>
/// The primary is written into the base theme's own accent resources rather than applied control by
/// control, which is what makes a checked box and a focus ring follow it without a rule each — and the
/// ink on it is worked out from it, since the base theme writes white there and white cannot be read on
/// a light colour.
/// </para>
/// </remarks>
internal static class Look
{
    /// <summary>The size every word is set at.</summary>
    private const double BodySize = 14.0;

    /// <summary>The height of a row of a control.</summary>
    private const double RowHeight = 30.0;

    /// <summary>How rounded a control is.</summary>
    private static readonly CornerRadius Small = new(5);

    /// <summary>The one edge a surface wears.</summary>
    private static readonly Thickness Edge = new(1);

    /// <summary>How long a colour takes to arrive.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(120);

    /// <summary>
    /// The typeface the program is set in: the platform's own, as the design uses, so that a dialog
    /// looks like the program that opened it without either shipping a face.
    /// </summary>
    private static readonly FontFamily Face = FontFamily.Default;

    /// <summary>A border, and a control's surface.</summary>
    private const string Line = "rorolala.fsagent.line";

    /// <summary>The elevated ground, which a control sits on.</summary>
    private const string Elevated = "rorolala.fsagent.elevated";

    /// <summary>The ground a hovered or held surface drops to.</summary>
    private const string Sunken = "rorolala.fsagent.sunken";

    /// <summary>The mid neutral, for text that is not the main thing.</summary>
    private const string Muted = "rorolala.fsagent.muted";

    /// <summary>The faint neutral, for the smallest print.</summary>
    private const string Faint = "rorolala.fsagent.faint";

    /// <summary>The strong border, which a hovered control firms up to.</summary>
    private const string Strong = "rorolala.fsagent.strong";

    /// <summary>The primary, lifted, which a hovered filled action takes.</summary>
    private const string PrimaryBright = "rorolala.fsagent.primary.bright";

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
        var bright = Mix(primary, Colors.White, 0.15);

        return [Palette(primary, ink, bright), .. Type(), .. Content(primary, ink, bright)];
    }

    /// <summary>
    /// The primary, written into the base theme's own accent resources, and the neutrals this dialog
    /// draws its grounds, borders and ink from.
    /// </summary>
    /// <remarks>
    /// The entries are given once per variant, because a lookup reads the dictionary of the variant
    /// it is resolving for and never a default one.
    /// </remarks>
    /// <param name="primary">The primary.</param>
    /// <param name="ink">What is written on the primary.</param>
    /// <param name="bright">The primary lifted towards white.</param>
    private static Style Palette(Color primary, Color ink, Color bright)
    {
        var style = new Style(selector => selector.OfType<Window>());

        style.Resources = new ResourceDictionary
        {
            ThemeDictionaries =
            {
                [ThemeVariant.Light] = Variant(primary, ink, bright, Neutrals.Light),
                [ThemeVariant.Dark] = Variant(primary, ink, bright, Neutrals.Dark),
            },
        };

        return style;
    }

    /// <summary>
    /// One variant's neutral ground, which is the design's own token set.
    /// </summary>
    /// <remarks>
    /// Three grounds and a three-step ramp of ink, as the Desktop's own look has: a dialog that drew its
    /// own greys would be a second answer to a question the program has already answered.
    /// </remarks>
    private readonly record struct Neutrals(
        Color Bg,
        Color Elevated,
        Color Sunken,
        Color Fg,
        Color Muted,
        Color Faint,
        Color Border,
        Color Strong,
        Color Shadow,
        Color ShadowSoft
    )
    {
        /// <summary>The light ground.</summary>
        /// <remarks>
        /// Hue-free, as the Desktop's own is: an agent window sits beside the program and has to age with
        /// it, so this ramp is that one's and not a second answer (see <c>RorolalaTheme.Grounds.Light</c>).
        /// </remarks>
        public static readonly Neutrals Light = new(
            Bg: Color.FromRgb(0xF7, 0xF7, 0xF7),
            Elevated: Color.FromRgb(0xFF, 0xFF, 0xFF),
            Sunken: Color.FromRgb(0xEC, 0xEC, 0xEC),
            Fg: Color.FromRgb(0x1D, 0x1D, 0x1D),
            Muted: Color.FromRgb(0x5D, 0x5D, 0x5D),
            Faint: Color.FromRgb(0x8B, 0x8B, 0x8B),
            Border: Color.FromRgb(0xDC, 0xDC, 0xDC),
            Strong: Color.FromRgb(0xC2, 0xC2, 0xC2),
            Shadow: Color.FromArgb(0x0F, 0x00, 0x00, 0x00),
            ShadowSoft: Color.FromArgb(0x0F, 0x00, 0x00, 0x00)
        );

        /// <summary>The dark ground.</summary>
        public static readonly Neutrals Dark = new(
            Bg: Color.FromRgb(0x14, 0x14, 0x14),
            Elevated: Color.FromRgb(0x1C, 0x1C, 0x1C),
            Sunken: Color.FromRgb(0x0F, 0x0F, 0x0F),
            Fg: Color.FromRgb(0xE6, 0xE6, 0xE6),
            Muted: Color.FromRgb(0x9A, 0x9A, 0x9A),
            Faint: Color.FromRgb(0x6B, 0x6B, 0x6B),
            Border: Color.FromRgb(0x2C, 0x2C, 0x2C),
            Strong: Color.FromRgb(0x3C, 0x3C, 0x3C),
            Shadow: Color.FromArgb(0x66, 0x00, 0x00, 0x00),
            ShadowSoft: Color.FromArgb(0x59, 0x00, 0x00, 0x00)
        );

        /// <summary>The soft shadow the design raises a surface with: a hairline of contact and a wide one.</summary>
        public BoxShadows Raised => new(
            new BoxShadow { OffsetX = 0, OffsetY = 1, Blur = 2, Spread = 0, Color = Shadow },
            [new BoxShadow { OffsetX = 0, OffsetY = 8, Blur = 24, Spread = 0, Color = ShadowSoft }]
        );
    }

    /// <summary>What one variant's palette is.</summary>
    /// <param name="primary">The primary.</param>
    /// <param name="ink">What is written on the primary.</param>
    /// <param name="bright">The primary lifted towards white.</param>
    /// <param name="n">The variant's neutrals.</param>
    private static ResourceDictionary Variant(Color primary, Color ink, Color bright, Neutrals n) =>
        new()
        {
            ["ThemeAccentColor"] = primary,
            ["ThemeAccentBrush"] = new SolidColorBrush(primary),
            ["HighlightColor"] = WithAlpha(primary, 0x59),
            ["HighlightBrush"] = new SolidColorBrush(WithAlpha(primary, 0x59)),
            ["HighlightForegroundColor"] = n.Fg,
            ["HighlightForegroundBrush"] = new SolidColorBrush(n.Fg),

            // The base theme's own names, pointed at the design's colours, so that a field, a popup and
            // a scrollbar follow without a rule each.
            ["ThemeBackgroundColor"] = n.Elevated,
            ["ThemeBackgroundBrush"] = new SolidColorBrush(n.Elevated),
            ["ThemeBorderLowColor"] = n.Border,
            ["ThemeBorderLowBrush"] = new SolidColorBrush(n.Border),
            ["ThemeBorderMidColor"] = n.Border,
            ["ThemeBorderMidBrush"] = new SolidColorBrush(n.Border),
            ["ThemeBorderHighColor"] = n.Strong,
            ["ThemeBorderHighBrush"] = new SolidColorBrush(n.Strong),
            ["ThemeControlMidColor"] = n.Elevated,
            ["ThemeControlMidBrush"] = new SolidColorBrush(n.Elevated),
            ["ThemeControlHighColor"] = n.Sunken,
            ["ThemeControlHighBrush"] = new SolidColorBrush(n.Sunken),
            ["ThemeControlHighlightMidColor"] = n.Sunken,
            ["ThemeControlHighlightMidBrush"] = new SolidColorBrush(n.Sunken),
            ["ThemeForegroundColor"] = n.Fg,
            ["ThemeForegroundBrush"] = new SolidColorBrush(n.Fg),
            ["ThemeForegroundLowColor"] = n.Faint,
            ["ThemeForegroundLowBrush"] = new SolidColorBrush(n.Faint),

            [Line] = new SolidColorBrush(n.Border),
            [Elevated] = new SolidColorBrush(n.Elevated),
            [Sunken] = new SolidColorBrush(n.Sunken),
            [Muted] = new SolidColorBrush(n.Muted),
            [Faint] = new SolidColorBrush(n.Faint),
            [Strong] = new SolidColorBrush(n.Strong),
            [PrimaryBright] = new SolidColorBrush(bright),
            ["rorolala.fsagent.shadow"] = n.Raised,
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
            On(
                selector => selector.OfType<Window>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated)
            ),
            // The id of a conflicting item is a key rather than a name, so it is set in the mono face.
            On(
                selector => selector.OfType<TextBlock>().Class("caption"),
                new Setter(TextBlock.FontSizeProperty, 12.5)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("faint"),
                Brushed(TextBlock.ForegroundProperty, Faint)
            ),
        ];

    /// <summary>The controls the dialog is made of: buttons and the one checkbox.</summary>
    /// <param name="primary">The primary.</param>
    /// <param name="ink">What is written on the primary.</param>
    /// <param name="bright">The primary lifted towards white.</param>
    private static Style[] Content(Color primary, Color ink, Color bright) =>
        [
            On(
                selector => selector.OfType<Button>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(11, 5)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<Button>().Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Sunken),
                Brushed(TemplatedControl.BorderBrushProperty, Strong)
            ),

            // The action the dialog exists for is the one surface the primary fills; the ink on it is
            // the ink that can be read there rather than the white the base theme would write.
            On(
                selector => selector.OfType<Button>().Class(PrimaryAction),
                Fixed(TemplatedControl.BackgroundProperty, primary),
                Fixed(TemplatedControl.ForegroundProperty, ink),
                Fixed(TemplatedControl.BorderBrushProperty, primary),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold)
            ),
            On(
                selector => selector.OfType<Button>().Class(PrimaryAction).Class(":pointerover"),
                Fixed(TemplatedControl.BackgroundProperty, bright),
                Fixed(TemplatedControl.BorderBrushProperty, bright)
            ),
            // The base theme repaints a hovered button's own part, which would take the primary off the
            // one action for as long as the pointer rested on it.
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(PrimaryAction)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Fixed(ContentPresenter.BackgroundProperty, bright),
                Fixed(ContentPresenter.BorderBrushProperty, bright)
            ),

            On(
                selector => selector.OfType<CheckBox>(),
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

            // Every part that changes colour on a state fades into it.
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

    /// <summary>One colour mixed into another by a fraction of the way there.</summary>
    private static Color Mix(Color colour, Color towards, double amount)
    {
        var other = 1 - amount;

        return Color.FromRgb(
            (byte)Math.Round((colour.R * other) + (towards.R * amount)),
            (byte)Math.Round((colour.G * other) + (towards.G * amount)),
            (byte)Math.Round((colour.B * other) + (towards.B * amount))
        );
    }

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

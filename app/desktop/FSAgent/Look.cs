using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
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
/// buttons, so only what those are drawn with is reproduced here: square corners, one-pixel edges,
/// the neutral hover the base theme already draws, and the accent spent on the one default action.
/// <para>
/// The accent is written into the base theme's own accent resources rather than applied control by
/// control, which is what makes a checked box and a focus edge follow it without a rule each — and
/// the ink on the accent is worked out from the accent, since the base theme writes white there and
/// white cannot be read on a light accent.
/// </para>
/// </remarks>
internal static class Look
{
    /// <summary>The size every word is set at.</summary>
    private const double BodySize = 13.0;

    /// <summary>The height of a row of a control: short enough to scan, tall enough to hit.</summary>
    private const double RowHeight = 26.0;

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

    /// <summary>The styles a dialog is drawn with, in one accent.</summary>
    /// <param name="accent">The colour anything accented is drawn in.</param>
    public static IReadOnlyList<IStyle> Styles(Color accent)
    {
        var ink = InkOn(accent);
        var held = Scaled(accent, 0.87);

        return [Palette(accent, ink, held), .. Type(), .. Content(accent, ink, held)];
    }

    /// <summary>
    /// The accent, written into the base theme's own accent resources, and the hairline this
    /// dialog draws its edges in.
    /// </summary>
    /// <remarks>
    /// The entries are given once per variant, because a lookup reads the dictionary of the variant
    /// it is resolving for and never a default one.
    /// </remarks>
    private static Style Palette(Color accent, Color ink, Color held)
    {
        var style = new Style(selector => selector.OfType<Window>());

        style.Resources = new ResourceDictionary
        {
            ThemeDictionaries =
            {
                [ThemeVariant.Light] = Accents(accent, ink, held, Color.FromArgb(0x33, 0, 0, 0)),
                [ThemeVariant.Dark] = Accents(accent, ink, held, Color.FromArgb(0x33, 0xFF, 0xFF, 0xFF)),
            },
        };

        return style;
    }

    /// <summary>What one variant's palette is.</summary>
    /// <param name="accent">The accent family.</param>
    /// <param name="ink">What is written on the accent.</param>
    /// <param name="held">The accent a held surface takes.</param>
    /// <param name="line">A hairline.</param>
    private static ResourceDictionary Accents(Color accent, Color ink, Color held, Color line) =>
        new()
        {
            ["ThemeAccentColor"] = accent,
            ["ThemeAccentBrush"] = new SolidColorBrush(accent),
            ["HighlightForegroundColor"] = ink,
            ["HighlightForegroundBrush"] = new SolidColorBrush(ink),
            ["HighlightColor"] = accent,
            ["HighlightBrush"] = new SolidColorBrush(accent),
            ["HighlightColor2"] = held,
            ["HighlightBrush2"] = new SolidColorBrush(held),
            [Line] = new SolidColorBrush(line),
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
    private static Style[] Content(Color accent, Color ink, Color held) =>
        [
            On(
                selector => selector.OfType<Button>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 4)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),

            // The default action is the one surface the accent fills, and the ink on it is the ink
            // that can be read there rather than the white the base theme would write.
            On(
                selector => selector.OfType<Button>().Class("default"),
                Fixed(TemplatedControl.BackgroundProperty, accent),
                Fixed(TemplatedControl.ForegroundProperty, ink),
                Fixed(TemplatedControl.BorderBrushProperty, accent)
            ),
            On(
                selector => selector.OfType<Button>().Class("default").Class(":pointerover"),
                Fixed(TemplatedControl.BackgroundProperty, held),
                Fixed(TemplatedControl.BorderBrushProperty, held)
            ),
            On(
                selector => selector.OfType<Button>().Class("default").Class(":pressed"),
                Fixed(TemplatedControl.BackgroundProperty, held),
                Fixed(TemplatedControl.BorderBrushProperty, held)
            ),

            On(
                selector => selector.OfType<CheckBox>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(Layoutable.MinHeightProperty, RowHeight)
            ),
            // A checked box is filled with the accent and ticked in the ink that can be read on it,
            // where the base theme leaves the box empty and draws the tick in the accent.
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("border"),
                Fixed(Border.BackgroundProperty, accent),
                Fixed(Border.BorderBrushProperty, accent)
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

    /// <summary>What is written on a surface filled with the accent: black or white, by contrast.</summary>
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

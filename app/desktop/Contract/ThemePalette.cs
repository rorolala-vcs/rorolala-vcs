using Avalonia.Media;

namespace RorolalaDesktop.Contract;

/// <summary>
/// The colours a look is drawn from, in force.
/// </summary>
/// <remarks>
/// Two sets of five, and two more that are one each: the face of a surface, the ground it stands on,
/// the hollow it sits in, the ink on it and the line that tells it from what is beside it, once for
/// each variant; and the primary and the accent, which do not follow the window. The semantic states are
/// one set per variant too, because what a state is coloured is not a preference. It is what
/// <c>theme.json</c> states, with the defaults filled in.
/// <para>
/// It is a value rather than a live reader on purpose: what a caller holds is the colours a look was
/// built with, and a colour changed later is a look built again, which is what the service does and
/// what raises the change. What a plugin asks for as it draws is <see cref="IPalette"/>, which is the
/// one set in force and is answered afresh every time.
/// </para>
/// <para>
/// The two defaults the user may choose are stated here rather than on the configuration, so that the
/// value a look is built from and the text the file ships are one statement: a default written in two
/// places is a default that disagrees with itself the first time one of them is changed.
/// </para>
/// </remarks>
public readonly record struct ThemePalette
{
    /// <summary>
    /// What the primary is when nothing states one.
    /// </summary>
    /// <remarks>
    /// A blue: the colour the program is known by, and one light enough that white on it can be read,
    /// which is what the look works out for it, since this primary ships with no ink of its own named.
    /// </remarks>
    public static readonly Color DefaultPrimary = Color.FromRgb(0x44, 0x6D, 0xD6);

    /// <summary>
    /// What the accent is when nothing states one.
    /// </summary>
    /// <remarks>
    /// A warm pink, and spent only on the marks a drag draws, never on a selection, so that a mark can
    /// always be told from a choice.
    /// </remarks>
    public static readonly Color DefaultAccent = Color.FromRgb(0xFF, 0x7E, 0xA2);

    /// <summary>
    /// What the light variant is drawn in when nothing states otherwise.
    /// </summary>
    /// <remarks>
    /// The ramp carries no hue, and that is deliberate rather than incidental: it began as the design's
    /// own warm sage, tuned to its lime primary, and kept that way under a primary the user chooses it
    /// read as a green film over the whole window and fought whatever colour was set. The two
    /// configured colours are then the only colour in a window that has not drawn an added or a
    /// removed thing.
    /// </remarks>
    public static ThemePalette DefaultLight =>
        new()
        {
            Bright = Color.FromRgb(0xFF, 0xFF, 0xFF),
            // The ground is a step away from the panel, because a dock is drawn on the panel and the
            // ground is what shows between two of them. The three were within eight values of one
            // another, which is a stack no screen shows.
            Mid = Color.FromRgb(0xE9, 0xEA, 0xEE),
            // The middle step: the title row, a dock strip, a table band and a well all sit here, one
            // step into the panel rather than at its face or at the window's own edge.
            Dark = Color.FromRgb(0xF3, 0xF4, 0xF7),
            Front = Color.FromRgb(0x1B, 0x1C, 0x1F),
            Splitter = Color.FromRgb(0xC9, 0xCC, 0xD3),
            Primary = DefaultPrimary,
            Accent = DefaultAccent,
            Semantic = SemanticPalette.Light,
        };

    /// <summary>What the dark variant is drawn in when nothing states otherwise.</summary>
    public static ThemePalette DefaultDark =>
        new()
        {
            Bright = Color.FromRgb(0x1A, 0x1B, 0x20),
            Mid = Color.FromRgb(0x0D, 0x0E, 0x11),
            Dark = Color.FromRgb(0x12, 0x13, 0x18),
            Front = Color.FromRgb(0xE8, 0xE9, 0xEC),
            Splitter = Color.FromRgb(0x34, 0x36, 0x3E),
            Primary = DefaultPrimary,
            Accent = DefaultAccent,
            Semantic = SemanticPalette.Dark,
        };

    /// <summary>The face a card is: what a surface that stands out from the ground is filled with.</summary>
    public required Color Bright { get; init; }

    /// <summary>The ground itself: what the program is drawn on where nothing stands on anything.</summary>
    public required Color Mid { get; init; }

    /// <summary>The hollow: what a surface that sits in the ground (an input, a well) is filled with.</summary>
    public required Color Dark { get; init; }

    /// <summary>The ink: what is written on the three of them.</summary>
    public required Color Front { get; init; }

    /// <summary>The line: what tells a surface from the ground, and one region from the next.</summary>
    public required Color Splitter { get; init; }

    /// <summary>What is chosen, which is one colour whatever the window is drawn in.</summary>
    public required Color Primary { get; init; }

    /// <summary>The drag marks, which are one colour for the same reason.</summary>
    public required Color Accent { get; init; }

    /// <summary>The states the program draws, as the look's own words for them.</summary>
    public required SemanticPalette Semantic { get; init; }

    /// <summary>This palette with the two configured colours replaced, or kept where nothing is given.</summary>
    /// <param name="primary">The primary to use, or nothing to keep this one.</param>
    /// <param name="accent">The accent to use, or nothing to keep this one.</param>
    /// <returns>The palette in force.</returns>
    public ThemePalette With(Color? primary, Color? accent) =>
        this with
        {
            Primary = primary ?? Primary,
            Accent = accent ?? Accent,
        };
}

using Avalonia.Media;

namespace RorolalaDesktop.Contract;

/// <summary>
/// The colours the program says a state is.
/// </summary>
/// <remarks>
/// They are the look's rather than a plugin's: a state means the same thing wherever it is drawn, and a
/// second set in a plugin is a second vocabulary for one answer. What a state is coloured is not the ground
/// it is drawn on, so both looks ship the same bright set: a badge stands on the look's face tinted with the
/// state's colour, and that colour is what its picture is drawn in, lifted on the dark look and deepened on
/// the light one. A user who wants the two looks
/// apart states them apart in <c>theme.json</c>, which is what the two variants' fields are for.
/// </remarks>
public readonly record struct SemanticPalette
{
    /// <summary>What cannot be acted on at the moment.</summary>
    public required Color Disabled { get; init; }

    /// <summary>What changed and can be recorded.</summary>
    public required Color Modify { get; init; }

    /// <summary>What asks for attention but is not a failure.</summary>
    public required Color Warning { get; init; }

    /// <summary>What went wrong in an operation.</summary>
    public required Color Error { get; init; }

    /// <summary>What is wrong with the work itself, darker than an operation's error.</summary>
    public required Color Fail { get; init; }

    /// <summary>What is new, recorded, or otherwise settled.</summary>
    public required Color Success { get; init; }

    /// <summary>What may be acted on, and the cyan a lock's holder is drawn in.</summary>
    public required Color Allow { get; init; }

    /// <summary>What may not be acted on, and the grey a lock held elsewhere is drawn in.</summary>
    public required Color Disallow { get; init; }

    /// <summary>What a run does only when it is forced.</summary>
    public required Color Force { get; init; }

    /// <summary>The first of the status colours, for a move the work confirms.</summary>
    public required Color StatusAlpha { get; init; }

    /// <summary>The second of the status colours, for a move the work only suspects.</summary>
    public required Color StatusBeta { get; init; }

    /// <summary>The third of the status colours, for a state that is neither of the first two.</summary>
    public required Color StatusDelta { get; init; }

    /// <summary>The dark look's set.</summary>
    public static SemanticPalette Dark =>
        new()
        {
            Disabled = Color.FromRgb(0x6E, 0x74, 0x80),
            Modify = Color.FromRgb(0xFF, 0xC8, 0x57),
            Warning = Color.FromRgb(0xFF, 0x9F, 0x43),
            Error = Color.FromRgb(0xFF, 0x6B, 0x6B),
            Fail = Color.FromRgb(0xD3, 0x2F, 0x2F),
            Success = Color.FromRgb(0x6F, 0xDF, 0x85),
            Allow = Color.FromRgb(0x4D, 0xD0, 0xE1),
            Disallow = Color.FromRgb(0x9A, 0xA0, 0xA6),
            Force = Color.FromRgb(0xFF, 0x5C, 0x9E),
            StatusAlpha = Color.FromRgb(0x64, 0xB5, 0xF6),
            StatusBeta = Color.FromRgb(0xBA, 0x68, 0xC8),
            StatusDelta = Color.FromRgb(0xF4, 0x8F, 0xB1),
        };

    /// <summary>
    /// The light look's set, which is the dark look's.
    /// </summary>
    /// <remarks>
    /// Stated as the dark one rather than written out again, because a state is one colour on either ground: a
    /// second copy would be a second statement of it, and the two would disagree the day one of them was
    /// changed.
    /// </remarks>
    public static SemanticPalette Light => Dark;
}

using Avalonia.Media;

namespace RorolalaDesktop.Fonts;

/// <summary>
/// The faces a Chinese, Japanese or Korean codepoint is drawn from, in the order they are asked for.
/// </summary>
/// <remarks>
/// The program carries Noto Sans for its Latin words, and that family has no CJK glyphs: without a
/// named fallback the toolkit asks the machine, and what a Windows machine answers is Microsoft YaHei
/// UI, whose strokes at fourteen pixels are dense enough that the shapes read as stepped rather than
/// drawn. Noto Sans SC is the same superfamily as the words beside it and keeps open counters at that
/// size, so it is asked for first; the machine's own face follows for a machine that has no Noto.
/// <para>
/// The families are named one per entry rather than in one comma list, because a fallback is resolved
/// by finding a family that has the glyph, and a list that mixes names would have to guess.
/// </para>
/// </remarks>
public static class Cjk
{
    /// <summary>
    /// The codepoints a CJK face is asked for.
    /// </summary>
    /// <remarks>
    /// Its punctuation and fullwidth forms are as much a part of the writing as the ideographs, and
    /// asking for them here keeps a Chinese sentence in one face rather than three.
    /// </remarks>
    private static readonly UnicodeRange[] Ranges =
    [
        new(0x2E80, 0x2EFF), // CJK radicals supplement
        new(0x3000, 0x303F), // CJK symbols and punctuation
        new(0x3040, 0x30FF), // Hiragana and Katakana
        new(0x3400, 0x4DBF), // CJK unified ideographs, extension A
        new(0x4E00, 0x9FFF), // CJK unified ideographs
        new(0xAC00, 0xD7AF), // Hangul syllables
        new(0xF900, 0xFAFF), // CJK compatibility ideographs
        new(0xFF00, 0xFFEF), // Halfwidth and fullwidth forms
    ];

    /// <summary>
    /// The families a CJK codepoint is looked for in, the shipped-looking one first.
    /// </summary>
    private static readonly string[] Families =
    [
        "Noto Sans SC",
        "Microsoft YaHei UI",
        "Microsoft YaHei",
        "Noto Sans CJK SC",
        "Source Han Sans SC",
        "PingFang SC",
        "Hiragino Sans GB",
        "WenQuanYi Micro Hei",
    ];

    /// <summary>The fallback list every CJK codepoint is matched against.</summary>
    public static IReadOnlyList<FontFallback> Fallbacks { get; } = Build();

    private static FontFallback[] Build()
    {
        var fallbacks = new List<FontFallback>(Families.Length * Ranges.Length);

        // Family-major so that every range of the first face is offered before the second face is
        // asked at all: a machine with Noto Sans SC answers every CJK codepoint from it.
        foreach (var family in Families)
        {
            var fontFamily = new FontFamily(family);

            foreach (var range in Ranges)
            {
                fallbacks.Add(new FontFallback { FontFamily = fontFamily, UnicodeRange = range });
            }
        }

        return [.. fallbacks];
    }
}

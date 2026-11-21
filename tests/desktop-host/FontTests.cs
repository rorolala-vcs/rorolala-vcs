using Avalonia.Media;

using RorolalaDesktop.Fonts;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The face the programs are drawn in: that the family the look names is one the assembly carries.
/// </summary>
/// <remarks>
/// The font files are fetched rather than vendored, so what a build embeds is not visible in the tree. A
/// test that asked for the family and got a glyph back is the one answer that says the fetch, the resource
/// path and the family name agree.
/// </remarks>
public sealed class FontTests
{
    /// <summary>Noto Sans is in the program, under the family name the look asks for.</summary>
    [Fact]
    public void TheProgramCarriesNotoSans()
    {
        Assert.True(Platform.IsUp);

        // Every face the programs ask for: the weights the look sets words in, and the italics a log or
        // an editor may ask for. A weight the set does not carry would be drawn by a synthesized one.
        foreach (
            var (style, weight) in new[]
            {
                (FontStyle.Normal, FontWeight.Normal),
                (FontStyle.Normal, FontWeight.SemiBold),
                (FontStyle.Normal, FontWeight.Bold),
                (FontStyle.Normal, FontWeight.ExtraBold),
                (FontStyle.Italic, FontWeight.Normal),
                (FontStyle.Italic, FontWeight.Bold),
            }
        )
        {
            var typeface = new Typeface(NotoSans.Family, style, weight);

            Assert.True(
                FontManager.Current.TryGetGlyphTypeface(typeface, out var glyph),
                $"{style} {weight} is missing from the embedded set"
            );
            // A static Noto face carries the weight in its family name, `Noto Sans SemiBold`, so what is
            // checked is the family the set belongs to rather than one spelling of its name.
            Assert.True(
                glyph.FamilyName.StartsWith("Noto Sans", StringComparison.Ordinal),
                $"{style} {weight} was drawn in {glyph.FamilyName}"
            );
        }
    }

    /// <summary>A word with no family of its own is set in Noto Sans.</summary>
    /// <remarks>
    /// The launcher has no look that sets a family on every control, so what it draws is this answer:
    /// the family the application was started with, which is the one the assembly carries.
    /// </remarks>
    [Fact]
    public void TheDefaultFamilyIsNotoSans()
    {
        Assert.True(Platform.IsUp);
        Assert.Equal(NotoSans.Family.Name, FontManager.Current.DefaultFontFamily.Name);

        // And a typeface that named no family at all is the same one, which is what a plugin drawing
        // with `FontFamily.Default` asks for.
        Assert.True(
            FontManager.Current.TryGetGlyphTypeface(
                new Typeface(FontFamily.Default, FontStyle.Normal, FontWeight.Normal),
                out var glyph
            )
        );
        Assert.Equal("Noto Sans", glyph.FamilyName);
    }
}

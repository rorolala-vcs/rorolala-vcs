using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Styling;
using FileSystemPlugin;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The look: what follows from the one colour the user chooses, and what a fill fades in from.
/// </summary>
/// <remarks>
/// Nothing here applies the look — that needs a running Avalonia, and the order it is applied in is
/// what Section 10 requires rather than anything a test can watch — so what is checked is the rule the
/// accent is read by, which is the part of the look that depends on a colour this program did not pick, and
/// the colours a fade runs between, which is the part a flash comes from.
/// </remarks>
public sealed class ThemeTests
{
    /// <summary>An accent light enough to read black on is written in black.</summary>
    /// <param name="accent">The accent to work out the ink for.</param>
    [Theory]
    [InlineData("#BFFF00")]
    [InlineData("#FFFFFF")]
    [InlineData("#C8D8E8")]
    public void WhatIsWrittenOnALightAccentIsBlack(string accent) =>
        Assert.Equal(Colors.Black, RorolalaTheme.InkOn(Color.Parse(accent)));

    /// <summary>An accent dark enough to read white on is written in white.</summary>
    /// <param name="accent">The accent to work out the ink for.</param>
    [Theory]
    [InlineData("#000080")]
    [InlineData("#000000")]
    [InlineData("#8B0000")]
    public void WhatIsWrittenOnADarkAccentIsWhite(string accent) =>
        Assert.Equal(Colors.White, RorolalaTheme.InkOn(Color.Parse(accent)));

    /// <summary>
    /// Every ink a mark is drawn in, and every ground it stands on, is a colour the look holds.
    /// </summary>
    /// <remarks>
    /// The names are written as literals in the plugin that draws the mark, because a plugin has nowhere else
    /// to read them from: a look that renamed one would leave the mark painted with nothing, which is a mark
    /// that is simply not on the card. Both variants are asked, because a look is two palettes and one of them
    /// could be missing what the other has.
    /// </remarks>
    [Fact]
    public void EveryInkAMarkIsDrawnInIsOneTheLookHolds()
    {
        var look = new RorolalaTheme(Color.Parse("#BFFF00"), Color.Parse("#FF6D00"), null);

        foreach (var variant in new[] { ThemeVariant.Light, ThemeVariant.Dark })
        {
            var palette = Assert.IsAssignableFrom<ResourceDictionary>(
                look.Palette.ThemeDictionaries[variant]
            );

            foreach (var ink in Enum.GetValues<LockInk>())
            {
                Assert.IsAssignableFrom<IBrush>(palette[Icons.Ink(ink)]);
                Assert.IsAssignableFrom<IBrush>(palette[Icons.Wash(ink)]);
            }
        }
    }

    /// <summary>
    /// What a fill fades in from is that fill at no alpha, never "no colour".
    /// </summary>
    /// <remarks>
    /// Avalonia fades a brush by interpolating its colour's channels and its alpha apart, and "no colour" is
    /// white: fading from it into the sunken ground would pass through a light grey — lighter than either end
    /// at the middle of the fade — which is a flash of light where a fill was meant to arrive. The look's part
    /// of that is the palette: a clear entry has to be the colour it fades to, so that the alpha is the only
    /// thing that changes.
    /// </remarks>
    /// <param name="clear">The entry a fill fades in from.</param>
    /// <param name="solid">The entry it fades to.</param>
    [Theory]
    [InlineData("rorolala.bg.sunken.clear", "rorolala.bg.sunken")]
    [InlineData("rorolala.border.clear", "rorolala.border")]
    public void WhatAFadeStartsFromIsTheColourItFadesTo(string clear, string solid)
    {
        var look = new RorolalaTheme(Color.Parse("#BFFF00"), Color.Parse("#FF6D00"), null);

        foreach (var variant in new[] { ThemeVariant.Light, ThemeVariant.Dark })
        {
            var palette = Assert.IsAssignableFrom<ResourceDictionary>(
                look.Palette.ThemeDictionaries[variant]
            );
            var from = Assert.IsAssignableFrom<ISolidColorBrush>(palette[clear]);
            var to = Assert.IsAssignableFrom<ISolidColorBrush>(palette[solid]);

            Assert.Equal(0, from.Color.A);
            Assert.Equal((to.Color.R, to.Color.G, to.Color.B), (from.Color.R, from.Color.G, from.Color.B));
        }
    }
}

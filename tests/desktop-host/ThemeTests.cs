using Avalonia.Media;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The look: what follows from the one colour the user chooses.
/// </summary>
/// <remarks>
/// Nothing here applies the look — that needs a running Avalonia, and the order it is applied in is
/// what Section 10 requires rather than anything a test can watch — so what is checked is the rule the
/// accent is read by, which is the part of the look that depends on a colour this program did not pick.
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
}

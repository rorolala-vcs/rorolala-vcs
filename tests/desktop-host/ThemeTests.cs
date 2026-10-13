using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Styling;
using FileSystemPlugin;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The look: what is built from the colours the user chooses, and what follows from them.
/// </summary>
/// <remarks>
/// Nothing here applies the look — that needs a running Avalonia, and the order it is applied in is
/// what the theme requires rather than anything a test can watch — so what is checked is the look
/// itself: the rule the ink on a filled surface is read by, that every name a plugin draws a mark with
/// is answered, that a fade runs between one colour and the same one at no alpha, and that the twelve
/// a person may change reach the roles they are named for.
/// </remarks>
public sealed class ThemeTests
{
    /// <summary>
    /// The look over a palette of the test's own, which is not the one a run starts with.
    /// </summary>
    private static RorolalaTheme Look() =>
        new(LookPalette, InkOfLime, WindowStyle.Windows);

    /// <summary>The two configured colours of the look every test here builds.</summary>
    private static ThemePalette LookPalette =>
        ThemePalette.DefaultLight with
        {
            Primary = Color.Parse("#BFFF00"),
            Accent = Color.Parse("#FF6D00"),
        };

    /// <summary>An ink other than the one the look would work out, so that the pair is told apart.</summary>
    private static Color InkOfLime => Color.Parse("#223300");

    /// <summary>One variant's palette out of the look, which is where the published names live.</summary>
    /// <param name="look">The look to read.</param>
    /// <param name="variant">Which of the two variants to read.</param>
    /// <returns>What that variant holds.</returns>
    private static ResourceDictionary Variant(RorolalaTheme look, ThemeVariant variant) =>
        Assert.IsAssignableFrom<ResourceDictionary>(look.Palette.ThemeDictionaries[variant]);

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
    /// The names come from the contract rather than from literals written in this test, so what is
    /// checked is that a plugin asking for a role is answered — which is also what says the names the
    /// plugins now share have not been renamed out from under them.
    /// </remarks>
    [Fact]
    public void EveryInkAMarkIsDrawnInIsOneTheLookHolds()
    {
        var look = Look();

        foreach (var variant in new[] { ThemeVariant.Light, ThemeVariant.Dark })
        {
            var palette = Variant(look, variant);

            foreach (var ink in Enum.GetValues<MarkInk>())
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
        var look = Look();

        foreach (var variant in new[] { ThemeVariant.Light, ThemeVariant.Dark })
        {
            var palette = Variant(look, variant);
            var from = Assert.IsAssignableFrom<ISolidColorBrush>(palette[clear]);
            var to = Assert.IsAssignableFrom<ISolidColorBrush>(palette[solid]);

            Assert.Equal(0, from.Color.A);
            Assert.Equal((to.Color.R, to.Color.G, to.Color.B), (from.Color.R, from.Color.G, from.Color.B));
        }
    }

    /// <summary>
    /// The five a plugin reads, and the names it may name instead, are the same colours.
    /// </summary>
    /// <remarks>
    /// A plugin reads a colour as a value through <c>IPalette</c> where it has to paint with it, and as a
    /// resource name where the variant has to answer for it. The two are one answer said twice, so what
    /// this checks is that they still say the same thing — which is the whole of what makes the interface
    /// and the published names two spellings rather than two palettes.
    /// </remarks>
    [Fact]
    public void WhatAPluginReadsIsWhatTheLookPublishes()
    {
        var light = ThemePalette.DefaultLight;
        var dark = ThemePalette.DefaultDark;
        var look = new RorolalaTheme(light, null, WindowStyle.Windows);

        var names = new (string Key, Color Light, Color Dark)[]
        {
            (PaletteKeys.Bright, light.Bright, dark.Bright),
            (PaletteKeys.Mid, light.Mid, dark.Mid),
            (PaletteKeys.Dark, light.Dark, dark.Dark),
            (PaletteKeys.Front, light.Front, dark.Front),
            (PaletteKeys.Splitter, light.Splitter, dark.Splitter),
            (PaletteKeys.Primary, light.Primary, dark.Primary),
            (PaletteKeys.Accent, light.Accent, dark.Accent),
        };

        foreach (var (key, onLight, onDark) in names)
        {
            Assert.Equal(onLight, Fill(Variant(look, ThemeVariant.Light), key));
            Assert.Equal(onDark, Fill(Variant(look, ThemeVariant.Dark), key));
        }
    }

    /// <summary>
    /// The twelve a person may change are the roles they name, in both variants.
    /// </summary>
    /// <remarks>
    /// Each is chosen as a colour no default is, so that a field read into the wrong role — or into
    /// neither — is a failure rather than a coincidence: the ten grounds and the two configured colours
    /// are the whole of what the panel edits, and a look built from them has to be built from all of
    /// them.
    /// </remarks>
    [Fact]
    public void EveryColourTheUserChoosesReachesTheRoleItNames()
    {
        var chosen = new ThemeConfiguration
        {
            Primary = Color.Parse("#010203"),
            Accent = Color.Parse("#040506"),
            ThemeDarkBright = Color.Parse("#101112"),
            ThemeDarkMid = Color.Parse("#131415"),
            ThemeDarkDark = Color.Parse("#161718"),
            ThemeDarkFront = Color.Parse("#191A1B"),
            ThemeDarkSplitter = Color.Parse("#1C1D1E"),
            ThemeLightBright = Color.Parse("#202122"),
            ThemeLightMid = Color.Parse("#232425"),
            ThemeLightDark = Color.Parse("#262728"),
            ThemeLightFront = Color.Parse("#292A2B"),
            ThemeLightSplitter = Color.Parse("#2C2D2E"),
        };

        var look = new RorolalaTheme(chosen.LightTheme, null, WindowStyle.Windows);

        Assert.Equal(chosen.ThemeLightBright, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Bright));
        Assert.Equal(chosen.ThemeLightMid, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Mid));
        Assert.Equal(chosen.ThemeLightDark, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Dark));
        Assert.Equal(chosen.ThemeLightFront, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Front));
        Assert.Equal(chosen.ThemeLightSplitter, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Splitter));

        var dark = new RorolalaTheme(chosen.Theme, null, WindowStyle.Windows);

        Assert.Equal(chosen.ThemeDarkBright, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Bright));
        Assert.Equal(chosen.ThemeDarkMid, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Mid));
        Assert.Equal(chosen.ThemeDarkDark, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Dark));
        Assert.Equal(chosen.ThemeDarkFront, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Front));
        Assert.Equal(chosen.ThemeDarkSplitter, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Splitter));

        // The two that do not follow the window are one colour each, in both variants.
        Assert.Equal(chosen.Primary, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Primary));
        Assert.Equal(chosen.Primary, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Primary));
        Assert.Equal(chosen.Accent, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.Accent));
        Assert.Equal(chosen.Accent, Fill(Variant(dark, ThemeVariant.Dark), PaletteKeys.Accent));
    }

    /// <summary>
    /// A file that names nothing is drawn in the colours the program ships with.
    /// </summary>
    /// <remarks>
    /// This is what says the defaults did not move when the ten grounds were made settable: the look a
    /// fresh install is drawn in has to be the look it was drawn in before the panel could change it.
    /// </remarks>
    [Fact]
    public void AFileThatNamesNothingIsTheLookTheProgramShipsWith()
    {
        var look = new RorolalaTheme(new ThemeConfiguration().LightTheme, null, WindowStyle.Windows);
        var dark = new RorolalaTheme(new ThemeConfiguration().Theme, null, WindowStyle.Windows);

        foreach (var (key, light, shade) in new (string Key, Color Light, Color Dark)[]
        {
            (PaletteKeys.Bright, ThemePalette.DefaultLight.Bright, ThemePalette.DefaultDark.Bright),
            (PaletteKeys.Mid, ThemePalette.DefaultLight.Mid, ThemePalette.DefaultDark.Mid),
            (PaletteKeys.Dark, ThemePalette.DefaultLight.Dark, ThemePalette.DefaultDark.Dark),
            (PaletteKeys.Front, ThemePalette.DefaultLight.Front, ThemePalette.DefaultDark.Front),
            (PaletteKeys.Splitter, ThemePalette.DefaultLight.Splitter, ThemePalette.DefaultDark.Splitter),
        })
        {
            Assert.Equal(light, Fill(Variant(look, ThemeVariant.Light), key));
            Assert.Equal(shade, Fill(Variant(dark, ThemeVariant.Dark), key));
        }
    }

    /// <summary>
    /// The two colours that stand on a filled surface are filled with the ink that goes with them.
    /// </summary>
    /// <remarks>
    /// A ground is never filled with the primary, so the ink it carries is the front ink — and a look
    /// built from a dark ground has to carry a light one, since words in the ground's own colour are
    /// words that are not there.
    /// </remarks>
    [Fact]
    public void AConfiguredInkIsWhatIsWrittenOnThePrimary()
    {
        var look = new RorolalaTheme(LookPalette, InkOfLime, WindowStyle.Windows);

        Assert.Equal(InkOfLime, Fill(Variant(look, ThemeVariant.Light), PaletteKeys.PrimaryText));
        Assert.Equal(InkOfLime, Fill(Variant(look, ThemeVariant.Dark), PaletteKeys.PrimaryText));
    }

    /// <summary>The colour one entry of a variant holds, with its alpha dropped.</summary>
    /// <param name="palette">The variant to read.</param>
    /// <param name="key">The entry to read.</param>
    /// <returns>The colour it is filled with.</returns>
    private static Color Fill(ResourceDictionary palette, string key)
    {
        var brush = Assert.IsAssignableFrom<ISolidColorBrush>(palette[key]);

        return Color.FromRgb(brush.Color.R, brush.Color.G, brush.Color.B);
    }
}

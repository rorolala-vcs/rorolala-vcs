using RorolalaDesktop.Configuration;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The theme a run names: which provider supplies it, and what happens when none does.
/// </summary>
/// <remarks>
/// Nothing here applies a theme — that needs a running Avalonia — so what is checked is the choice
/// the host makes, including the stop it makes when the choice cannot be honoured.
/// </remarks>
public sealed class ThemeTests
{
    /// <summary>The built-in theme is the one the default preference names.</summary>
    [Fact]
    public void TheBuiltInThemeIsTheDefaultOne()
    {
        Assert.Equal("rorolala.theme.default", RorolalaTheme.Id);
        Assert.Equal("rorolala.theme.default", PreferenceConfiguration.DefaultTheme);
    }

    /// <summary>The id <c>fluent</c> means the base theme alone, which needs no provider.</summary>
    [Fact]
    public void FluentMeansTheBaseThemeAlone()
    {
        var service = new ThemeService(new ThemeRegistry());

        service.Apply(ThemeService.Fluent);
    }

    /// <summary>A theme with no provider stops the program with its own code.</summary>
    [Fact]
    public void AnIdNoProviderSuppliesStopsTheProgram()
    {
        var service = new ThemeService(new ThemeRegistry());

        var failure = Assert.Throws<ConfigurationFailure>(() => service.Apply("no.such.theme"));

        Assert.Equal(ExitCode.Unavailable, failure.Code);
        Assert.Contains("no.such.theme", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A registered theme is found by its id.</summary>
    [Fact]
    public void ARegisteredThemeIsFoundByItsId()
    {
        var themes = new ThemeRegistry();
        themes.Add(Core.Id, new FakeTheme("it.theme.default"));

        Assert.NotNull(themes.Find("it.theme.default"));
        Assert.Null(themes.Find("it.theme.other"));
    }

    /// <summary>The first registration wins when two themes carry one id.</summary>
    [Fact]
    public void TheFirstRegistrationWinsWhenTwoThemesCarryOneId()
    {
        var themes = new ThemeRegistry();
        var first = new FakeTheme("it.theme.same");
        themes.Add(Core.Id, first);
        themes.Add(Core.Id, new FakeTheme("it.theme.same"));

        Assert.Same(first, themes.Find("it.theme.same"));
    }
}

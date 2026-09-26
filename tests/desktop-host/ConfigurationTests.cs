using Avalonia.Media;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The configuration files: what is written when nothing is there, what a plugin reads, and what is
/// refused.
/// </summary>
/// <remarks>
/// The reader is the host's own, and the files are real files under a scratch data directory, so
/// what is checked is what the program meets rather than a stand-in for it.
/// </remarks>
public sealed class ConfigurationTests
{
    /// <summary>Begins each test from no configuration at all.</summary>
    public ConfigurationTests() => DataHome.Clean();

    /// <summary>A missing preference file is written as the documented defaults.</summary>
    [Fact]
    public void APreferenceFileThatIsNotThereIsWrittenAsTheDocumentedDefaults()
    {
        var preference = ConfigurationLoader.LoadPreference();

        Assert.Equal("en", preference.Language);
        Assert.True(File.Exists(ConfigPaths.Preference));
        Assert.Contains(
            "\"_version\": 1",
            File.ReadAllText(ConfigPaths.Preference),
            StringComparison.Ordinal
        );
    }

    /// <summary>A missing theme file is written as the documented defaults, in the documented spellings.</summary>
    [Fact]
    public void AThemeFileThatIsNotThereIsWrittenAsTheDocumentedDefaults()
    {
        var theme = ConfigurationLoader.LoadTheme();

        Assert.Equal(ThemeConfiguration.DefaultMode, theme.ModeOrDefault);
        Assert.Equal(ThemeConfiguration.DefaultPrimary, theme.PrimaryOrDefault);
        Assert.Equal(ThemeConfiguration.DefaultAccent, theme.AccentOrDefault);
        Assert.True(File.Exists(ConfigPaths.Theme));

        // Read back as text, because what the file says is what a person edits: the colours have to be
        // the six digits the file documents rather than whatever a colour prints itself as.
        var written = File.ReadAllText(ConfigPaths.Theme);
        Assert.Contains("\"_version\": 1", written, StringComparison.Ordinal);
        Assert.Contains("\"mode\": \"system\"", written, StringComparison.Ordinal);
        Assert.Contains("\"primary\": \"#00BCD4\"", written, StringComparison.Ordinal);
        Assert.Contains("\"accent\": \"#FF4081\"", written, StringComparison.Ordinal);
    }

    /// <summary>A field the file does not name is a choice not made, which is not the same as the default.</summary>
    [Fact]
    public void AThemeFileThatNamesNothingFallsBackToTheDefaults()
    {
        Given(ConfigPaths.Theme, """{"_version": 1}""");

        var theme = ConfigurationLoader.LoadTheme();

        Assert.Null(theme.Mode);
        Assert.Null(theme.Primary);
        Assert.Null(theme.Accent);
        Assert.Equal(ThemeConfiguration.DefaultMode, theme.ModeOrDefault);
        Assert.Equal(ThemeConfiguration.DefaultPrimary, theme.PrimaryOrDefault);
        Assert.Equal(ThemeConfiguration.DefaultAccent, theme.AccentOrDefault);
    }

    /// <summary>A choice taken back is removed from the file rather than written down as the default.</summary>
    [Fact]
    public void AChoiceTakenBackIsWrittenAsItsAbsence()
    {
        Given(ConfigPaths.Theme, """{"_version": 1, "primary": "#3366FF"}""");

        var theme = ConfigurationLoader.LoadTheme();
        theme.Primary = null;
        ConfigurationLoader.WriteTheme(theme);

        var written = File.ReadAllText(ConfigPaths.Theme);
        Assert.DoesNotContain("primary", written, StringComparison.Ordinal);
        Assert.Null(ConfigurationLoader.LoadTheme().Primary);
    }

    /// <summary>A theme file that will not read stops the program with its own code.</summary>
    [Fact]
    public void AThemeFileThatIsNotJsonIsRefusedWithTheThemeCode()
    {
        Given(ConfigPaths.Theme, "{ not json");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadTheme);

        Assert.Equal(ExitCode.Theme, failure.Code);
        Assert.Contains("not valid JSON", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A mode that is none of the three is refused rather than defaulted.</summary>
    [Fact]
    public void AModeThatNamesNoVariantIsRefused()
    {
        Given(ConfigPaths.Theme, """{"_version": 1, "mode": "dusk"}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadTheme);

        Assert.Equal(ExitCode.Theme, failure.Code);
        Assert.Contains("system, light or dark", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>An accent that is not six digits after the hash is refused rather than approximated.</summary>
    [Theory]
    [InlineData("BFFF00")]
    [InlineData("#FFF")]
    [InlineData("#FFBFFF00")]
    [InlineData("#BFFFGG")]
    public void AnAccentThatIsNotSixDigitsIsRefused(string accent)
    {
        Given(ConfigPaths.Theme, $$"""{"_version": 1, "accent": "{{accent}}"}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadTheme);

        Assert.Equal(ExitCode.Theme, failure.Code);
        Assert.Contains("#RRGGBB", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A primary that is not six digits is refused too, and a refusal names the field it refuses.</summary>
    [Fact]
    public void APrimaryThatIsNotSixDigitsIsRefusedByName()
    {
        Given(ConfigPaths.Theme, """{"_version": 1, "primary": "#FFF"}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadTheme);

        Assert.Equal(ExitCode.Theme, failure.Code);
        Assert.Contains("`primary`", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>The variant and colours a theme file states are the ones read back.</summary>
    [Fact]
    public void TheModeAndColoursAThemeFileStatesAreRead()
    {
        Given(
            ConfigPaths.Theme,
            """{"_version": 1, "mode": "dark", "primary": "#3366FF", "accent": "#FF0000"}"""
        );

        var theme = ConfigurationLoader.LoadTheme();

        Assert.Equal(ColorMode.Dark, theme.ModeOrDefault);
        Assert.Equal(Color.FromRgb(0x33, 0x66, 0xFF), theme.PrimaryOrDefault);
        Assert.Equal(Color.FromRgb(0xFF, 0x00, 0x00), theme.AccentOrDefault);
    }

    /// <summary>A missing plugins file states no plugin rather than being a mistake.</summary>
    [Fact]
    public void APluginsFileThatIsNotThereStatesNoPlugin()
    {
        Assert.Empty(ConfigurationLoader.LoadPlugins().Plugins);
    }

    /// <summary>A plugins file that will not read stops the program with its own code.</summary>
    [Fact]
    public void APluginsFileThatIsNotJsonIsRefusedWithThePluginsCode()
    {
        Given(ConfigPaths.Plugins, "{ not json");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPlugins);

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("not valid JSON", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A plugins file of a version this program cannot read is refused.</summary>
    [Fact]
    public void APluginsVersionThisProgramCannotReadIsRefused()
    {
        Given(ConfigPaths.Plugins, """{"_version": 2, "plugins": {}}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPlugins);

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("_version", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A plugin named twice in the file is refused, rather than the last one winning.</summary>
    [Fact]
    public void APluginsKeyThatIsRepeatedIsRefused()
    {
        Given(
            ConfigPaths.Plugins,
            """{"_version": 1, "plugins": {"it.alpha": {}, "it.alpha": {}}}"""
        );

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPlugins);

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("repeated", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A key that answers to no discovered plugin is refused.</summary>
    [Fact]
    public void APluginsKeyThatNamesNoDiscoveredPluginIsRefused()
    {
        Given(
            ConfigPaths.Plugins,
            """{"_version": 1, "plugins": {"it.nothing": {"enabled": true}}}"""
        );

        var plugins = ConfigurationLoader.LoadPlugins();

        var failure = Assert.Throws<ConfigurationFailure>(() =>
            ConfigurationLoader.ReconcilePlugins(plugins, [new PluginId("it.alpha")])
        );

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("names no discovered plugin", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A discovered plugin the file does not name is enabled with order zero.</summary>
    [Fact]
    public void APluginTheFileDoesNotNameIsEnabledWithOrderZero()
    {
        Given(
            ConfigPaths.Plugins,
            """{"_version": 1, "plugins": {"it.alpha": {"enabled": false, "order": 3}}}"""
        );

        var plugins = ConfigurationLoader.LoadPlugins();
        ConfigurationLoader.ReconcilePlugins(
            plugins,
            [new PluginId("it.alpha"), new PluginId("it.beta")]
        );

        Assert.Equal(new PluginState(Enabled: false, Order: 3), plugins.Plugins[new PluginId("it.alpha")]);
        Assert.Equal(new PluginState(Enabled: true, Order: 0), plugins.Plugins[new PluginId("it.beta")]);
    }

    /// <summary>A preference file that will not read stops the program with its own code.</summary>
    [Fact]
    public void APreferenceFileThatIsNotJsonIsRefusedWithThePreferenceCode()
    {
        Given(ConfigPaths.Preference, "{ not json");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPreference);

        Assert.Equal(ExitCode.Preference, failure.Code);
    }

    /// <summary>A preference file of a version this program cannot read is refused.</summary>
    [Fact]
    public void APreferenceVersionThisProgramCannotReadIsRefused()
    {
        Given(ConfigPaths.Preference, """{"_version": 7}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPreference);

        Assert.Equal(ExitCode.Preference, failure.Code);
    }

    /// <summary>A plugin's own section is handed back as the type asked for, and otherwise as the fallback.</summary>
    [Fact]
    public void APluginSectionIsReadAsTheTypesAskedFor()
    {
        Given(
            ConfigPaths.Preference,
            """{"_version": 1, "plugin": {"it.alpha": {"view": "tree", "depth": 4, "flat": false}}}"""
        );

        var preference = ConfigurationLoader.LoadPreference();
        var config = new PluginConfigView(new SettingRegistry(preference), new PluginId("it.alpha"));

        Assert.Equal("tree", config.ReadKeyAs("view", "list"));
        Assert.Equal(4, config.ReadKeyAs("depth", 0));
        Assert.False(config.ReadKeyAs("flat", true));
        Assert.Equal("fallback", config.ReadKeyAs("nothing", "fallback"));
    }

    /// <summary>
    /// A setting a plugin declared reads as its declared default until the user chooses another, and as the
    /// choice once there is one — without the plugin repeating the default where it reads.
    /// </summary>
    [Fact]
    public void ADeclaredSettingReadsAsItsDefaultUntilItIsChosen()
    {
        Given(ConfigPaths.Preference, """{"_version": 1, "language": "en"}""");

        var preference = ConfigurationLoader.LoadPreference();
        var settings = new SettingRegistry(preference);
        var owner = new PluginId("it.alpha");

        settings.Declare(owner, new PluginSetting("Commands/move", SettingKind.Text, "label", "mv"));

        var config = new PluginConfigView(settings, owner);

        Assert.Equal("mv", config.ReadKeyAs("Commands/move", "nothing"));
        Assert.False(settings.Chosen(owner, "Commands/move"));

        // Kept the way the value's own kind is written, and read back as that kind rather than as text.
        settings.Keep(owner, settings.Of(owner)[0], "mv -v");
        Assert.True(settings.Chosen(owner, "Commands/move"));
        Assert.Equal("mv -v", config.ReadKeyAs("Commands/move", "nothing"));

        // Taking the value away is the way back to the default: what is then in force is the declaration's
        // rather than a copy of it, so a default changed later is not kept out by the copy.
        settings.Keep(owner, settings.Of(owner)[0], null);
        Assert.False(settings.Chosen(owner, "Commands/move"));
        Assert.Equal("mv", config.ReadKeyAs("Commands/move", "nothing"));
    }

    /// <summary>Writes a file under the scratch configuration root.</summary>
    /// <param name="path">Where the file goes.</param>
    /// <param name="content">What it says.</param>
    private static void Given(string path, string content)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, content);
    }
}

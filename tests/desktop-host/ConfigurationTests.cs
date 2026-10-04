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
        Assert.Contains("\"primary\": \"#FF7EA2\"", written, StringComparison.Ordinal);
        Assert.Contains("\"primaryText\": \"#42212A\"", written, StringComparison.Ordinal);
        Assert.Contains("\"accent\": \"#7EA2FF\"", written, StringComparison.Ordinal);
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

    /// <summary>
    /// The ink the look ships with belongs to the primary it ships with, and is not an ink for another colour.
    /// </summary>
    /// <remarks>
    /// What the file the program writes holds is a pair. A file holding that ink beside another primary is one
    /// whose primary was changed after the program wrote it, and the ink was never chosen for the colour it
    /// would now be written on: the reported fault was a black primary drawing its words in the shipped dark
    /// maroon, which is dark on dark. Such an ink is left out so that the look works one out by contrast, while
    /// an ink the file names for a primary of its own is the user's and is kept.
    /// </remarks>
    [Fact]
    public void TheInkTheLookShipsWithIsNotAnInkForAnotherPrimary()
    {
        var shipped = new ThemeConfiguration
        {
            Primary = ThemeConfiguration.DefaultPrimary,
            PrimaryText = ThemeConfiguration.DefaultPrimaryText,
        };

        Assert.Equal(ThemeConfiguration.DefaultPrimaryText, shipped.PrimaryTextOrDefault);

        var changed = new ThemeConfiguration
        {
            Primary = Colors.Black,
            PrimaryText = ThemeConfiguration.DefaultPrimaryText,
        };

        Assert.Null(changed.PrimaryTextOrDefault);
    }

    /// <summary>An ink the file names, for a primary of its own, is the user's and stands.</summary>
    [Fact]
    public void AnInkNamedForAPrimaryOfItsOwnStands()
    {
        var named = new ThemeConfiguration { Primary = Colors.Black, PrimaryText = Colors.White };

        Assert.Equal(Colors.White, named.PrimaryTextOrDefault);
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
            """{"_version": 1, "mode": "dark", "primary": "#3366FF", "primaryText": "#EEFFEE", "accent": "#FF0000"}"""
        );

        var theme = ConfigurationLoader.LoadTheme();

        Assert.Equal(ColorMode.Dark, theme.ModeOrDefault);
        Assert.Equal(Color.FromRgb(0x33, 0x66, 0xFF), theme.PrimaryOrDefault);
        Assert.Equal(Color.FromRgb(0xEE, 0xFF, 0xEE), theme.PrimaryText);
        Assert.Equal(Color.FromRgb(0xFF, 0x00, 0x00), theme.AccentOrDefault);
    }

    /// <summary>An ink that is not named is left to the look to work out rather than defaulted here.</summary>
    [Fact]
    public void AnInkTheFileDoesNotNameIsLeftToBeWorkedOut()
    {
        Given(ConfigPaths.Theme, """{"_version": 1, "primary": "#3366FF"}""");

        Assert.Null(ConfigurationLoader.LoadTheme().PrimaryText);
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
        Given(ConfigPaths.Plugins, """{"_version": 3, "plugins": []}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPlugins);

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("_version", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A plugin named twice in the file is refused, rather than the last one winning.</summary>
    [Fact]
    public void APluginNamedTwiceIsRefused()
    {
        Given(
            ConfigPaths.Plugins,
            """{"_version": 1, "plugins": [{"id": "it.alpha"}, {"id": "it.alpha"}]}"""
        );

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPlugins);

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("repeated", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A name that answers to no discovered plugin is refused.</summary>
    [Fact]
    public void APluginsNameThatNamesNoDiscoveredPluginIsRefused()
    {
        Given(
            ConfigPaths.Plugins,
            """{"_version": 1, "plugins": [{"id": "it.nothing", "enabled": true}]}"""
        );

        var plugins = ConfigurationLoader.LoadPlugins();

        var failure = Assert.Throws<ConfigurationFailure>(() =>
            ConfigurationLoader.ReconcilePlugins(plugins, [new PluginId("it.alpha")])
        );

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("names no discovered plugin", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A plugin entry the file does not name a plugin for is refused.</summary>
    [Fact]
    public void APluginsEntryWithoutAnIdIsRefused()
    {
        Given(ConfigPaths.Plugins, """{"_version": 1, "plugins": [{"enabled": false}]}""");

        var failure = Assert.Throws<ConfigurationFailure>(ConfigurationLoader.LoadPlugins);

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("must state `id`", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>
    /// A discovered plugin the file does not name is enabled and put at the end of the order.
    /// </summary>
    [Fact]
    public void APluginTheFileDoesNotNameIsEnabledAtTheEnd()
    {
        Given(
            ConfigPaths.Plugins,
            """{"_version": 1, "plugins": [{"id": "it.alpha", "enabled": false}]}"""
        );

        var plugins = ConfigurationLoader.LoadPlugins();
        ConfigurationLoader.ReconcilePlugins(
            plugins,
            [new PluginId("it.alpha"), new PluginId("it.beta")]
        );

        Assert.Equal(
            new[]
            {
                new PluginState(new PluginId("it.alpha"), Enabled: false),
                new PluginState(new PluginId("it.beta"), Enabled: true),
            },
            plugins.Plugins
        );
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

    /// <summary>
    /// A preset keeps no value of its own: it is shown as the option the settings it stands for agree with,
    /// choosing one writes them, a setting that no longer agrees makes the whole fall back, and taking the
    /// preset back to its default clears every setting it stands for.
    /// </summary>
    [Fact]
    public void APresetIsShownFromTheSettingsItStandsForAndWritesThemWhenChosen()
    {
        Given(ConfigPaths.Preference, """{"_version": 1, "language": "en"}""");

        var preference = ConfigurationLoader.LoadPreference();
        var settings = new SettingRegistry(preference);
        var owner = new PluginId("it.alpha");

        var copy = new PluginSetting("Commands/copy", SettingKind.Text, "copy", "cp");
        var move = new PluginSetting("Commands/move", SettingKind.Text, "move", "mv");
        settings.Declare(owner, copy);
        settings.Declare(owner, move);

        var posix = new SettingOption("posix", "label", Writes(("Commands/copy", "cp"), ("Commands/move", "mv")));
        var custom = new SettingOption("custom", "label");
        var preset = new PluginSetting("Commands/preset", SettingKind.Preset, "label", null, 0, false, [posix, custom]);
        settings.Declare(owner, preset);

        // The declared defaults are what the `posix` option names, so that is what is shown before anything
        // is chosen — nothing is kept for the preset itself.
        Assert.Equal("posix", settings.Value(owner, preset));
        Assert.False(settings.Chosen(owner, preset.Id));

        // One setting changed by hand is a whole that agrees with no option, so the fallback is shown.
        settings.Keep(owner, copy, "cp -r");
        Assert.Equal("custom", settings.Value(owner, preset));

        // Choosing an option writes every setting it names, which then agree with it again.
        settings.Choose(owner, preset, posix);
        Assert.Equal("cp", settings.Value(owner, copy));
        Assert.Equal("mv", settings.Value(owner, move));
        Assert.Equal("posix", settings.Value(owner, preset));

        // Choosing the fallback writes nothing, and taking the preset back clears what it stands for.
        settings.Choose(owner, preset, custom);
        Assert.Equal("cp", settings.Value(owner, copy));
        settings.Keep(owner, preset, null);
        Assert.False(settings.Chosen(owner, copy.Id));
        Assert.False(settings.Chosen(owner, move.Id));
        Assert.Equal("posix", settings.Value(owner, preset));
    }

    /// <summary>
    /// A multi-choice setting with nothing stored is shown as every option, which is the default a list of
    /// options is declared with; ticking one keeps the list; and unticking the last keeps an empty list,
    /// which is not the same as never having chosen — the default must not come back on its own.
    /// </summary>
    [Fact]
    public void AMultiChoiceSettingWithNothingStoredIsShownAsEveryOption()
    {
        Given(ConfigPaths.Preference, """{"_version": 1, "language": "en"}""");

        var preference = ConfigurationLoader.LoadPreference();
        var settings = new SettingRegistry(preference);
        var owner = new PluginId("it.alpha");

        var setting = new PluginSetting(
            "Hides/providers",
            SettingKind.MultiChoice,
            "label",
            null,
            0,
            false,
            [new SettingOption("dot_file", "one"), new SettingOption("dot_dir", "two")]
        );
        settings.Declare(owner, setting);

        var config = new PluginConfigView(settings, owner);

        // Never chosen: the panel shows every option, and nothing is stored for the reader to disagree with.
        Assert.False(settings.Chosen(owner, setting.Id));
        Assert.Equal("dot_file,dot_dir", settings.Value(owner, setting));

        // One option ticked is what is kept, as a list rather than as text.
        settings.Keep(owner, setting, "dot_dir");
        Assert.Equal(["dot_dir"], config.ReadKeyAs<string[]>(setting.Id)!);

        // Every option unticked keeps an empty list, which reads as that rather than as the default again.
        settings.Keep(owner, setting, string.Empty);
        Assert.True(settings.Chosen(owner, setting.Id));
        Assert.Empty(config.ReadKeyAs<string[]>(setting.Id) ?? []);

        // Taking the setting back is the default again, which the panel and the reader both work out.
        settings.Keep(owner, setting, null);
        Assert.False(settings.Chosen(owner, setting.Id));
        Assert.Equal("dot_file,dot_dir", settings.Value(owner, setting));
    }

    /// <summary>
    /// The switch that shows what is hidden reads as off until the user turns it on, and keeps what they
    /// chose — it is what decides whether a listing leaves the hidden entries out.
    /// </summary>
    [Fact]
    public void TheShowHiddenSwitchReadsAsOffUntilItIsTurnedOn()
    {
        Given(ConfigPaths.Preference, """{"_version": 1, "language": "en"}""");

        var preference = ConfigurationLoader.LoadPreference();
        var settings = new SettingRegistry(preference);
        var owner = new PluginId("it.alpha");

        settings.Declare(owner, new PluginSetting("Hides/show", SettingKind.Bool, "label", "false"));

        var config = new PluginConfigView(settings, owner);

        Assert.False(config.ReadKeyAs("Hides/show", true));

        settings.Keep(owner, settings.Of(owner)[0], "true");
        Assert.True(config.ReadKeyAs("Hides/show", false));

        settings.Keep(owner, settings.Of(owner)[0], "false");
        Assert.False(config.ReadKeyAs("Hides/show", true));
    }

    /// <summary>A preset option's writes, built from pairs.</summary>
    /// <param name="pairs">Setting identity to the value it is written as.</param>
    private static IReadOnlyDictionary<string, string> Writes(
        params (string Id, string Value)[] pairs
    ) => pairs.ToDictionary(pair => pair.Id, pair => pair.Value, StringComparer.Ordinal);

    /// <summary>Writes a file under the scratch configuration root.</summary>
    /// <param name="path">Where the file goes.</param>
    /// <param name="content">What it says.</param>
    private static void Given(string path, string content)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, content);
    }
}

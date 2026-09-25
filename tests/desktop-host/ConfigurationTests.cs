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

        Assert.Equal("rorolala.theme.default", preference.Theme);
        Assert.Equal("en", preference.Language);
        Assert.True(File.Exists(ConfigPaths.Preference));
        Assert.Contains(
            "\"_version\": 1",
            File.ReadAllText(ConfigPaths.Preference),
            StringComparison.Ordinal
        );
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
        var config = new PluginConfigView(preference, new PluginId("it.alpha"));

        Assert.Equal("tree", config.ReadKeyAs("view", "list"));
        Assert.Equal(4, config.ReadKeyAs("depth", 0));
        Assert.False(config.ReadKeyAs("flat", true));
        Assert.Equal("fallback", config.ReadKeyAs("nothing", "fallback"));
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

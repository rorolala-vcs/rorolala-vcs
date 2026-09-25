using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Plugins;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The plugins: what is discovered, what stops the program, and the order they are loaded in.
/// </summary>
/// <remarks>
/// The plugins are real assemblies, built as fixtures and loaded through the host's own load
/// context, so what is exercised is the contract boundary and not a stand-in for it.
/// </remarks>
public sealed class PluginTests
{
    /// <summary>Begins each test from no configuration at all.</summary>
    public PluginTests() => DataHome.Clean();

    /// <summary>A discovered plugin the file does not name is enabled with order zero.</summary>
    [Fact]
    public void EveryDiscoveredPluginIsEnabledWithOrderZeroWithoutAFile()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(new PluginsConfiguration());

        Assert.Equal(2, manager.Discovered.Count);
        Assert.Equal(
            new PluginState(Enabled: true, Order: 0),
            manager.Configuration.Plugins[new PluginId("it.alpha")]
        );
    }

    /// <summary>A dependency nothing discovered answers to stops the program.</summary>
    [Fact]
    public void ADependencyThatIsNotDiscoveredStopsTheProgram()
    {
        var manager = new PluginManager(Fixtures.Only("ItOrphan"));

        var failure = Assert.Throws<ConfigurationFailure>(() =>
            manager.Load(new PluginsConfiguration())
        );

        Assert.Equal(ExitCode.Plugins, failure.Code);
        Assert.Contains("it.missing", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A dependency the user disabled stops the program.</summary>
    [Fact]
    public void ADependencyThatIsDisabledStopsTheProgram()
    {
        var configuration = new PluginsConfiguration();
        configuration.Plugins[new PluginId("it.alpha")] = new PluginState(Enabled: false, Order: 0);

        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        Assert.Throws<ConfigurationFailure>(() => manager.Load(configuration));
    }

    /// <summary>A dependency cycle stops the program.</summary>
    [Fact]
    public void ADependencyCycleStopsTheProgram()
    {
        var manager = new PluginManager(Fixtures.Only("ItCycleOne", "ItCycleTwo"));

        var failure = Assert.Throws<ConfigurationFailure>(() =>
            manager.Load(new PluginsConfiguration())
        );

        Assert.Contains("cycle", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>A dependency is loaded before the plugin that depends on it.</summary>
    [Fact]
    public void ADependencyIsLoadedBeforeThePluginThatDependsOnIt()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(new PluginsConfiguration());

        Assert.Equal(
            new[] { "it.alpha", "it.beta" },
            manager.LoadOrder.Select(planned => planned.Id.Value).ToArray()
        );
    }

    /// <summary>An order that contradicts a dependency is kept, and the user is told who follows whom.</summary>
    [Fact]
    public void AnOrderThatContradictsADependencyIsKeptAndReported()
    {
        var configuration = new PluginsConfiguration();
        configuration.Plugins[new PluginId("it.alpha")] = new PluginState(Enabled: true, Order: 5);
        configuration.Plugins[new PluginId("it.beta")] = new PluginState(Enabled: true, Order: 0);

        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(configuration);

        Assert.Equal(
            new[] { "it.alpha", "it.beta" },
            manager.LoadOrder.Select(planned => planned.Id.Value).ToArray()
        );
        Assert.Contains(
            manager.OrderingNotes,
            note =>
                note.Subject == "it.beta"
                && note.Description.Contains("it.alpha", StringComparison.Ordinal)
        );
    }

    /// <summary>A plugin built against another contract is left out, and so is what depends on it.</summary>
    [Fact]
    public void APluginBuiltAgainstAnotherContractIsLeftOutWithItsDependents()
    {
        var manager = new PluginManager(Fixtures.Only("ItStale", "ItDependent"));

        manager.Load(new PluginsConfiguration());

        Assert.DoesNotContain(manager.LoadOrder, planned => planned.Id.Value == "it.stale");
        Assert.DoesNotContain(manager.LoadOrder, planned => planned.Id.Value == "it.dependent");
        Assert.Contains(manager.Problems, problem => problem.Subject == "it.stale");
        Assert.Contains(manager.Problems, problem => problem.Subject == "it.dependent");
    }

    /// <summary>A plugin whose start throws is left out, and the rest carry on.</summary>
    [Fact]
    public void APluginThatThrowsWhileStartingIsLeftOut()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItThrows"));
        manager.Load(new PluginsConfiguration());

        manager.InitializeAll(Host.Services());

        var throwing = manager.LoadOrder.Single(plugin => plugin.Id.Value == "it.throws");
        Assert.True(throwing.Skipped);
        Assert.False(throwing.Initialized);

        var quiet = manager.LoadOrder.Single(plugin => plugin.Id.Value == "it.alpha");
        Assert.True(quiet.Initialized);

        Assert.Contains(manager.Problems, problem => problem.Subject == "it.throws");
    }

    /// <summary>A directory holding no plugin discovers none, which is not a mistake.</summary>
    [Fact]
    public void ADirectoryWithNothingInItDiscoversNothing()
    {
        var manager = new PluginManager(Scratch.New("empty"));

        manager.Load(new PluginsConfiguration());

        Assert.Empty(manager.Discovered);
        Assert.Empty(manager.LoadOrder);
        Assert.Empty(manager.Problems);
    }
}

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

    /// <summary>A discovered plugin the file does not name is enabled and put at the end.</summary>
    [Fact]
    public void EveryDiscoveredPluginIsEnabledAndAppendedWithoutAFile()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(new PluginsConfiguration());

        Assert.Equal(2, manager.Discovered.Count);
        Assert.Equal(
            new[]
            {
                new PluginState(new PluginId("it.alpha"), Enabled: true),
                new PluginState(new PluginId("it.beta"), Enabled: true),
            },
            manager.Configuration.Plugins
        );
    }

    /// <summary>A dependency nothing discovered answers to leaves the plugin out rather than stopping.</summary>
    [Fact]
    public void ADependencyThatIsNotDiscoveredLeavesThePluginOut()
    {
        var manager = new PluginManager(Fixtures.Only("ItOrphan"));

        manager.Load(new PluginsConfiguration());

        Assert.Empty(manager.LoadOrder);
        Assert.Contains(
            manager.Faults(manager.Configuration.Plugins),
            fault =>
                fault.Plugin.Value == "it.orphan"
                && fault.Dependency.Value == "it.missing"
                && fault.Kind == PluginFaultKind.DependencyMissing
        );
    }

    /// <summary>A dependency the user disabled leaves the dependent out rather than stopping.</summary>
    [Fact]
    public void ADependencyThatIsDisabledLeavesTheDependentOut()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(Order(("it.alpha", false)));

        Assert.Empty(manager.LoadOrder);
        Assert.Contains(
            manager.Faults(manager.Configuration.Plugins),
            fault =>
                fault.Plugin.Value == "it.beta"
                && fault.Dependency.Value == "it.alpha"
                && fault.Kind == PluginFaultKind.DependencyDisabled
        );
    }

    /// <summary>A dependency cycle stops the program, because no order can honor one.</summary>
    [Fact]
    public void ADependencyCycleStopsTheProgram()
    {
        var manager = new PluginManager(Fixtures.Only("ItCycleOne", "ItCycleTwo"));

        var failure = Assert.Throws<ConfigurationFailure>(() =>
            manager.Load(new PluginsConfiguration())
        );

        Assert.Contains("cycle", failure.Message, StringComparison.Ordinal);
    }

    /// <summary>The file's sequence is the load order, and a dependency the user put first is honored.</summary>
    [Fact]
    public void TheFileSequenceIsTheLoadOrder()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(Order(("it.alpha", true), ("it.beta", true)));

        Assert.Equal(
            new[] { "it.alpha", "it.beta" },
            manager.LoadOrder.Select(planned => planned.Id.Value).ToArray()
        );
    }

    /// <summary>A dependency placed after its dependent leaves the dependent out.</summary>
    [Fact]
    public void AnOrderThatPutsADependencyLastLeavesTheDependentOut()
    {
        var manager = new PluginManager(Fixtures.Only("ItAlpha", "ItBeta"));

        manager.Load(Order(("it.beta", true), ("it.alpha", true)));

        Assert.Equal(
            new[] { "it.alpha" },
            manager.LoadOrder.Select(planned => planned.Id.Value).ToArray()
        );
        Assert.Contains(
            manager.Faults(manager.Configuration.Plugins),
            fault =>
                fault.Plugin.Value == "it.beta"
                && fault.Dependency.Value == "it.alpha"
                && fault.Kind == PluginFaultKind.DependencyLater
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

    /// <summary>A configuration holding the named plugins, in the order they are given.</summary>
    /// <param name="states">Each plugin's identity and whether it is enabled.</param>
    /// <returns>The user state a file would have held.</returns>
    private static PluginsConfiguration Order(params (string Id, bool Enabled)[] states)
    {
        var configuration = new PluginsConfiguration();

        foreach (var (id, enabled) in states)
        {
            configuration.Plugins.Add(new PluginState(new PluginId(id), enabled));
        }

        return configuration;
    }
}

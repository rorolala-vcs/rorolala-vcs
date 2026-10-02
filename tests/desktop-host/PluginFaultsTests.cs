using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Plugins;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What the order and the switches cost a plugin, worked out from them alone.
/// </summary>
/// <remarks>
/// This is the rule the plugin manager draws on every card, so it is checked without a plugin
/// assembly or a window: what a card says is a fact about the sequence, and the sequence is all
/// these tests state.
/// </remarks>
public sealed class PluginFaultsTests
{
    /// <summary>A dependency the user put after the plugin is what stops it.</summary>
    [Fact]
    public void ADependencyPlacedLaterIsAFault()
    {
        var faults = PluginFaults.Of(
            Order(("it.beta", true), ("it.alpha", true)),
            Graph(("it.alpha", []), ("it.beta", ["it.alpha"]))
        );

        Assert.Equal(
            new[] { new PluginFault(Id("it.beta"), Id("it.alpha"), PluginFaultKind.DependencyLater) },
            faults
        );
    }

    /// <summary>A dependency the user disabled is what stops the plugin.</summary>
    [Fact]
    public void ADisabledDependencyIsAFault()
    {
        var faults = PluginFaults.Of(
            Order(("it.alpha", false), ("it.beta", true)),
            Graph(("it.alpha", []), ("it.beta", ["it.alpha"]))
        );

        Assert.Equal(
            new[] { new PluginFault(Id("it.beta"), Id("it.alpha"), PluginFaultKind.DependencyDisabled) },
            faults
        );
    }

    /// <summary>A dependency that was never discovered is what stops the plugin.</summary>
    [Fact]
    public void AMissingDependencyIsAFault()
    {
        var faults = PluginFaults.Of(
            Order(("it.beta", true)),
            Graph(("it.beta", ["it.ghost"]))
        );

        Assert.Equal(
            new[] { new PluginFault(Id("it.beta"), Id("it.ghost"), PluginFaultKind.DependencyMissing) },
            faults
        );
    }

    /// <summary>
    /// A plugin whose dependency cannot load is stopped too, even though its own place is fine.
    /// </summary>
    [Fact]
    public void AFaultIsPassedOnToWhatDependsOnIt()
    {
        var faults = PluginFaults.Of(
            Order(("it.alpha", false), ("it.beta", true), ("it.gamma", true)),
            Graph(("it.alpha", []), ("it.beta", ["it.alpha"]), ("it.gamma", ["it.beta"]))
        );

        Assert.Contains(
            faults,
            fault =>
                fault.Plugin == Id("it.beta") && fault.Kind == PluginFaultKind.DependencyDisabled
        );
        Assert.Contains(
            faults,
            fault =>
                fault.Plugin == Id("it.gamma")
                && fault.Dependency == Id("it.beta")
                && fault.Kind == PluginFaultKind.DependencyUnavailable
        );
    }

    /// <summary>A plugin a higher layer marked unloadable is not blamed on its own card.</summary>
    /// <remarks>
    /// What a version mismatch means belongs to the plugin that has it; the card of a plugin that is
    /// switched off says nothing, because switching it off was the user's choice rather than a fault.
    /// </remarks>
    [Fact]
    public void ADisabledPluginIsNotFaultedItself()
    {
        var faults = PluginFaults.Of(
            Order(("it.beta", false)),
            Graph(("it.beta", ["it.ghost"]))
        );

        Assert.Empty(faults);
    }

    /// <summary>A plugin known to be unloadable elsewhere passes the fault on.</summary>
    [Fact]
    public void ABrokenPluginStopsWhatDependsOnIt()
    {
        var faults = PluginFaults.Of(
            Order(("it.alpha", true), ("it.beta", true)),
            Graph(("it.alpha", []), ("it.beta", ["it.alpha"])),
            new HashSet<PluginId> { Id("it.alpha") }
        );

        Assert.Equal(
            new[] { new PluginFault(Id("it.beta"), Id("it.alpha"), PluginFaultKind.DependencyUnavailable) },
            faults
        );
    }

    /// <summary>Two plugins waiting on each other are faults, and the later one is blamed first.</summary>
    [Fact]
    public void TwoPluginsThatWaitOnEachOtherAreBothFaulted()
    {
        var faults = PluginFaults.Of(
            Order(("it.alpha", true), ("it.beta", true)),
            Graph(("it.alpha", ["it.beta"]), ("it.beta", ["it.alpha"]))
        );

        Assert.Contains(
            faults,
            fault =>
                fault.Plugin == Id("it.alpha") && fault.Kind == PluginFaultKind.DependencyLater
        );
        Assert.Contains(
            faults,
            fault =>
                fault.Plugin == Id("it.beta")
                && fault.Kind == PluginFaultKind.DependencyUnavailable
        );
    }

    /// <summary>A plugin identity.</summary>
    /// <param name="value">The identity's text.</param>
    /// <returns>The identity.</returns>
    private static PluginId Id(string value) => new(value);

    /// <summary>The user's order, as the file would hold it.</summary>
    /// <param name="states">Each plugin's identity and whether it is enabled.</param>
    /// <returns>The sequence to answer for.</returns>
    private static List<PluginState> Order(params (string Id, bool Enabled)[] states) =>
        states.Select(state => new PluginState(Id(state.Id), state.Enabled)).ToList();

    /// <summary>What every discovered plugin declares, as discovery would answer it.</summary>
    /// <param name="plugins">Each plugin's identity and the identities it depends on.</param>
    /// <returns>The dependency map to answer from.</returns>
    private static Dictionary<PluginId, IReadOnlyList<PluginId>> Graph(
        params (string Id, string[] Dependencies)[] plugins
    ) =>
        plugins.ToDictionary(
            plugin => Id(plugin.Id),
            plugin => (IReadOnlyList<PluginId>)plugin.Dependencies.Select(Id).ToArray()
        );
}

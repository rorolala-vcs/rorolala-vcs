using RorolalaDesktop.Contract;

namespace ItCycleOne;

/// <summary>One half of a dependency cycle, together with <c>it.cycle_two</c>.</summary>
public sealed class CycleOnePlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.cycle_one"),
            "it_cycle_one.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId("it.cycle_two")]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

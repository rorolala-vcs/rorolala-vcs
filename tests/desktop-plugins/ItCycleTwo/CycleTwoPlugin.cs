using RorolalaDesktop.Contract;

namespace ItCycleTwo;

/// <summary>The other half of a dependency cycle, together with <c>it.cycle_one</c>.</summary>
public sealed class CycleTwoPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.cycle_two"),
            "it_cycle_two.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId("it.cycle_one")]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

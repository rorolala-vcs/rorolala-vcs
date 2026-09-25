using RorolalaDesktop.Contract;

namespace ItOrphan;

/// <summary>A fixture plugin whose declared dependency is never discovered.</summary>
public sealed class OrphanPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.orphan"),
            "it_orphan.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId("it.missing")]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

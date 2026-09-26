using RorolalaDesktop.Contract;

namespace ItStale;

/// <summary>
/// A fixture plugin built against a contract version this host does not speak, so it is refused.
/// </summary>
public sealed class StalePlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.stale"),
            "it_stale.name",
            new Version(9, 9, 9),
            []
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

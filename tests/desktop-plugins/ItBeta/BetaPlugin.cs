using RorolalaDesktop.Contract;

namespace ItBeta;

/// <summary>A fixture plugin that depends on <c>it.alpha</c>.</summary>
public sealed class BetaPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.beta"),
            "it_beta.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId("it.alpha")]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

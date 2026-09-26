using RorolalaDesktop.Contract;

namespace ItAlpha;

/// <summary>A fixture plugin with no dependencies: the bottom of a dependency tier.</summary>
public sealed class AlphaPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.alpha"),
            "it_alpha.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            []
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

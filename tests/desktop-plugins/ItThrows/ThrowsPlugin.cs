using RorolalaDesktop.Contract;

namespace ItThrows;

/// <summary>A fixture plugin that throws while starting, so it is left out and the rest carry on.</summary>
public sealed class ThrowsPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.throws"),
            "it_throws.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            []
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host) =>
        throw new InvalidOperationException("this plugin refuses to start");
}

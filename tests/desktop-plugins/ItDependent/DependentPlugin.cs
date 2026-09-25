using RorolalaDesktop.Contract;

namespace ItDependent;

/// <summary>
/// A fixture plugin that depends on <c>it.stale</c>, so it is left out with it when that plugin is
/// refused.
/// </summary>
public sealed class DependentPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("it.dependent"),
            "it_dependent.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId("it.stale")]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
    }
}

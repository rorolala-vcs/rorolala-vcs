namespace RorolalaDesktop.Contract;

/// <summary>
/// What a plugin says about itself: who it is, what it is called, what it was built against, and
/// what must be loaded before it.
/// </summary>
/// <remarks>
/// Everything here is declared by the plugin and none of it by the user. <c>plugins.json</c> records
/// only whether a plugin is enabled and how the user wants it ordered among its peers.
/// </remarks>
/// <param name="Id">The stable identity, globally unique.</param>
/// <param name="DisplayNameKey">
/// An i18n key the host renders to obtain the plugin's display name.
/// </param>
/// <param name="ContractVersion">
/// The version of the contract assembly the plugin was compiled against.
/// </param>
/// <param name="Dependencies">Other plugins that must be loaded first.</param>
public sealed record PluginManifest(
    PluginId Id,
    string DisplayNameKey,
    Version ContractVersion,
    IReadOnlyList<PluginId> Dependencies
);

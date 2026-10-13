using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a sync carries, as the three settings that choose it.
/// </summary>
/// <remarks>
/// The command line takes a sync apart into three parts — the index, the storage, the Layout — and a run that
/// leaves one behind is a run whose consequences the user has to know, which is why what is left out is the
/// user's answer and the warning before a run is read from it. All three are on until they are turned off: a
/// reader who has not thought about it means a sync that carries everything, and a sync that quietly left a
/// part behind would be a run that did less than it said.
/// <para>
/// Read through the config rather than kept here, because the setting is the user's and the file is where it
/// lives: what the dropdown beside the sync buttons changed is what every dock that draws one reads.
/// </para>
/// </remarks>
internal static class SyncSettings
{
    /// <summary>The setting that says whether the index is carried.</summary>
    public const string Index = "Sync/index";

    /// <summary>The setting that says whether the Layout is carried.</summary>
    public const string Layout = "Sync/layout";

    /// <summary>The setting that says whether the storage is carried.</summary>
    public const string Storage = "Sync/storage";

    /// <summary>Where the three are read from, once the plugin has declared them.</summary>
    private static IPluginConfig? _config;

    /// <summary>Declares the three, so that the preference panel shows them and keeps what is chosen.</summary>
    /// <param name="config">The plugin's own settings.</param>
    public static void Declare(IPluginConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);

        _config = config;

        // Named for what is carried rather than for the flag that leaves it out, because the box a user
        // ticks is what a sync does: a setting called "--no-index" would be off for "carry the index".
        Declare(config, Index, "rorolala_vcs.sync.index", 50);
        Declare(config, Layout, "rorolala_vcs.sync.layout", 51);
        Declare(config, Storage, "rorolala_vcs.sync.storage", 52);
    }

    /// <summary>What is to be carried, as the three settings stand now.</summary>
    /// <returns>The scope a sync is to be made in.</returns>
    public static SyncScope Chosen() =>
        new(
            _config?.ReadKeyAs(Index, true) ?? true,
            _config?.ReadKeyAs(Layout, true) ?? true,
            _config?.ReadKeyAs(Storage, true) ?? true
        );

    /// <summary>Whether one setting says its part is carried, which is what one box stands for.</summary>
    /// <remarks>
    /// Asked of the scope rather than of the config, so that a box and a run are answered by the same reading:
    /// a box that asked the file and a run that asked the scope would be two answers that could differ.
    /// </remarks>
    /// <param name="setting">Which of the three.</param>
    /// <returns>Whether that part is carried.</returns>
    public static bool Carries(string setting)
    {
        var scope = Chosen();

        return setting switch
        {
            Layout => scope.Layout,
            Storage => scope.Storage,
            _ => scope.Index,
        };
    }

    /// <summary>Keeps one of the three, which is what a box in the dropdown does when it is ticked.</summary>
    /// <param name="setting">Which of the three.</param>
    /// <param name="on">Whether it is to be carried.</param>
    public static void Choose(string setting, bool on) =>
        _config?.Keep(setting, on ? "true" : "false");

    /// <summary>Declares one of the three, as something that is on or off.</summary>
    /// <param name="config">The plugin's own settings.</param>
    /// <param name="id">The setting's identity.</param>
    /// <param name="labelKey">What the preference panel names it.</param>
    /// <param name="order">Where it stands among the plugin's settings.</param>
    private static void Declare(IPluginConfig config, string id, string labelKey, int order) =>
        config.Add(new PluginSetting(id, SettingKind.Bool, labelKey, "true", order));
}

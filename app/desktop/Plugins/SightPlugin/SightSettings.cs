using RorolalaDesktop.Contract;

namespace SightPlugin;

/// <summary>
/// What a preview reads and how it is drawn, as the user's answers rather than the plugin's.
/// </summary>
/// <remarks>
/// None of these is a control in the dock: the dock holds the preview, the previewer list, the
/// metadata and the switch that shows it, and nothing else. The limits are read afresh each time a
/// file is opened, so a value changed in the preference panel takes effect on the next preview.
/// </remarks>
internal static class SightSettings
{
    /// <summary>The setting the text limit is kept under.</summary>
    public const string TextLimitId = "Sight/text_limit";

    /// <summary>The setting the image limit is kept under.</summary>
    public const string ImageLimitId = "Sight/image_limit";

    /// <summary>The setting text wrapping is kept under.</summary>
    public const string WrapId = "Sight/wrap";

    /// <summary>The setting the model grid is kept under.</summary>
    public const string ModelGridId = "Sight/model_grid";

    /// <summary>How many MiB of text are read until it is called too large.</summary>
    private const int DefaultTextLimit = 2;

    /// <summary>How many MiB of an image are read until it is called too large.</summary>
    private const int DefaultImageLimit = 64;

    /// <summary>The largest limit a setting may state, which is a guard rather than a choice.</summary>
    private const int LargestLimit = 4096;

    /// <summary>Where the settings are read from, once the plugin has declared them.</summary>
    private static IPluginConfig? _config;

    /// <summary>Declares the four, so that the preference panel shows them and keeps what is chosen.</summary>
    /// <param name="config">The plugin's own settings.</param>
    public static void Declare(IPluginConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);

        _config = config;

        config.Add(
            new PluginSetting(
                TextLimitId,
                SettingKind.Number,
                "rorolala_sight.setting.text_limit",
                DefaultTextLimit.ToString(System.Globalization.CultureInfo.InvariantCulture),
                10
            )
        );
        config.Add(
            new PluginSetting(
                ImageLimitId,
                SettingKind.Number,
                "rorolala_sight.setting.image_limit",
                DefaultImageLimit.ToString(System.Globalization.CultureInfo.InvariantCulture),
                20
            )
        );
        config.Add(
            new PluginSetting(
                WrapId,
                SettingKind.Bool,
                "rorolala_sight.setting.wrap",
                "true",
                30
            )
        );
        config.Add(
            new PluginSetting(
                ModelGridId,
                SettingKind.Bool,
                "rorolala_sight.setting.model_grid",
                "true",
                40
            )
        );
    }

    /// <summary>How many bytes of text are read until the file is called too large.</summary>
    public static long TextLimitBytes => Bytes(Number(TextLimitId, DefaultTextLimit));

    /// <summary>How many bytes of an image are read until the file is called too large.</summary>
    public static long ImageLimitBytes => Bytes(Number(ImageLimitId, DefaultImageLimit));

    /// <summary>Whether text lines are wrapped rather than read across.</summary>
    public static bool Wrap => _config?.ReadKeyAs(WrapId, true) ?? true;

    /// <summary>Whether a model is drawn on a grid.</summary>
    public static bool ModelGrid => _config?.ReadKeyAs(ModelGridId, true) ?? true;

    /// <summary>One limit, in MiB, as it is stored.</summary>
    /// <param name="id">The setting.</param>
    /// <param name="fallback">What it is until it is changed.</param>
    /// <returns>The limit, clamped to something a machine can hold.</returns>
    private static int Number(string id, int fallback)
    {
        var value = _config?.ReadKeyAs(id, fallback) ?? fallback;

        return Math.Clamp(value, 1, LargestLimit);
    }

    /// <summary>MiB as bytes.</summary>
    /// <param name="mib">How many MiB.</param>
    /// <returns>The bytes.</returns>
    private static long Bytes(int mib) => mib * 1024L * 1024L;
}

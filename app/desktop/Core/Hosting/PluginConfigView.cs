using System.Text.Json;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// One plugin's own settings, as that plugin sees them: what the file states, and what the plugin declared.
/// </summary>
/// <remarks>
/// The host interprets nothing beyond the value's own kind. A setting the file does not state reads as the
/// default the plugin declared for it, and one that is neither stated nor declared reads as the fallback the
/// caller passed — so a plugin may read a key it never declared, which is what a hand-written file has.
/// <para>
/// Booleans, integers, floating-point numbers, strings, lists and enums are read directly; enums are read by
/// name.
/// </para>
/// </remarks>
internal sealed class PluginConfigView : IPluginConfig
{
    /// <summary>How a value is read when it is not an enum.</summary>
    private static readonly JsonSerializerOptions Options = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    /// <summary>What every owner declared, and what each is worth.</summary>
    private readonly SettingRegistry _settings;

    /// <summary>The plugin whose settings these are.</summary>
    private readonly PluginId _id;

    /// <summary>Makes a view of one plugin's settings.</summary>
    /// <param name="settings">What every owner declared, and what each is worth.</param>
    /// <param name="id">The plugin whose settings this is.</param>
    public PluginConfigView(SettingRegistry settings, PluginId id)
    {
        _settings = settings;
        _id = id;
    }

    /// <inheritdoc />
    public T? ReadKeyAs<T>(string id, T? fallback = default) =>
        _settings.InForce(_id, id) is { } element ? Read<T>(element, fallback) : fallback;

    /// <inheritdoc />
    public void Add(PluginSetting setting) => _settings.Declare(_id, setting);

    /// <summary>
    /// Reads one value as the requested type, or the fallback where it does not read as one.
    /// </summary>
    /// <remarks>
    /// A value that is there but does not read as the requested type is treated as one that is not there.
    /// The host never interprets a plugin's settings, so a mismatch between what a plugin declared and what
    /// it asks for is the plugin's to make sense of, and the fallback is what its own default is.
    /// </remarks>
    /// <param name="element">What is stored, or the declared default.</param>
    /// <param name="fallback">What to answer where it does not read as the requested type.</param>
    private static T? Read<T>(JsonElement element, T? fallback)
    {
        try
        {
            var type = typeof(T);

            if (type.IsEnum)
            {
                var name =
                    element.ValueKind == JsonValueKind.String
                        ? element.GetString()
                        : element.GetRawText();

                return (T)Enum.Parse(type, name!, ignoreCase: true);
            }

            return element.Deserialize<T>(Options);
        }
        catch (Exception error)
            when (error
                    is JsonException
                        or FormatException
                        or InvalidOperationException
                        or NotSupportedException
                        or ArgumentException
            )
        {
            return fallback;
        }
    }
}

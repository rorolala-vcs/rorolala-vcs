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

        // Re-said for this plugin's own settings only: the registry knows every owner, and a plugin is not
        // told about a setting it does not own. Both halves are forwarded, because a listener acts on the
        // value and the identity together.
        _settings.Changed += setting =>
        {
            if (settings.Of(_id).Any(declared => string.Equals(declared.Id, setting, StringComparison.Ordinal)))
            {
                Changed?.Invoke(setting);
            }
        };
    }

    /// <inheritdoc />
    public event Action<string>? Changed;

    /// <inheritdoc />
    public T? ReadKeyAs<T>(string id, T? fallback = default) =>
        _settings.InForce(_id, id) is { } element ? Read<T>(element, fallback) : fallback;

    /// <inheritdoc />
    public void Keep(string id, string? value)
    {
        // Written as the value's own kind where the plugin declared one, and as text otherwise: the host
        // interprets no key, so a plugin keeping one it never declared is keeping a string.
        _settings.Keep(
            _id,
            _settings.Of(_id).FirstOrDefault(setting => string.Equals(setting.Id, id, StringComparison.Ordinal))
                ?? new PluginSetting(id, SettingKind.Text, id),
            value
        );
    }

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

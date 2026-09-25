using System.Text.Json;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// One plugin's own section of <c>preference.json</c>, as that plugin sees it.
/// </summary>
/// <remarks>
/// The host does not interpret any of it: a key is handed back as the type asked for, and a key that
/// is not stated, or that does not read as that type, reads as the fallback. Booleans, integers,
/// floating-point numbers, strings, lists and enums are read directly; enums are read by name.
/// </remarks>
internal sealed class PluginConfigView : IPluginConfig
{
    /// <summary>How a value is read when it is not an enum.</summary>
    private static readonly JsonSerializerOptions Options = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    /// <summary>The plugin's section, or an empty one when it states none.</summary>
    private readonly IReadOnlyDictionary<string, JsonElement> _section;

    /// <summary>Makes a view of one plugin's section.</summary>
    /// <param name="preference">Everything the user's preferences state.</param>
    /// <param name="id">The plugin whose section this is.</param>
    public PluginConfigView(PreferenceConfiguration preference, PluginId id) =>
        _section = preference.Plugin.TryGetValue(id, out var section)
            ? section
            : new Dictionary<string, JsonElement>(StringComparer.Ordinal);

    /// <inheritdoc />
    public T? ReadKeyAs<T>(string key, T? fallback = default)
    {
        if (!_section.TryGetValue(key, out var element))
        {
            return fallback;
        }

        try
        {
            return Read<T>(element);
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
            // A key that is there but does not read as the requested type is treated as one that is
            // not there. The host never interprets a plugin's keys, so a mismatch is the plugin's to
            // make sense of, and the fallback is what its own default is.
            return fallback;
        }
    }

    /// <summary>Reads one value as the requested type.</summary>
    private static T? Read<T>(JsonElement element)
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
}

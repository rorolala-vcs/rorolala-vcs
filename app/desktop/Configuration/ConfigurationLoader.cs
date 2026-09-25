using System.Text.Json;
using System.Text.Json.Serialization;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Configuration;

/// <summary>
/// Reads and writes the two configuration files, and refuses what it cannot.
/// </summary>
/// <remarks>
/// A file that is not there is not a mistake: the program uses the defaults and writes a file, so
/// that the next run and the plugin manager have something to edit. A file that is there but
/// unreadable, malformed, or names something this program does not support stops the program with a
/// reason on standard error (Section 14.1), because a silently ignored configuration error is worse
/// than a loud stop.
/// <para>
/// The JSON is read a property at a time rather than deserialized into a type, so that a repeated
/// key — which a deserializer would quietly keep the last of — is caught and refused.
/// </para>
/// </remarks>
internal static class ConfigurationLoader
{
    /// <summary>How the files are written, so a person can read and edit them.</summary>
    private static readonly JsonSerializerOptions Indented = new() { WriteIndented = true };

    /// <summary>
    /// Reads <c>plugins.json</c>, or an empty configuration when the file is not there.
    /// </summary>
    /// <remarks>
    /// The keys are checked against the discovered plugins later, once discovery has run; this reads
    /// the file's own shape and grammar.
    /// </remarks>
    /// <exception cref="ConfigurationFailure">The file is unreadable, malformed, or unsupported.</exception>
    public static PluginsConfiguration LoadPlugins()
    {
        var path = ConfigPaths.Plugins;

        if (!File.Exists(path))
        {
            return new PluginsConfiguration();
        }

        using var document = ParseJson(ReadText(path, ExitCode.Plugins), path, ExitCode.Plugins);

        return ParsePlugins(document.RootElement, path);
    }

    /// <summary>
    /// Reads <c>preference.json</c>, writing and answering the defaults when the file is not there.
    /// </summary>
    /// <exception cref="ConfigurationFailure">The file is unreadable, malformed, or unsupported.</exception>
    public static PreferenceConfiguration LoadPreference()
    {
        var path = ConfigPaths.Preference;

        if (!File.Exists(path))
        {
            var defaults = new PreferenceConfiguration();
            WritePreference(defaults);
            return defaults;
        }

        using var document = ParseJson(
            ReadText(path, ExitCode.Preference),
            path,
            ExitCode.Preference
        );

        return ParsePreference(document.RootElement, path);
    }

    /// <summary>
    /// Completes the plugins' state for the plugins that were discovered, and writes it back.
    /// </summary>
    /// <remarks>
    /// A plugin the file does not name is enabled with order <c>0</c>. A key the file names that no
    /// discovered plugin answers to is refused: it is either a plugin that was removed, whose state
    /// would otherwise linger, or a mistake in the file.
    /// </remarks>
    /// <param name="config">What the file stated, or empty when it was not there.</param>
    /// <param name="discovered">Every plugin discovery found.</param>
    /// <returns>The completed state, written to the file.</returns>
    /// <exception cref="ConfigurationFailure">A key names no discovered plugin.</exception>
    public static PluginsConfiguration ReconcilePlugins(
        PluginsConfiguration config,
        IReadOnlyCollection<PluginId> discovered
    )
    {
        foreach (var id in config.Plugins.Keys)
        {
            if (!discovered.Contains(id))
            {
                throw new ConfigurationFailure(
                    ExitCode.Plugins,
                    $"{ConfigPaths.Plugins}: `{id}` names no discovered plugin"
                );
            }
        }

        foreach (var id in discovered)
        {
            config.Plugins.TryAdd(id, new PluginState(Enabled: true, Order: 0));
        }

        WritePlugins(config);

        return config;
    }

    /// <summary>Writes the plugins' user state.</summary>
    /// <param name="config">What to write.</param>
    public static void WritePlugins(PluginsConfiguration config)
    {
        var dto = new PluginsDto { Version = PluginsConfiguration.SchemaVersion };

        foreach (var (id, state) in config.Plugins)
        {
            dto.Plugins[id.Value] = new PluginStateDto
            {
                Enabled = state.Enabled,
                Order = state.Order,
            };
        }

        Write(ConfigPaths.Plugins, dto, ExitCode.Plugins);
    }

    /// <summary>Writes the user's preferences.</summary>
    /// <param name="config">What to write.</param>
    public static void WritePreference(PreferenceConfiguration config)
    {
        var dto = new PreferenceDto
        {
            Version = PreferenceConfiguration.SchemaVersion,
            Theme = config.Theme,
            Language = config.Language,
        };

        foreach (var (id, section) in config.Plugin)
        {
            dto.Plugin[id.Value] = section;
        }

        Write(ConfigPaths.Preference, dto, ExitCode.Preference);
    }

    /// <summary>Reads the shape of <c>plugins.json</c>.</summary>
    private static PluginsConfiguration ParsePlugins(JsonElement root, string path)
    {
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw Fail(ExitCode.Plugins, path, "the file must be a JSON object");
        }

        CheckVersion(root, path, ExitCode.Plugins, PluginsConfiguration.SchemaVersion);

        if (
            !root.TryGetProperty("plugins", out var plugins)
            || plugins.ValueKind != JsonValueKind.Object
        )
        {
            throw Fail(ExitCode.Plugins, path, "`plugins` must be an object");
        }

        var config = new PluginsConfiguration();
        var seen = new HashSet<string>(StringComparer.Ordinal);

        foreach (var property in plugins.EnumerateObject())
        {
            if (!seen.Add(property.Name))
            {
                throw Fail(ExitCode.Plugins, path, $"`{property.Name}` is repeated");
            }

            if (!PluginId.IsWellFormed(property.Name))
            {
                throw Fail(ExitCode.Plugins, path, $"`{property.Name}` is not a plugin id");
            }

            config.Plugins[new PluginId(property.Name)] = ParseState(
                property.Value,
                path,
                property.Name
            );
        }

        return config;
    }

    /// <summary>Reads the shape of one plugin's state entry.</summary>
    private static PluginState ParseState(JsonElement element, string path, string id)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw Fail(ExitCode.Plugins, path, $"`{id}` must be an object");
        }

        var enabled = true;
        var order = 0;

        foreach (var property in element.EnumerateObject())
        {
            switch (property.Name)
            {
                case "enabled":
                    if (
                        property.Value.ValueKind
                        is not (JsonValueKind.True or JsonValueKind.False)
                    )
                    {
                        throw Fail(ExitCode.Plugins, path, $"`{id}.enabled` must be true or false");
                    }

                    enabled = property.Value.GetBoolean();
                    break;

                case "order":
                    if (!TryInt(property.Value, out order))
                    {
                        throw Fail(ExitCode.Plugins, path, $"`{id}.order` must be an integer");
                    }

                    break;

                default:
                    // A field this program does not know is left alone, so a file written by a later
                    // version still loads here rather than stopping the program.
                    break;
            }
        }

        return new PluginState(enabled, order);
    }

    /// <summary>Reads the shape of <c>preference.json</c>.</summary>
    private static PreferenceConfiguration ParsePreference(JsonElement root, string path)
    {
        if (root.ValueKind != JsonValueKind.Object)
        {
            throw Fail(ExitCode.Preference, path, "the file must be a JSON object");
        }

        CheckVersion(root, path, ExitCode.Preference, PreferenceConfiguration.SchemaVersion);

        var config = new PreferenceConfiguration();

        if (root.TryGetProperty("theme", out var theme))
        {
            if (theme.ValueKind != JsonValueKind.String)
            {
                throw Fail(ExitCode.Preference, path, "`theme` must be a string");
            }

            config.Theme = theme.GetString()!;
        }

        if (root.TryGetProperty("language", out var language))
        {
            if (language.ValueKind != JsonValueKind.String)
            {
                throw Fail(ExitCode.Preference, path, "`language` must be a string");
            }

            config.Language = language.GetString()!;
        }

        if (root.TryGetProperty("plugin", out var plugin))
        {
            ParsePluginSections(plugin, path, config);
        }

        return config;
    }

    /// <summary>Reads every plugin's own section, keeping each value as it stands.</summary>
    private static void ParsePluginSections(
        JsonElement plugin,
        string path,
        PreferenceConfiguration config
    )
    {
        if (plugin.ValueKind != JsonValueKind.Object)
        {
            throw Fail(ExitCode.Preference, path, "`plugin` must be an object");
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);

        foreach (var section in plugin.EnumerateObject())
        {
            if (!seen.Add(section.Name))
            {
                throw Fail(ExitCode.Preference, path, $"`plugin.{section.Name}` is repeated");
            }

            if (!PluginId.IsWellFormed(section.Name))
            {
                throw Fail(ExitCode.Preference, path, $"`{section.Name}` is not a plugin id");
            }

            if (section.Value.ValueKind != JsonValueKind.Object)
            {
                throw Fail(
                    ExitCode.Preference,
                    path,
                    $"`plugin.{section.Name}` must be an object"
                );
            }

            var keys = new Dictionary<string, JsonElement>(StringComparer.Ordinal);

            foreach (var key in section.Value.EnumerateObject())
            {
                // Cloned: the value outlives the document it was read from, which is disposed here.
                keys[key.Name] = key.Value.Clone();
            }

            config.Plugin[new PluginId(section.Name)] = keys;
        }
    }

    /// <summary>Refuses a file whose <c>_version</c> is absent or is one this program cannot read.</summary>
    private static void CheckVersion(JsonElement root, string path, ExitCode code, int supported)
    {
        if (!root.TryGetProperty("_version", out var version))
        {
            throw Fail(code, path, "`_version` is missing");
        }

        if (!TryInt(version, out var stated) || stated != supported)
        {
            throw Fail(code, path, $"`_version` must be {supported}");
        }
    }

    /// <summary>Whether an element is a JSON integer, and what it is.</summary>
    private static bool TryInt(JsonElement element, out int value)
    {
        value = 0;

        return element.ValueKind == JsonValueKind.Number && element.TryGetInt32(out value);
    }

    /// <summary>Reads a file, refusing one that cannot be read.</summary>
    private static string ReadText(string path, ExitCode code)
    {
        try
        {
            return File.ReadAllText(path);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            throw new ConfigurationFailure(code, $"{path}: {error.Message}");
        }
    }

    /// <summary>Parses a file as JSON, refusing one that is not.</summary>
    private static JsonDocument ParseJson(string text, string path, ExitCode code)
    {
        try
        {
            return JsonDocument.Parse(
                text,
                new JsonDocumentOptions
                {
                    CommentHandling = JsonCommentHandling.Skip,
                    AllowTrailingCommas = true,
                }
            );
        }
        catch (JsonException error)
        {
            throw new ConfigurationFailure(code, $"{path}: not valid JSON — {error.Message}");
        }
    }

    /// <summary>Writes a file, making the directory it sits in.</summary>
    private static void Write<T>(string path, T value, ExitCode code)
    {
        try
        {
            System.IO.Directory.CreateDirectory(ConfigPaths.Root);
            File.WriteAllText(path, JsonSerializer.Serialize(value, Indented));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            throw new ConfigurationFailure(code, $"{path}: {error.Message}");
        }
    }

    /// <summary>A reason that stops the program, with the code the process exits with.</summary>
    private static ConfigurationFailure Fail(ExitCode code, string path, string reason) =>
        new(code, $"{path}: {reason}");

    /// <summary>How <c>plugins.json</c> is written.</summary>
    private sealed class PluginsDto
    {
        [JsonPropertyName("_version")]
        public int Version { get; set; }

        [JsonPropertyName("plugins")]
        public Dictionary<string, PluginStateDto> Plugins { get; set; } =
            new(StringComparer.Ordinal);
    }

    /// <summary>How one plugin's state is written.</summary>
    private sealed class PluginStateDto
    {
        [JsonPropertyName("enabled")]
        public bool Enabled { get; set; }

        [JsonPropertyName("order")]
        public int Order { get; set; }
    }

    /// <summary>How <c>preference.json</c> is written.</summary>
    private sealed class PreferenceDto
    {
        [JsonPropertyName("_version")]
        public int Version { get; set; }

        [JsonPropertyName("theme")]
        public string Theme { get; set; } = PreferenceConfiguration.DefaultTheme;

        [JsonPropertyName("language")]
        public string Language { get; set; } = PreferenceConfiguration.DefaultLanguage;

        [JsonPropertyName("plugin")]
        public Dictionary<string, Dictionary<string, JsonElement>> Plugin { get; set; } =
            new(StringComparer.Ordinal);
    }
}

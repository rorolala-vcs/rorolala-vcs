using System.Text.Json;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Configuration;

/// <summary>The user's state for one plugin: whether it loads, and how the user ordered it.</summary>
/// <param name="Enabled">Whether the plugin loads.</param>
/// <param name="Order">The user's ordering within one dependency tier.</param>
internal sealed record PluginState(bool Enabled, int Order);

/// <summary>
/// <c>plugins.json</c>: the user's state for plugins that have been discovered.
/// </summary>
/// <remarks>
/// The file stores user state only. It does not list plugin paths, dependencies, contract versions,
/// or display names — those are declared by the plugin itself and discovered from the assembly.
/// </remarks>
internal sealed class PluginsConfiguration
{
    /// <summary>The schema version this program reads and writes.</summary>
    public const int SchemaVersion = 1;

    /// <summary>Every plugin the user has state for, in the order the file states them.</summary>
    public Dictionary<PluginId, PluginState> Plugins { get; } = [];
}

/// <summary>
/// <c>preference.json</c>: the user's preferences, and each plugin's own section.
/// </summary>
/// <remarks>
/// The host never interprets a plugin's keys. It reads the section for a plugin, hands it over
/// through <c>IPluginConfig</c>, and the plugin reads what it put there.
/// </remarks>
internal sealed class PreferenceConfiguration
{
    /// <summary>The schema version this program reads and writes.</summary>
    public const int SchemaVersion = 1;

    /// <summary>The theme used when the file names none.</summary>
    public const string DefaultTheme = "rorolala.theme.default";

    /// <summary>The locale used when neither the command line nor the file names one.</summary>
    public const string DefaultLanguage = "en";

    /// <summary>The id of the theme to apply, or <c>fluent</c> for the base theme alone.</summary>
    public string Theme { get; set; } = DefaultTheme;

    /// <summary>The fallback locale, used only when the command line passes no language.</summary>
    public string Language { get; set; } = DefaultLanguage;

    /// <summary>Each plugin's own section, keyed by the plugin's identity.</summary>
    public Dictionary<PluginId, Dictionary<string, JsonElement>> Plugin { get; } = [];
}

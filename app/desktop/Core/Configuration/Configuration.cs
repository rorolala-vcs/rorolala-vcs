using System.Text.Json;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Configuration;

/// <summary>Which of the two variants the program is drawn in.</summary>
internal enum ColorMode
{
    /// <summary>Whichever variant the desktop is using, followed as it changes.</summary>
    System,

    /// <summary>The light variant, whatever the desktop is using.</summary>
    Light,

    /// <summary>The dark variant, whatever the desktop is using.</summary>
    Dark,
}

/// <summary>
/// <c>theme.json</c>: the two things the user chooses about how the program looks.
/// </summary>
/// <remarks>
/// The look itself is not one of them. It is the program's own — rectangles, one-pixel edges, hover in
/// the variant's ink and the accent spent on what is selected — and it is what makes the program look
/// like one thing rather than like whatever it was assembled out of. What is left to choose is the
/// variant it is drawn in and the one colour anything accented is drawn in.
/// <para>
/// It is a file of its own rather than a section of <c>preference.json</c> for the same reason: these
/// two are not a preference about the program but what the program is drawn in, and nothing else is
/// allowed beside them.
/// </para>
/// </remarks>
internal sealed class ThemeConfiguration
{
    /// <summary>The schema version this program reads and writes.</summary>
    public const int SchemaVersion = 1;

    /// <summary>The variant used when the file names none.</summary>
    public const ColorMode DefaultMode = ColorMode.System;

    /// <summary>
    /// The accent used when the file names none.
    /// </summary>
    /// <remarks>
    /// The lemon the program was drawn in while the accent was a constant, so that a file nobody has
    /// written yet is the look the program has always had.
    /// </remarks>
    public static readonly Color DefaultAccent = Color.FromRgb(0xBF, 0xFF, 0x00);

    /// <summary>The variant the program is drawn in.</summary>
    public ColorMode Mode { get; set; } = DefaultMode;

    /// <summary>The colour anything accented is drawn in.</summary>
    public Color Accent { get; set; } = DefaultAccent;
}

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
    public const int SchemaVersion = 2;

    /// <summary>The locale used when neither the command line nor the file names one.</summary>
    public const string DefaultLanguage = "en";

    /// <summary>The fallback locale, used only when the command line passes no language.</summary>
    public string Language { get; set; } = DefaultLanguage;

    /// <summary>Each plugin's own section, keyed by the plugin's identity.</summary>
    public Dictionary<PluginId, Dictionary<string, JsonElement>> Plugin { get; } = [];
}

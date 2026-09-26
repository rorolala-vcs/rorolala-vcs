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
/// <c>theme.json</c>: the three things the user chooses about how the program looks.
/// </summary>
/// <remarks>
/// The look itself is not one of them. It is the program's own — rectangles, one-pixel edges, hover in
/// the variant's ink and the two colours spent on what is selected and what asks for attention — and it
/// is what makes the program look like one thing rather than like whatever it was assembled out of. What
/// is left to choose is the variant it is drawn in and the two colours it is drawn with (Section 10).
/// <para>
/// Every field is optional, and a field that is not there is the default: the file holds only what the
/// user chose, so taking a choice back is removing it rather than writing the default down (Section 5.4).
/// That is why the fields are nullable rather than defaulted — <c>null</c> is "the file says nothing",
/// which is not the same as "the file chose the default".
/// </para>
/// <para>
/// It is a file of its own rather than a section of <c>preference.json</c> for the same reason: these
/// are not a preference about the program but what the program is drawn in, and nothing else is allowed
/// beside them.
/// </para>
/// </remarks>
internal sealed class ThemeConfiguration
{
    /// <summary>The schema version this program reads and writes.</summary>
    public const int SchemaVersion = 1;

    /// <summary>The variant used when the file names none.</summary>
    public const ColorMode DefaultMode = ColorMode.System;

    /// <summary>
    /// The primary used when the file names none.
    /// </summary>
    /// <remarks>
    /// A pale cyan: a light, quiet brand colour that the ink on it can be read against.
    /// </remarks>
    public static readonly Color DefaultPrimary = Color.FromRgb(0xB2, 0xEB, 0xF2);

    /// <summary>
    /// The accent used when the file names none.
    /// </summary>
    /// <remarks>
    /// A pale pink, and spent only on the marks a drag draws — never on a selection, so that a mark can
    /// always be told from a choice.
    /// </remarks>
    public static readonly Color DefaultAccent = Color.FromRgb(0xF8, 0xBB, 0xD0);

    /// <summary>
    /// What is written on the primary when the file names none.
    /// </summary>
    /// <remarks>
    /// A dark tone of the primary's own hue, the way the design this look comes from names its own ink for
    /// its own lime rather than working one out. A primary the user chooses has no such name, so the look
    /// works one out by contrast when the file states none (Section 10).
    /// </remarks>
    public static readonly Color DefaultPrimaryText = Color.FromRgb(0x0E, 0x24, 0x27);

    /// <summary>The variant the program is drawn in, or nothing when the file names none.</summary>
    public ColorMode? Mode { get; set; }

    /// <summary>The colour the brand and everything selected is drawn in, or nothing.</summary>
    public Color? Primary { get; set; }

    /// <summary>
    /// What is written on a surface filled with the primary, or nothing to work it out by contrast.
    /// </summary>
    public Color? PrimaryText { get; set; }

    /// <summary>The colour the drag marks are drawn in, or nothing.</summary>
    public Color? Accent { get; set; }

    /// <summary>The variant in force, which is the default when the file names none.</summary>
    public ColorMode ModeOrDefault => Mode ?? DefaultMode;

    /// <summary>The primary in force, which is the default when the file names none.</summary>
    public Color PrimaryOrDefault => Primary ?? DefaultPrimary;

    /// <summary>The accent in force, which is the default when the file names none.</summary>
    public Color AccentOrDefault => Accent ?? DefaultAccent;
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
    public const int SchemaVersion = 1;

    /// <summary>The locale used when neither the command line nor the file names one.</summary>
    public const string DefaultLanguage = "en";

    /// <summary>The fallback locale, used only when the command line passes no language.</summary>
    public string Language { get; set; } = DefaultLanguage;

    /// <summary>Each plugin's own section, keyed by the plugin's identity.</summary>
    public Dictionary<PluginId, Dictionary<string, JsonElement>> Plugin { get; } = [];
}

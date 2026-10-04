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
    /// A warm pink: a quiet brand colour that the dark ink on it can be read against.
    /// </remarks>
    public static readonly Color DefaultPrimary = Color.FromRgb(0xFF, 0x7E, 0xA2);

    /// <summary>
    /// The accent used when the file names none.
    /// </summary>
    /// <remarks>
    /// A pale blue, and spent only on the marks a drag draws — never on a selection, so that a mark can
    /// always be told from a choice.
    /// </remarks>
    public static readonly Color DefaultAccent = Color.FromRgb(0x7E, 0xA2, 0xFF);

    /// <summary>
    /// What is written on the primary when the file names none.
    /// </summary>
    /// <remarks>
    /// A dark tone of the primary's own hue, the way the design this look comes from names its own ink for
    /// its own lime rather than working one out. A primary the user chooses has no such name, so the look
    /// works one out by contrast when the file states none (Section 10).
    /// </remarks>
    public static readonly Color DefaultPrimaryText = Color.FromRgb(0x42, 0x21, 0x2A);

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

    /// <summary>
    /// What is written on a surface filled with the primary, or nothing to work one out by contrast.
    /// </summary>
    /// <remarks>
    /// The ink the look ships with is the ink <em>for the primary it ships with</em>: a dark tone of that
    /// colour's own hue, named rather than worked out, the way the design it comes from names its own. A file
    /// holding that ink beside another primary is one whose primary was changed after the program wrote the
    /// pair, and the named ink was never chosen for the colour it would now be written on — dark words on a dark
    /// fill, for a primary the user darkened. Such an ink is left out rather than written on, so that what
    /// stands on a filled surface is worked out from what it stands on, which is what the look does for every
    /// primary the file names no ink for.
    /// </remarks>
    public Color? PrimaryTextOrDefault =>
        PrimaryText == DefaultPrimaryText && PrimaryOrDefault != DefaultPrimary ? null : PrimaryText;
}

/// <summary>
/// The user's state for one plugin: whether it loads, and where it stands.
/// </summary>
/// <remarks>
/// The position is not held here but by the sequence the record sits in: the order is what the user
/// arranged by hand, and a number beside each plugin would be a second statement of it that can
/// disagree with the first.
/// </remarks>
/// <param name="Id">The plugin's identity.</param>
/// <param name="Enabled">Whether the plugin loads.</param>
internal sealed record PluginState(PluginId Id, bool Enabled);

/// <summary>
/// <c>plugins.json</c>: the user's state for plugins that have been discovered.
/// </summary>
/// <remarks>
/// The file stores user state only. It does not list plugin paths, dependencies, contract versions,
/// or display names — those are declared by the plugin itself and discovered from the assembly.
/// <para>
/// The sequence is the load order. It is the user's to arrange rather than the program's to work
/// out, so a dependency that is disabled, absent, or placed later makes the dependent unloadable
/// rather than being silently reordered, and the plugin manager says so on the card it belongs to.
/// </para>
/// </remarks>
internal sealed class PluginsConfiguration
{
    /// <summary>
    /// The schema version this program reads and writes.
    /// </summary>
    /// <remarks>
    /// It stays at the lowest it has ever been and is never raised. A change to the file's shape is
    /// made in place, so a file of an older shape is refused for its shape rather than converted,
    /// and no reader here ever grows a branch for a version it no longer writes.
    /// </remarks>
    public const int SchemaVersion = 1;

    /// <summary>Every plugin the user has state for, in the order the user put them.</summary>
    public List<PluginState> Plugins { get; } = [];

    /// <summary>The state of one plugin, or nothing when the file does not name it.</summary>
    /// <param name="id">The plugin's identity.</param>
    /// <returns>The state the file holds, or nothing.</returns>
    public PluginState? Find(PluginId id) => Plugins.Find(state => state.Id == id);

    /// <summary>Whether the file leaves a plugin enabled; a plugin it does not name is not enabled.</summary>
    /// <param name="id">The plugin's identity.</param>
    /// <returns>Whether the plugin loads.</returns>
    public bool Enabled(PluginId id) => Find(id)?.Enabled ?? false;

    /// <summary>Whether the file holds any state for a plugin.</summary>
    /// <param name="id">The plugin's identity.</param>
    /// <returns>Whether the plugin is named.</returns>
    public bool Contains(PluginId id) => Find(id) is not null;
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

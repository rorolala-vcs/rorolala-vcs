using System.Text.Json;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Configuration;

/// <summary>
/// What every owner has declared as settable, and what each declaration is worth right now.
/// </summary>
/// <remarks>
/// It stands between the two halves of a preference: the file, which holds only what the user chose, and
/// the declaration, which says what a setting is and what it is until then. The file alone cannot tell a
/// setting that was never touched from one that does not exist; the declarations alone cannot tell what
/// the user chose. This reads the two together, which is what lets a plugin ask for a value and be given
/// the declared default without repeating it.
/// <para>
/// A declaration's identity is <c>Group/Key</c>, and that whole identity is the key it is kept under in
/// its owner's own section — so the name a reader sees in the panel and the name the file holds are one
/// name, and neither has to be translated to find the other.
/// </para>
/// <para>
/// An owner is a plugin, or the kernel for what the host itself declares; the kernel's own section is
/// kept under its identity like any other.
/// </para>
/// </remarks>
internal sealed class SettingRegistry
{
    /// <summary>What the user chose, as the file states it.</summary>
    private readonly PreferenceConfiguration _preference;

    /// <summary>What each owner declared, in the order it declared them.</summary>
    private readonly Dictionary<PluginId, List<PluginSetting>> _declared = [];

    /// <summary>Makes a registry over what the file states.</summary>
    /// <param name="preference">Everything the user's preferences state.</param>
    public SettingRegistry(PreferenceConfiguration preference) => _preference = preference;

    /// <summary>
    /// The owners that declared anything, in the order they were first heard from.
    /// </summary>
    /// <remarks>
    /// Declaration order is load order, the kernel first, because the kernel declares before any plugin
    /// is started — which is the same order the panel wants to list them in.
    /// </remarks>
    public IReadOnlyList<PluginId> Owners => [.. _declared.Keys];

    /// <summary>What one owner declared, in the order it declared them.</summary>
    /// <param name="owner">The owner to ask about.</param>
    public IReadOnlyList<PluginSetting> Of(PluginId owner) =>
        _declared.TryGetValue(owner, out var settings) ? settings : [];

    /// <summary>
    /// Declares a setting.
    /// </summary>
    /// <remarks>
    /// A second declaration of an identity already declared is ignored rather than replacing the first, so
    /// that a plugin initializing twice — or two plugins claiming one name — cannot change what the panel
    /// shows after it has shown it.
    /// </remarks>
    /// <param name="owner">The owner declaring it.</param>
    /// <param name="setting">The setting.</param>
    public void Declare(PluginId owner, PluginSetting setting)
    {
        if (!_declared.TryGetValue(owner, out var settings))
        {
            settings = [];
            _declared[owner] = settings;
        }

        if (!settings.Any(other => string.Equals(other.Id, setting.Id, StringComparison.Ordinal)))
        {
            settings.Add(setting);
        }
    }

    /// <summary>Whether the user has chosen a value for a setting.</summary>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="id">The setting's identity.</param>
    public bool Chosen(PluginId owner, string id) => Stored(owner, id) is not null;

    /// <summary>
    /// What a setting is worth, as the text the panel shows and takes.
    /// </summary>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="setting">The setting.</param>
    /// <returns>The chosen value, the declared default, or nothing.</returns>
    public string? Value(PluginId owner, PluginSetting setting) =>
        Stored(owner, setting.Id) is { } stored ? Show(setting.Kind, stored) : setting.Default;

    /// <summary>
    /// Keeps what the user chose, or takes the setting back to its default when nothing is given.
    /// </summary>
    /// <remarks>
    /// The file is written at once rather than at exit, because a setting the user chose and a run that
    /// then failed is a choice that would otherwise be lost with the failure.
    /// </remarks>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="setting">The setting.</param>
    /// <param name="text">What the panel holds, or nothing to leave the setting at its default.</param>
    public void Keep(PluginId owner, PluginSetting setting, string? text)
    {
        var section = Section(owner);

        if (string.IsNullOrEmpty(text))
        {
            section.Remove(setting.Id);
        }
        else if (Element(setting.Kind, text) is { } element)
        {
            section[setting.Id] = element;
        }

        ConfigurationLoader.WritePreference(_preference);
    }

    /// <summary>
    /// The value in force for a reader: what the user chose, or the declaration's own default.
    /// </summary>
    /// <remarks>
    /// A key that was stored without ever being declared reads as itself, so that a hand-written file is
    /// still read the way it says.
    /// </remarks>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="id">The setting's identity.</param>
    /// <returns>The element in force, or nothing when there is none.</returns>
    public JsonElement? InForce(PluginId owner, string id)
    {
        if (Stored(owner, id) is { } stored)
        {
            return stored;
        }

        return Declared(owner, id) is { Default: { Length: > 0 } text } setting
            ? Element(setting.Kind, text)
            : null;
    }

    /// <summary>What one owner declared under an identity, or nothing.</summary>
    /// <param name="owner">The owner to ask about.</param>
    /// <param name="id">The identity to look for.</param>
    private PluginSetting? Declared(PluginId owner, string id) =>
        Of(owner).FirstOrDefault(setting => string.Equals(setting.Id, id, StringComparison.Ordinal));

    /// <summary>The value the file states for a setting, or nothing when it states none.</summary>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="id">The setting's identity.</param>
    private JsonElement? Stored(PluginId owner, string id) =>
        _preference.Plugin.TryGetValue(owner, out var section) && section.TryGetValue(id, out var element)
            ? element
            : null;

    /// <summary>One owner's own section, made when it has none yet.</summary>
    /// <param name="owner">The owner the section belongs to.</param>
    private Dictionary<string, JsonElement> Section(PluginId owner)
    {
        if (!_preference.Plugin.TryGetValue(owner, out var section))
        {
            section = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
            _preference.Plugin[owner] = section;
        }

        return section;
    }

    /// <summary>
    /// What a value is written as, by the kind of setting it belongs to.
    /// </summary>
    /// <remarks>
    /// The written form is a JSON scalar of the setting's own kind rather than always a string: a boolean
    /// written as text would be a boolean a reader could not read, and the file is meant to be read by a
    /// person as well as by this program.
    /// </remarks>
    /// <param name="kind">The kind of the setting.</param>
    /// <param name="text">The value as the panel holds it.</param>
    /// <returns>What to write, or nothing when the text is not a value of that kind.</returns>
    private static JsonElement? Element(SettingKind kind, string text) =>
        kind switch
        {
            SettingKind.Bool when bool.TryParse(text, out var flag) => JsonSerializer.SerializeToElement(flag),
            SettingKind.Number when double.TryParse(text, out var number) => JsonSerializer.SerializeToElement(number),
            SettingKind.Bool or SettingKind.Number => null,
            _ => JsonSerializer.SerializeToElement(text),
        };

    /// <summary>What a written value reads as in the panel.</summary>
    /// <param name="kind">The kind of the setting.</param>
    /// <param name="element">What the file holds.</param>
    private static string Show(SettingKind kind, JsonElement element) =>
        kind switch
        {
            SettingKind.Bool => element.ValueKind == JsonValueKind.True ? "true" : "false",
            SettingKind.Number => element.GetRawText(),
            _ => element.ValueKind == JsonValueKind.String ? element.GetString() ?? string.Empty : element.GetRawText(),
        };
}

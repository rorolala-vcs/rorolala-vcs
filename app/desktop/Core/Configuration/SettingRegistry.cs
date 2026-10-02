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
    /// Raised when a setting was kept or taken back, so that whoever acts on it acts at once.
    /// </summary>
    /// <remarks>
    /// Raised after the file is written, so a listener that reads the setting back reads what was written.
    /// A setting kept without being declared raises it too: what the file states is a reader's business,
    /// not this registry's. The identity is what was written, which is how the owner of one setting acts on
    /// it alone.
    /// </remarks>
    public event Action<string>? Changed;

    /// <summary>
    /// What a setting is worth, as the text the panel shows and takes.
    /// </summary>
    /// <remarks>
    /// A multi-choice setting with nothing stored reads as every option it offers, which is the default a
    /// list of options is declared with: the panel draws them all ticked, and a reader that works its own
    /// default out from the options is given the same answer.
    /// </remarks>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="setting">The setting.</param>
    /// <returns>The chosen value, the declared default, or nothing.</returns>
    public string? Value(PluginId owner, PluginSetting setting) =>
        setting.Kind == SettingKind.Preset
            ? Preset(owner, setting)
            : InForce(owner, setting.Id) is { } element
                ? Show(setting.Kind, element)
                : setting.Kind == SettingKind.MultiChoice
                    ? string.Join(',', (setting.Options ?? []).Select(option => option.Value))
                    : setting.Default;

    /// <summary>
    /// Chooses one option of a preset: writes every setting the option names, or, for the fallback,
    /// nothing.
    /// </summary>
    /// <remarks>
    /// Nothing is kept for the preset itself — it stands for other settings and has no value of its own
    /// — so choosing writes them and the choice is then read back from what they are worth.
    /// </remarks>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="setting">The preset.</param>
    /// <param name="option">The option that was chosen.</param>
    public void Choose(PluginId owner, PluginSetting setting, SettingOption option)
    {
        if (option.Writes is not { Count: > 0 } writes)
        {
            return;
        }

        foreach (var (id, text) in writes)
        {
            Write(owner, id, text);
        }

        ConfigurationLoader.WritePreference(_preference);

        foreach (var (id, _) in writes)
        {
            Changed?.Invoke(id);
        }
    }

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
        // A preset keeps nothing of its own: taking it back to its default is taking away every setting
        // it stands for, so that each reads as its declaration's default again.
        if (setting.Kind == SettingKind.Preset)
        {
            var written = Section(owner);

            foreach (var id in Written(setting))
            {
                written.Remove(id);
            }

            ConfigurationLoader.WritePreference(_preference);

            foreach (var id in Written(setting))
            {
                Changed?.Invoke(id);
            }

            return;
        }

        var section = Section(owner);

        // Nothing at all is what taking the setting back writes, and what the user asks for when they
        // reset it. An empty text is not that: a multi-choice setting with nothing chosen keeps an empty
        // list, because "nothing is chosen" and "this was never chosen" are different states and the
        // second would read as the declaration's default again the moment the last box was unticked.
        if (text is null)
        {
            section.Remove(setting.Id);
        }
        else if (Element(setting.Kind, text) is { } element)
        {
            section[setting.Id] = element;
        }

        ConfigurationLoader.WritePreference(_preference);
        Changed?.Invoke(setting.Id);
    }

    /// <summary>
    /// The value in force for a reader: what the user chose, or the declaration's own default.
    /// </summary>
    /// <remarks>
    /// A key that was stored without ever being declared reads as itself, so that a hand-written file is
    /// still read the way it says. A multi-choice setting with nothing stored has nothing in force here:
    /// what its options mean as a default is the declaration's business, and the panel and the reader work
    /// it out from the options they both see.
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

    /// <summary>The value a preset is shown as: the option the settings agree with, or the fallback.</summary>
    /// <remarks>
    /// Only the option whose every written setting is currently worth what it names counts as agreed;
    /// when none does, the one option naming nothing is shown, which is the state a hand-adjusted set of
    /// settings is in.
    /// </remarks>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="setting">The preset.</param>
    /// <returns>The value of the option to show, or nothing when the preset offers none.</returns>
    private string? Preset(PluginId owner, PluginSetting setting)
    {
        var options = setting.Options ?? [];

        foreach (var option in options)
        {
            if (
                option.Writes is { Count: > 0 } writes
                && writes.All(pair => string.Equals(Effective(owner, pair.Key), pair.Value, StringComparison.Ordinal))
            )
            {
                return option.Value;
            }
        }

        return options.FirstOrDefault(option => option.Writes is null)?.Value;
    }

    /// <summary>The value in force for an identity: what was chosen, or the declaration's default.</summary>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="id">The setting's identity.</param>
    private string? Effective(PluginId owner, string id) =>
        Stored(owner, id) is { } stored
            ? Show(Declared(owner, id)?.Kind ?? SettingKind.Text, stored)
            : Declared(owner, id)?.Default;

    /// <summary>Writes one setting by identity, the way choosing a preset option does.</summary>
    /// <param name="owner">The owner of the setting.</param>
    /// <param name="id">The setting's identity.</param>
    /// <param name="text">What to write, or nothing to leave the setting at its default.</param>
    private void Write(PluginId owner, string id, string text)
    {
        var section = Section(owner);

        if (string.IsNullOrEmpty(text))
        {
            section.Remove(id);
            return;
        }

        var kind = Declared(owner, id)?.Kind ?? SettingKind.Text;

        if (Element(kind, text) is { } element)
        {
            section[id] = element;
        }
    }

    /// <summary>Every identity a preset's options name, however it is chosen.</summary>
    /// <param name="setting">The preset.</param>
    private static IEnumerable<string> Written(PluginSetting setting) =>
        (setting.Options ?? [])
            .SelectMany(option => option.Writes?.Keys ?? [])
            .Distinct(StringComparer.Ordinal);

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
            SettingKind.MultiChoice => JsonSerializer.SerializeToElement(Chosen(text)),
            _ => JsonSerializer.SerializeToElement(text),
        };

    /// <summary>
    /// The values a multi-choice setting holds, read from the text they are written as.
    /// </summary>
    /// <remarks>
    /// An empty text is no values at all rather than one empty value, because taking every value away is
    /// one of the states the setting has — and the one that reads as the default.
    /// </remarks>
    /// <param name="text">The values, comma-separated.</param>
    private static string[] Chosen(string text) =>
        text.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);

    /// <summary>What a written value reads as in the panel.</summary>
    /// <param name="kind">The kind of the setting.</param>
    /// <param name="element">What the file holds.</param>
    private static string Show(SettingKind kind, JsonElement element) =>
        kind switch
        {
            SettingKind.Bool => element.ValueKind == JsonValueKind.True ? "true" : "false",
            SettingKind.Number => element.GetRawText(),
            SettingKind.MultiChoice => string.Join(
                ',',
                element.ValueKind == JsonValueKind.Array
                    ? element.EnumerateArray().Select(value => value.GetString() ?? string.Empty)
                    : []
            ),
            _ => element.ValueKind == JsonValueKind.String ? element.GetString() ?? string.Empty : element.GetRawText(),
        };
}

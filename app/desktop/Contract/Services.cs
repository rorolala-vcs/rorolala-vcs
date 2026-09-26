namespace RorolalaDesktop.Contract;

/// <summary>
/// Where a plugin writes what it wants to say.
/// </summary>
/// <remarks>
/// What a plugin writes reaches the Log dock, which is the host's one log and not a second system a
/// plugin keeps for itself. A repeated message is one entry with a count rather than many lines.
/// </remarks>
public interface ILog
{
    /// <summary>Writes a message about the finest steps.</summary>
    /// <param name="message">What to say.</param>
    void Trace(string message);

    /// <summary>Writes a message useful while developing.</summary>
    /// <param name="message">What to say.</param>
    void Debug(string message);

    /// <summary>Writes a message about what happened.</summary>
    /// <param name="message">What to say.</param>
    void Info(string message);

    /// <summary>Writes a message about something that may need attention.</summary>
    /// <param name="message">What to say.</param>
    void Warn(string message);

    /// <summary>Writes a message about something that went wrong.</summary>
    /// <param name="message">What to say.</param>
    void Error(string message);
}

/// <summary>
/// Where a plugin adds the translations it carries.
/// </summary>
public interface II18n
{
    /// <summary>
    /// Registers a directory of translation files, read after the ones already registered.
    /// </summary>
    /// <remarks>
    /// First registration wins: a key already supplied is not replaced by a later directory. The
    /// host's own directory is registered first, so a plugin cannot overwrite what the host says.
    /// </remarks>
    /// <param name="directory">The directory every one of the plugin's translation files sits under.</param>
    void RegisterDirectory(string directory);
}

/// <summary>
/// The section of <c>preference.json</c> belonging to one plugin, and what the plugin asks the host to
/// show of it.
/// </summary>
/// <remarks>
/// The host never interprets a plugin's keys; it hands the plugin the section and the plugin reads it.
/// A key the plugin's section does not state, or that does not read as the requested type, reads as the
/// setting's own default and then as the fallback.
/// <para>
/// A plugin may also <em>declares what its keys are</em>, which is what lets the host show them: a
/// setting has an identity of the form <c>Group/Key</c>, and everything a reader needs to render one
/// without being told what it means.
/// </para>
/// </remarks>
public interface IPluginConfig
{
    /// <summary>
    /// Reads one of the plugin's settings as a value of the requested type.
    /// </summary>
    /// <typeparam name="T">The type to read the value as.</typeparam>
    /// <param name="id">The setting's identity, written as <c>Group/Key</c>.</param>
    /// <param name="fallback">What to answer when the setting is not stored and states no default.</param>
    /// <returns>The value, the setting's default, or <paramref name="fallback"/>.</returns>
    T? ReadKeyAs<T>(string id, T? fallback = default);

    /// <summary>
    /// Declares a setting, so that the host shows it and keeps what the user chooses.
    /// </summary>
    /// <remarks>
    /// Called during initialization, with the rest of what a plugin registers. A setting declared
    /// after the window is shown is not supported, the same as any other registration (§6.1).
    /// </remarks>
    /// <param name="setting">The setting to declare.</param>
    void Add(PluginSetting setting);
}

/// <summary>What sort of value a setting holds.</summary>
public enum SettingKind
{
    /// <summary>Free text.</summary>
    Text,

    /// <summary>Something that is on or off.</summary>
    Bool,

    /// <summary>A number.</summary>
    Number,

    /// <summary>One of the values the setting offers.</summary>
    Choice,
}

/// <summary>One value a <see cref="SettingKind.Choice"/> setting offers.</summary>
/// <param name="Value">The value, as it is kept.</param>
/// <param name="LabelKey">An i18n key naming it.</param>
public sealed record SettingOption(string Value, string LabelKey);

/// <summary>
/// One setting a plugin declares: what it is called, what it means, and what it is until it is changed.
/// </summary>
/// <remarks>
/// A setting's identity is <c>Group/Key</c>. The part before the slash is the group it is shown under
/// within its owner; the whole of it is what it is kept under in that owner's own section, so that the
/// name a reader sees and the name the file holds are the same name.
/// </remarks>
/// <param name="Id">The identity, written as <c>Group/Key</c>.</param>
/// <param name="Kind">What sort of value it holds.</param>
/// <param name="LabelKey">An i18n key naming it.</param>
/// <param name="Default">
/// What it holds until the user chooses another, written the way it is kept: <c>true</c> or <c>false</c>
/// for a boolean, a number written out for a number, and the text itself otherwise. Nothing means it
/// holds nothing until it is set.
/// </param>
/// <param name="Order">Where it sits among the settings of its group.</param>
/// <param name="RestartRequired">Whether it takes effect on the next start rather than at once.</param>
/// <param name="Options">The values a <see cref="SettingKind.Choice"/> offers, and nothing otherwise.</param>
public sealed record PluginSetting(
    string Id,
    SettingKind Kind,
    string LabelKey,
    string? Default = null,
    int Order = 0,
    bool RestartRequired = false,
    IReadOnlyList<SettingOption>? Options = null
);

/// <summary>
/// What Rorolala can do, injected over the C ABI.
/// </summary>
/// <remarks>
/// A plugin calls this; it never invokes the <c>rola</c> command line and never reimplements
/// Rorolala semantics. The surface corresponds to what the C ABI exports today and is filled in as
/// the exports are; the file-level storage operations are a known gap with no export yet.
/// </remarks>
public interface IRola { }

/// <summary>
/// The host's windows being come back to.
/// </summary>
/// <remarks>
/// A plugin that reads the world outside the program — a filesystem, a device, a share — has nothing to tell it
/// that the user went away and came back, which is the moment something out there is most likely to have changed.
/// The host knows, because its windows tell it, so the host says so.
/// </remarks>
public interface IRefocus
{
    /// <summary>Raised when one of the host's windows is activated again.</summary>
    event Action? Regained;
}

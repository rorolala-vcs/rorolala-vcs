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
/// The section of <c>preference.json</c> belonging to one plugin.
/// </summary>
/// <remarks>
/// The host never interprets a plugin's keys; it hands the plugin the section and the plugin reads
/// it. A key the plugin's section does not state, or that does not read as the requested type,
/// reads as the fallback.
/// </remarks>
public interface IPluginConfig
{
    /// <summary>
    /// Reads one of the plugin's keys as a value of the requested type.
    /// </summary>
    /// <typeparam name="T">The type to read the value as.</typeparam>
    /// <param name="key">The key, within the plugin's own section.</param>
    /// <param name="fallback">What to answer when the key is not stated or does not read as a value.</param>
    /// <returns>The value, or <paramref name="fallback"/>.</returns>
    T? ReadKeyAs<T>(string key, T? fallback = default);
}

/// <summary>
/// What Rorolala can do, injected over the C ABI.
/// </summary>
/// <remarks>
/// A plugin calls this; it never invokes the <c>rola</c> command line and never reimplements
/// Rorolala semantics. The surface corresponds to what the C ABI exports today and is filled in as
/// the exports are; the file-level storage operations are a known gap with no export yet.
/// </remarks>
public interface IRola { }

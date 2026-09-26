using System.Diagnostics.CodeAnalysis;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The host's translations, which the plugins add theirs to.
/// </summary>
/// <remarks>
/// The host's own directory is registered before any plugin's, and merging is first-registration-
/// wins, so a plugin cannot overwrite a word the host says and a plugin loaded earlier cannot be
/// overwritten by one loaded later.
/// </remarks>
internal sealed class I18nService : II18n
{
    /// <inheritdoc />
    public void RegisterDirectory(string directory) =>
        RolaI18N.RegisterTranslationDirectory(directory);

    /// <summary>The form a key is written in, in the language the program speaks.</summary>
    /// <param name="key">The key, as the files nest it.</param>
    /// <returns>What the key says, or the key itself when nothing states it.</returns>
    [SuppressMessage(
        "Performance",
        "CA1822:Mark members as static",
        Justification = "the host hands this service to whatever needs words and those holders read the language through it — `_i18n.Get(...)` — so it is reached as a role rather than as a type; what is behind it is the process's own translations, which is why the method itself needs no instance"
    )]
    public string Get(string key) => RolaI18N.Get(key);

    /// <summary>The form a key is written in, with one value in its first place.</summary>
    /// <param name="key">The key, as the files nest it.</param>
    /// <param name="first">The value for the first place the form leaves.</param>
    /// <returns>What the key says, with the value in it.</returns>
    [SuppressMessage(
        "Performance",
        "CA1822:Mark members as static",
        Justification = "as the other overload: reached through the service a holder was handed rather than through the type"
    )]
    public string Get(string key, object? first) => RolaI18N.Get(key, first);
}

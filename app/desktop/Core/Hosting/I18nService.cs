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
    public string Get(string key) => RolaI18N.Get(key);

    /// <summary>The form a key is written in, with one value in its first place.</summary>
    /// <param name="key">The key, as the files nest it.</param>
    /// <param name="first">The value for the first place the form leaves.</param>
    /// <returns>What the key says, with the value in it.</returns>
    public string Get(string key, object? first) => RolaI18N.Get(key, first);
}

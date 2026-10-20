using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;

namespace RorolalaLauncher;

/// <summary>One language the program may speak, and what it calls itself in it.</summary>
/// <param name="Locale">The locale, as the translation files name it.</param>
/// <param name="Name">The language's own name for itself, which is what its readers are looking for.</param>
internal sealed record LanguageOption(string Locale, string Name);

/// <summary>
/// The language this program speaks, and the choice of it.
/// </summary>
/// <remarks>
/// The Desktop program is handed a language by the command line and keeps one in <c>preference.json</c>; the
/// launcher is started by a person rather than by a command, so the same two answers are asked for here,
/// and one more for the first run: a machine whose own language is one the files speak is spoken to in it,
/// because there is no choice written down yet to say otherwise. A choice, once written, is the choice.
/// <para>
/// The two languages a person may choose between are stated here rather than read from the translation
/// files: a file names the languages it writes in, and a choice is what this build offers, and the two are
/// not the same question. A form stated in the file but not offered here is a form the command line and the
/// Desktop program may speak and this window may not, which is why the startup choice keeps any language the
/// preference names while the menu offers these two.
/// </para>
/// </remarks>
internal static class LauncherLanguage
{
    /// <summary>The language the files are written in when nothing else says.</summary>
    private const string Fallback = "en";

    /// <summary>The two languages the menu offers, in the order they are read.</summary>
    public static readonly IReadOnlyList<LanguageOption> Options =
    [
        new("en", "English"),
        new("zh-CN", "中文"),
    ];

    /// <summary>The language in force, as the files name it.</summary>
    public static string? Current => RolaI18N.Locale;

    /// <summary>
    /// The two languages as one of them, which is the one a locale is written in.
    /// </summary>
    /// <remarks>
    /// The region is dropped rather than compared: the files state <c>en</c> and <c>zh-CN</c>, and a machine
    /// may hand over <c>en-GB</c> or <c>zh-Hans</c> for the same two languages — a menu that matched the
    /// whole name would mark neither of them as the one in force.
    /// </remarks>
    /// <param name="locale">The locale in force, or nothing.</param>
    /// <returns>The option it is, which is the first when it is none.</returns>
    public static LanguageOption Of(string? locale) =>
        locale is not null && locale.StartsWith("zh", StringComparison.OrdinalIgnoreCase)
            ? Options[1]
            : Options[0];

    /// <summary>
    /// The language a run speaks.
    /// </summary>
    /// <param name="asked">What the command line handed over, if anything.</param>
    /// <param name="preferred">What the shared preference file says, if it says anything.</param>
    /// <param name="system">The language the machine is set to.</param>
    /// <returns>The locale to speak, as the translation files name one.</returns>
    public static string Choose(string? asked, string? preferred, string? system)
    {
        if (!string.IsNullOrWhiteSpace(asked))
        {
            return AsLocale(asked);
        }

        if (!string.IsNullOrWhiteSpace(preferred))
        {
            return AsLocale(preferred);
        }

        return system is not null && system.StartsWith("zh", StringComparison.OrdinalIgnoreCase)
            ? "zh-CN"
            : Fallback;
    }

    /// <summary>
    /// Writes the language chosen into the preference file the Desktop program reads.
    /// </summary>
    /// <remarks>
    /// The choice is the Desktop program's own file rather than a file of this program's: the two programs
    /// speak to one person, and a language chosen in one of them is a language the other should speak. What
    /// the file holds besides the language — each plugin's own section — is read and written back as it was,
    /// since this program understands none of it and must not be the thing that drops it.
    /// </remarks>
    /// <param name="locale">The locale chosen.</param>
    /// <exception cref="ConfigurationFailure">The file could not be read or written.</exception>
    public static void Save(string locale)
    {
        var preference = ConfigurationLoader.LoadPreference();

        preference.Language = locale;

        ConfigurationLoader.WritePreference(preference);
    }

    /// <summary>
    /// A language as the files name one.
    /// </summary>
    /// <remarks>
    /// A locale on a machine may carry the encoding it is written in — <c>zh_CN.UTF-8</c> — and separates the
    /// region with an underscore where the files use a hyphen, so neither is left in the name spoken by.
    /// </remarks>
    /// <param name="locale">The locale.</param>
    /// <returns>Its name.</returns>
    private static string AsLocale(string locale)
    {
        var language = locale.Trim().Split('.', 2)[0];

        return language.Replace('_', '-');
    }
}

using System.Globalization;
using RorolalaDesktop.Contract;

namespace TerminalPlugin;

/// <summary>
/// What a Terminal dock is started with: the program, its arguments, and how the screen is drawn.
/// </summary>
/// <remarks>
/// They are settings rather than constants because a shell is the user's answer: a machine may have
/// PowerShell, a POSIX shell or neither named the way this program guesses, and a user may want one started
/// with a flag. What they are until they are changed is the system's own shell — the one Windows is opened
/// with, and the one a Unix login says in <c>SHELL</c> — and a font that is monospaced, since a grid of one
/// advance per cell is what a terminal draws on.
/// </remarks>
internal static class TerminalSettings
{
    /// <summary>The setting the program is kept under.</summary>
    public const string Shell = "Terminal/shell";

    /// <summary>The setting the arguments are kept under.</summary>
    public const string Arguments = "Terminal/arguments";

    /// <summary>The setting the font family is kept under.</summary>
    public const string Font = "Terminal/font";

    /// <summary>The setting the font size is kept under.</summary>
    public const string FontSize = "Terminal/font_size";

    /// <summary>The face a terminal is drawn in until one is chosen, which is a monospaced fallback chain.</summary>
    private const string DefaultFont = "Cascadia Mono,Consolas,Menlo,DejaVu Sans Mono,monospace";

    /// <summary>The size a terminal is drawn at until one is chosen.</summary>
    private const string DefaultSize = "13";

    /// <summary>Where the settings are read from, once the plugin has declared them.</summary>
    private static IPluginConfig? _config;

    /// <summary>Declares the four, so that the preference panel shows them and keeps what is chosen.</summary>
    /// <param name="config">The plugin's own settings.</param>
    public static void Declare(IPluginConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);

        _config = config;

        config.Add(new PluginSetting(Shell, SettingKind.Text, "rorolala_terminal.setting.shell", DefaultShell(), 10));
        config.Add(new PluginSetting(Arguments, SettingKind.Text, "rorolala_terminal.setting.arguments", string.Empty, 20));
        config.Add(new PluginSetting(Font, SettingKind.Text, "rorolala_terminal.setting.font", DefaultFont, 30));
        config.Add(new PluginSetting(FontSize, SettingKind.Text, "rorolala_terminal.setting.font_size", DefaultSize, 40));
    }

    /// <summary>The program a terminal runs.</summary>
    /// <returns>The program, as the system names it.</returns>
    public static string Program() =>
        _config?.ReadKeyAs(Shell, DefaultShell()) is { Length: > 0 } program ? program : DefaultShell();

    /// <summary>The arguments the program is run with.</summary>
    /// <returns>The arguments, in order.</returns>
    public static IReadOnlyList<string> Chosen() => Split(_config?.ReadKeyAs(Arguments, string.Empty) ?? string.Empty);

    /// <summary>The font family the screen is drawn in.</summary>
    /// <returns>The family, with its fallbacks.</returns>
    public static string Family() =>
        _config?.ReadKeyAs(Font, DefaultFont) is { Length: > 0 } font ? font : DefaultFont;

    /// <summary>The size the screen is drawn at.</summary>
    /// <returns>The size in points, or the default when what is stored is not a number.</returns>
    public static double Size() =>
        double.TryParse(
            _config?.ReadKeyAs(FontSize, DefaultSize),
            NumberStyles.Float,
            CultureInfo.InvariantCulture,
            out var size
        ) && size > 0
            ? size
            : double.Parse(DefaultSize, CultureInfo.InvariantCulture);

    /// <summary>
    /// The shell a machine is opened with.
    /// </summary>
    /// <remarks>
    /// Windows is answered with PowerShell, which is the shell the system carries; elsewhere the user's own
    /// login shell is read from <c>SHELL</c>, which is what a terminal started outside a desktop would run,
    /// and a machine that names none gets the one a POSIX system always has.
    /// </remarks>
    /// <returns>The program.</returns>
    private static string DefaultShell()
    {
        if (OperatingSystem.IsWindows())
        {
            return "powershell.exe";
        }

        return Environment.GetEnvironmentVariable("SHELL") is { Length: > 0 } shell ? shell : "/bin/sh";
    }

    /// <summary>
    /// Splits an argument line the way a shell's own words are split.
    /// </summary>
    /// <remarks>
    /// Whitespace separates the words, double quotes group them, and a backslash takes the meaning away from
    /// the character after it — the three things an argument set in a preference needs. What it is not is a
    /// shell: there is no expanding, no variable and no pipeline, since the program is run directly and the
    /// arguments are handed to it as they are.
    /// </remarks>
    /// <param name="line">The line as it was typed.</param>
    /// <returns>The arguments, in order.</returns>
    internal static IReadOnlyList<string> Split(string line)
    {
        ArgumentNullException.ThrowIfNull(line);

        var arguments = new List<string>();
        var word = new System.Text.StringBuilder();
        var quoted = false;
        var started = false;

        for (var at = 0; at < line.Length; at++)
        {
            var character = line[at];

            if (character == '\\' && at + 1 < line.Length)
            {
                word.Append(line[++at]);
                started = true;

                continue;
            }

            if (character == '"')
            {
                quoted = !quoted;
                started = true;

                continue;
            }

            if (!quoted && char.IsWhiteSpace(character))
            {
                if (started)
                {
                    arguments.Add(word.ToString());
                    word.Clear();
                    started = false;
                }

                continue;
            }

            word.Append(character);
            started = true;
        }

        if (started)
        {
            arguments.Add(word.ToString());
        }

        return arguments;
    }
}

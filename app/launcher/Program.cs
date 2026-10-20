using System.Globalization;
using Avalonia;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher;

/// <summary>
/// The launcher's start: what the run works out before there is a window, and the window itself.
/// </summary>
/// <remarks>
/// What is worked out here is worked out once and read everywhere else, because it is all decided before
/// there is anything to draw: the language the program speaks, the look it is drawn in, where its own files
/// are, and how this machine draws a frame. A failure here is a reason on standard error and an exit code,
/// while there is still no window to put a notice in.
/// <para>
/// The command line program is the one beside this one, and it is asked how the machine draws a frame: a
/// window style is a reading of the environment, and that reading is written once in the command line
/// rather than again here. That answer is what an unchosen frame is, and what this program hands to the
/// Desktop program when it opens a window.
/// </para>
/// </remarks>
internal static class Program
{
    /// <summary>What the language this program speaks is handed over under, matching <c>rola desktop</c>.</summary>
    private const string LanguagePrefix = "-Lang:";

    /// <summary>Where this program keeps its own files.</summary>
    public static LauncherFiles Files { get; private set; } = null!;

    /// <summary>The command line program beside this one.</summary>
    public static RolaRunner Rola { get; private set; } = null!;

    /// <summary>The accounts on this machine.</summary>
    public static Accounts Accounts { get; private set; } = null!;

    /// <summary>What a connection is checked through.</summary>
    public static Connection Connection { get; private set; } = null!;

    /// <summary>What this program was told.</summary>
    public static LauncherSettings Settings { get; private set; } = null!;

    /// <summary>What the user chose about how the programs look, which the Desktop program reads too.</summary>
    public static ThemeConfiguration Theme { get; private set; } = null!;

    /// <summary>How this machine draws a window's own frame, as the command line probed it.</summary>
    public static WindowStyle MachineFrame { get; private set; }

    /// <summary>How this window's frame is drawn: the user's choice, or the machine's own answer.</summary>
    public static WindowStyle Frame { get; private set; }

    /// <summary>The lines this run says about itself, read from beside the program.</summary>
    public static IReadOnlyList<string> SloganLines { get; private set; } = [];

    /// <summary>
    /// Switches the language the program speaks, and keeps the choice where the Desktop program reads it.
    /// </summary>
    /// <remarks>
    /// The file is written before anything is said differently: a choice that could not be kept is one the
    /// window does not pretend to have made, which is why this throws rather than swallowing the failure and
    /// leaving a window speaking a language the next run would not find.
    /// </remarks>
    /// <param name="locale">The locale chosen.</param>
    /// <exception cref="ConfigurationFailure">The preference file could not be read or written.</exception>
    public static void SetLanguage(string locale)
    {
        LauncherLanguage.Save(locale);

        RolaI18N.SetLocale(locale);

        // What the command line is asked in changes with the window, so that a reason it reports is written
        // in the language the person reading the window is reading.
        Rola.Language = RolaI18N.Locale ?? locale;
    }

    /// <summary>The program's entry point.</summary>
    /// <param name="args">What the program was started with.</param>
    [STAThread]
    public static void Main(string[] args)
    {
        // The host's own directory first: merging is first-registration-wins, so what the launcher says
        // cannot be overwritten by a file beside it that says the same key.
        RolaI18N.SetTranslationDirectory(Path.Combine(AppContext.BaseDirectory, "i18n"));
        RolaI18N.SetLocale(ChooseLanguage(args));

        ThemeConfiguration theme;

        try
        {
            theme = ConfigurationLoader.LoadTheme();
        }
        catch (ConfigurationFailure failure)
        {
            Console.Error.WriteLine(failure.Message);
            Environment.Exit((int)failure.Code);

            return;
        }

        Theme = theme;

        Files = LauncherFiles.ForThisRun().Made();
        Settings = LauncherSettings.Read(Files.Settings);
        Rola = new RolaRunner(Files.Rola, RolaI18N.Locale ?? "en");
        Accounts = new Accounts(Rola);
        Connection = new Connection(Rola, Accounts, Files.Probe);
        MachineFrame = Rola.ProbeWindowStyle(Files.Beside) ?? WindowStyle.Windows;
        Frame = Settings.Frame ?? MachineFrame;

        // Read once for the run: the welcome is drawn again whenever the look or the language changes, and a
        // rotation that started from a different line because the window was redrawn would not be one
        // rotation but several.
        SloganLines = Slogans.Read(Files.Slogans);

        BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
    }

    /// <summary>The application the toolkit is started on.</summary>
    /// <returns>The builder.</returns>
    public static AppBuilder BuildAvaloniaApp() =>
        AppBuilder
            .Configure<App>()
            .UsePlatformDetect()
            .UseWaylandWithFallback()
            .LogToTrace();

    /// <summary>
    /// The language this program speaks.
    /// </summary>
    /// <remarks>
    /// The command line hands a Desktop window a language; the launcher is started by a person, so what it
    /// speaks is the shared preference file — a choice made in the Desktop program and kept there — and,
    /// where nothing has been chosen yet, the language the machine is set to when the files speak it.
    /// </remarks>
    /// <param name="args">What the program was started with.</param>
    /// <returns>The locale to speak.</returns>
    private static string ChooseLanguage(IEnumerable<string> args)
    {
        var asked = args
            .FirstOrDefault(argument => argument.StartsWith(LanguagePrefix, StringComparison.Ordinal))
            ?[LanguagePrefix.Length..];

        string? preferred = null;

        if (File.Exists(ConfigPaths.Preference))
        {
            try
            {
                preferred = ConfigurationLoader.LoadPreference().Language;
            }
            catch (ConfigurationFailure)
            {
                // A preference file that will not read is not a reason to refuse to open: the look is what
                // matters to the window, and it is read from its own file.
                preferred = null;
            }
        }

        return LauncherLanguage.Choose(asked, preferred, CultureInfo.CurrentUICulture.Name);
    }
}

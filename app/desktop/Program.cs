using Avalonia;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;

namespace RorolalaDesktop;

class Program
{
    /// The language the command line asked this program to speak, as a locale — `zh-CN`, `en`.
    ///
    /// The command line chooses a language for the run and for itself, and hands the answer over
    /// rather than leaving this program to work it out again: what a run speaks and what the
    /// window it opens speaks are then one choice, made in one place. Null when this program is
    /// started on its own rather than through `rola desktop`.
    public static string? Language { get; private set; }

    /// The directory the command line was run from.
    ///
    /// A window is opened onto the work the run was being made in, so where that was comes from
    /// the run; looked up here instead it would be this program's own directory, which is wherever
    /// the export put it. Null when this program is started on its own.
    public static string? CurrentDirectory { get; private set; }

    /// What the run worked out before Avalonia started.
    ///
    /// Recorded the way the language is: Avalonia makes the window itself, so the window's side of
    /// the program asks for what was worked out rather than being handed it.
    public static DesktopState? State { get; private set; }

    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        // Recorded before Avalonia starts: what was handed over is state of the program, and a
        // reader asks for it without asking whether the answer has arrived yet.
        Record(args);

        // The host's own directory is named before any plugin's: merging is first-registration-wins,
        // so a plugin cannot overwrite a word the host says.
        RolaI18N.SetTranslationDirectory(Path.Combine(AppContext.BaseDirectory, "i18n"));

        DesktopState state;

        try
        {
            // Configuration and the plugin load order, worked out before Avalonia starts, so that a
            // failure here is a reason on standard error and an exit code while there is still no
            // window to put a dialog in.
            state = DesktopStartup.Load();
        }
        catch (ConfigurationFailure failure)
        {
            Console.Error.WriteLine(failure.Message);
            Environment.Exit((int)failure.Code);

            return;
        }

        // The command line's choice wins; started on its own, the file's fallback speaks.
        RolaI18N.SetLocale(Language ?? state.Preference.Language);

        State = state;

        BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
    }

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
        => AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();

    /// Records what the run handed over: each argument this program answers to, and nothing else.
    ///
    /// An argument that is not one of these is left alone rather than reported — the arguments are
    /// the command line's, and this program is not the only thing that may be reading them.
    private static void Record(string[] args)
    {
        foreach (var arg in args)
        {
            if (arg.StartsWith(LanguagePrefix, StringComparison.Ordinal))
            {
                Language = arg[LanguagePrefix.Length..];
            }
            else if (arg.StartsWith(DirectoryPrefix, StringComparison.Ordinal))
            {
                CurrentDirectory = arg[DirectoryPrefix.Length..];
            }
        }
    }

    /// What the language is handed over as, matching `rola desktop`.
    private const string LanguagePrefix = "-Lang:";

    /// What the directory is handed over as, matching `rola desktop`.
    private const string DirectoryPrefix = "-CurrentDir:";
}

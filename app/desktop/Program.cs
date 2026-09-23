using Avalonia;
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

    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        // Recorded before Avalonia starts: what was handed over is state of the program, and a
        // reader asks for it without asking whether the answer has arrived yet.
        Record(args);

        // Read before anything is drawn, from beside the program — an export lays them there — and
        // in the language the command line chose. Started on its own there is no language to set,
        // and the files' own fallback speaks.
        RolaI18N.SetTranslationDirectory(Path.Combine(AppContext.BaseDirectory, "i18n"));

        if (Language is { } language)
        {
            RolaI18N.SetLocale(language);
        }

        BuildAvaloniaApp()
            .StartWithClassicDesktopLifetime(args);
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

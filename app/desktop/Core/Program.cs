using Avalonia;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;

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

    /// The style the window's own frame is drawn in.
    ///
    /// What the command line asked for, or — started on its own, where there is nobody to ask — the style
    /// the machine's own desktop is drawn in, which is worked out before Avalonia starts because the window
    /// and its look are both built from it.
    public static WindowStyle WindowStyle { get; private set; } = WindowStyles.Here();

    /// <summary>
    /// Raised when a later start of the program asks this one to come forward.
    /// </summary>
    /// <remarks>
    /// A second start of the program does not open a window of its own: what it says is where it was made,
    /// and what that comes to is this run's to work out — a dock opened onto that directory in the main
    /// window, which is not something a channel can decide. Raised on the window's thread.
    /// </remarks>
    public static event Action<string?>? Called;

    /// <summary>
    /// Says that a later start asked this one to come forward.
    /// </summary>
    /// <remarks>
    /// What a run that was called says is raised through this rather than written to a property, because the
    /// thing that answers a call is the host — a dock opened onto the directory, in a window — and the host
    /// does not exist when the call is received. A run that nobody is listening to yet has nobody to tell,
    /// which cannot happen: listening begins with the host, and the host is what answers.
    /// </remarks>
    /// <param name="directory">Where the run that called was made.</param>
    public static void CalledFrom(string? directory) => Called?.Invoke(directory);

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

        // Before anything else, because what a second start is may not be a start at all: one program per
        // user means a start that finds one running is a caller rather than a window, and it ends here with
        // the answer that says which of the two it was.
        var start = SingleInstance.Acquire(CurrentDirectory);

        if (!start.IsFirst)
        {
            Environment.Exit((int)(start.Called ? ExitCode.Called : ExitCode.Clean));

            return;
        }

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
    //
    // The two options below are what make the window's own frame work on a Linux desktop. Neither
    // backend draws decorations unless it is asked to: X11 hides the window manager's and draws its own
    // only for a window that extended into them *and* only when this option is on, and Wayland lets the
    // compositor decide until it is told otherwise — a compositor that would draw a server-side title bar
    // would then draw one over a window that has its own. Both are experimental in the toolkit and both
    // are wanted here: a compositor's title bar is the thing this program does not have.
    public static AppBuilder BuildAvaloniaApp() =>
#pragma warning disable AVALONIA_X11_CSD, AVALONIA_WAYLAND_FORCE_CSD
        AppBuilder
            .Configure<App>()
            .With(() => new X11PlatformOptions { EnableDrawnDecorations = true })
            .With(() => new WaylandPlatformOptions { ForceDrawnDecorations = true })
            .UsePlatformDetect()
            .UseWaylandWithFallback()
            .LogToTrace();
#pragma warning restore AVALONIA_X11_CSD, AVALONIA_WAYLAND_FORCE_CSD

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
            else if (arg.StartsWith(StylePrefix, StringComparison.Ordinal))
            {
                // A name that is none of the three leaves the machine's own answer standing. The command
                // line refuses a name it does not know before it gets this far, so what is left here is a
                // run started by hand with one written wrong, and a window is a poor place to report it.
                if (WindowStyles.Named(arg[StylePrefix.Length..]) is { } asked)
                {
                    WindowStyle = asked;
                }
            }
        }
    }

    /// What the language is handed over as, matching `rola desktop`.
    private const string LanguagePrefix = "-Lang:";

    /// What the directory is handed over as, matching `rola desktop`.
    private const string DirectoryPrefix = "-CurrentDir:";

    /// What the window style is handed over as, matching `rola desktop`.
    private const string StylePrefix = "-WindowStyle:";
}

using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Media;
using Avalonia.Themes.Simple;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher;

/// <summary>
/// The application: one look, and one window.
/// </summary>
/// <remarks>
/// The launcher is the application's main window, not a dialog over an unseen owner: a dialog is a
/// floating window with a hidden companion behind it, and the launcher is the thing a person is looking
/// at. On a desktop that is not tiling the window keeps the system title bar and border, is fixed at its
/// own size and cannot be resized; under a tiling window manager the manager draws the frame and the
/// window is resizable so it can be placed like any other. The two columns inside it follow the window:
/// the Vaults dock keeps its width and the Workspaces take what is left.
/// </remarks>
public sealed class App : Application
{
    /// <inheritdoc />
    public override void Initialize()
    {
        // The base theme every look is layered onto: rectangular controls, opaque neutral colours, and no
        // accent of its own. What the launcher draws itself is drawn from the palette in force.
        Styles.Add(new SimpleTheme());

        // Text is drawn with grayscale antialiasing rather than the platform's default, which is subpixel on
        // Windows and grayscale on Wayland: the two programs are read as one look, and on one of the platforms
        // the default reads as coloured, uneven edges beside the shapes it is set in. The toolkit exposes the mode
        // on the visual rather than as a styleable property, so it is put on every window as it opens and inherits
        // down that window's tree from there.
        Window.WindowOpenedEvent.AddClassHandler<Window>(
            (window, _) => TextOptions.SetTextRenderingMode(window, TextRenderingMode.Antialias)
        );
    }

    /// <inheritdoc />
    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // The launcher is the only window, so closing it ends the program: there is no hidden owner
            // left behind to keep the lifetime open.
            desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;

            var theme = new LauncherTheme(this, Program.Theme);

            theme.Apply();

            // A desktop that changes its own variant while the program runs is a launcher that follows it,
            // which is what a file that names "system" asked for.
            ActualThemeVariantChanged += (_, _) => theme.Refresh();

            var window = new LauncherWindow(
                Program.Files,
                Program.Rola,
                Program.Accounts,
                Program.Connection,
                theme,
                Program.Settings
            );

            window.Closed += (_, _) => desktop.Shutdown();
            window.Opened += (_, _) => _ = window.Load();

            desktop.MainWindow = window;

            window.Show();
        }

        base.OnFrameworkInitializationCompleted();
    }
}

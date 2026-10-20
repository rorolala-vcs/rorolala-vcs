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
/// The window is opened as a dialog over a window that is not seen. That is not a trick to keep the window
/// on top: a tiling window manager keeps a dialog where it was put rather than tiling it with everything
/// else, and a launcher that was tiled would be a launcher in a quarter of the screen with a list it cannot
/// show. The unseen window is what a dialog is a dialog of, and it is transparent, a pixel wide, and put
/// where no screen reaches.
/// </remarks>
public sealed class App : Application
{
    /// <inheritdoc />
    public override void Initialize()
    {
        // The base theme every look is layered onto: rectangular controls, opaque neutral colours, and no
        // accent of its own. What the launcher draws itself is drawn from the palette in force.
        Styles.Add(new SimpleTheme());
    }

    /// <inheritdoc />
    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // What ends the program is the launcher being closed, and not whatever else the toolkit counts:
            // the window it was opened over is still up when the launcher is done, and it is not a window
            // anybody is looking at.
            desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;

            var theme = new LauncherTheme(this, Program.Theme);

            theme.Apply();

            // A desktop that changes its own variant while the program runs is a launcher that follows it,
            // which is what a file that names "system" asked for.
            ActualThemeVariantChanged += (_, _) => theme.Refresh();

            var owner = new Window
            {
                Width = 1,
                Height = 1,
                CanResize = false,
                ShowInTaskbar = false,
                WindowDecorations = Avalonia.Controls.WindowDecorations.None,
                WindowStartupLocation = WindowStartupLocation.Manual,
                Position = new PixelPoint(-32000, -32000),
                Background = Brushes.Transparent,
                Opacity = 0,
            };

            var window = new LauncherWindow(
                Program.Files,
                Program.Rola,
                Program.Accounts,
                Program.Connection,
                theme,
                Program.Settings,
                Program.Frame
            );

            owner.Show();

            desktop.MainWindow = owner;

            window.Closed += (_, _) => desktop.Shutdown();

            _ = window.ShowDialog(owner);
        }

        base.OnFrameworkInitializationCompleted();
    }
}

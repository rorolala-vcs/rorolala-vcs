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
/// On a desktop that draws its own frame the window is opened as a dialog over a window that is not seen:
/// a tiling window manager does not tile a dialog, and a launcher tiled into a corner would have a list
/// it cannot show. Under a tiling window manager the launcher is a window in its own right instead — the
/// manager tiles that one and keeps no hidden companion behind it — and the window is made resizable so
/// the manager can place it like any other window. The banner inside it is sized from the window's width
/// and the picture's own ratio, so both arrangements show the picture at its own shape.
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

            if (Program.Tiling)
            {
                // Under a tiling window manager a dialog over an unseen owner is a floating window with a
                // hidden companion: the manager tiles the owner and keeps the launcher over it, which is
                // two windows for one person. The launcher is the window instead, and the manager tiles
                // it like any other.
                desktop.MainWindow = window;
                window.Show();
            }
            else
            {
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

                owner.Show();

                desktop.MainWindow = owner;

                _ = window.ShowDialog(owner);
            }
        }

        base.OnFrameworkInitializationCompleted();
    }
}

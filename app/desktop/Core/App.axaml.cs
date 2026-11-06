using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Avalonia.Threading;
using RorolalaDesktop.Configuration;

namespace RorolalaDesktop;

public partial class App : Application
{
    /// <summary>
    /// The stable name of the dock a called run opens onto the directory it was made in.
    /// </summary>
    /// <remarks>
    /// The File System's own, named rather than asked for: the File System plugin is one the program ships
    /// with and the kernel already has docks of, so what is named is a name the program knows. A program
    /// whose file browser is not there opens nothing and says so in the log rather than refusing to come
    /// forward, which is the same judgement every other missing name is met with.
    /// </remarks>
    private const string BrowserDock = "rorolala.file_system.browser";

    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // Nothing is drawn without what the run worked out, so a start that has none is a start
            // that went wrong rather than one to paper over.
            var state =
                Program.State
                ?? throw new InvalidOperationException("the program was started without its state");

            // What ends the program is every window being gone rather than a particular one closing, and the
            // main window is one of the windows rather than the one that decides: it can be closed while a
            // dock lives on in a window of its own, and opened again from the Window menu. The toolkit's own
            // rules are all about one named window (the last one, or the main one) so none of them is what
            // this program means, and the ending is asked for by name instead.
            desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;

            try
            {
                var host = new Desktop(state);

                host.Host.Windows.End = () => desktop.Shutdown();

                // The one dock a called run opens is the file browser, which is what a directory is looked
                // at in: the host does not know that, and the plugin that owns the dock does not hear about a
                // call until it is asked, so what is asked for is stated here, by the name the run that
                // opened the window over it would have opened it by.
                // A caller is answered with a window being opened or brought forward, which only the
                // window's thread may do; `SingleInstance` hands it over there. Subscribed before the window
                // is shown, because a window that is up is not one a listener can be added to afterwards.
                Program.Called += directory => host.Host.Windows.Show(BrowserDock, directory);

                // What is left of the shell is done once the window is on the screen, and a caller that asked
                // to be told the window is up is told at the same moment. Neither is done here: both are put
                // in at a priority below the drawing of the window, so that the frame it was just given is
                // drawn before the work that would hold the thread starts. The line goes first, because what
                // it says is that there is a window to look at rather than that the work behind it is done.
                host.ShellOpened += () =>
                {
                    Dispatcher.UIThread.Post(ReadySignal.Send, DispatcherPriority.Loaded);
                    Dispatcher.UIThread.Post(
                        () => host.Assemble(host.FinishLoading),
                        DispatcherPriority.Background
                    );
                };

                desktop.MainWindow = host.StartShell();
            }
            catch (ConfigurationFailure failure)
            {
                // A configuration the host cannot honour while the shell is being made is still a reason on
                // standard error and an exit code, because there is no window to put it in yet. What the work
                // after the window fails at is recorded and drawn over instead, so that a plugin that will not
                // start is not a program that will not open.
                Console.Error.WriteLine(failure.Message);
                Environment.Exit((int)failure.Code);
            }
        }

        base.OnFrameworkInitializationCompleted();
    }
}

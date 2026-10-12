using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
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
            // rules are all about one named window — the last one, or the main one — so none of them is what
            // this program means, and the ending is asked for by name instead.
            desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;

            try
            {
                var host = new Desktop(state);

                host.Host.Windows.End = () => desktop.Shutdown();

                desktop.MainWindow = host.Start();

                // The one dock a called run opens is the file browser, which is what a directory is looked
                // at in: the host does not know that, and the plugin that owns the dock does not hear about a
                // call until it is asked — so what is asked for is stated here, by the name the run that
                // opened the window over it would have opened it by.
                // A caller is answered with a window being opened or brought forward, which only the
                // window's thread may do; `SingleInstance` hands it over there.
                Program.Called += directory => host.Host.Windows.Show(BrowserDock, directory);

                // And a later start of the program is heard from here on, which is where the host that
                // answers it exists: what a caller is answered with is a dock opened in a window, and a
                // window is not something that can be made before Avalonia has started.
                SingleInstance.Listen(Program.CalledFrom, host.Host.Log);
            }
            catch (ConfigurationFailure failure)
            {
                // A theme no provider supplies is only known once plugins have registered theirs,
                // which is after Avalonia has started but still before the window exists.
                Console.Error.WriteLine(failure.Message);
                Environment.Exit((int)failure.Code);
            }
        }

        base.OnFrameworkInitializationCompleted();
    }
}

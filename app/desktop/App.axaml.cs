using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using RorolalaDesktop.Configuration;

namespace RorolalaDesktop;

public partial class App : Application
{
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

            try
            {
                desktop.MainWindow = new Desktop(state).Start();
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

using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;

using RorolalaDesktop;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Progress;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The host's progress stack: a capsule per run, and the capsule of a finished run leaving it.
/// </summary>
/// <remarks>
/// The stack is a control over a window, so it is checked where a window is: what it holds is found in its visual
/// tree, and what the hub says is what the capsule is drawn from.
/// </remarks>
public sealed class ProgressPanelTests
{
    /// <summary>A started run gets a capsule, and the capsule goes when the run ends.</summary>
    [Fact]
    public void ACapsuleIsDrawnForARunAndLeavesWhenItEnds()
    {
        var services = Host.Services();
        var hub = services.Progress;
        var panel = new ProgressStack(hub, services.Icons);
        var window = new Window
        {
            Width = 520,
            Height = 360,
            Content = new Grid { Children = { panel } },
        };

        window.Show();

        Pump();

        Assert.False(panel.IsVisible, "the stack is drawn with no run going");

        var run = hub.Begin("rorolala_vcs.progress.sync", null);

        run.Records.Read("""{"signal":"begin","id":1,"what":"扫描索引","total":10}""");
        run.Records.Read("""{"signal":"advance","id":1,"done":4}""");
        run.Touch();

        Pump();

        Assert.True(panel.IsVisible, "the stack is not drawn for a running run");
        Assert.Contains(
            panel.GetVisualDescendants().OfType<TextBlock>(),
            text => text.Text == RolaI18N.Get(run.TitleKey)
        );
        Assert.Contains(panel.GetVisualDescendants().OfType<ProgressBar>(), bar => bar.Value == 4);

        run.End();

        Pump();

        // The capsule fades before it is taken out, so it is still in the stack for a moment after the run ends.
        Assert.True(panel.IsVisible, "the stack vanished before its capsule could fade");

        Thread.Sleep(350);
        Pump();

        Assert.False(panel.IsVisible, "the stack stayed after its last capsule left");
    }

    /// <summary>A main window carries the host's stack, which is what every window shows.</summary>
    [Fact]
    public void AMainWindowCarriesTheHostStack()
    {
        var services = Host.Services();
        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);
        services.Windows.OpenMain();

        var main = services.Windows.Main;

        try
        {
            var stack = main.GetVisualDescendants().OfType<ProgressStack>().Single();

            Assert.False(stack.IsVisible);
        }
        finally
        {
            main.Close();
        }
    }

    /// <summary>Lets the window run a few turns of its loop, which is what lays a control out and draws it.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 20; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }
}

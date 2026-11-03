using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;

using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;

using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Overview's progress stack: a capsule per command, and the capsule of a finished command leaving it.
/// </summary>
/// <remarks>
/// The stack is a control over a window, so it is checked where a window is: what it holds is found in its visual
/// tree, and what the hub says is what the capsule is drawn from.
/// </remarks>
public sealed class ProgressPanelTests
{
    /// <summary>A started command gets a capsule, and the capsule goes when the command ends.</summary>
    [Fact]
    public void ACapsuleIsDrawnForACommandAndLeavesWhenItEnds()
    {
        var hub = new ProgressHub();
        var services = Host.Services();
        var panel = new ProgressPanel(hub, services.Icons);
        var window = new Window
        {
            Width = 520,
            Height = 360,
            Content = new Grid { Children = { panel } },
        };

        window.Show();

        Pump();

        Assert.False(panel.IsVisible, "the stack is drawn with no command running");

        var run = hub.Begin("rorolala_vcs.progress.sync");

        run.Records.Read(
            """{"signal":"begin","id":1,"what":"扫描索引","total":10}"""
        );
        run.Records.Read("""{"signal":"advance","id":1,"done":4}""");
        hub.Touch();

        Pump();

        Assert.True(panel.IsVisible, "the stack is not drawn for a running command");
        Assert.Contains(
            panel.GetVisualDescendants().OfType<TextBlock>(),
            text => text.Text == run.Title
        );
        Assert.Contains(panel.GetVisualDescendants().OfType<ProgressBar>(), bar => bar.Value == 4);

        hub.End(run);

        Pump();

        // The capsule fades before it is taken out, so it is still in the stack for a moment after the run ends.
        Assert.True(panel.IsVisible, "the stack vanished before its capsule could fade");

        Thread.Sleep(350);
        Pump();

        Assert.False(panel.IsVisible, "the stack stayed after its last capsule left");
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

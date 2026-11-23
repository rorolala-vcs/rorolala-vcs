using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using TerminalPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A control in a dock behind another tab stops its clock.
/// </summary>
/// <remarks>
/// A dock is hidden by taking its view out of the drawing, not out of the tree, so a control that only
/// stopped on being detached would go on polling a shell or turning a ring for a tab nobody is looking
/// at. What is checked is that the effective visibility is what the clocks follow.
/// </remarks>
public sealed class VisibilityTests
{
    /// <summary>A terminal and a ring stop when the dock around them is hidden, and start again when it is shown.</summary>
    [Fact]
    public void AControlInAHiddenDockStopsItsClock()
    {
        var services = Host.Services();
        var directory = Scratch.New("visibility");
        var terminal = new TerminalView(
            services.Log.For("it.terminal"),
            "a-program-that-is-not-there",
            [],
            directory,
            "Noto Sans",
            13
        );
        var entry = new VirtualEntry(
            Path.Combine(directory, "coming.psd"),
            EntryKind.File,
            VirtualEntryState.Pending,
            0.25f,
            []
        );
        var ring = new VirtualRing(entry, 32);
        var panel = new StackPanel { Children = { terminal, ring } };
        var window = new Window { Width = 300, Height = 200, Content = panel };

        window.Show();

        try
        {
            Pump();

            Assert.True(terminal.Polling, "the terminal did not start polling");
            Assert.True(ring.Spinning, "the ring did not start turning");

            panel.IsVisible = false;
            Pump();

            Assert.False(terminal.Polling, "the terminal kept polling while hidden");
            Assert.False(ring.Spinning, "the ring kept turning while hidden");

            panel.IsVisible = true;
            Pump();

            Assert.True(terminal.Polling, "the terminal did not resume polling");
            Assert.True(ring.Spinning, "the ring did not resume turning");
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Lets the layout and the drawing catch up.</summary>
    private static void Pump()
    {
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
        Thread.Sleep(10);
    }
}

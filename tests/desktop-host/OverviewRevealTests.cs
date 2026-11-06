using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Threading;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A path in an Overview row, and where the browser is asked to show it.
/// </summary>
/// <remarks>
/// The two halves meet in the row: the cell is what the user presses, and the browser is what answers where the path
/// is. What is checked here is the request that crosses, the path as the work spells it, joined to where the reading
/// was made, and that the cell a row has no path on does nothing.
/// </remarks>
public sealed class OverviewRevealTests
{
    /// <summary>Pressing a row's path asks the browser for the place it names, in the dock that is open.</summary>
    [Fact]
    public void APathInARowIsShownInTheBrowser()
    {
        var (services, plugin, views, registration) = Hosts();
        var root = Scratch.New("overview-reveal");

        services.Windows.OpenMain();
        _ = services.Docks.Activate(registration);

        var cell = OverviewView.Cell("art/hero.psd", false, root, plugin);
        var window = new Window
        {
            Width = 600,
            Height = 80,
            Content = cell,
        };

        window.Show();
        window.Activate();
        Pump();

        try
        {
            Click(window, cell);

            var view = Assert.Single(views);
            var request = view.Requests.Last();

            Assert.Equal(registration.DockNameId, request.DockNameId);
            Assert.Equal(Path.Combine(root, "art", "hero.psd"), request.StartAt);

            // Shown in the dock the user has rather than in another one beside it.
            Assert.Single(services.Docks.Instances);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A row with no path on a side has nothing to show, and a press says nothing.</summary>
    [Fact]
    public void ACellWithNoPathShowsNothing()
    {
        var (services, plugin, views, registration) = Hosts();
        var root = Scratch.New("overview-empty");

        services.Windows.OpenMain();
        _ = services.Docks.Activate(registration);

        var cell = OverviewView.Cell(string.Empty, false, root, plugin);
        var window = new Window
        {
            Width = 600,
            Height = 80,
            Content = cell,
        };

        window.Show();
        window.Activate();
        Pump();

        try
        {
            Click(window, cell);

            // Nothing was asked to be shown: the dock heard only what it was made with.
            var view = Assert.Single(views);

            Assert.Equal([null], view.Requests.Select(request => request.StartAt));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A path of the work's is joined to where the reading was made, spelled as the system spells it.</summary>
    [Fact]
    public void APathOfTheWorksIsJoinedToWhereItWasRead()
    {
        var root = Scratch.New("overview-join");

        Assert.Equal(
            Path.Combine(root, "art", "hero.psd"),
            OverviewPaths.Place("art/hero.psd", root)
        );

        // Nothing to join a path to is nothing to show: a place of the work's is only as good as the work it was read
        // over, and a path whose root is not known is not one the browser can be sent to.
        Assert.Null(OverviewPaths.Place("art/hero.psd", string.Empty));
        Assert.Null(OverviewPaths.Place(string.Empty, root));
    }

    /// <summary>A cell that names an address is a place as it stands, since only its answerer knows its shape.</summary>
    [Fact]
    public void AnAddressIsAPlaceAsItStands()
    {
        var root = Scratch.New("overview-address");

        Assert.Equal(
            "rola@vaults/origin/@",
            OverviewPaths.Place("rola@vaults/origin/@", root)
        );
    }

    /// <summary>The host over the program's windows, and a dock whose view keeps what it is told.</summary>
    /// <returns>The services, the plugin's host, the views made, and the registration.</returns>
    private static (
        HostServices Services,
        IPluginHost Plugin,
        List<RecordingDockView> Views,
        DockRegistration Registration
    ) Hosts()
    {
        var services = Host.Services();

        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);

        var plugin = services.For(new PluginId("it.overview"), 1);
        var views = new List<RecordingDockView>();
        var registration = new DockRegistration(
            Core.Id,
            FileSystem.DirectoryDockNameId,
            "it_dock.browser",
            DockOpenMode.New,
            DockSide.Center,
            () =>
            {
                var view = new RecordingDockView();

                views.Add(view);

                return view;
            }
        );

        services.Docks.Register(registration);

        return (services, plugin, views, registration);
    }

    /// <summary>A press and a release in the middle of a control, which is what a click on a row's path is.</summary>
    /// <param name="window">The window the control is in.</param>
    /// <param name="control">The control.</param>
    private static void Click(Window window, Control control)
    {
        var at = control.TranslatePoint(
            new Point(control.Bounds.Width / 2, control.Bounds.Height / 2),
            window
        );

        Assert.NotNull(at);

        window.MouseDown(at.Value, MouseButton.Left);
        window.MouseUp(at.Value, MouseButton.Left);

        Dispatcher.UIThread.RunJobs();
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>Lets the window lay itself out.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }
}

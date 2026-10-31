using Avalonia.Controls;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A place asked to be shown in a dock: the dock the user worked in is the one that shows it.
/// </summary>
/// <remarks>
/// What a view that names a path asks the host for, and the answers the host owes it: no second instance beside one
/// the user already has, the dock behind another one brought forward, and one made when there is none at all.
/// </remarks>
public sealed class RevealTests
{
    /// <summary>A place is shown in the dock the user worked in, and no second one is made beside it.</summary>
    [Fact]
    public void APlaceIsShownInTheDockWorkedIn()
    {
        var (services, plugin) = Hosts();
        var views = new List<RecordingDockView>();
        var registration = Register(services, views, "it.reveal.browser");

        services.Windows.OpenMain();

        var first = services.Docks.Activate(registration)!;

        _ = services.Docks.Activate(registration);

        // The user was working in the first one, not the one made after it.
        services.Docks.WorkedIn(first);

        plugin.Windows.Reveal(new DockRequest(registration.DockNameId, "/shown"));

        Assert.Equal(2, services.Docks.Instances.Count);

        // Each dock was told nothing when it was made, and the one the user worked in was told where to look.
        Assert.Equal("/shown", views[0].Requests.Last().StartAt);
        Assert.Equal([null], views[1].Requests.Select(request => request.StartAt));
        Assert.Equal(new FocusedDock(first.DockNameId, first.Ordinal), plugin.Focus.Current);

        // And its tab is the one chosen: the dock the work went to is the one on screen.
        var group = DockTree.GroupOf(services.Docks.Layout, first.DockNameId, first.Ordinal);

        Assert.NotNull(group);
        Assert.Equal(0, group.Active);
    }

    /// <summary>A dock that is not open is made, and told what to show as it is made.</summary>
    [Fact]
    public void ADockIsMadeWhenNoneIsOpen()
    {
        var (services, plugin) = Hosts();
        var views = new List<RecordingDockView>();
        var registration = Register(services, views, "it.reveal.browser");

        plugin.Windows.Reveal(new DockRequest(registration.DockNameId, "/made"));

        var made = Assert.Single(services.Docks.Instances);
        var view = Assert.Single(views);

        Assert.Equal(["/made"], view.Requests.Select(request => request.StartAt));
        Assert.Equal(new FocusedDock(made.DockNameId, made.Ordinal), plugin.Focus.Current);
        Assert.Null(made.Window);
    }

    /// <summary>Showing a dock brings its tab forward, which is what makes it the one on screen.</summary>
    [Fact]
    public void ShowingADockChoosesItsTab()
    {
        var (services, plugin) = Hosts();
        var views = new List<RecordingDockView>();
        var registration = Register(services, views, "it.reveal.browser");

        services.Docks.Register(
            new DockRegistration(
                Core.Id,
                "it.reveal.other",
                "it_dock.other",
                DockOpenMode.New,
                DockSide.Center,
                () => new RecordingDockView()
            )
        );

        services.Windows.OpenMain();

        var browser = services.Docks.Activate(registration)!;

        // Another dock is the tab on screen, which is what leaves the browser behind it while still being the dock
        // the user worked in.
        _ = services.Docks.Activate(services.Docks.Registrations.Single(one => one.DockNameId == "it.reveal.other"));

        plugin.Windows.Reveal(new DockRequest(registration.DockNameId, "/first"));

        var group = DockTree.GroupOf(services.Docks.Layout, browser.DockNameId, browser.Ordinal);

        Assert.NotNull(group);
        Assert.Equal(0, group.Active);
        Assert.Equal(new FocusedDock(browser.DockNameId, browser.Ordinal), plugin.Focus.Current);
    }

    /// <summary>The host, over windows the program's own, and one plugin to read it through.</summary>
    /// <returns>The services and the plugin's host.</returns>
    private static (HostServices Services, IPluginHost Plugin) Hosts()
    {
        var services = Host.Services();

        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);

        return (services, services.For(new PluginId("it.reveal"), 1));
    }

    /// <summary>Registers a dock whose view keeps what it is told, over the views made for it.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="views">Where the views that are made are kept, in the order they were.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <returns>The registration.</returns>
    private static DockRegistration Register(
        HostServices services,
        List<RecordingDockView> views,
        string nameId
    )
    {
        var registration = new DockRegistration(
            Core.Id,
            nameId,
            "it_dock.reveal",
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

        return registration;
    }
}

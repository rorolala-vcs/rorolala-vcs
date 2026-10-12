using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The host's windows: which window a dock is in, and what happens to the one it left.
/// </summary>
/// <remarks>
/// Nothing here draws, and nothing here makes a window: a host with no platform has no way to make one, so
/// what is exercised is the bookkeeping a window is opened from and closed over. What a window looks like is
/// the look's, and what a window is for is this.
/// </remarks>
public sealed class WindowTests
{
    /// <summary>
    /// A dock moved into a window of its own is in that window, and its own placement says so.
    /// </summary>
    /// <remarks>
    /// The two are one change: a dock whose window moved and whose placement did not would be drawn in a
    /// region of the window it left, and one whose placement moved and whose window did not would be drawn
    /// in the middle of the window it is still in.
    /// </remarks>
    [Fact]
    public void ADockMovedIntoAWindowOfItsOwnIsInThatWindow()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        var other = New("it.dock.other");
        services.Docks.Register(registration);
        services.Docks.Register(other);

        var mine = services.Docks.Activate(registration);
        var theirs = services.Docks.Activate(other);

        Assert.NotNull(mine);
        Assert.NotNull(theirs);

        services.Windows.ToOwnWindow(mine);

        Assert.NotNull(mine.Window);
        Assert.Equal(DockPlacement.Window, mine.Placement);

        // The dock beside it is where it was: one dock's window is that dock's, and nothing else moved.
        Assert.Null(theirs.Window);
    }

    /// <summary>Asking twice for the same window is asking for the one that is already there.</summary>
    [Fact]
    public void ADockAlreadyInAWindowOfItsOwnIsNotMovedAgain()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        services.Windows.ToOwnWindow(instance);

        var window = instance.Window;

        Assert.NotNull(window);

        services.Windows.ToOwnWindow(instance);

        Assert.Equal(window, instance.Window);
        Assert.Single(services.Docks.Instances);
    }

    /// <summary>
    /// A dock in a window of its own is not written into the layout of the main window.
    /// </summary>
    /// <remarks>
    /// The layout states where in the main window each dock sits, and a dock that is somewhere else has no
    /// region to state: written down as though it were in one, it would be read back into the main window
    /// and leave the window it was left in.
    /// </remarks>
    [Fact]
    public void ADockInAWindowOfItsOwnIsNotWrittenIntoTheLayout()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);
        Assert.Contains(services.Docks.Snapshot().Docks, dock => dock.DockNameId == "it.dock.browser");

        services.Windows.ToOwnWindow(instance);

        Assert.DoesNotContain(services.Docks.Snapshot().Docks, dock => dock.DockNameId == "it.dock.browser");
    }

    /// <summary>A dock opened in a window of its own is there before anything draws it.</summary>
    /// <remarks>
    /// The window is stated as the instance is made rather than assigned after it, because there is no
    /// moment at which a dock opened in a window of its own is in the main one: it would be drawn in a
    /// region of the main window for as long as the assignment took.
    /// </remarks>
    [Fact]
    public void ADockOpenedInAWindowOfItsOwnIsNeverInTheMainOne()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        var instance = services.Docks.Create(
            registration,
            DockPlacement.Window,
            new DockRequest(registration.DockNameId, "/work"),
            "dock-1"
        );

        Assert.NotNull(instance);
        Assert.Equal("dock-1", instance.Window);
        Assert.Equal(DockPlacement.Window, instance.Placement);
        Assert.Equal("/work", instance.Request.StartAt);
        Assert.Empty(services.Docks.Snapshot().Docks);
    }

    /// <summary>
    /// A window that is asked for when there is no way to make one opens nothing and throws nothing.
    /// </summary>
    /// <remarks>
    /// What a headless host is: a request for a window is a request for something it was never given, and
    /// the answer to it is that nothing happened rather than a failure — what is being asked for is the
    /// window, and the rest of the request is not something a window is needed for.
    /// </remarks>
    [Fact]
    public void AWindowAskedForWhereNoneCanBeMadeOpensNothing()
    {
        var services = Host.Services();

        services.Windows.OpenMain();

        Assert.False(services.Windows.MainIsUp);
        Assert.Equal(0, services.Windows.Count);
    }

    /// <summary>The windows a dock is offered are one dock's, in the order they are registered.</summary>
    [Fact]
    public void TheItemsOfADocksOwnMenuAreThatDocksAndInOrder()
    {
        var services = Host.Services();
        var early = services.For(new PluginId("it.early"), 0);
        var late = services.For(new PluginId("it.late"), 1);

        late.DockMenus.Add(
            DockMenuTarget.Tab,
            new DockMenuItem(null, new ContextMenuItem("everywhere-late", 0, _ => { }))
        );
        early.DockMenus.Add(
            DockMenuTarget.Tab,
            new DockMenuItem("it.dock.browser", new ContextMenuItem("browser-late", 20, _ => { }))
        );
        early.DockMenus.Add(
            DockMenuTarget.Tab,
            new DockMenuItem("it.dock.browser", new ContextMenuItem("browser-first", 10, _ => { }))
        );

        Assert.Equal(
            ["browser-first", "browser-late", "everywhere-late"],
            Items(services, "it.dock.browser")
        );

        // A menu about one dock says nothing about another, and the one that is every dock's is in both.
        Assert.Equal(["everywhere-late"], Items(services, "it.dock.tree"));
    }

    /// <summary>What one dock's menu offers, by label key.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="dockNameId">The dock the menu belongs to.</param>
    /// <returns>The label keys, in the order they are shown.</returns>
    private static string[] Items(HostServices services, string dockNameId) =>
        [.. services.Windows.DockMenuItems(DockMenuTarget.Tab, dockNameId).Select(item => item.LabelKey)];

    /// <summary>A registration for a dock that may be made again, as a browser is.</summary>
    /// <param name="nameId">Its stable name id.</param>
    /// <returns>The registration.</returns>
    private static DockRegistration New(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.New,
            DockPlacement.Center,
            _ => new FakeDockView()
        );
}

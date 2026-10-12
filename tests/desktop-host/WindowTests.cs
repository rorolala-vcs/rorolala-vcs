using Avalonia;
using Avalonia.Controls;
using Avalonia.VisualTree;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using ContractMenuItem = RorolalaDesktop.Contract.MenuItem;

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
    /// A dock moved into a window of its own is in that window, and keeps the region it was in.
    /// </summary>
    /// <remarks>
    /// The window and the region are two questions: being in a window of its own is a change of window and
    /// not of region, so a dock pulled out of the bottom band comes back to it when it returns. A placement
    /// that meant "a window of its own" would be the one name meaning both, which is what this checks is not
    /// the case.
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

        services.Docks.Move(mine, DockPlacement.Bottom);
        services.Windows.ToOwnWindow(mine);

        Assert.NotNull(mine.Window);
        Assert.Equal(DockPlacement.Bottom, mine.Placement);

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
            DockPlacement.Center,
            new DockRequest(registration.DockNameId, "/work"),
            "dock-1"
        );

        Assert.NotNull(instance);
        Assert.Equal("dock-1", instance.Window);
        Assert.Equal(DockPlacement.Center, instance.Placement);
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

    /// <summary>
    /// A dock moved into a window of its own is drawn in that window and in no other.
    /// </summary>
    /// <remarks>
    /// The whole point of moving one, and the one thing that cannot be checked by bookkeeping: the dock has
    /// to end up on the tree of the window it was moved to, and off the tree of the window it left. A dock
    /// that is in the right window and drawn nowhere is an empty window with a menu bar across the top —
    /// which is what this test exists because it was once the case.
    /// <para>
    /// The window it arrives in is the one the shell names, which is the one the host last made, and that is
    /// the window of the dock that was moved: what is checked is that the control went from the window it was
    /// in to a window it was not, which is the whole of the defect this is here to keep out.
    /// </para>
    /// </remarks>
    [Fact]
    public void ADockMovedIntoAWindowOfItsOwnIsDrawnInThatWindow()
    {
        var control = new Border();

        var (services, disposed) = Windows();

        try
        {
            var registration = new DockRegistration(
                Core.Id,
                "it.dock.visible",
                "it_dock.title",
                DockOpenMode.New,
                DockPlacement.Center,
                _ => new FakeDockView { Control = control }
            );
            services.Docks.Register(registration);

            var instance = services.Docks.Activate(registration);

            Assert.NotNull(instance);

            var main = (MainWindow)services.Shell.Window!;

            // Before the move it is drawn in the window that is up, which is what makes the check after it
            // mean something: the same control, in another window.
            var was = Window(control);

            Assert.Same(main, was);

            services.Windows.ToOwnWindow(instance);

            var detached = (MainWindow)services.Shell.Window!;

            Assert.True(control.IsAttachedToVisualTree(), "the dock is drawn somewhere");

            // Drawn in a window of its own now, and not the one it was in: the control is on another window's
            // tree, which is the whole of what tearing a dock out is.
            var now = Window(control);

            Assert.True(!ReferenceEquals(was, now), $"still drawn in the window it was moved out of: {detached.Tag}");
        }
        finally
        {
            foreach (var dispose in disposed)
            {
                dispose();
            }
        }
    }

    /// <summary>
    /// A window that is for a dock shows that dock's plugin's menus and the kernel's own.
    /// </summary>
    /// <remarks>
    /// What a window of one dock is for is that dock, so a menu about a tree has nothing to say in a window
    /// that holds a browser — while what the program itself has to say, which is the Window menu and the way
    /// back to the main window, is said in every window there is.
    /// <para>
    /// Asked of the menu bar rather than of a window, because what a window shows is this question and
    /// nothing else about windows: a row is what is drawn from the answer, and drawing one needs no
    /// window.
    /// </para>
    /// </remarks>
    [Fact]
    public void AWindowForADockShowsThatDocksPluginMenusAndTheKernels()
    {
        var services = Host.Services();
        var mine = new PluginId("it.mine");

        services.Menu.AddTopLevel(Core.Id, -1, "menu.window", 100);
        services.Menu.AddTopLevel(mine, 1, "rorolala_file_system.menu.operations", 50);
        services.Menu.AddTopLevel(new PluginId("it.other"), 2, "rorolala_file_system.hidden", 60);

        Assert.Equal(
            ["rorolala_file_system.menu.operations", "rorolala_file_system.hidden", "menu.window"],
            Keys(MainWindow.Visible(services.Menu, null))
        );

        Assert.Equal(
            ["rorolala_file_system.menu.operations", "menu.window"],
            Keys(MainWindow.Visible(services.Menu, mine))
        );
    }

    /// <summary>The label keys of some top-level menus, in the order they are shown.</summary>
    /// <param name="menus">The menus.</param>
    /// <returns>Their label keys.</returns>
    private static string[] Keys(IReadOnlyList<TopLevelMenu> menus) =>
        [.. menus.Select(menu => menu.LabelKey)];

    /// <summary>The window a control is drawn in, or nothing when it is in none.</summary>
    /// <param name="control">The control.</param>
    /// <returns>The window above it.</returns>
    private static Window? Window(Control control)
    {
        for (Visual? at = control; at is not null; at = at.GetVisualParent())
        {
            if (at is Window window)
            {
                return window;
            }
        }

        return null;
    }

    /// <summary>
    /// A host whose windows are real ones, so that what a dock ends up drawn in can be asked.
    /// </summary>
    /// <remarks>
    /// The windows are the program's own — the shell a dock is drawn in, and everything that follows from it.
    /// Nothing is closed at the end of a test that uses this: what is being read is where a control was put,
    /// which is answered the moment it is put there, and a window closed from a test body is a window the
    /// toolkit is still tearing down when the next test reads its tree.
    /// </remarks>
    /// <returns>Everything the host holds, and how to take the test's windows down.</returns>
    private static (HostServices Services, IReadOnlyList<Action> Disposed) Windows()
    {
        var services = Host.Services();
        var made = new List<MainWindow>();

        services.Windows.Make = (key, dock) =>
        {
            var window = new MainWindow(services, key, dock);

            services.Shell.Window = window;
            made.Add(window);

            return window;
        };

        return (services, []);
    }

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

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
        var (services, _) = Windows();
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
        var (services, _) = Windows();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        // A dock that stays in the main window, so that tearing this one out leaves it standing.
        var kept = New("it.dock.kept");
        services.Docks.Register(kept);
        services.Docks.Activate(kept);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        services.Windows.ToOwnWindow(instance);

        var window = instance.Window;

        Assert.NotNull(window);

        services.Windows.ToOwnWindow(instance);

        // The same window, and no second one: what is already there is brought forward instead. The key is
        // what says so — a window made again would have the next one — and the count would not, since it
        // counts the main window too and that moves with the docks in it.
        Assert.Equal(window, instance.Window);
        Assert.Single(services.Docks.Instances, one => one.DockNameId == registration.DockNameId);
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

    /// <summary>
    /// A dock whose window is closed comes back to the main window, and stays what it was.
    /// </summary>
    /// <remarks>
    /// The thing a window closing must not leave behind: a dock that still says it is in a window that is
    /// gone. Nothing can show a dock like that — the next time it is asked for there is no window to ask, so
    /// it is invisible for the rest of the run, and a toggle is worse than invisible: choosing it again turns
    /// it over rather than bringing it back.
    /// <para>
    /// A toggle is the dock this is asked of, because that is the one that lives on after its window closes,
    /// which is what makes it the one that can be stranded.
    /// </para>
    /// </remarks>
    [Fact]
    public void ADockWhoseWindowIsClosedComesBackToTheMainWindow()
    {
        var (services, _) = Windows();
        var registration = new DockRegistration(
            Core.Id,
            "it.dock.toggle",
            "it_dock.title",
            DockOpenMode.Toggle,
            DockPlacement.Bottom,
            _ => new FakeDockView { Control = new Border() }
        );
        services.Docks.Register(registration);

        // Another dock in the main window, so that it is still there when the toggle is torn out of it.
        services.Docks.Register(New("it.dock.kept"));
        services.Docks.Activate(services.Docks.Registrations[1]);

        var toggle = services.Docks.Activate(registration);

        Assert.NotNull(toggle);

        services.Windows.ToOwnWindow(toggle);

        var own = (MainWindow)services.Shell.Window!;

        Assert.NotNull(toggle.Window);

        own.Close();

        // Back in the main window, which is where it came from, and open as it was: the window it was in is
        // gone, and a dock a window took with it is a dock the main window still has.
        Assert.Null(toggle.Window);
        Assert.True(toggle.IsOpen);

        // And asking for it again is asking for it where it is: the window it is in is the main one, so a
        // toggle turns over as it always does — once to hide it, and once to show it again.
        Assert.Same(toggle, services.Docks.Activate(registration));
        Assert.False(toggle.IsOpen);

        Assert.Same(toggle, services.Docks.Activate(registration));
        Assert.True(toggle.IsOpen);
    }

    /// <summary>
    /// A dock opened in a window of its own is there before anything draws it.
    /// </summary>
    /// <remarks>
    /// The window is stated as the instance is made rather than assigned after it, because there is no
    /// moment at which a dock opened in a window of its own is in the main one: it would be drawn in a
    /// region of the main window for as long as the assignment took. What is asked of the manager rather
    /// than of the windows, because it is the manager's own answer: a window of the host's does not exist
    /// yet at this point, and the dock is already where it is going.
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
    /// A layer that is one dock keeps no chrome of its own.
    /// </summary>
    /// <remarks>
    /// What a window of a dock's own is: the dock, under the window's own row. No strip of tabs, because
    /// there is nothing to switch between; no splitter, because there is nothing to resize it against; and no
    /// empty band where a region is not, because there are no regions. A window that drew the chrome anyway
    /// would be a strip around a view that nothing can be put beside — and the one thing a torn-out dock must
    /// not look like is the window it came out of.
    /// <para>
    /// Asked of the layer rather than of a window, because the chrome is the layer's: what a window does with
    /// it is put it under the row. The two layers are compared by how much they hold, since a layer that
    /// kept its regions would hold the five of them whether or not any dock were in it.
    /// </para>
    /// </remarks>
    [Fact]
    public void AWindowThatIsOneDockDrawsNoRegionChrome()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.visible");
        services.Docks.Register(registration);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        // The layer of a window with docks in it, and the layer of a window that is one dock.
        var many = new DockArea(services.Docks, services.I18n, services.Windows, null);
        var alone = new DockArea(services.Docks, services.I18n, services.Windows, "w1");

        Assert.False(many.Alone);
        Assert.True(alone.Alone);

        // A layer that is not one dock holds the surface it is laid out in: the five regions, the four
        // splitters, and the boxes a drag could land in. It draws no dock itself.
        Assert.IsAssignableFrom<Grid>(many.Content);
        Assert.Null(many.Sole);

        // A layer of one dock holds nothing until it is given the dock, since what it is for is that dock.
        Assert.Null(alone.Sole);
    }

    /// <summary>
    /// A window of the host's can be closed, and the host knows it is gone.
    /// </summary>
    /// <remarks>
    /// The smallest thing the tests above rest on, asked on its own: whether closing a window works at all
    /// under the platform they are run on. Kept because the answer is not obvious — everything else here
    /// depends on it, and a failure of this one would say what a failure of the others means.
    /// </remarks>
    [Fact]
    public void AWindowOfTheHostsCanBeClosed()
    {
        var (services, _) = Windows();

        services.Windows.OpenMain();

        Assert.True(services.Windows.MainIsUp);

        var main = (MainWindow)services.Shell.Window!;

        main.Close();

        Assert.False(services.Windows.MainIsUp);
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
    /// <remarks>
    /// The view is given a control, because a test that makes a real window makes a real dock area, and an
    /// area cannot put nothing in a region: what a dock is drawn as is part of what a window is made of.
    /// </remarks>
    /// <param name="nameId">Its stable name id.</param>
    /// <returns>The registration.</returns>
    private static DockRegistration New(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.New,
            DockPlacement.Center,
            _ => new FakeDockView { Control = new Border() }
        );
}

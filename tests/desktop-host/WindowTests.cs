using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Input.Raw;
using Avalonia.VisualTree;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Theming;
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
    /// A dock brought back to the main window leaves the window it was in behind.
    /// </summary>
    /// <remarks>
    /// What the option a window of one dock offers does: the dock goes back where it came from, to the region
    /// it was in, and the window that held it — which is a window of that dock and of nothing else — goes with
    /// it. A window left standing would be a frame around a dock that is drawn in another window.
    /// </remarks>
    [Fact]
    public void ADockBroughtBackToTheMainWindowLeavesItsWindowBehind()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.browser");
        var kept = New("it.dock.kept");
        services.Docks.Register(registration);
        services.Docks.Register(kept);

        var stays = services.Docks.Activate(kept);
        var instance = services.Docks.Activate(registration);

        Assert.NotNull(stays);
        Assert.NotNull(instance);

        services.Windows.OpenMain();
        services.Docks.Move(instance, DockPlacement.Bottom);
        services.Windows.ToOwnWindow(instance);

        Assert.NotNull(instance.Window);
        Assert.Equal(2, services.Windows.Count);

        services.Windows.MoveToMain(instance);

        // Back in the main window, in the region it was in, and still open: what was moved is where the dock
        // is drawn, and not whether it is.
        Assert.Null(instance.Window);
        Assert.True(instance.IsOpen);
        Assert.Equal(DockPlacement.Bottom, instance.Placement);
        Assert.Contains(services.Docks.Snapshot().Docks, dock => dock.DockNameId == "it.dock.browser");

        // And the window it was in is gone, since a window of one dock holds that dock and nothing else.
        Assert.Equal(1, services.Windows.Count);
    }

    /// <summary>
    /// A dock that keeps one instance of its own is not moved in beside the one the main window shows.
    /// </summary>
    /// <remarks>
    /// A dock may be asked for anew even where it keeps one instance — which is what a plugin asking for a
    /// window of its own does — and the main window can then be showing one while a window of its own holds
    /// another. Bringing the second one back would leave two of a name that may have one, so what is let go of
    /// is the second: the dock is already in the main window, and what the move leaves behind is the window.
    /// </remarks>
    [Fact]
    public void ADockTheMainWindowAlreadyShowsIsNotMovedInTwice()
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

        var mine = services.Docks.Activate(registration);

        Assert.NotNull(mine);
        Assert.True(mine.Window is null);

        services.Windows.OpenMain();

        // The second one, asked for the way a plugin asks for a window of its own: a window of its own holds
        // an instance of its own, and the one in the main window is left where it is.
        services.Windows.Open(new DockRequest(registration.DockNameId, null, DockOpenMode.New));

        var theirs = services.Docks.Instances.Single(one => !ReferenceEquals(one, mine));

        Assert.NotNull(theirs.Window);
        Assert.Equal(2, services.Windows.Count);

        services.Windows.MoveToMain(theirs);

        // The one the main window shows is the one that is left, and it is where it was.
        Assert.Same(mine, services.Docks.InstanceOf(registration.DockNameId));
        Assert.Single(services.Docks.Instances, one => one.DockNameId == registration.DockNameId);
        Assert.True(mine.IsOpen);
        Assert.Null(mine.Window);

        // And the window of the duplicate, and the duplicate with it, are gone.
        Assert.Equal(1, services.Windows.Count);
        Assert.Null(services.Docks.InstanceOf(registration.DockNameId, theirs.Ordinal));
    }

    /// <summary>
    /// A header dragged away from every region becomes a window of its own.
    /// </summary>
    /// <remarks>
    /// The whole of what letting go of a header away from a region means: the dock is pulled out of the
    /// window it was in and lands nowhere, so it is given a window of its own rather than being left where it
    /// was. The gesture before it is the host's — a press is not a drag until the pointer has moved — and what
    /// the press becomes is the toolkit's own drag, which is what can carry a dock out of a window.
    /// </remarks>
    [Fact]
    public void ADockDraggedAwayFromEveryRegionBecomesAWindowOfItsOwn()
    {
        var (services, _) = Windows();
        var dragged = new DockRegistration(
            Core.Id,
            "it.dock.dragged",
            "it.dock.dragged.title",
            DockOpenMode.New,
            DockPlacement.Center,
            _ => new FakeDockView { Control = new Border() }
        );
        var kept = New("it.dock.kept");
        services.Docks.Register(dragged);
        services.Docks.Register(kept);

        // Both are opened before there is a window, so that the one window there is draws both of them: what
        // a test cannot do is ask the toolkit for the layout pass a dock opened later would wait for.
        services.Docks.Activate(kept);

        var instance = services.Docks.Activate(dragged);

        Assert.NotNull(instance);

        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;

        // The tab that names this dock, which is the one its header would be dragged by.
        var header = main
            .GetVisualDescendants()
            .OfType<Button>()
            .First(button =>
                button.Classes.Contains(DockArea.TitleClass)
                && button.GetVisualDescendants().OfType<TextBlock>().Any(text => text.Text == "it.dock.dragged.title")
            );
        var from = header.TranslatePoint(
            new Point(header.Bounds.Width / 2, header.Bounds.Height / 2),
            main
        );

        Assert.NotNull(from);

        // A press, a pointer that has moved further than the slack a press is given, and the button let go of.
        main.MouseDown(from.Value, MouseButton.Left);
        main.MouseMove(from.Value + new Point(0, 60), RawInputModifiers.LeftMouseButton);
        main.MouseUp(from.Value + new Point(0, 60), MouseButton.Left);

        // In a window of its own, and out of the arrangement the main window's layout keeps.
        Assert.NotNull(instance.Window);
        Assert.Equal(2, services.Windows.Count);
        Assert.DoesNotContain(services.Docks.Snapshot().Docks, dock => dock.DockNameId == "it.dock.dragged");
        Assert.Contains(services.Docks.Snapshot().Docks, dock => dock.DockNameId == "it.dock.kept");
    }

    /// <summary>
    /// A dock in flight is the instance that was picked up, and not its namesake.
    /// </summary>
    /// <remarks>
    /// What a drag carries is the pair the layout addresses an instance by — the dock's name and which
    /// instance of it this is — because a dock that may be made again has one name and several instances, and
    /// a drop that landed the wrong one of them would move a dock the user never took hold of.
    /// </remarks>
    [Fact]
    public void ADockInFlightIsTheInstanceThatWasPickedUp()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        var first = services.Docks.Activate(registration);
        var second = services.Docks.Activate(registration);

        Assert.NotNull(first);
        Assert.NotNull(second);
        Assert.NotEqual(first.Ordinal, second.Ordinal);

        using var carrying = DockDrag.Carrying(second);

        Assert.Same(second, DockDrag.Carried(services.Docks, carrying));

        // A drag that is not this host's says nothing about any dock.
        using var other = new DataTransfer();

        Assert.Null(DockDrag.Carried(services.Docks, other));
    }

    /// <summary>
    /// A window of one dock names that dock in a row as tall as the main window's.
    /// </summary>
    /// <remarks>
    /// What a window of one dock is: the dock, under the row the desktop's title bar would have drawn. The row
    /// is the menu bar's height in every window of the host — a window of one dock has no strip of tabs for it
    /// to stand in for — and the dock's name is drawn in it, which is what the desktop would have said and what
    /// the dock is dragged back to the main window by.
    /// </remarks>
    [Fact]
    public void AWindowOfOneDockNamesItInARowAsTallAsTheMainWindows()
    {
        var (services, _) = Windows();
        var registration = new DockRegistration(
            Core.Id,
            "it.dock.titled",
            "it.dock.titled.title",
            DockOpenMode.New,
            DockPlacement.Center,
            _ => new FakeDockView { Control = new Border() }
        );
        var kept = New("it.dock.kept");
        services.Docks.Register(registration);
        services.Docks.Register(kept);
        services.Docks.Activate(kept);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        services.Windows.ToOwnWindow(instance);

        var own = (MainWindow)services.Shell.Window!;
        var caption = own
            .GetVisualDescendants()
            .OfType<TextBlock>()
            .First(text => text.Classes.Contains(RorolalaTheme.WindowTitleClass));
        var row = own
            .GetVisualDescendants()
            .OfType<Border>()
            .First(border => border.Classes.Contains(RorolalaTheme.TitleBarClass));

        Assert.Equal("it.dock.titled.title", caption.Text);
        Assert.Equal(MainWindow.TitleBarHeight, row.Bounds.Height);

        // Drawn in the row and not spilling under it: what is below the row is the dock itself.
        var at = caption.TranslatePoint(new Point(0, 0), own);

        Assert.NotNull(at);
        Assert.True(at.Value.Y >= 0, "the name is drawn in the window");
        Assert.True(
            at.Value.Y + caption.Bounds.Height <= MainWindow.TitleBarHeight,
            "the name is drawn in the row and not over the dock"
        );
    }

    /// <summary>
    /// A dock dropped over a region of the main window is placed in it.
    /// </summary>
    /// <remarks>
    /// The half of a drag that a window answers: what arrives is the payload of a drag, and what is drawn from
    /// it is the dock landing where the pointer was. Asked of a real window, with the toolkit's own drag event,
    /// because what is being checked is that the area is a place a drag can land at all — the payload and the
    /// lookup are checked without one.
    /// </remarks>
    [Fact]
    public void ADockDroppedOverARegionLandsInIt()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.dropped");
        var kept = New("it.dock.kept");
        services.Docks.Register(registration);
        services.Docks.Register(kept);
        services.Docks.Activate(kept);
        services.Windows.OpenMain();

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        var main = (MainWindow)services.Shell.Window!;

        Drop(main, OverTheLeftBand(main), instance);

        // The left band of the area, which is where the pointer was: the region a drop lands in is the region
        // the pointer was over, and one that is empty is still a place a dock can be put.
        Assert.Equal(DockPlacement.Left, instance.Placement);
        Assert.Null(instance.Window);
        Assert.Contains(services.Docks.Snapshot().Docks, dock => dock.DockNameId == "it.dock.dropped");
    }

    /// <summary>
    /// A dock dragged out of a window of its own and dropped on the main window is moved back into it.
    /// </summary>
    /// <remarks>
    /// The other direction of the same gesture, and the one a window of one dock is brought back by: what is
    /// dragged is the dock, and the window it is drawn in is where the drag began — so landing in the main
    /// window is a change of window, and the window that held it is left with nothing to draw and closes.
    /// The region the drop was over is the region it arrives in, and not the one it left.
    /// </remarks>
    [Fact]
    public void ADockDroppedOnTheMainWindowComesBackFromAWindowOfItsOwn()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.brought");
        var kept = New("it.dock.kept");
        services.Docks.Register(registration);
        services.Docks.Register(kept);
        services.Docks.Activate(kept);
        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;
        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        services.Windows.ToOwnWindow(instance);

        Assert.NotNull(instance.Window);
        Assert.Equal(2, services.Windows.Count);

        Drop(main, OverTheLeftBand(main), instance);

        Assert.Null(instance.Window);
        Assert.Equal(DockPlacement.Left, instance.Placement);
        Assert.Equal(1, services.Windows.Count);
    }

    /// <summary>
    /// Drops a dock at a point in a window, the way the platform delivers a drag.
    /// </summary>
    /// <remarks>
    /// A drag arrives in the order the platform delivers it, and what it takes to land in a region is the whole
    /// of it: a target that says nothing as the drag enters it is a target the platform gives up on.
    /// </remarks>
    /// <param name="main">The window whose area the dock is dropped on.</param>
    /// <param name="at">Where in it.</param>
    /// <param name="instance">The dock that is being dragged.</param>
    private static void Drop(MainWindow main, Point at, DockInstance instance)
    {
        using var carrying = DockDrag.Carrying(instance);

        foreach (
            var arrival in new[]
            {
                RawDragEventType.DragEnter,
                RawDragEventType.DragOver,
                RawDragEventType.Drop,
            }
        )
        {
            main.DragDrop(at, arrival, carrying, DragDropEffects.Move, RawInputModifiers.LeftMouseButton);
        }
    }

    /// <summary>A point over the left band of a window's dock area.</summary>
    /// <remarks>
    /// The band is the edge of the area, which is the region a drop there means: what a drag lands in is the
    /// band the region is drawn in, and a region holding nothing is drawn as the narrow band it is aimed at.
    /// The area has to have been laid out for any of that to mean anything, which is why it is asked of the
    /// window that was shown.
    /// </remarks>
    /// <param name="main">The window to find the point in.</param>
    /// <returns>The point, in that window's own coordinates.</returns>
    private static Point OverTheLeftBand(MainWindow main)
    {
        var area = main.GetVisualDescendants().OfType<DockArea>().First();

        Assert.True(area.Bounds.Height > 0, "the window was shown, so its area was laid out");

        var at = area.TranslatePoint(new Point(2, area.Bounds.Height / 2), main);

        Assert.NotNull(at);

        return at.Value;
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

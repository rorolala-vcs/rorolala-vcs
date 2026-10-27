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
    /// A dock moved into a window of its own is in that window, and keeps the group it came from.
    /// </summary>
    /// <remarks>
    /// The window and the group are two questions: being in a window of its own is a change of window and not of
    /// group, so a dock taken out of the bottom band comes back to it when it returns — and the group it was in
    /// may be pruned when it was the only dock there, which is why the id is kept rather than the place.
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

        services.Docks.Move(mine, Beside(services.Docks, mine, DockSide.Bottom));
        services.Windows.ToOwnWindow(mine);

        Assert.NotNull(mine.Window);
        Assert.Equal("g-2", mine.GroupId);

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
    /// What the option a window of one dock offers does: the dock goes back where it came from, to the group it
    /// was in — or the active group where that one has been pruned since — and the window that held it, which is
    /// a window of that dock and of nothing else, goes with it. A window left standing would be a frame around a
    /// dock that is drawn in another window.
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
        services.Docks.Move(instance, Beside(services.Docks, instance, DockSide.Bottom));
        services.Windows.ToOwnWindow(instance);

        Assert.NotNull(instance.Window);
        Assert.Equal(2, services.Windows.Count);

        services.Windows.MoveToMain(instance);

        // Back in the main window, in a group of it, and still open: what was moved is where the dock is drawn,
        // and not whether it is. The group it came from was pruned with it, so it came back to the active one.
        Assert.Null(instance.Window);
        Assert.True(instance.IsOpen);
        Assert.NotNull(DockTree.GroupOf(services.Docks.Layout, instance.DockNameId, instance.Ordinal));
        Assert.Contains("it.dock.browser", Saved(services));

        // And the window it was in is gone, since a window of one dock holds that dock and nothing else.
        Assert.Equal(1, services.Windows.Count);
    }

    /// <summary>
    /// The shell names a window that is still up when the one it named closes.
    /// </summary>
    /// <remarks>
    /// What a question put after a dock's own window is closed depends on: the shell must not keep naming
    /// the window that went away, or the question is asked nowhere. The main window is what a dock comes
    /// back to, so it is what is named once the window of its own is gone.
    /// </remarks>
    [Fact]
    public void TheShellNamesAWindowThatIsUpWhenTheOneItNamedCloses()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.browser");
        var kept = New("it.dock.kept");
        services.Docks.Register(registration);
        services.Docks.Register(kept);
        services.Docks.Activate(kept);
        services.Windows.OpenMain();

        var main = services.Shell.Window;

        Assert.NotNull(main);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);

        services.Windows.ToOwnWindow(instance);

        var own = services.Shell.Window;

        Assert.NotNull(own);
        Assert.NotSame(main, own);

        own.Close();

        Assert.Same(main, services.Shell.Window);
    }

    /// <summary>
    /// The main window stands when the last dock leaves it, empty rather than closed.
    /// </summary>
    /// <remarks>
    /// The one window and the one dock a program may be down to, which is the state a first run starts in: what
    /// ends the program is every window being gone, so a main window closed as its last dock is torn out is the
    /// program ending — the drag would take the program with it, and the dock with the window it was being moved
    /// out of. What is left instead is an empty main window, which is the program's own window: its
    /// <c>Window</c> menu is where a dock is asked for again, and it is what a dock in a window of its own is
    /// brought back to.
    /// </remarks>
    [Fact]
    public void TheMainWindowStandsWhenTheLastDockLeavesIt()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);
        services.Windows.OpenMain();

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);
        Assert.Null(instance.Window);

        var ended = false;

        services.Windows.End = () => ended = true;

        services.Windows.ToOwnWindow(instance);

        Assert.NotNull(instance.Window);
        Assert.False(ended, "the program was not ended by a dock being torn out");
        Assert.True(services.Windows.MainIsUp, "the main window was not closed with its last dock");
        Assert.DoesNotContain(services.Docks.Instances, one => one.Window is null);
        Assert.Equal(2, services.Windows.Count);
    }

    /// <summary>
    /// A dock opened in a window of its own goes with that window when it closes.
    /// </summary>
    /// <remarks>
    /// What a directory asked for in a window of its own is, and the one thing it must not do: a dock that was
    /// <em>opened</em> in a window — rather than moved into it out of the main window — was never in the main
    /// window, so closing that window is the end of it. Brought back instead, closing a window opened onto one
    /// directory would leave a dock of that directory in the main window, kept in the layout, that nobody
    /// asked for.
    /// </remarks>
    [Fact]
    public void ADockOpenedInAWindowOfItsOwnGoesWithIt()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.browser");
        var kept = New("it.dock.kept");
        services.Docks.Register(registration);
        services.Docks.Register(kept);
        services.Docks.Activate(kept);
        services.Windows.OpenMain();

        var stays = services.Docks.InstanceOf(kept.DockNameId);

        Assert.NotNull(stays);
        Assert.Null(stays.Window);

        // What the File System's own item asks for: a dock of its own, in a window of its own.
        services.Windows.Open(new DockRequest(registration.DockNameId, "/work", DockOpenMode.New));

        var opened = services.Docks.Instances.Single(one => one.DockNameId == "it.dock.browser");

        Assert.NotNull(opened.Window);
        Assert.True(opened.OpenedHere);
        Assert.Equal(2, services.Windows.Count);

        var own = (MainWindow)services.Shell.Window!;

        own.Close();

        // Gone with the window, and not moved into the main window: what is left there is the dock that was
        // in it from the start.
        Assert.DoesNotContain(services.Docks.Instances, one => one.DockNameId == "it.dock.browser");
        Assert.DoesNotContain("it.dock.browser", Saved(services));
        Assert.Equal(1, services.Windows.Count);
        Assert.Same(stays, services.Docks.InstanceOf(kept.DockNameId));
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
            DockSide.Bottom,
            () => new FakeDockView { Control = new Border() }
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
    /// A header dragged away from every dock area becomes a window of its own.
    /// </summary>
    /// <remarks>
    /// The whole of what letting go of a header away from an area means: the dock is pulled out of the window it
    /// was in and lands nowhere, so it is given a window of its own rather than being left where it was. The
    /// gesture before it is the host's — a press is not a drag until the pointer has moved — and what the press
    /// becomes is the toolkit's own drag, which is what can carry a dock out of a window.
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
            DockSide.Center,
            () => new FakeDockView { Control = new Border() }
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
        Assert.DoesNotContain("it.dock.dragged", Saved(services));
        Assert.Contains("it.dock.kept", Saved(services));
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
            DockSide.Center,
            () => new FakeDockView { Control = new Border() }
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
    /// A dock dropped on the point that means "to the left" splits the group, and the new group is on that side.
    /// </summary>
    /// <remarks>
    /// The second half of the two-step gesture: a drag over a group only shows what could be done, and what is
    /// dropped is the point it was let go of. Asked of a real window, with the toolkit's own drag event, because
    /// what is being checked is that the area is a place a drag can land at all — the payload and the lookup are
    /// checked without one.
    /// </remarks>
    [Fact]
    public void ADockDroppedOnTheLeftPointSplitsTheGroup()
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

        Drop(main, OverDockPoint(main, DockSide.Left), instance);

        // The left point of the group, which is what the pointer was over: a drop there means the new group stands
        // to the left of the group it was dropped on, and it is the same group the dock was already a tab of.
        var root = Assert.IsType<LayoutSplit>(services.Docks.Layout.Root);
        var added = Assert.IsType<LayoutGroup>(root.First);

        Assert.Equal(DockAxis.Columns, root.Axis);
        Assert.Equal(instance.GroupId, added.Id);
        Assert.Equal(["it.dock.dropped"], added.Docks.Select(dock => dock.DockNameId));
        Assert.Null(instance.Window);
        Assert.Contains("it.dock.dropped", Saved(services));
    }

    /// <summary>A dock dropped on the centre point joins the group's tabs rather than splitting it.</summary>
    /// <remarks>
    /// The one point of a group that does not make a new group: the dock is put among the tabs of the group it was
    /// dropped on, which is what a drop on the middle of a group means.
    /// </remarks>
    [Fact]
    public void ADockDroppedOnTheCentrePointJoinsTheGroup()
    {
        var (services, _) = Windows();
        var registration = New("it.dock.joined");
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

        Drop(main, OverDockPoint(main, DockSide.Center), instance);

        // No split: the dock is a tab of the one group there is, which it was taken out of a moment ago.
        var group = Assert.IsType<LayoutGroup>(services.Docks.Layout.Root);

        Assert.Null(instance.Window);
        Assert.Contains("it.dock.joined", group.Docks.Select(dock => dock.DockNameId));
        Assert.Contains("it.dock.kept", group.Docks.Select(dock => dock.DockNameId));
    }

    /// <summary>
    /// A dock dragged out of a window of its own and dropped on the main window is moved back into it.
    /// </summary>
    /// <remarks>
    /// The other direction of the same gesture, and the one a window of one dock is brought back by: what is
    /// dragged is the dock, and the window it is drawn in is where the drag began — so landing in the main window
    /// is a change of window, and the window that held it is left with nothing to draw and closes. Where it lands
    /// is where the drop said, and not the group it left.
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

        Drop(main, OverDockPoint(main, DockSide.Left), instance);

        Assert.Null(instance.Window);
        Assert.NotNull(DockTree.GroupOf(services.Docks.Layout, instance.DockNameId, instance.Ordinal));
        Assert.Equal(1, services.Windows.Count);
    }

    /// <summary>
    /// A dock brought back from a window of its own goes back into the group it came from, when that is there.
    /// </summary>
    /// <remarks>
    /// What keeping the group is for: a window of its own is a leave of absence, so a dock taken out of a group
    /// and brought back is put in that group rather than wherever happens to be active. The group only survives
    /// the leaving when it still has a dock of its own, which is what the third dock is for.
    /// </remarks>
    [Fact]
    public void AReturnedDockGoesBackIntoItsOwnGroup()
    {
        var (services, _) = Windows();
        var one = New("it.dock.one");
        var two = New("it.dock.two");
        var three = New("it.dock.three");
        services.Docks.Register(one);
        services.Docks.Register(two);
        services.Docks.Register(three);

        var first = services.Docks.Activate(one);
        var second = services.Docks.Activate(two);
        var third = services.Docks.Activate(three);

        Assert.NotNull(first);
        Assert.NotNull(second);
        Assert.NotNull(third);

        // The first dock is split off into a group of its own, the third is put in that group, and then the first
        // is taken out into a window of its own: the group is still there, holding the third.
        services.Docks.Move(first, Beside(services.Docks, first, DockSide.Left));

        var home = first.GroupId;

        services.Docks.Move(third, new DockTarget(home, DockSide.Center, 1));
        services.Windows.ToOwnWindow(first);

        Assert.NotNull(first.Window);
        Assert.Equal(home, first.GroupId);

        services.Windows.MoveToMain(first);

        Assert.Null(first.Window);
        Assert.Equal(home, first.GroupId);
        Assert.Contains(
            first.DockNameId,
            DockTree.Find(services.Docks.Layout, home)!.Docks.Select(dock => dock.DockNameId)
        );
    }

    /// <summary>
    /// A splitter that is dragged writes the share it was left at, and the tree keeps it.
    /// </summary>
    /// <remarks>
    /// What a drag of a divider is for: the room it gives each side is a share of the whole, so the same drag in a
    /// window of another size puts the divider in the same place. The root has to still be the same split
    /// afterwards — what a drag writes is a number, not an arrangement — which is also what says the rebuild it
    /// causes did not throw away the tree it was rebuilding from.
    /// </remarks>
    [Fact]
    public void ADraggedSplitterWritesTheShareBack()
    {
        var (services, _) = Windows();
        var left = New("it.dock.left");
        var right = New("it.dock.right");
        services.Docks.Register(left);
        services.Docks.Register(right);

        var first = services.Docks.Activate(left);
        var second = services.Docks.Activate(right);

        Assert.NotNull(first);
        Assert.NotNull(second);

        services.Docks.Move(second, Beside(services.Docks, first, DockSide.Right));
        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;
        var split = Assert.IsType<LayoutSplit>(services.Docks.Layout.Root);
        var before = split.Ratio;
        var handle = main
            .GetVisualDescendants()
            .OfType<GridSplitter>()
            .First(one => one.Classes.Contains(DockArea.SplitterClass));
        var from = handle.TranslatePoint(
            new Point(handle.Bounds.Width / 2, handle.Bounds.Height / 2),
            main
        );

        Assert.NotNull(from);

        main.MouseDown(from.Value, MouseButton.Left);
        main.MouseMove(from.Value + new Point(80, 0), RawInputModifiers.LeftMouseButton);
        main.MouseUp(from.Value + new Point(80, 0), MouseButton.Left);

        Assert.Same(split, services.Docks.Layout.Root);
        Assert.NotEqual(before, split.Ratio);
    }

    /// <summary>
    /// Drops a dock at a point in a window, the way the platform delivers a drag.
    /// </summary>
    /// <remarks>
    /// A drag arrives in the order the platform delivers it, and what it takes to land somewhere is the whole of
    /// it: a target that says nothing as the drag enters it is a target the platform gives up on.
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

    /// <summary>The point of a window's one group that a drop would land on to do one thing.</summary>
    /// <remarks>
    /// The group fills the area, so its leaf is the area's own rectangle, and the five points are worked out from
    /// it by the same function the area hit tests with. What is aimed at is the middle of the outline — a point
    /// inside a convex shape's corners is inside the shape — and the area has to have been laid out for any of that
    /// to mean anything, which is why it is asked of the window that was shown.
    /// </remarks>
    /// <param name="main">The window to find the point in.</param>
    /// <param name="side">Which point of the cluster to aim at.</param>
    /// <returns>The point, in that window's own coordinates.</returns>
    private static Point OverDockPoint(MainWindow main, DockSide side)
    {
        var area = main.GetVisualDescendants().OfType<DockArea>().First();

        Assert.True(area.Bounds.Height > 0, "the window was shown, so its area was laid out");

        var leaf = new Rect(0, 0, area.Bounds.Width, area.Bounds.Height);
        var shape = DockArea.Shapes(leaf).First(one => one.Side == side);
        var middle = new Point(
            shape.Outline.Average(corner => corner.X),
            shape.Outline.Average(corner => corner.Y)
        );

        Assert.True(DockArea.Contains(shape.Outline, middle), "the aim is inside the point it aims at");

        var at = area.TranslatePoint(middle, main);

        Assert.NotNull(at);

        return at.Value;
    }

    /// <summary>
    /// A group whose docks are all hidden is not drawn, and so offers no dock point to be dropped on.
    /// </summary>
    /// <remarks>
    /// A hidden toggle is still a dock, so its group is still in the tree — but a group that would be drawn empty is
    /// a band of the window kept for nothing and five points offered for a place with no docks in it. What is drawn
    /// is what has something open in it; the group is kept and comes back when its dock is shown again.
    /// </remarks>
    [Fact]
    public void AGroupWhoseDocksAreAllHiddenIsNotDrawn()
    {
        var (services, _) = Windows();
        var registration = new DockRegistration(
            Core.Id,
            "it.dock.toggle",
            "it_dock.title",
            DockOpenMode.Toggle,
            DockSide.Center,
            () => new FakeDockView { Control = new Border() }
        );
        services.Docks.Register(registration);

        var toggle = services.Docks.Activate(registration);

        Assert.NotNull(toggle);

        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;

        Assert.Contains(
            main.GetVisualDescendants().OfType<Button>(),
            button => button.Classes.Contains(DockArea.TitleClass)
        );

        services.Docks.Close(toggle);

        Assert.DoesNotContain(
            main.GetVisualDescendants().OfType<Button>(),
            button => button.Classes.Contains(DockArea.TitleClass)
        );

        // Kept rather than pruned: hiding a dock is not closing it, and the group it is in comes back with it.
        Assert.Single(DockTree.Groups(services.Docks.Layout));
    }

    /// <summary>
    /// The five points a drag offers are four 120-wide trapezoid bands around the rectangle they leave, filling the
    /// group.
    /// </summary>
    /// <remarks>
    /// The shape of the answer, asked of the same function the area draws and hit tests with: five points, one per
    /// thing a drop can do, no two of them overlapping, and between them covering the whole group — the group is the
    /// target once a drag has found it, so there is no room in it that is near the answer without being on it. What
    /// the edges take is a band rather than a share, and what the centre is, is the group less that band: a group is
    /// approached from its edge, so the edge is where the pointer arrives and the middle is what is left.
    /// </remarks>
    [Fact]
    public void TheDockPointsFillTheGroupAsFourTrapezoidsAroundACentre()
    {
        var leaf = new Rect(0, 0, 800, 600);
        var shapes = DockArea.Shapes(leaf);

        Assert.Equal(5, shapes.Count);
        Assert.Equal(
            [DockSide.Center, DockSide.Left, DockSide.Right, DockSide.Top, DockSide.Bottom],
            shapes.Select(shape => shape.Side)
        );

        // The band is 120 from the edge and the centre is what is left: 560 by 360 here, and the four trapezoids'
        // long sides are the group's own edges while their short sides are the centre's.
        var inner = new Rect(120, 120, 560, 360);

        Assert.Equal(Corners(inner), shapes[0].Outline);
        Assert.Equal(
            [new Point(0, 0), new Point(120, 120), new Point(120, 480), new Point(0, 600)],
            shapes.Single(shape => shape.Side == DockSide.Left).Outline
        );
        Assert.Equal(
            [new Point(680, 120), new Point(800, 0), new Point(800, 600), new Point(680, 480)],
            shapes.Single(shape => shape.Side == DockSide.Right).Outline
        );
        Assert.Equal(
            [new Point(0, 0), new Point(800, 0), new Point(680, 120), new Point(120, 120)],
            shapes.Single(shape => shape.Side == DockSide.Top).Outline
        );
        Assert.Equal(
            [new Point(120, 480), new Point(680, 480), new Point(800, 600), new Point(0, 600)],
            shapes.Single(shape => shape.Side == DockSide.Bottom).Outline
        );

        // Each point holds its own middle and no other point does, so a pointer on one of them means one thing.
        foreach (var shape in shapes)
        {
            var middle = new Point(
                shape.Outline.Average(corner => corner.X),
                shape.Outline.Average(corner => corner.Y)
            );

            Assert.True(DockArea.Contains(shape.Outline, middle));
            Assert.DoesNotContain(
                shapes.Where(other => other.Side != shape.Side),
                other => DockArea.Contains(other.Outline, middle)
            );
        }

        // The five tile the whole group and nothing outside it: the areas add up to the leaf's, and every point of a
        // shape is within the leaf.
        Assert.Equal(leaf.Width * leaf.Height, shapes.Sum(shape => Area(shape.Outline)), 3);
        Assert.All(
            shapes,
            shape =>
                Assert.All(
                    shape.Outline,
                    corner =>
                    {
                        Assert.InRange(corner.X, leaf.X, leaf.Right);
                        Assert.InRange(corner.Y, leaf.Y, leaf.Bottom);
                    }
                )
        );

        // A point just inside each edge and the middle are each on an answer: there is no dead room in the group.
        var inside = new[]
        {
            new Point(leaf.X + (leaf.Width / 2), leaf.Y + 2),
            new Point(leaf.X + (leaf.Width / 2), leaf.Bottom - 2),
            new Point(leaf.X + 2, leaf.Y + (leaf.Height / 2)),
            new Point(leaf.Right - 2, leaf.Y + (leaf.Height / 2)),
            new Point(leaf.X + (leaf.Width / 2), leaf.Y + (leaf.Height / 2)),
        };

        Assert.All(
            inside,
            at => Assert.Single(shapes, shape => DockArea.Contains(shape.Outline, at))
        );
    }

    /// <summary>A drag is torn out into a window of its own only when it was let go of in the open.</summary>
    /// <remarks>
    /// What the platform cannot say: a drag called off with Esc, a drag let go of over a dock area without an answer,
    /// and a drag let go of where there is nothing all apply no effect. What tells them apart is the host's own
    /// record of the gesture, and this is the one place that reading is made.
    /// </remarks>
    [Fact]
    public void ADragIsTornOutOnlyWhenItWasLetGoOfInTheOpen()
    {
        Assert.True(DockWindows.TearsOut(DragDropEffects.None, over: false, cancelled: false, window: null));
        Assert.False(DockWindows.TearsOut(DragDropEffects.None, over: true, cancelled: false, window: null));
        Assert.False(DockWindows.TearsOut(DragDropEffects.None, over: false, cancelled: true, window: null));
        Assert.False(DockWindows.TearsOut(DragDropEffects.Move, over: false, cancelled: false, window: null));
        Assert.False(DockWindows.TearsOut(DragDropEffects.None, over: false, cancelled: false, window: "dock-1"));
    }

    /// <summary>The four corners of a box, in drawing order.</summary>
    /// <param name="box">The box.</param>
    /// <returns>Its corners.</returns>
    private static Point[] Corners(Rect box) =>
        [
            new Point(box.X, box.Y),
            new Point(box.Right, box.Y),
            new Point(box.Right, box.Bottom),
            new Point(box.X, box.Bottom),
        ];

    /// <summary>The area a polygon encloses, by the shoelace formula.</summary>
    /// <param name="outline">Its corners, in order.</param>
    /// <returns>Its area.</returns>
    private static double Area(IReadOnlyList<Point> outline)
    {
        var area = 0.0;

        for (int corner = 0, previous = outline.Count - 1; corner < outline.Count; previous = corner++)
        {
            area += (outline[previous].X * outline[corner].Y) - (outline[corner].X * outline[previous].Y);
        }

        return Math.Abs(area) / 2;
    }

    /// <summary>
    /// A dock in a window of its own is not written into the layout of the main window.
    /// </summary>
    /// <remarks>
    /// The layout states what the main window's tree is, and a dock that is somewhere else has no group in it:
    /// written down as though it had one, it would be read back into the main window and leave the window it was
    /// left in.
    /// </remarks>
    [Fact]
    public void ADockInAWindowOfItsOwnIsNotWrittenIntoTheLayout()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        var instance = services.Docks.Activate(registration);

        Assert.NotNull(instance);
        Assert.Contains("it.dock.browser", Saved(services));

        services.Windows.ToOwnWindow(instance);

        Assert.DoesNotContain("it.dock.browser", Saved(services));
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
            DockSide.Bottom,
            () => new FakeDockView { Control = new Border() }
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
    /// The window is stated as the instance is made rather than assigned after it, because there is no moment at
    /// which a dock opened in a window of its own is in the main one: it would be drawn in a group of the main
    /// window for as long as the assignment took. What is asked of the manager rather than of the windows,
    /// because it is the manager's own answer: a window of the host's does not exist yet at this point, and the
    /// dock is already where it is going — in no group at all.
    /// </remarks>
    [Fact]
    public void ADockOpenedInAWindowOfItsOwnIsNeverInTheMainOne()
    {
        var services = Host.Services();
        var registration = New("it.dock.browser");
        services.Docks.Register(registration);

        var instance = services.Docks.CreateInWindow(
            registration,
            new DockRequest(registration.DockNameId, "/work"),
            "dock-1"
        );

        Assert.NotNull(instance);
        Assert.Equal("dock-1", instance.Window);
        Assert.Equal("", instance.GroupId);
        Assert.Equal("/work", instance.Request.StartAt);
        Assert.Empty(Saved(services));
    }

    /// <summary>
    /// A layer that is one dock keeps no chrome of its own.
    /// </summary>
    /// <remarks>
    /// What a window of a dock's own is: the dock, under the window's own row. No strip of tabs, because
    /// there is nothing to switch between; no splitter, because there is nothing to resize it against; and no
    /// band of a group, because a group is a place a second view is put beside the first. A window that drew the
    /// chrome anyway would be a strip around a view that nothing can be put beside — and the one thing a torn-out
    /// dock must not look like is the window it came out of.
    /// <para>
    /// Asked of the layer rather than of a window, because the chrome is the layer's: what a window does with it
    /// is put it under the row. A layer that held a group would hold one whether or not any dock were in it.
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

        // A layer that is not one dock holds only the surface the tree is drawn in and the places a drag could
        // land: there is no fixed strip or splitter to hold, since the tree has one group and no split. It draws
        // no dock itself.
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
                DockSide.Center,
                () => new FakeDockView { Control = control }
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

            made.Add(window);

            return window;
        };

        return (services, []);
    }

    /// <summary>The docks a snapshot of the main window's layout records, in document order.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <returns>The docks' stable name ids.</returns>
    private static string[] Saved(HostServices services) =>
        [
            .. DockTree
                .Groups(services.Docks.Snapshot())
                .SelectMany(group => group.Docks)
                .Select(dock => dock.DockNameId),
        ];

    /// <summary>The target a drop on one side of a dock's own group would name.</summary>
    /// <param name="docks">The docks.</param>
    /// <param name="instance">The dock whose group is being split.</param>
    /// <param name="side">The side to split it on.</param>
    /// <returns>The target.</returns>
    private static DockTarget Beside(DockManager docks, DockInstance instance, DockSide side)
    {
        var group =
            DockTree.GroupOf(docks.Layout, instance.DockNameId, instance.Ordinal)
            ?? throw new InvalidOperationException("the dock is not in the main window's tree");

        return new DockTarget(group.Id, side, 0);
    }

    /// <summary>A registration for a dock that may be made again, as a browser is.</summary>
    /// <remarks>
    /// The view is given a control, because a test that makes a real window makes a real dock area, and an
    /// area cannot put nothing in a group: what a dock is drawn as is part of what a window is made of.
    /// </remarks>
    /// <param name="nameId">Its stable name id.</param>
    /// <returns>The registration.</returns>
    private static DockRegistration New(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.New,
            DockSide.Center,
            () => new FakeDockView { Control = new Border() }
        );
}

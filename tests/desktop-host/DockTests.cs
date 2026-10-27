using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The docks: the shape of the layout tree, where a dock goes, and what survives a restart.
/// </summary>
/// <remarks>
/// Nothing here draws. What is exercised is the bookkeeping the dock area and the <c>Window</c> menu are built
/// from, with views that have no control — and the layout tree itself, which is pure and is what all of that
/// bookkeeping writes.
/// </remarks>
public sealed class DockTests
{
    /// <summary>
    /// A layout made from scratch is the arrangement a new user is given, as a tree and as ratios.
    /// </summary>
    /// <remarks>
    /// The same docks the five fixed regions used to start with, in the same order: what is worked on in the
    /// middle and the tree down the right, expressed as one split. The shares are what the old pixel sizes were
    /// on the window the program opens at, because a ratio is the only geometry a layout stores now.
    /// </remarks>
    [Fact]
    public void AFreshLayoutIsTheArrangementANewUserIsGiven()
    {
        var layout = DockLayout.Starting();
        var split = Assert.IsType<LayoutSplit>(layout.Root);
        var first = Assert.IsType<LayoutGroup>(split.First);
        var second = Assert.IsType<LayoutGroup>(split.Second);

        Assert.Equal(DockAxis.Columns, split.Axis);
        Assert.Equal(DockLayout.StartingRatio, split.Ratio);
        Assert.Equal("g-1", first.Id);
        Assert.Equal(
            ["rorolala.core.plugin_manager", "rorolala.core.preference", "rorolala.file_system.browser"],
            first.Docks.Select(dock => dock.DockNameId)
        );
        Assert.Equal("g-2", second.Id);
        Assert.Equal(["rorolala.file_system.tree"], second.Docks.Select(dock => dock.DockNameId));
        Assert.Equal("g-1", layout.ActiveGroup);
        Assert.All(first.Docks, dock => Assert.Equal(0, dock.Ordinal));
        Assert.Equal(0, first.Active);
    }

    /// <summary>A group holds at least one dock, so the starting arrangement has no empty place in it.</summary>
    [Fact]
    public void AStartingLayoutHasNoEmptyGroup()
    {
        Assert.All(DockTree.Groups(DockLayout.Starting()), group => Assert.NotEmpty(group.Docks));
    }

    /// <summary>A dock dropped on the middle of a group joins its tabs at the end.</summary>
    [Fact]
    public void InsertingIntoACenterAppendsToTheTabOrder()
    {
        var layout = DockLayout.Starting();
        var group = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-1"));

        var at = DockTree.Insert(
            layout,
            new DockTarget(group.Id, DockSide.Center, group.Docks.Count),
            Dock("it.dock.new")
        );

        Assert.Equal(group.Id, at);
        Assert.Equal("it.dock.new", group.Docks[^1].DockNameId);
        Assert.Equal(group.Docks.Count - 1, group.Active);
    }

    /// <summary>A dock dropped between two tabs goes between them rather than at the end.</summary>
    [Fact]
    public void InsertingAtAnIndexMovesTheTab()
    {
        var layout = DockLayout.Starting();
        var group = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-1"));

        DockTree.Insert(layout, new DockTarget(group.Id, DockSide.Center, 1), Dock("it.dock.new"));

        Assert.Equal("it.dock.new", group.Docks[1].DockNameId);
        Assert.Equal(1, group.Active);
    }

    /// <summary>
    /// A dock dropped on an edge of a group gets a group of its own beside it, on that side.
    /// </summary>
    /// <remarks>
    /// The left of the first group of a row of two is the case that has a chain to flatten: the new split would
    /// be a column split inside a column split, so the two are one chain nested to the right, and the shares
    /// are recomputed so the groups end up exactly where the drop showed they would.
    /// </remarks>
    [Fact]
    public void InsertingOnAnEdgeSplitsTheGroupOnThatSide()
    {
        var layout = DockLayout.Starting();
        var group = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-1"));
        var right = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-2"));

        var made = DockTree.Insert(layout, new DockTarget(group.Id, DockSide.Left, 0), Dock("it.dock.split"));

        // The chain is one split of a new group against the two there were: the new group on the left, and the
        // group that was dropped on keeping its own share of the room it had.
        var root = Assert.IsType<LayoutSplit>(layout.Root);
        var added = Assert.IsType<LayoutGroup>(root.First);
        var rest = Assert.IsType<LayoutSplit>(root.Second);

        Assert.Equal(DockAxis.Columns, root.Axis);
        Assert.Equal(made, added.Id);
        Assert.Equal("it.dock.split", Assert.Single(added.Docks).DockNameId);
        Assert.Same(group, rest.First);
        Assert.Same(right, rest.Second);
        Assert.Equal(0.5 * DockLayout.StartingRatio, root.Ratio, 6);
        Assert.Equal(DockLayout.StartingRatio, root.Ratio + ((1 - root.Ratio) * rest.Ratio), 6);
    }

    /// <summary>A dock dropped on the right of the second group makes a split on the other side.</summary>
    [Fact]
    public void InsertingOnTheOtherSidePutsTheNewGroupSecond()
    {
        var layout = DockLayout.Starting();
        var right = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-2"));

        var made = DockTree.Insert(layout, new DockTarget(right.Id, DockSide.Right, 0), Dock("it.dock.split"));

        var root = Assert.IsType<LayoutSplit>(layout.Root);
        var rest = Assert.IsType<LayoutSplit>(root.Second);

        Assert.Equal(DockAxis.Columns, root.Axis);
        Assert.Same(right, rest.First);
        Assert.Equal(made, Assert.IsType<LayoutGroup>(rest.Second).Id);
        Assert.Equal(0.5, rest.Ratio);
    }

    /// <summary>A row drop above a group makes a row split, with the new group on top.</summary>
    [Fact]
    public void InsertingAboveAGroupSplitsItInRows()
    {
        var layout = DockLayout.Starting();
        var group = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-2"));

        var made = DockTree.Insert(layout, new DockTarget(group.Id, DockSide.Top, 0), Dock("it.dock.split"));

        var root = Assert.IsType<LayoutSplit>(layout.Root);
        var rest = Assert.IsType<LayoutSplit>(root.Second);

        Assert.Equal(DockAxis.Rows, rest.Axis);
        Assert.Equal(made, Assert.IsType<LayoutGroup>(rest.First).Id);
        Assert.Same(group, rest.Second);
    }

    /// <summary>Taking the last dock out of a group takes the group and collapses its split.</summary>
    [Fact]
    public void RemovingTheLastDockCollapsesTheSplit()
    {
        var layout = TwoGroups();

        Assert.True(DockTree.Remove(layout, "it.dock.a", 0));

        var group = Assert.IsType<LayoutGroup>(layout.Root);
        Assert.Equal("g-2", group.Id);
        Assert.Equal("it.dock.b", Assert.Single(group.Docks).DockNameId);
        Assert.Equal("g-2", layout.ActiveGroup);
    }

    /// <summary>Taking the last dock out of the only group leaves no arrangement at all.</summary>
    [Fact]
    public void RemovingARootGroupLeavesAnEmptyLayout()
    {
        var layout = new DockLayout { ActiveGroup = "g-1", Root = Group("g-1", "it.dock.a") };

        Assert.True(DockTree.Remove(layout, "it.dock.a", 0));

        Assert.Null(layout.Root);
        Assert.Null(layout.ActiveGroup);
    }

    /// <summary>Taking out a dock the tree does not hold changes nothing.</summary>
    [Fact]
    public void RemovingADockThatIsNotThereChangesNothing()
    {
        var layout = TwoGroups();

        Assert.False(DockTree.Remove(layout, "it.dock.gone", 0));
        Assert.Equal(["g-1", "g-2"], DockTree.Groups(layout).Select(group => group.Id));
    }

    /// <summary>
    /// A chain of splits on one axis is nested to the right, and keeps the geometry it had.
    /// </summary>
    /// <remarks>
    /// A hand-edited file, or a drop, can leave a column split inside a column split. Two splits of one axis
    /// cannot be one split — a split has two children — so what normalization does is give the chain one shape:
    /// the seam between the first two moves into the right-hand split, and the shares are recomputed so the
    /// groups are drawn where they were.
    /// </remarks>
    [Fact]
    public void SameAxisChainsAreFlattened()
    {
        var left = Group("g-1", "it.dock.a");
        var middle = Group("g-2", "it.dock.b");
        var right = Group("g-3", "it.dock.c");
        var layout = new DockLayout
        {
            ActiveGroup = "g-1",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Columns,
                Ratio = 0.5,
                First = new LayoutSplit
                {
                    Axis = DockAxis.Columns,
                    Ratio = 0.5,
                    First = left,
                    Second = middle,
                },
                Second = right,
            },
        };

        DockTree.Normalize(layout);

        var root = Assert.IsType<LayoutSplit>(layout.Root);
        var rest = Assert.IsType<LayoutSplit>(root.Second);

        Assert.Equal(DockAxis.Columns, root.Axis);
        Assert.Same(left, root.First);
        Assert.Same(middle, rest.First);
        Assert.Same(right, rest.Second);
        Assert.Equal(0.25, root.Ratio, 6);
        Assert.Equal(1.0 / 3, rest.Ratio, 6);
    }

    /// <summary>A group that has been emptied by hand-editing is collapsed rather than kept.</summary>
    [Fact]
    public void AnEmptyGroupIsCollapsed()
    {
        var layout = new DockLayout
        {
            ActiveGroup = "g-1",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Rows,
                Ratio = 0.5,
                First = new LayoutGroup { Id = "g-1" },
                Second = Group("g-2", "it.dock.b"),
            },
        };

        DockTree.Normalize(layout);

        Assert.Equal("g-2", Assert.IsType<LayoutGroup>(layout.Root).Id);
    }

    /// <summary>A dock written down twice is kept once, and the first place is the one it keeps.</summary>
    [Fact]
    public void ADockWrittenTwiceIsKeptOnce()
    {
        var layout = new DockLayout
        {
            ActiveGroup = "g-1",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Columns,
                Ratio = 0.5,
                First = Group("g-1", "it.dock.a"),
                Second = Group("g-2", "it.dock.a"),
            },
        };

        DockTree.Normalize(layout);

        var group = Assert.IsType<LayoutGroup>(layout.Root);
        Assert.Equal("it.dock.a", Assert.Single(group.Docks).DockNameId);
    }

    /// <summary>A ratio written as exactly nothing or everything is brought in from the ends.</summary>
    [Fact]
    public void ARatioAtTheEndsIsBroughtIn()
    {
        var layout = new DockLayout
        {
            ActiveGroup = "g-1",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Columns,
                Ratio = 0,
                First = Group("g-1", "it.dock.a"),
                Second = Group("g-2", "it.dock.b"),
            },
        };

        DockTree.Normalize(layout);

        Assert.Equal(DockTree.MinShare, Assert.IsType<LayoutSplit>(layout.Root).Ratio);
    }

    /// <summary>A group id that is not one, or is one twice, is replaced by one that is.</summary>
    [Fact]
    public void AGroupIdThatIsNotOneIsReplaced()
    {
        var layout = new DockLayout
        {
            ActiveGroup = "left",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Columns,
                Ratio = 0.5,
                First = Group("left", "it.dock.a"),
                Second = Group("right", "it.dock.b"),
            },
        };

        DockTree.Normalize(layout);

        var ids = DockTree.Groups(layout).Select(group => group.Id).ToArray();

        Assert.All(ids, id => Assert.True(DockTree.IsGroupId(id), $"`{id}` is not a group id"));
        Assert.Equal(ids.Length, ids.Distinct(StringComparer.Ordinal).Count());
        Assert.Null(layout.ActiveGroup);
    }

    /// <summary>
    /// The next group id is one past the largest number the tree says, and is not then reused.
    /// </summary>
    /// <remarks>
    /// The largest id can name a group that has just been pruned while a dock in a window of its own still
    /// remembers it, so the run keeps its own mark of what it has handed out: a remembered id has to mean one
    /// group and not two.
    /// </remarks>
    [Fact]
    public void TheNextGroupIdIsOnePastTheLargestAndIsNotReused()
    {
        var layout = DockLayout.Starting();
        var group = Assert.IsType<LayoutGroup>(DockTree.Find(layout, "g-1"));

        var made = DockTree.Insert(layout, new DockTarget(group.Id, DockSide.Bottom, 0), Dock("it.dock.x"));

        Assert.Equal("g-3", made);

        Assert.True(DockTree.Remove(layout, "it.dock.x", 0));

        Assert.Equal("g-4", DockTree.NextGroupId(layout));
    }

    /// <summary>The least sizes of a split are its children's, plus the room its divider takes.</summary>
    [Fact]
    public void MinimumsPropagateThroughNestedSplits()
    {
        var first = Group("g-1", "it.dock.a");
        var second = Group("g-2", "it.dock.b");

        Assert.Equal(DockTree.MinColumn, DockTree.MinimumWidth(first));
        Assert.Equal(DockTree.MinRow, DockTree.MinimumHeight(first));

        var columns = new LayoutSplit
        {
            Axis = DockAxis.Columns,
            Ratio = 0.5,
            First = first,
            Second = second,
        };

        Assert.Equal((2 * DockTree.MinColumn) + DockTree.SplitterSize, DockTree.MinimumWidth(columns));
        Assert.Equal(DockTree.MinRow, DockTree.MinimumHeight(columns));

        var rows = new LayoutSplit
        {
            Axis = DockAxis.Rows,
            Ratio = 0.5,
            First = first,
            Second = second,
        };

        Assert.Equal((2 * DockTree.MinRow) + DockTree.SplitterSize, DockTree.MinimumHeight(rows));
        Assert.Equal(DockTree.MinColumn, DockTree.MinimumWidth(rows));

        var nested = new LayoutSplit
        {
            Axis = DockAxis.Columns,
            Ratio = 0.5,
            First = columns,
            Second = first,
        };

        Assert.Equal(
            DockTree.MinimumWidth(columns) + DockTree.MinColumn + DockTree.SplitterSize,
            DockTree.MinimumWidth(nested)
        );
    }

    /// <summary>A toggle dock has one instance, which is shown or hidden.</summary>
    [Fact]
    public void AToggleDockHasOneInstanceThatIsShownOrHidden()
    {
        var (manager, _, _) = Make();
        var registration = Toggle("it.dock.toggle");
        manager.Register(registration);

        var first = manager.Activate(registration);
        Assert.NotNull(first);
        Assert.True(first.IsOpen);

        var second = manager.Activate(registration);
        Assert.Same(first, second);
        Assert.False(second!.IsOpen);

        Assert.Single(manager.Instances);
    }

    /// <summary>A created dock makes an instance every time.</summary>
    [Fact]
    public void ACreatedDockMakesAnInstanceEachTime()
    {
        var (manager, _, _) = Make();
        var registration = Created("it.dock.new");
        manager.Register(registration);

        manager.Activate(registration);
        manager.Activate(registration);

        Assert.Equal(2, manager.Instances.Count);
        Assert.Equal(new[] { 0, 1 }, manager.Instances.Select(instance => instance.Ordinal).ToArray());
    }

    /// <summary>Closing a toggle hides it and keeps it, so showing it again is the same instance.</summary>
    [Fact]
    public void ClosingAToggleHidesItAndKeepsIt()
    {
        var (manager, _, _) = Make();
        var registration = Toggle("it.dock.toggle");
        manager.Register(registration);

        var instance = manager.Activate(registration)!;
        manager.Close(instance);

        Assert.False(instance.IsOpen);
        Assert.Single(manager.Instances);
    }

    /// <summary>Closing a created dock discards it, and its place in the tree goes with it.</summary>
    [Fact]
    public void ClosingACreatedDockDiscardsItAndItsPlace()
    {
        var (manager, _, _) = Make();
        var registration = Created("it.dock.new");
        manager.Register(registration);

        var instance = manager.Activate(registration)!;
        manager.Close(instance);

        Assert.Empty(manager.Instances);
        Assert.Null(manager.Layout.Root);
    }

    /// <summary>
    /// The only dock of a group has no second place in that group, so dropping it on its own edge does nothing.
    /// </summary>
    [Fact]
    public void MovingTheOnlyDockOfAGroupOntoItsOwnEdgeDoesNothing()
    {
        var (manager, _, _) = Make();
        var registration = Created("it.dock.one");
        manager.Register(registration);

        var instance = manager.Activate(registration)!;
        var group = Assert.Single(DockTree.Groups(manager.Layout));

        manager.Move(instance, new DockTarget(group.Id, DockSide.Bottom, 0));

        Assert.Same(group, Assert.Single(DockTree.Groups(manager.Layout)));
        Assert.Equal(group.Id, instance.GroupId);
    }

    /// <summary>A dock dropped among its own group's tabs is reordered rather than split off.</summary>
    [Fact]
    public void ADockDroppedWithinItsGroupIsReordered()
    {
        var (manager, _, _) = Make();
        var one = Created("it.dock.one");
        var two = Created("it.dock.two");
        manager.Register(one);
        manager.Register(two);

        var first = manager.Activate(one)!;
        var second = manager.Activate(two)!;
        var group = Assert.Single(DockTree.Groups(manager.Layout));

        Assert.Equal([first.DockNameId, second.DockNameId], group.Docks.Select(dock => dock.DockNameId));

        manager.Move(second, new DockTarget(group.Id, DockSide.Center, 0));

        Assert.Equal([second.DockNameId, first.DockNameId], group.Docks.Select(dock => dock.DockNameId));
        Assert.Equal(group.Id, second.GroupId);
    }

    /// <summary>A dock opened without a place goes into the active group, at the side it asks for.</summary>
    [Fact]
    public void ANewDockGoesIntoTheActiveGroup()
    {
        var (manager, _, _) = Make();
        var one = Created("it.dock.one");
        var two = new DockRegistration(
            Core.Id,
            "it.dock.two",
            "it_dock.title",
            DockOpenMode.New,
            DockSide.Bottom,
            () => new FakeDockView()
        );
        manager.Register(one);
        manager.Register(two);

        var first = manager.Activate(one)!;
        manager.Activate(two);

        var group = Assert.IsType<LayoutGroup>(DockTree.Find(manager.Layout, first.GroupId));
        var split = Assert.IsType<LayoutSplit>(manager.Layout.Root);

        // The bottom side split the group in two rows, and the new dock is the one below.
        Assert.Equal(DockAxis.Rows, split.Axis);
        Assert.Same(group, split.First);
    }

    /// <summary>The layout records what is open, and a fresh manager opens it again in the same shape.</summary>
    [Fact]
    public void TheLayoutRecordsWhatIsOpenAndRestoresIt()
    {
        var (manager, _, _) = Make();
        var toggle = Toggle("it.dock.toggle");
        var created = Created("it.dock.new");
        manager.Register(toggle);
        manager.Register(created);
        manager.Activate(toggle);
        manager.Activate(created);
        manager.Activate(created);

        var saved = manager.Snapshot();

        var (restored, _, _) = Make();
        restored.Register(toggle);
        restored.Register(created);
        restored.Restore(saved);

        Assert.Equal(Names(saved), Names(restored.Snapshot()));
    }

    /// <summary>A hidden toggle is not written to the layout.</summary>
    [Fact]
    public void AHiddenToggleIsNotWrittenToTheLayout()
    {
        var (manager, _, _) = Make();
        var registration = Toggle("it.dock.toggle");
        manager.Register(registration);
        manager.Close(manager.Activate(registration)!);

        Assert.Empty(Names(manager.Snapshot()));
    }

    /// <summary>A saved name that no dock registers is dropped, and the rest is kept.</summary>
    [Fact]
    public void ASavedNameThatNoDockRegistersIsDropped()
    {
        var (manager, _, _) = Make();
        manager.Register(Created("it.dock.new"));

        var layout = new DockLayout
        {
            ActiveGroup = "g-1",
            Root = Group("g-1", "it.dock.gone", "it.dock.new"),
        };

        manager.Restore(layout);

        var group = Assert.IsType<LayoutGroup>(manager.Layout.Root);
        Assert.Equal(["it.dock.new"], group.Docks.Select(dock => dock.DockNameId));
        Assert.Single(manager.Instances);
    }

    /// <summary>Tab order and the chosen tab are part of the arrangement that comes back.</summary>
    [Fact]
    public void TabOrderAndActiveTabSurviveASaveAndRestore()
    {
        var (manager, _, _) = Make();
        var one = Toggle("it.dock.one");
        var two = Toggle("it.dock.two");
        var three = Toggle("it.dock.three");
        manager.Register(one);
        manager.Register(two);
        manager.Register(three);

        var first = manager.Activate(one)!;
        manager.Activate(two);
        manager.Activate(three);

        var group = Assert.Single(DockTree.Groups(manager.Layout));
        manager.SelectTab(group.Id, 0);

        var saved = manager.Snapshot();

        var (restored, _, _) = Make();
        restored.Register(one);
        restored.Register(two);
        restored.Register(three);
        restored.Restore(saved);

        var back = Assert.Single(DockTree.Groups(restored.Layout));

        Assert.Equal(["it.dock.one", "it.dock.two", "it.dock.three"], back.Docks.Select(dock => dock.DockNameId));
        Assert.Equal(0, back.Active);
        Assert.True(restored.InstanceOf("it.dock.one")!.IsOpen);
        Assert.Equal(first.GroupId, back.Id);
    }

    /// <summary>The group last worked in comes back as the active one.</summary>
    [Fact]
    public void ActiveGroupSurvivesASaveAndRestore()
    {
        var (manager, _, _) = Make();
        var one = Toggle("it.dock.one");
        manager.Register(one);

        var instance = manager.Activate(one)!;

        // A second group, made by splitting the first, and then chosen.
        var registration = Created("it.dock.two");
        manager.Register(registration);
        var other = manager.Activate(registration)!;
        manager.Move(
            other,
            new DockTarget(instance.GroupId, DockSide.Right, 0)
        );
        manager.ActivateGroup(other.GroupId);

        var saved = manager.Snapshot();

        var (restored, _, _) = Make();
        restored.Register(one);
        restored.Register(registration);
        restored.Restore(saved);

        Assert.Equal(other.GroupId, restored.Layout.ActiveGroup);
    }

    /// <summary>
    /// A ratio written down is read back as what was written, rounded where it is written.
    /// </summary>
    [Fact]
    public void RatiosSurviveASaveAndRestore()
    {
        DataHome.Clean();
        var log = new LogService(new NotificationService());
        var layout = DockLayout.Starting();

        Assert.IsType<LayoutSplit>(layout.Root).Ratio = 1.0 / 3;

        LayoutStore.Save(log, layout);

        var read = LayoutStore.Load(log);

        Assert.Equal(1.0 / 3, Assert.IsType<LayoutSplit>(read.Root).Ratio, 6);
        Assert.Equal(DockLayout.SchemaVersion, read.Version);
        Assert.Contains(
            "\"_version\": 2",
            File.ReadAllText(ConfigPaths.Layout),
            StringComparison.Ordinal
        );
    }

    /// <summary>A layout written by an earlier build is not read, and the starting arrangement is used.</summary>
    [Fact]
    public void AVersionOneLayoutIsDiscardedAndTheStartingLayoutIsUsed()
    {
        DataHome.Clean();
        Given(ConfigPaths.Layout, """{"_version": 1, "leftWidth": 258, "docks": []}""");
        var log = new LogService(new NotificationService());

        var layout = LayoutStore.Load(log);

        Assert.Equal(
            DockLayout.StartingRatio,
            Assert.IsType<LayoutSplit>(layout.Root).Ratio
        );
        Assert.Contains(log.Entries, entry => entry.Level == LogLevel.Warn);

        // Left where it was: what replaces it is the next save, not this read.
        Assert.Contains("\"_version\": 1", File.ReadAllText(ConfigPaths.Layout), StringComparison.Ordinal);
    }

    /// <summary>A layout of a version this program does not know starts over rather than being guessed at.</summary>
    [Fact]
    public void AnUnknownVersionStartsOver()
    {
        DataHome.Clean();
        Given(ConfigPaths.Layout, """{"_version": 99, "root": null}""");
        var log = new LogService(new NotificationService());

        var layout = LayoutStore.Load(log);

        Assert.NotNull(layout.Root);
        Assert.Contains(log.Entries, entry => entry.Level == LogLevel.Warn);
    }

    /// <summary>A dock dropped into a main window holding none makes the root group, whichever side it asked for.</summary>
    [Fact]
    public void InsertingIntoAnEmptyLayoutMakesTheRootGroup()
    {
        var layout = new DockLayout();

        var at = DockTree.Insert(layout, new DockTarget("", DockSide.Left, 0), Dock("it.dock.only"));

        var group = Assert.IsType<LayoutGroup>(layout.Root);
        Assert.Equal(at, group.Id);
        Assert.Equal("it.dock.only", Assert.Single(group.Docks).DockNameId);
        Assert.Equal(group.Id, layout.ActiveGroup);
    }

    /// <summary>A hand-edited file with an empty group, an invalid active tab and an invalid active group is repaired.</summary>
    [Fact]
    public void AHandEditedFileWithAnEmptyGroupIsNormalized()
    {
        DataHome.Clean();
        Given(
            ConfigPaths.Layout,
            """
            {
              "_version": 2,
              "activeGroup": "g-9",
              "root": {
                "kind": "split",
                "axis": "columns",
                "ratio": 0.5,
                "first": { "kind": "group", "id": "g-1", "active": 0, "docks": [] },
                "second": {
                  "kind": "group",
                  "id": "g-2",
                  "active": 5,
                  "docks": [{ "nameId": "it.dock.a", "ordinal": 0, "meta": {} }]
                }
              }
            }
            """
        );
        var log = new LogService(new NotificationService());

        var layout = LayoutStore.Load(log);

        var group = Assert.IsType<LayoutGroup>(layout.Root);
        Assert.Equal("g-2", group.Id);
        Assert.Equal(0, group.Active);
        Assert.Null(layout.ActiveGroup);
    }

    /// <summary>A layout that is not the shape this program writes is started over rather than half-read.</summary>
    [Fact]
    public void ALayoutThatWillNotReadStartsOver()
    {
        DataHome.Clean();
        Given(ConfigPaths.Layout, "{ not json");
        var log = new LogService(new NotificationService());

        var layout = LayoutStore.Load(log);

        Assert.NotNull(layout.Root);
        Assert.Contains(log.Entries, entry => entry.Level == LogLevel.Warn);
    }

    /// <summary>A node of a kind this program does not draw is a file that will not read, not a half-drawn tree.</summary>
    [Fact]
    public void ALayoutWithAnUnknownNodeKindStartsOver()
    {
        DataHome.Clean();
        Given(
            ConfigPaths.Layout,
            """
            {
              "_version": 2,
              "activeGroup": "g-1",
              "root": { "kind": "float", "id": "g-1", "active": 0, "docks": [] }
            }
            """
        );
        var log = new LogService(new NotificationService());

        var layout = LayoutStore.Load(log);

        Assert.NotNull(layout.Root);
        Assert.Contains(log.Entries, entry => entry.Level == LogLevel.Warn);
    }

    /// <summary>Registering one name id twice is refused, so two docks cannot share a name.</summary>
    [Fact]
    public void RegisteringOneNameIdTwiceIsRefused()
    {
        var (manager, _, _) = Make();
        manager.Register(Toggle("it.dock.toggle"));

        Assert.Throws<InvalidOperationException>(() => manager.Register(Toggle("it.dock.toggle")));
    }

    /// <summary>A dock whose view cannot be made does not open, and is reported.</summary>
    [Fact]
    public void ADockWhoseViewCannotBeMadeDoesNotOpen()
    {
        var (manager, log, popups) = Make();
        var registration = new DockRegistration(
            Core.Id,
            "it.dock.broken",
            "it_dock_broken",
            DockOpenMode.Toggle,
            DockSide.Center,
            () => throw new InvalidOperationException("no view for you")
        );
        manager.Register(registration);

        var instance = manager.Activate(registration);

        Assert.Null(instance);
        Assert.Empty(manager.Instances);
        Assert.Null(manager.Layout.Root);
        Assert.Contains(log.Entries, entry => entry.Level == LogLevel.Error);
        Assert.Single(popups.Take());
    }

    /// <summary>Makes a dock manager over scratch state.</summary>
    /// <returns>The manager, and what it reports through.</returns>
    private static (DockManager Manager, LogService Log, PopupService Popups) Make()
    {
        // A directory of nothing is enough: a key nothing states reads as the key itself.
        RolaI18N.SetTranslationDirectory(Scratch.New("i18n"));

        var notifications = new NotificationService();
        var log = new LogService(notifications);
        var popups = new PopupService(notifications);
        var manager = new DockManager(new DockRegistry(), new I18nService(), log, popups);

        return (manager, log, popups);
    }

    /// <summary>A registration for a toggle dock.</summary>
    /// <param name="nameId">Its stable name id.</param>
    /// <returns>The registration.</returns>
    private static DockRegistration Toggle(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.Toggle,
            DockSide.Center,
            () => new FakeDockView()
        );

    /// <summary>A registration for a created dock.</summary>
    /// <param name="nameId">Its stable name id.</param>
    /// <returns>The registration.</returns>
    private static DockRegistration Created(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.New,
            DockSide.Center,
            () => new FakeDockView()
        );

    /// <summary>A recorded dock.</summary>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <returns>The record.</returns>
    private static LayoutDock Dock(string nameId) => new() { DockNameId = nameId };

    /// <summary>A group holding one-dock-per-name records.</summary>
    /// <param name="id">The group's id.</param>
    /// <param name="dockNameIds">The docks it holds, in tab order.</param>
    /// <returns>The group.</returns>
    private static LayoutGroup Group(string id, params string[] dockNameIds)
    {
        var group = new LayoutGroup { Id = id };

        foreach (var nameId in dockNameIds)
        {
            group.Docks.Add(Dock(nameId));
        }

        return group;
    }

    /// <summary>Two groups in one column split.</summary>
    /// <returns>The layout.</returns>
    private static DockLayout TwoGroups() =>
        new()
        {
            ActiveGroup = "g-1",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Columns,
                Ratio = 0.5,
                First = Group("g-1", "it.dock.a"),
                Second = Group("g-2", "it.dock.b"),
            },
        };

    /// <summary>Every dock a layout records, in document order.</summary>
    /// <param name="layout">The layout.</param>
    /// <returns>The docks' stable name ids.</returns>
    private static string[] Names(DockLayout layout) =>
        [
            .. DockTree
                .Groups(layout)
                .SelectMany(group => group.Docks)
                .Select(dock => dock.DockNameId),
        ];

    /// <summary>Writes a file the host will read, creating the directory it goes in.</summary>
    /// <param name="path">Where to write.</param>
    /// <param name="content">What to write.</param>
    private static void Given(string path, string content)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, content);
    }
}

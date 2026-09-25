using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The docks: how many instances a dock has, where they are, and what survives a restart.
/// </summary>
/// <remarks>
/// Nothing here draws. What is exercised is the bookkeeping the dock area and the <c>Window</c> menu
/// are built from, with views that have no control.
/// </remarks>
public sealed class DockTests
{
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

    /// <summary>A created dock makes an instance every time it is activated.</summary>
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

    /// <summary>Closing a created dock discards it.</summary>
    [Fact]
    public void ClosingACreatedDockDiscardsIt()
    {
        var (manager, _, _) = Make();
        var registration = Created("it.dock.new");
        manager.Register(registration);

        var instance = manager.Activate(registration)!;
        manager.Close(instance);

        Assert.Empty(manager.Instances);
    }

    /// <summary>The layout records what is open, and a fresh manager opens it again.</summary>
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

        Assert.Equal(
            manager.Instances.Select(instance => (instance.DockNameId, instance.Placement)).ToArray(),
            restored.Instances
                .Select(instance => (instance.DockNameId, instance.Placement))
                .ToArray()
        );
    }

    /// <summary>A hidden toggle is not written to the layout.</summary>
    [Fact]
    public void AHiddenToggleIsNotWrittenToTheLayout()
    {
        var (manager, _, _) = Make();
        var registration = Toggle("it.dock.toggle");
        manager.Register(registration);
        manager.Close(manager.Activate(registration)!);

        Assert.Empty(manager.Snapshot().Docks);
    }

    /// <summary>A saved name that no dock registers is dropped, and the rest is kept.</summary>
    [Fact]
    public void ASavedNameThatNoDockRegistersIsDropped()
    {
        var (manager, _, _) = Make();
        manager.Register(Created("it.dock.new"));

        var layout = new DockLayout();
        layout.Docks.Add(
            new LayoutDock
            {
                DockNameId = "it.dock.gone",
                Ordinal = 0,
                Placement = DockPlacement.Center,
            }
        );
        layout.Docks.Add(
            new LayoutDock
            {
                DockNameId = "it.dock.new",
                Ordinal = 0,
                Placement = DockPlacement.Left,
            }
        );

        manager.Restore(layout);

        var restored = Assert.Single(manager.Instances);
        Assert.Equal("it.dock.new", restored.DockNameId);
        Assert.Equal(DockPlacement.Left, restored.Placement);
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
            DockPlacement.Center,
            _ => throw new InvalidOperationException("no view for you")
        );
        manager.Register(registration);

        var instance = manager.Activate(registration);

        Assert.Null(instance);
        Assert.Empty(manager.Instances);
        Assert.Contains(log.Entries, entry => entry.Level == LogLevel.Error);
        Assert.Single(popups.Take());
    }

    /// <summary>Makes a dock manager over scratch state.</summary>
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
    private static DockRegistration Toggle(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.Toggle,
            DockPlacement.Center,
            _ => new FakeDockView()
        );

    /// <summary>A registration for a created dock.</summary>
    private static DockRegistration Created(string nameId) =>
        new(
            Core.Id,
            nameId,
            "it_dock.title",
            DockOpenMode.New,
            DockPlacement.Left,
            _ => new FakeDockView()
        );
}

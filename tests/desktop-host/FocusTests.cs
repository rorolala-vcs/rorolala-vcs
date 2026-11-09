using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Which dock the program is working in.
/// </summary>
/// <remarks>
/// The record is the host's: what is checked here happens in real windows on the headless platform, because
/// a press in a dock's view is what moves it. The dock's tab is the only mark that says which one it is.
/// </remarks>
public sealed class FocusTests
{
    /// <summary>A press in a dock makes it the one being worked in.</summary>
    /// <remarks>
    /// Two docks split side by side, so that the record has somewhere to move to: what a reader has to be able to
    /// tell is which of the two the work will go to.
    /// </remarks>
    [Fact]
    public void ADockAPressLandsInIsTheOneWorkedIn()
    {
        var (services, plugin) = Hosts();
        var left = new Border { Background = Brushes.Gray };
        var right = new Border { Background = Brushes.Gray };
        var first = Activate(services, "it.focus.left", "it_dock.left", left);
        var second = Activate(services, "it.focus.right", "it_dock.right", right);

        Beside(services, second, first, DockSide.Right);

        var main = Open(services);

        Pump();

        Click(main, left);

        Assert.Equal("it.focus.left", services.Docks.Focus.Current?.DockNameId);
        Assert.Equal(
            new FocusedDock(first.DockNameId, first.Ordinal),
            plugin.Focus.Current
        );

        Click(main, right);

        Assert.Equal("it.focus.right", services.Docks.Focus.Current?.DockNameId);

        // The last dock of each kind is kept apart from where the user is now: a plugin acting from one dock
        // needs the one the user left behind, which is what the Overview asks for.
        Assert.Same(first, services.Docks.Recent(first.DockNameId));
        Assert.Same(second, services.Docks.Recent(second.DockNameId));
    }

    /// <summary>Choosing a dock's tab works in it, and the chosen tab is the one marked.</summary>
    [Fact]
    public void ChoosingATabIsWorkingInThatDock()
    {
        var (services, _) = Hosts();
        var left = new Border { Background = Brushes.Gray };
        var right = new Border { Background = Brushes.Gray };
        var first = Activate(services, "it.focus.left", "it_dock.left", left);

        Activate(services, "it.focus.right", "it_dock.right", right);

        var main = Open(services);

        Pump();

        // Both docks are tabs of one group, so the one the group settles on is the one drawn; the other is a
        // header and nothing else, and clicking it is what chooses it.
        var header = main
            .GetVisualDescendants()
            .OfType<Button>()
            .First(button =>
                button.Classes.Contains(DockArea.TitleClass)
                && button.GetVisualDescendants().OfType<TextBlock>().Any(text => text.Text == "it_dock.left")
            );

        var at = header.TranslatePoint(
            new Point(header.Bounds.Width / 2, header.Bounds.Height / 2),
            main
        );

        Assert.NotNull(at);

        main.MouseDown(at.Value, MouseButton.Left);
        main.MouseUp(at.Value, MouseButton.Left);
        Pump();

        Assert.Equal(first.DockNameId, services.Docks.Focus.Current?.DockNameId);
        Assert.Contains(DockArea.SelectedClass, header.Classes);
    }

    /// <summary>A dock that is opened is the one being worked in.</summary>
    [Fact]
    public void AnOpenedDockIsWorkedIn()
    {
        var (services, _) = Hosts();
        var view = new Border { Background = Brushes.Gray };
        var main = Open(services);

        Assert.Null(services.Docks.Focus.Current);

        var dock = Activate(services, "it.focus.opened", "it_dock.opened", view);

        Pump();

        Assert.Equal(
            new FocusedDock(dock.DockNameId, dock.Ordinal),
            services.Docks.Focus.Current
        );
    }

    /// <summary>Closing the dock being worked in passes the focus to one that is still there, and lets it go when none is.</summary>
    [Fact]
    public void ClosingTheDockWorkedInPassesTheFocusOn()
    {
        var (services, _) = Hosts();
        var left = new Border { Background = Brushes.Gray };
        var right = new Border { Background = Brushes.Gray };
        var first = Activate(services, "it.focus.left", "it_dock.left", left);
        var second = Activate(services, "it.focus.right", "it_dock.right", right);

        var main = Open(services);

        Pump();

        // The group opens on the dock made last, which is the one drawn; the other is a tab and nothing else.
        Click(main, right);

        Assert.Equal(second.DockNameId, services.Docks.Focus.Current?.DockNameId);

        // The dock beside it, the other tab of its group, is what the work falls to.
        services.Docks.Close(second);

        Assert.Equal(
            new FocusedDock(first.DockNameId, first.Ordinal),
            services.Docks.Focus.Current
        );

        Pump();

        services.Docks.Close(first);

        Assert.Null(services.Docks.Focus.Current);
    }

    /// <summary>A dock moved into a window of its own is still the dock being worked in.</summary>
    /// <remarks>
    /// The record is the program's rather than a window's: the dock is the same dock before and after the move.
    /// </remarks>
    [Fact]
    public void ADockMovedToAWindowOfItsOwnKeepsTheFocus()
    {
        var (services, _) = Hosts();
        var view = new Border { Background = Brushes.Gray };
        var dock = Activate(services, "it.focus.detached", "it_dock.detached", view);
        var main = Open(services);

        Pump();
        Click(main, view);

        Assert.Equal(dock.DockNameId, services.Docks.Focus.Current?.DockNameId);

        services.Windows.ToOwnWindow(dock);

        Pump();

        Assert.Equal(dock.DockNameId, services.Docks.Focus.Current?.DockNameId);
    }

    /// <summary>A plugin reads the dock being worked in, and is told when it says something else.</summary>
    [Fact]
    public void APluginReadsTheDockBeingWorkedIn()
    {
        var (services, plugin) = Hosts();
        var view = new Border { Background = Brushes.Gray };
        var main = Open(services);

        var seen = new List<FocusedDock?>();

        plugin.Focus.Changed += () => seen.Add(plugin.Focus.Current);

        Assert.Null(plugin.Focus.Current);

        var dock = Activate(services, "it.focus.read", "it_dock.read", view);

        Pump();
        Click(main, view);
        Pump();

        Assert.Equal(
            new FocusedDock(dock.DockNameId, dock.Ordinal),
            plugin.Focus.Current
        );
        Assert.Contains(plugin.Focus.Current, seen);
    }

    /// <summary>A press in the middle of a control, which is what makes its dock the one worked in.</summary>
    /// <param name="window">The window the control is in.</param>
    /// <param name="control">The control.</param>
    private static void Click(Window window, Control control)
    {
        Assert.True(
            control.Bounds.Width > 0 && control.Bounds.Height > 0,
            "the dock has not been laid out, so there is nothing to press"
        );

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

    /// <summary>Lets the window run a few turns of its loop, which is what lays a dock area out.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>Everything the host holds, over a window factory of the program's own.</summary>
    /// <returns>The services, and the host one plugin sees.</returns>
    private static (HostServices Services, IPluginHost Plugin) Hosts()
    {
        var services = Host.Services();

        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);

        return (services, services.For(new PluginId("it.focus"), 1));
    }

    /// <summary>Registers a dock and opens it, which is what a plugin asking for one does.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <param name="titleKey">The i18n key its title is read from.</param>
    /// <param name="view">The control it is drawn as.</param>
    /// <returns>The dock.</returns>
    private static DockInstance Activate(
        HostServices services,
        string nameId,
        string titleKey,
        Control view
    )
    {
        var registration = new DockRegistration(
            Core.Id,
            nameId,
            titleKey,
            DockOpenMode.New,
            DockSide.Center,
            () => new FakeDockView { Control = view }
        );

        services.Docks.Register(registration);

        return services.Docks.Activate(registration)
            ?? throw new InvalidOperationException($"`{nameId}` could not be opened");
    }

    /// <summary>Opens the main window over the docks that are already open.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <returns>The window.</returns>
    private static MainWindow Open(HostServices services)
    {
        services.Windows.OpenMain();

        return (MainWindow)services.Shell.Window!;
    }

    /// <summary>Moves a dock to the side of the one it shares a group with, which is what a split is.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="instance">The dock to move.</param>
    /// <param name="beside">The dock whose group it is put beside.</param>
    /// <param name="side">Which side.</param>
    private static void Beside(
        HostServices services,
        DockInstance instance,
        DockInstance beside,
        DockSide side
    )
    {
        var group =
            DockTree.GroupOf(services.Docks.Layout, beside.DockNameId, beside.Ordinal)
            ?? throw new InvalidOperationException("the dock is not in the main window's tree");

        services.Docks.Move(instance, new DockTarget(group.Id, side, 0));
    }
}

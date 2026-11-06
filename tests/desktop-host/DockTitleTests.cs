using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Theming;
using MenuItem = Avalonia.Controls.MenuItem;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A dock that names itself by where it is: what its tab says, what its tip says, and what a window of its own says.
/// </summary>
/// <remarks>
/// The host draws a dock's name in three places, the tab, the window a dock of its own is shown in, and the top of the
/// tab's menu, and the three are one answer: what a dock says about itself. What is checked here is that they follow
/// it, which is the whole of the feature: a browser is one dock looked at in several windows, and a strip of tabs all
/// called "Directories" is a strip nobody can tell apart.
/// </remarks>
public sealed class DockTitleTests
{
    /// <summary>A dock that says where it is has its word, its tip and its menu said for it.</summary>
    [Fact]
    public void ATabSaysWhatItsDockSays()
    {
        var (services, views, main, _) = Open(2);

        views[0].Name("art", @"C:\work\art");
        views[1].Name("hero", @"C:\work\hero");
        Pump();

        var tabs = Tabs(main).ToArray();

        Assert.Equal(2, tabs.Length);
        Assert.Equal(["art", "hero"], tabs.Select(TabName));
        Assert.Equal([@"C:\work\art", @"C:\work\hero"], tabs.Select(TabTip));
        Assert.Equal([@"C:\work\art", @"C:\work\hero"], tabs.Select(TabHeading));

        // Where a dock goes somewhere else, the three say the new place at once.
        views[0].Name("art-2", @"C:\work\art-2");
        Pump();

        Assert.Equal(["art-2", "hero"], Tabs(main).Select(TabName));
        Assert.Equal([@"C:\work\art-2", @"C:\work\hero"], Tabs(main).Select(TabTip));
        Assert.Equal([@"C:\work\art-2", @"C:\work\hero"], Tabs(main).Select(TabHeading));
    }

    /// <summary>A dock that says nothing is called what it was registered as.</summary>
    [Fact]
    public void ADockWithNothingToSayKeepsItsRegisteredName()
    {
        var (_, views, main, _) = Open(1);

        views[0].Name(null, null);
        Pump();

        Assert.Equal(["it_dock.title"], Tabs(main).Select(TabName));
    }

    /// <summary>A long name keeps its end and leaves its front out.</summary>
    [Fact]
    public void ALongNameKeepsItsEnd()
    {
        var title = new string('a', 60) + @"C:\work\art";
        var word = DockArea.TabTitle(title);

        // What is kept is the tail, with the ellipsis standing where everything before it was.
        Assert.Equal("..." + title[^(DockArea.TabWordLength - 3)..], word);
        Assert.True(word.Length <= DockArea.TabWordLength);
        Assert.EndsWith(@"C:\work\art", word, StringComparison.Ordinal);

        // A name that fits is the whole of it.
        Assert.Equal("art", DockArea.TabTitle("art"));
    }

    /// <summary>A window of one dock is that dock, so it is named by it and renamed with it.</summary>
    [Fact]
    public void AWindowOfOneDockIsRenamedWithIt()
    {
        var (services, views, _, docks) = Open(1);

        views[0].Name("art", @"C:\work\art");
        Pump();

        services.Windows.ToOwnWindow(docks[0]);

        var own = (MainWindow)services.Shell.Window!;

        Pump();

        var caption = Assert.Single(Captions(own));

        Assert.Equal("art", caption.Text);
        Assert.Equal(@"C:\work\art", ToolTip.GetTip(caption));

        views[0].Name("hero", @"C:\work\hero");
        Pump();

        Assert.Equal("hero", caption.Text);
        Assert.Equal(@"C:\work\hero", ToolTip.GetTip(caption));
    }

    /// <summary>The tabs of one window, in the order their strip draws them.</summary>
    /// <param name="window">The window.</param>
    /// <returns>The tab buttons.</returns>
    private static IEnumerable<Button> Tabs(Window window) =>
        window
            .GetVisualDescendants()
            .OfType<Button>()
            .Where(button => button.Classes.Contains(DockArea.TitleClass));

    /// <summary>The word a tab says.</summary>
    /// <param name="tab">The tab.</param>
    /// <returns>What it says.</returns>
    private static string TabName(Button tab) =>
        ((Grid)tab.Content!).Children.OfType<TextBlock>().First().Text!;

    /// <summary>What a tab says where the pointer rests on it.</summary>
    /// <param name="tab">The tab.</param>
    /// <returns>The tip.</returns>
    private static string TabTip(Button tab) => (string)ToolTip.GetTip(tab)!;

    /// <summary>The line at the top of a tab's menu, which says which dock the menu is about.</summary>
    /// <param name="tab">The tab.</param>
    /// <returns>What it says.</returns>
    private static string TabHeading(Button tab) =>
        (string)((MenuItem)((ContextMenu)tab.ContextMenu!).Items[0]!).Header!;

    /// <summary>The names in the row of a window that is one dock.</summary>
    /// <param name="window">The window.</param>
    /// <returns>The name controls, in the order they are drawn.</returns>
    private static IEnumerable<TextBlock> Captions(Window window) =>
        window
            .GetVisualDescendants()
            .OfType<TextBlock>()
            .Where(text => text.Classes.Contains(RorolalaTheme.WindowTitleClass));

    /// <summary>Opens docks that name themselves, in the main window, and lays them out.</summary>
    /// <param name="count">How many instances to open.</param>
    /// <returns>Everything the host holds, the views, the window and the docks.</returns>
    private static (
        HostServices Services,
        List<TitledDockView> Views,
        MainWindow Main,
        List<DockInstance> Docks
    ) Open(int count)
    {
        var services = Host.Services();

        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);

        var views = new List<TitledDockView>();
        var registration = new DockRegistration(
            Core.Id,
            "it.title.browser",
            "it_dock.title",
            DockOpenMode.New,
            DockSide.Center,
            () =>
            {
                var view = new TitledDockView();

                views.Add(view);

                return view;
            }
        );

        services.Docks.Register(registration);

        var docks = new List<DockInstance>();

        for (var at = 0; at < count; at++)
        {
            docks.Add(
                services.Docks.Activate(registration)
                    ?? throw new InvalidOperationException("the dock could not be opened")
            );
        }

        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;

        Pump();

        return (services, views, main, docks);
    }

    /// <summary>Lets the windows run a few turns of their loop, which is what lays a dock area out.</summary>
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

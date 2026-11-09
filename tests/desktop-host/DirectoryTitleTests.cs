using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a browser calls the place it is looking at, which is what its tab and its window are called.
/// </summary>
/// <remarks>
/// A browser is one dock made again for every place, so the name it was registered under says what kind of dock it is
/// and not where it is. What it says instead is checked here over a scratch tree: the real path of the directory it is
/// looking at, the address of a place it was sent to, the drive or the computer where there is no path, and a name
/// that follows every step.
/// </remarks>
public sealed class DirectoryTitleTests
{
    /// <summary>The address bar reads from the tree's base, which is `.`, rather than from the drive.</summary>
    [Fact]
    public void TheAddressBarIsRelativeToTheBase()
    {
        var (root, _, whole, window, control) = Open();

        try
        {
            var bar = control.GetVisualDescendants().OfType<NavigationBar>().Single();

            Assert.Equal(["."], Crumbs(bar));

            var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;

            whole.Go(art);
            Pump();

            Assert.Equal([".", "art"], Crumbs(bar));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The words the address bar's crumbs say, in order.</summary>
    /// <param name="bar">The address bar.</param>
    /// <returns>The crumbs.</returns>
    private static string[] Crumbs(NavigationBar bar) =>
        [
            .. bar.GetVisualDescendants()
                .OfType<Button>()
                .Where(button => button.Classes.Contains("ghost"))
                .Select(button => button.Content as string)
                .OfType<string>(),
        ];

    /// <summary>A browser inside the base is named by its part below the base, and the detail is the whole path.</summary>
    [Fact]
    public void ABrowserIsNamedRelativelyUnderTheBase()
    {
        var (root, shared, whole, window, control) = Open();

        try
        {
            var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;
            var deep = Directory.CreateDirectory(Path.Combine(art, "shots")).FullName;

            whole.Go(art);
            Pump();

            Assert.Equal(Path.GetRelativePath(shared.BaseDir, art), control.Title);
            Assert.Equal(art, control.Detail);

            whole.Go(deep);
            Pump();

            Assert.Equal(Path.GetRelativePath(shared.BaseDir, deep), control.Title);
            Assert.Equal(deep, control.Detail);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The base itself is named by its last part, and the detail is the whole path.</summary>
    [Fact]
    public void ABrowserAtTheBaseIsNamedByTheBasesLeaf()
    {
        var (root, _, _, window, control) = Open();

        try
        {
            Assert.Equal(Path.GetFileName(root), control.Title);
            Assert.Equal(root, control.Detail);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A directory outside the base is named by the same real path, since there is nothing to shorten it against.</summary>
    [Fact]
    public void ADirectoryOutsideTheBaseIsNamedByItsRealPath()
    {
        var (_, _, whole, window, control) = Open();
        var elsewhere = Scratch.New("title-elsewhere");

        try
        {
            whole.Go(elsewhere);
            Pump();

            Assert.Equal(elsewhere, control.Title);
            Assert.Equal(elsewhere, control.Detail);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A drive or a computer has a name too: the step itself where there is no directory name.</summary>
    [Fact]
    public void ADriveAndTheComputerHaveNames()
    {
        var (root, _, whole, window, control) = Open();
        var drive = Path.GetPathRoot(root)!;

        try
        {
            whole.Go(drive);
            Pump();

            // A root is a directory with nothing for a name, so it is shown as the step it is.
            Assert.Equal(drive, control.Title);

            whole.Go(Browser.Computer);
            Pump();

            Assert.Equal(RolaI18N.Get("rorolala_file_system.computer"), control.Title);
            Assert.Equal(RolaI18N.Get("rorolala_file_system.computer"), control.Detail);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A place a plugin answers for is named by its address, which is the real path of a place.</summary>
    [Fact]
    public void APlaceIsNamedByItsAddress()
    {
        var (_, _, whole, window, control) = Open();

        try
        {
            whole.Go("logic@somewhere");
            Pump();

            Assert.Equal("logic@somewhere", control.Title);
            Assert.Equal("logic@somewhere", control.Detail);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A title says something else as the browser moves, and a dock's name is its control's.</summary>
    [Fact]
    public void ATitleFollowsTheBrowserAndADockSaysIt()
    {
        var services = Host.Services();
        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);

        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var root = Scratch.New("title-dock");
        var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var whole = new Browser(host.Log, shared, root, host.LogicalViews);
        var registration = new DockRegistration(
            Core.Id,
            "it.title.browser",
            "it_dock.title",
            DockOpenMode.New,
            DockSide.Center,
            () => new DirectoryDock(host, shared, whole, new Clip(host.Log), new LogicalClip())
        );

        services.Docks.Register(registration);

        var dock = services.Docks.Activate(registration)!;

        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;

        Pump();

        try
        {
            // The host was given the name the dock says, not the one it was registered as.
            Assert.Equal(Path.GetFileName(root), dock.Title);
            Assert.NotEqual("it_dock.title", dock.Title);

            whole.Go(art);
            Pump();

            Assert.Equal(Path.GetRelativePath(shared.BaseDir, art), dock.Title);

            var tab = main
                .GetVisualDescendants()
                .OfType<Button>()
                .Single(button => button.Classes.Contains(RorolalaDesktop.Docking.DockArea.TitleClass));

            // The tab's word is the path, shortened at its front where a strip of tabs has no room for all of it.
            Assert.Equal(
                RorolalaDesktop.Docking.DockArea.TabTitle(dock.Title!),
                ((Grid)tab.Content!).Children.OfType<TextBlock>().First().Text
            );
        }
        finally
        {
            main.Close();
        }
    }

    /// <summary>A base changed under a dock retitles the directory it is looking at.</summary>
    [Fact]
    public void ABaseChangeRetitlesTheSameDirectory()
    {
        var (root, shared, whole, window, control) = Open();
        var elsewhere = Scratch.New("title-new-base");
        var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;

        try
        {
            whole.Go(art);
            Pump();

            Assert.Equal(Path.GetRelativePath(shared.BaseDir, art), control.Title);

            shared.BaseDir = elsewhere;
            Pump();

            Assert.Equal(art, control.Title);
            Assert.Equal(art, control.Detail);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A directory browser over a scratch root, on a headless window, drawn once.</summary>
    /// <returns>The root, the shared answers, the location, the window, and the dock's control.</returns>
    private static (string Root, Shared Shared, Browser Browser, Window Window, DirectoryControl Dock) Open()
    {
        var root = Scratch.New("title");
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var control = new DirectoryControl(
            host,
            shared,
            browser,
            new Clip(host.Log),
            new LogicalClip()
        );
        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = control,
        };

        host.LogicalViews.Register("logic", new Place());

        window.Show();
        window.Activate();
        Pump();

        return (root, shared, browser, window, control);
    }

    /// <summary>Lets the window run a few turns of its loop, which is what lays a dock out and draws it.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>A place that answers one address, so that a browser can be sent to one.</summary>
    private sealed class Place : ILogicalViewProvider
    {
        /// <inheritdoc />
        public string Title(LogicalAddress address) => "the place";

        /// <inheritdoc />
        public IReadOnlyList<LogicalEntry> Entries(LogicalAddress address) => [];

        /// <inheritdoc />
        public LogicalAddress? Parent(LogicalAddress address) => null;

        /// <inheritdoc />
        public IReadOnlyList<LogicalColumn> Columns(LogicalAddress address) => [];

        /// <inheritdoc />
        public bool Accepts(LogicalAddress address, LogicalDrag drag) => false;

        /// <inheritdoc />
        public void Drop(LogicalAddress address, LogicalDrag drag) { }
    }
}

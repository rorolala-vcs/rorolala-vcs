using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A path shown in a directory dock: the directory holding it, and the file chosen where there is one.
/// </summary>
/// <remarks>
/// What the Overview's rows ask for, read where it is answered: a dock is what knows its listing, its rules and where
/// it is, so what a showing comes to is checked here rather than from the outside. Nothing is drawn but the window
/// the dock needs to be laid out in.
/// </remarks>
public sealed class BrowserRevealTests
{
    /// <summary>A file is shown by its directory, with its row chosen and the version graph on it.</summary>
    [Fact]
    public void AFileIsShownByItsDirectoryWithTheFileChosen()
    {
        var (root, browser, window, control) = Open();

        try
        {
            var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;
            var file = Path.Combine(art, "hero.psd");

            File.WriteAllText(file, "x");

            control.Reveal(file);
            Pump();

            Assert.Equal(art, browser.Current);

            var list = window.GetVisualDescendants().OfType<ListBox>().First();

            Assert.Equal(file, Assert.IsType<Entry>(list.SelectedItem).Path);

            // The one answer a view of a file's history follows is said as well as the choice: a row chosen from
            // elsewhere is the file the program is about in the way a click is.
            Assert.Equal(file, ChosenEntry.Current?.Path);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A file that is gone is shown by the directory it was in, and nothing is chosen for it.</summary>
    [Fact]
    public void AFileThatIsGoneIsShownByItsDirectory()
    {
        var (root, browser, window, control) = Open();

        try
        {
            var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;

            File.WriteAllText(Path.Combine(art, "hero.psd"), "x");

            control.Reveal(Path.Combine(art, "gone.psd"));
            Pump();

            Assert.Equal(art, browser.Current);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A path whose directory is not there either leaves the dock where the user left it.</summary>
    [Fact]
    public void APathNowhereLeavesTheDockWhereItIs()
    {
        var (root, browser, window, control) = Open();

        try
        {
            var before = browser.Current;

            control.Reveal(Path.Combine(root, "nowhere", "gone.psd"));
            Pump();

            Assert.Equal(before, browser.Current);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>
    /// A dock made over a place takes that place as its own; one told a path later goes there as an ordinary step.
    /// </summary>
    /// <remarks>
    /// The two answers a dock gives to being told what to show: opened onto a directory it takes a place of its own,
    /// and shown a path afterwards it navigates the way the user's own steps do. A file is the second kind, nothing
    /// can be opened onto a file, and what says it worked is the file chosen.
    /// </remarks>
    [Fact]
    public void ADockShownAPathAfterBeingOpenedGoesThereAndChoosesTheFile()
    {
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var root = Scratch.New("reveal-dock");
        var art = Directory.CreateDirectory(Path.Combine(root, "art")).FullName;
        var file = Path.Combine(art, "hero.psd");

        File.WriteAllText(file, "x");

        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var whole = new Browser(host.Log, shared, root, host.LogicalViews);
        var dock = new DirectoryDock(
            host,
            shared,
            whole,
            new Clip(host.Log),
            new LogicalClip()
        );
        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = dock.View,
        };

        window.Show();
        window.Activate();
        Pump();

        try
        {
            // Made over a place: the dock is taken out of step to look at it, as a browser opened for the caller is.
            dock.Opened(new DockRequest(FileSystem.DirectoryDockNameId, art));
            Pump();

            // Shown a file afterwards: a step of its own, with the file chosen.
            dock.Opened(new DockRequest(FileSystem.DirectoryDockNameId, file));
            Pump();

            Assert.Equal(file, ChosenEntry.Current?.Path);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A directory dock over a scratch root, on a headless window, drawn once.</summary>
    /// <returns>The root, the location, the window, and the dock's control.</returns>
    private static (string Root, Browser Browser, Window Window, DirectoryControl Dock) Open()
    {
        var root = Scratch.New("reveal");
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

        window.Show();
        window.Activate();
        Pump();

        return (root, browser, window, control);
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
}

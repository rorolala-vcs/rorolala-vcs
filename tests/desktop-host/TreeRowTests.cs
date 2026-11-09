using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// How a tree row lays its name, its icon and its badges out.
/// </summary>
/// <remarks>
/// A tree is read at whatever width a dock is given, and a long name must not be drawn over the badges at the
/// row's right. The row is a grid for that reason, and the checks here measure the arrangement rather than
/// comparing a picture: headless layout is deterministic, and a picture would only say that the same arrangement
/// was drawn.
/// </remarks>
public sealed class TreeRowTests
{
    /// <summary>Begins each test with no provider of its own, since the catalogues are the run's.</summary>
    public TreeRowTests()
    {
        DataHome.Clean();
        HideRegistry.Clear();
        EntryMarkProviders.Clear();
    }

    /// <summary>A long name ends before the badges, which keep the width they need at the row's right.</summary>
    [Fact]
    public void ALongNameDoesNotDrawOverTheBadges()
    {
        var directory = Scratch.New("tree-row-narrow");
        var name = "a-file-name-long-enough-that-it-cannot-fit-in-a-narrow-tree.txt";

        File.WriteAllText(Path.Combine(directory, name), "x");

        EntryMarkProviders.Register(new FixedMarks());

        var (window, _) = OpenAt(directory, width: 220);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, Path.Combine(directory, name)));
            var label = Named<TextBlock>(row, "entry-name");
            var badges = Named<StackPanel>(row, "entry-badges");

            Assert.Equal(TextTrimming.CharacterEllipsis, label.TextTrimming);
            Assert.True(
                Right(window, label) <= Left(window, badges) + 1,
                $"the name ends at {Right(window, label)} and the badges begin at {Left(window, badges)}"
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The badges sit at the row's right edge at either width.</summary>
    [Fact]
    public void TheBadgesStayAtTheRightEdge()
    {
        var directory = Scratch.New("tree-row-wide");

        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        EntryMarkProviders.Register(new FixedMarks());

        var (window, _) = OpenAt(directory, width: 900);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, Path.Combine(directory, "hero.psd")));
            var header = Assert.IsType<Border>(row.Header);
            var badges = Named<StackPanel>(row, "entry-badges");

            Assert.InRange(Right(window, badges), Right(window, header) - 2, Right(window, header) + 1);

            window.Width = 220;
            PumpFrames(window, 20);

            row = Until(window, () => Row(window, Path.Combine(directory, "hero.psd")));
            header = Assert.IsType<Border>(row.Header);
            badges = Named<StackPanel>(row, "entry-badges");

            Assert.InRange(Right(window, badges), Right(window, header) - 2, Right(window, header) + 1);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree row wears its badges at twenty pixels, while the entry's icon stays at sixteen.</summary>
    [Fact]
    public void ABadgeIsTwentyPixels()
    {
        var directory = Scratch.New("tree-row-badge");

        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        EntryMarkProviders.Register(new FixedMarks());

        var (window, _) = OpenAt(directory, width: 900);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, Path.Combine(directory, "hero.psd")));
            var badge = Assert.IsAssignableFrom<Control>(Named<StackPanel>(row, "entry-badges").Children[0]);
            var icon = Named<Control>(row, "entry-icon");

            Assert.Equal(20, badge.Bounds.Width, 1);
            Assert.Equal(20, badge.Bounds.Height, 1);
            Assert.Equal(16, icon.Bounds.Width, 1);
            Assert.Equal(16, icon.Bounds.Height, 1);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree dock over a directory, shown in a window of a given width.</summary>
    /// <param name="directory">The directory the tree is rooted at.</param>
    /// <param name="width">How wide the window is.</param>
    /// <returns>The window, and the location it reads.</returns>
    private static (Window Window, Browser Browser) OpenAt(string directory, double width)
    {
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);

        RolaI18N.SetTranslationDirectory(Path.Combine(AppContext.BaseDirectory, "i18n"));

        HideRegistry.Declare(host);
        Icons.Declare(host.Icons);

        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);
        var dock = new TreeDock(host, browser, new Clip(host.Log));

        var window = new Window
        {
            Content = dock.View,
            Width = width,
            Height = 600,
        };

        window.Show();
        window.Activate();

        return (window, browser);
    }

    /// <summary>The row standing for a path, once it is drawn.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <param name="path">The path the row stands for.</param>
    /// <returns>The row, or nothing while it is not drawn.</returns>
    private static TreeViewItem? Row(Window window, string path) =>
        window.GetVisualDescendants()
            .OfType<TreeViewItem>()
            .FirstOrDefault(item => string.Equals(item.Tag as string, path, StringComparison.Ordinal));

    /// <summary>The step the tree in a window is rooted at, or nothing while the tree is not drawn yet.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <returns>The root step, or nothing.</returns>
    private static TreeViewItem? Root(Window window) =>
        window.GetVisualDescendants()
            .OfType<TreeView>()
            .SelectMany(tree => tree.Items.OfType<TreeViewItem>())
            .FirstOrDefault(step => step.Bounds.Height > 0);

    /// <summary>A named control under a row.</summary>
    /// <typeparam name="T">What the control is.</typeparam>
    /// <param name="row">The row to look under.</param>
    /// <param name="name">The name the row gave it.</param>
    /// <returns>The control.</returns>
    /// <exception cref="InvalidOperationException">The row has no such control.</exception>
    private static T Named<T>(TreeViewItem row, string name)
        where T : Control =>
        row.GetVisualDescendants()
            .OfType<T>()
            .FirstOrDefault(control => string.Equals(control.Name, name, StringComparison.Ordinal))
        ?? throw new InvalidOperationException($"the row `{row.Tag}` has no `{name}`");

    /// <summary>Where a control begins, in the window's own coordinates.</summary>
    /// <param name="window">The window the control is in.</param>
    /// <param name="control">The control.</param>
    /// <returns>The left edge.</returns>
    private static double Left(Window window, Visual control) =>
        control.TranslatePoint(new Point(0, 0), window)?.X
        ?? throw new InvalidOperationException($"`{control.GetType().Name}` is not drawn in the window");

    /// <summary>Where a control ends, in the window's own coordinates.</summary>
    /// <param name="window">The window the control is in.</param>
    /// <param name="control">The control.</param>
    /// <returns>The right edge.</returns>
    private static double Right(Window window, Visual control) =>
        control.TranslatePoint(new Point(control.Bounds.Width, 0), window)?.X
        ?? throw new InvalidOperationException($"`{control.GetType().Name}` is not drawn in the window");

    /// <summary>Waits for something to be drawn, which happens a turn or so after it is asked for.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="find">What is being waited for, which answers nothing until it is there.</param>
    /// <returns>What was waited for.</returns>
    /// <exception cref="InvalidOperationException">It was never drawn.</exception>
    private static T Until<T>(Window window, Func<T?> find)
        where T : class
    {
        for (var turn = 0; turn < 400; turn++)
        {
            if (find() is { } found)
            {
                return found;
            }

            PumpFrames(window, 1);
        }

        throw new InvalidOperationException("what was asked for was never drawn");
    }

    /// <summary>Draws the window for a number of turns.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="turns">How many turns to draw it for.</param>
    private static void PumpFrames(Window window, int turns)
    {
        for (var turn = 0; turn < turns; turn++)
        {
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Dispatcher.UIThread.RunJobs();
            Thread.Sleep(10);
        }
    }

    /// <summary>A mark provider that puts one fixed badge on every entry.</summary>
    private sealed class FixedMarks : IEntryMarkProvider
    {
        /// <inheritdoc />
        public string Id => "it.tree.row.marks";

        /// <inheritdoc />
        public bool Applies(string directory) => true;

        /// <inheritdoc />
        public EntryMark Mark(Entry entry) => new(Text: "it", IconKey: Icons.CreateNewFolder, Ink: MarkInk.Accent);
    }
}

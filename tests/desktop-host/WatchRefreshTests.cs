using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A change the files make is read where it happened, and drawn as the one entry it reached.
/// </summary>
/// <remarks>
/// What the watcher says is one directory, or one entry of one directory, and what answers it is the location
/// showing that directory and the row of that entry: a change in one place is not a reason to read every
/// location, nor to make every row again. The pieces are checked where they are told apart, from the routing
/// through the shared answers to a dock's view and to the tree.
/// </remarks>
public sealed class WatchRefreshTests
{
    /// <summary>Begins each test with no hide provider and no configuration of its own.</summary>
    public WatchRefreshTests()
    {
        DataHome.Clean();
        global::FileSystemPlugin.HideRegistry.Clear();
        EntryColourProviders.Clear();
        EntryMarkProviders.Clear();
    }

    /// <summary>A change of one directory reaches the location showing it, and no other.</summary>
    [Fact]
    public void AChangeOfOneDirectoryReachesOnlyWhatShowsIt()
    {
        var one = Scratch.New("route-one");
        var other = Scratch.New("route-other");

        File.WriteAllText(Path.Combine(one, "one.txt"), "x");
        File.WriteAllText(Path.Combine(other, "one.txt"), "x");

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(one, new HideRegistry(host.Config), host.Config, host.Files);

        using var showing = new Browser(host.Log, shared, one, host.LogicalViews);
        using var elsewhere = new Browser(host.Log, shared, other, host.LogicalViews);

        var here = 0;
        var there = 0;

        showing.Changed += () => here++;
        elsewhere.Changed += () => there++;

        shared.DirectoryChanged(one);
        Pump();

        Assert.Equal(1, here);
        Assert.Equal(0, there);
    }

    /// <summary>A change of one entry is that entry named, once, to the location that holds it.</summary>
    [Fact]
    public void AChangeOfOneEntryNamesItOnceToTheLocationThatHoldsIt()
    {
        var one = Scratch.New("dirty-one");
        var other = Scratch.New("dirty-other");
        var file = Path.Combine(one, "one.txt");

        File.WriteAllText(file, "x");
        File.WriteAllText(Path.Combine(other, "one.txt"), "x");

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(one, new HideRegistry(host.Config), host.Config, host.Files);

        using var showing = new Browser(host.Log, shared, one, host.LogicalViews);
        using var elsewhere = new Browser(host.Log, shared, other, host.LogicalViews);

        shared.EntryChanged(file);
        Pump();

        Assert.Equal([file], showing.TakeDirty());
        Assert.Empty(showing.TakeDirty());
        Assert.Empty(elsewhere.TakeDirty());
    }

    /// <summary>
    /// A listing is brought to what a read holds without the rows it already draws being made again.
    /// </summary>
    [Fact]
    public void AListingKeepsTheRowsItHasAndMakesTheOneThatIsNew()
    {
        var directory = Scratch.New("apply");
        var one = Path.Combine(directory, "one.txt");
        var two = Path.Combine(directory, "two.txt");
        var three = Path.Combine(directory, "three.txt");

        var host = Host.Services().For(new PluginId("test"), 1);
        var view = new ListBrowser<string>(host, new Named(host, [one, two]));
        var window = Show(view);

        try
        {
            var list = window.GetVisualDescendants().OfType<ListBox>().Single();
            var first = list.ContainerFromIndex(0);
            var second = list.ContainerFromIndex(1);

            view.Assign(new Named(host, [one, two, three]));
            view.Apply();
            Pump();

            Assert.Equal(3, list.ItemCount);
            Assert.Same(first, list.ContainerFromIndex(0));
            Assert.Same(second, list.ContainerFromIndex(1));
            Assert.NotNull(list.ContainerFromIndex(2));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A key named as changed draws that entry again, and leaves the others as they are.</summary>
    [Fact]
    public void AKeyNamedAsChangedDrawsOnlyThatEntryAgain()
    {
        var directory = Scratch.New("apply-dirty");
        var one = Path.Combine(directory, "one.txt");
        var two = Path.Combine(directory, "two.txt");

        var host = Host.Services().For(new PluginId("test"), 1);
        var view = new ListBrowser<string>(host, new Named(host, [one, two]));
        var window = Show(view);

        try
        {
            var list = window.GetVisualDescendants().OfType<ListBox>().Single();
            var first = RowOf(list.ContainerFromIndex(0));
            var second = RowOf(list.ContainerFromIndex(1));

            view.Apply([one]);
            Pump();

            Assert.NotSame(first, RowOf(list.ContainerFromIndex(0)));
            Assert.Same(second, RowOf(list.ContainerFromIndex(1)));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A change of one file's content in a grid makes that tile again and no other.</summary>
    [Fact]
    public void AContentChangeInAGridMakesThatTileAgain()
    {
        var root = Scratch.New("grid-watch");

        for (var index = 0; index < 3; index++)
        {
            File.WriteAllText(Path.Combine(root, $"file-{index}.txt"), "x");
        }

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var control = new DirectoryControl(host, shared, browser, new Clip(host.Log), new LogicalClip());
        var window = new Window { Width = 900, Height = 600, Content = control };

        window.Show();
        window.Activate();

        try
        {
            control.OpenAt(root);
            Pump();

            var list = window.GetVisualDescendants().OfType<ListBox>().First();
            var paths = Items(list);
            var changed = paths.Select((path, at) => (path, at)).First(one => one.path.EndsWith("file-1.txt", StringComparison.Ordinal)).at;
            var before = Containers(list).Select(RowOf).ToArray();

            shared.EntryChanged(paths[changed]);
            Pump();

            var after = Containers(list).Select(RowOf).ToArray();

            for (var at = 0; at < paths.Count; at++)
            {
                if (at == changed)
                {
                    Assert.NotSame(before[at], after[at]);
                }
                else
                {
                    Assert.Same(before[at], after[at]);
                }
            }
        }
        finally
        {
            window.Close();
            browser.Dispose();
        }
    }

    /// <summary>A change in one open step of a tree leaves the rows of the other step as they were.</summary>
    [Fact]
    public void AChangeInOneStepOfATreeLeavesTheOtherStepAsItWas()
    {
        var root = Scratch.New("tree-watch");
        var alpha = Directory.CreateDirectory(Path.Combine(root, "alpha")).FullName;
        var beta = Directory.CreateDirectory(Path.Combine(root, "beta")).FullName;

        File.WriteAllText(Path.Combine(alpha, "a.txt"), "x");
        File.WriteAllText(Path.Combine(beta, "b.txt"), "x");

        var (window, shared) = OpenTree(root);

        try
        {
            var rootRow = Until(() => Root(window));

            rootRow.IsExpanded = true;
            Pump();

            var alphaRow = Until(() => RowByPath(window, alpha));
            var betaRow = Until(() => RowByPath(window, beta));

            alphaRow.IsExpanded = true;
            betaRow.IsExpanded = true;

            Until(() => betaRow.Items.OfType<TreeViewItem>().Any());
            Until(() => alphaRow.Items.OfType<TreeViewItem>().Any());

            var before = betaRow.Items.OfType<TreeViewItem>().ToArray();

            File.WriteAllText(Path.Combine(alpha, "new.txt"), "y");
            shared.DirectoryChanged(alpha);
            Pump();

            Assert.Equal(before, betaRow.Items.OfType<TreeViewItem>().ToArray());
            Assert.Contains(
                alphaRow.Items.OfType<TreeViewItem>(),
                row => Same(row.Tag as string, Path.Combine(alpha, "new.txt"))
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>
    /// A reading is let go of when the change is under it, and of the directories holding it, and not of its
    /// neighbours.
    /// </summary>
    [Fact]
    public void AReadingIsSourOnlyByAChangeUnderItOrInADirectoryHoldingIt()
    {
        var work = Scratch.New("subtree");
        var alpha = Path.Combine(work, "a");
        var alphaDeep = Path.Combine(alpha, "deep");
        var beta = Path.Combine(work, "b");

        // A reading of a directory is about the work under it, so a change deeper than it sours it.
        Assert.True(Subtree.Holds(alpha, alphaDeep));

        // A change in it sours it too.
        Assert.True(Subtree.Holds(alpha, alpha));

        // And a reading of a directory holding the change: what that one lists is an entry whose own state moved.
        Assert.True(Subtree.Holds(work, alphaDeep));

        // A reading of another branch is not made wrong by the change and is kept.
        Assert.False(Subtree.Holds(beta, alphaDeep));
    }

    /// <summary>The listing a test draws, whose items are paths and whose key is the path.</summary>
    private sealed class Named(IPluginHost host, IReadOnlyList<string> items) : Listing<string>(host, items)
    {
        /// <inheritdoc />
        public override object Key(string item) => item;

        /// <inheritdoc />
        public override string Name(string item) => Path.GetFileName(item);

        /// <inheritdoc />
        public override Control Icon(string item, int size) =>
            new Border { Width = size, Height = size, Background = Brushes.Gray };

        /// <inheritdoc />
        public override void Open(string item) { }

        /// <inheritdoc />
        public override void Fill(
            ContextMenu menu,
            Control from,
            IReadOnlyList<string> chosen,
            bool force,
            string? words = null
        ) { }
    }

    /// <summary>Shows a view in a headless window and lays it out.</summary>
    /// <param name="view">The view.</param>
    /// <returns>The window, which the caller closes.</returns>
    private static Window Show(Control view)
    {
        var window = new Window
        {
            Width = 700,
            Height = 300,
            Content = view,
        };

        window.Show();
        Pump();

        return window;
    }

    /// <summary>The paths a listing of a directory's entries shows, in the order it shows them.</summary>
    /// <param name="list">The list drawing them.</param>
    /// <returns>The paths.</returns>
    private static List<string> Items(ListBox list) =>
        [.. ((IEnumerable<Entry>)list.ItemsSource!).Select(entry => entry.Path)];

    /// <summary>The container of every row the list holds.</summary>
    /// <param name="list">The list.</param>
    /// <returns>The containers, by index.</returns>
    private static Control?[] Containers(ListBox list)
    {
        var found = new Control?[list.ItemCount];

        for (var at = 0; at < found.Length; at++)
        {
            found[at] = list.ContainerFromIndex(at);
        }

        return found;
    }

    /// <summary>The row a container holds, which is what a template makes for one entry.</summary>
    /// <param name="container">The list's container for the entry, or nothing.</param>
    /// <returns>The row, or nothing when the container is not realized.</returns>
    private static Control? RowOf(Control? container) =>
        container?.GetVisualDescendants().OfType<ContentPresenter>().FirstOrDefault()?.Child;

    /// <summary>A tree dock over a directory, shown in a window, with the answers it reads.</summary>
    /// <param name="directory">The directory the tree is rooted at.</param>
    /// <returns>The window, which the caller closes, and the shared answers.</returns>
    private static (Window Window, Shared Shared) OpenTree(string directory)
    {
        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);

        global::FileSystemPlugin.HideRegistry.Declare(host);

        var shared = new Shared(
            directory,
            new global::FileSystemPlugin.HideRegistry(host.Config),
            host.Config,
            host.Files
        );
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);
        var dock = new TreeDock(host, browser, new Clip(host.Log));

        var window = new Window
        {
            Content = dock.View,
            Width = 900,
            Height = 600,
        };

        window.Show();
        window.Activate();

        return (window, shared);
    }

    /// <summary>The row the tree is rooted at, or nothing while it is not drawn.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <returns>The root row.</returns>
    private static TreeViewItem? Root(Window window) =>
        window.GetVisualDescendants()
            .OfType<TreeView>()
            .SelectMany(tree => tree.Items.OfType<TreeViewItem>())
            .FirstOrDefault();

    /// <summary>The step a path is drawn as, or nothing while it is not drawn.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <param name="path">The directory the step stands for.</param>
    /// <returns>The step, or nothing.</returns>
    private static TreeViewItem? RowByPath(Window window, string path) =>
        window.GetVisualDescendants()
            .OfType<TreeViewItem>()
            .FirstOrDefault(step => Same(step.Tag as string, path));

    /// <summary>Waits for something a turn or so late to be drawn.</summary>
    /// <typeparam name="T">What is waited for.</typeparam>
    /// <param name="until">What is waited for, answering nothing until it is there.</param>
    /// <returns>What was waited for.</returns>
    private static T Until<T>(Func<T?> until)
        where T : class
    {
        for (var turn = 0; turn < 200; turn++)
        {
            if (until() is { } there)
            {
                return there;
            }

            Pump();
            Thread.Sleep(10);
        }

        throw new InvalidOperationException("what was waited for never appeared");
    }

    /// <summary>Waits for something a turn or so late to happen.</summary>
    /// <param name="until">What is waited for.</param>
    private static void Until(Func<bool> until)
    {
        for (var turn = 0; turn < 200; turn++)
        {
            if (until())
            {
                return;
            }

            Pump();
            Thread.Sleep(10);
        }

        throw new InvalidOperationException("what was waited for never happened");
    }

    /// <summary>Draws the window for a few turns.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 20; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>Whether two paths name the same place.</summary>
    /// <param name="left">One path, or nothing.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether they are the same place.</returns>
    private static bool Same(string? left, string? right) =>
        left is not null
        && right is not null
        && string.Equals(Full(left), Full(right), OperatingSystem.IsWindows()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal);

    /// <summary>A path as it is compared.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The path, made full and without a trailing separator.</returns>
    private static string Full(string path) => Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
}

using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;
using Icons = FileSystemPlugin.Icons;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a listing and a tree show of an entry that is not on disk, and what a user can do to one.
/// </summary>
/// <remarks>
/// The catalogue is the run's, so every case clears it first and puts only what it is about. What is checked is
/// what a reader sees: which entries a listing stages, what the real file does to the entry waiting for it, and
/// that nothing a gesture can do reaches one.
/// </remarks>
public sealed class VirtualEntryTests
{
    /// <summary>Begins each test from an empty catalogue and no provider of its own.</summary>
    public VirtualEntryTests()
    {
        DataHome.Clean();
        VirtualEntries.Clear();
        HideRegistry.Clear();
        EntryMarkProviders.Clear();
        EntryColourProviders.Clear();
    }

    /// <summary>An entry that is not on disk is listed among the files of its directory, in their order.</summary>
    [Fact]
    public void AVirtualEntryIsListedWhereItWouldBe()
    {
        var directory = Scratch.New("virtual-listing");
        File.WriteAllText(Path.Combine(directory, "b.txt"), "x");
        File.WriteAllText(Path.Combine(directory, "d.txt"), "x");
        Directory.CreateDirectory(Path.Combine(directory, "a-dir"));

        var pending = Path.Combine(directory, "c.txt");

        VirtualEntries.Place(pending, EntryKind.File, VirtualEntryState.Pending);

        var (_, browser) = Client(directory);

        Assert.Equal(new[] { "a-dir", "b.txt", "c.txt", "d.txt" }, Names(browser));
        Assert.False(File.Exists(pending));
    }

    /// <summary>A file that has arrived takes the place of the entry that was waiting for it.</summary>
    [Fact]
    public void ARealFileCoversAVirtualEntry()
    {
        var directory = Scratch.New("virtual-covered");
        var path = Path.Combine(directory, "here.txt");

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Pending);

        File.WriteAllText(path, "x");

        var (_, browser) = Client(directory);

        Assert.Null(VirtualEntries.Find(path));
        Assert.Single(browser.Shown, entry => string.Equals(entry.Path, path, StringComparison.Ordinal));
        Assert.IsNotType<Grid>(Icons.For(new Entry(path, EntryKind.File), 16));
    }

    /// <summary>Nothing a gesture can do reaches a virtual entry, and everything reaches a real one.</summary>
    [Fact]
    public void AVirtualEntryCannotBeHandled()
    {
        var directory = Scratch.New("virtual-inert");
        var real = Path.Combine(directory, "real");
        Directory.CreateDirectory(real);

        var pendingFile = new Entry(Path.Combine(directory, "pending.txt"), EntryKind.File);
        var pendingDirectory = new Entry(Path.Combine(directory, "pending-dir"), EntryKind.Directory);

        VirtualEntries.Place(pendingFile.Path, EntryKind.File, VirtualEntryState.Pending);
        VirtualEntries.Place(pendingDirectory.Path, EntryKind.Directory, VirtualEntryState.Pending);

        var (host, browser) = Client(directory);
        var files = FilesOf(host, browser);

        foreach (var entry in new[] { pendingFile, pendingDirectory })
        {
            Assert.True(files.Inert(entry));
            Assert.False(files.Openable(entry));
            Assert.False(files.Seekable(entry));
            Assert.False(files.CanDrag(entry));
            Assert.False(files.Renamable(entry));
            Assert.False(files.Removable(entry));
        }

        var actual = new Entry(real, EntryKind.Directory);

        Assert.False(files.Inert(actual));
        Assert.True(files.Openable(actual));
        Assert.True(files.Seekable(actual));
        Assert.True(files.CanDrag(actual));
        Assert.True(files.Renamable(actual));
        Assert.True(files.Removable(actual));
    }

    /// <summary>A proportion is held between nothing and all of it, and moving one is not a change of the set.</summary>
    [Fact]
    public void AProportionIsClampedAndMovingItLeavesTheSetAlone()
    {
        var directory = Scratch.New("virtual-advance");
        var path = Path.Combine(directory, "growing.psd");

        var before = VirtualEntries.Shape;

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Pending);

        Assert.Equal(before + 1, VirtualEntries.Shape);

        Assert.True(VirtualEntries.Advance(path, 2f));

        var entry = VirtualEntries.Find(path);

        Assert.NotNull(entry);
        Assert.Equal(VirtualEntryState.Progress, entry.State);
        Assert.Equal(1f, entry.Progress);
        Assert.Equal(before + 1, VirtualEntries.Shape);

        Assert.True(VirtualEntries.Advance(path, -1f));
        Assert.Equal(0f, VirtualEntries.Find(path)!.Progress);

        Assert.True(VirtualEntries.Withdraw(path));
        Assert.Equal(before + 2, VirtualEntries.Shape);
    }

    /// <summary>What has gone is written in the look's red, and a loss does not cover something on its way.</summary>
    [Fact]
    public void ALostEntryIsRedAndDoesNotCoverSomethingOnItsWay()
    {
        var directory = Scratch.New("virtual-lost");
        var path = Path.Combine(directory, "gone.psd");
        var real = new Entry(Path.Combine(directory, "kept.psd"), EntryKind.File);

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Lost);

        var (host, browser) = Client(directory);
        var files = FilesOf(host, browser);

        Assert.Equal(PaletteKeys.Del, files.NameInk(new Entry(path, EntryKind.File)));
        Assert.Null(files.NameInk(real));

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Pending);
        Assert.Null(files.NameInk(new Entry(path, EntryKind.File)));

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Lost);
        Assert.Equal(VirtualEntryState.Pending, VirtualEntries.Find(path)!.State);
    }

    /// <summary>A virtual entry wears what it was given, and asks no provider when it was given nothing.</summary>
    [Fact]
    public void ABadgeIsWornOnlyWhenOneWasGiven()
    {
        var directory = Scratch.New("virtual-badges");
        var path = Path.Combine(directory, "waiting.psd");

        var (host, browser) = Client(directory);
        host.Icons.Add(Icons.Dot, Icons.Picture(Icons.Dot));

        VirtualEntries.Place(
            path,
            EntryKind.File,
            VirtualEntryState.Pending,
            0f,
            [new EntryMark(IconKey: Icons.Dot)]
        );

        var files = FilesOf(host, browser);

        Assert.Single(files.Badges(new Entry(path, EntryKind.File)));

        VirtualEntries.Withdraw(path);
        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Pending);

        Assert.Empty(files.Badges(new Entry(path, EntryKind.File)));
    }

    /// <summary>A tree draws a virtual entry as a row with nothing under it and no menu.</summary>
    [Fact]
    public void ATreeShowsAVirtualEntryAsALeafWithNoMenu()
    {
        var directory = Scratch.New("virtual-tree");
        var lost = Path.Combine(directory, "gone.txt");
        var folder = Path.Combine(directory, "pending-folder");

        VirtualEntries.Place(lost, EntryKind.File, VirtualEntryState.Lost);
        VirtualEntries.Place(folder, EntryKind.Directory, VirtualEntryState.Pending);

        var window = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            foreach (var path in new[] { lost, folder })
            {
                var row = Until(window, () => Row(window, path));

                Assert.Empty(row.Items);
                Assert.Null(Assert.IsType<Border>(row.Header).ContextMenu);
            }
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A reading's lost paths are joined to the Workspace's root, and a tree's deletions are told apart.</summary>
    [Fact]
    public void ReadingsNameThePathsTheySayAreGone()
    {
        var root = Scratch.New("virtual-readings");
        var lost = LostPaths.Of(root, ["A/hero.psd", "bg.psd"]);

        Assert.Equal(
            new[] { Path.Combine(root, "A", "hero.psd"), Path.Combine(root, "bg.psd") },
            lost
        );

        TreeChange[] changes =
        [
            new("A/hero.psd", "File", "deleted", null),
            new("A/sub", "Directory", "deleted", null),
            new("A/bg.psd", "File", "modified", null),
            new("A/old.psd", "File", "renamed", "A/new.psd"),
        ];

        var losses = TreeLosses.Of(root, changes);

        Assert.Equal(
            new[] { Path.Combine(root, "A", "hero.psd"), Path.Combine(root, "A", "sub") },
            losses.Select(loss => loss.Path)
        );
        Assert.Equal(EntryKind.File, losses[0].Kind);
        Assert.Equal(EntryKind.Directory, losses[1].Kind);
    }

    /// <summary>An entry's icon is what it says: faded under a ring, or coloured as what has gone.</summary>
    [Fact]
    public void AnIconOfAVirtualEntryCarriesItsState()
    {
        var directory = Scratch.New("virtual-icon");
        var pending = Path.Combine(directory, "pending.psd");
        var lost = Path.Combine(directory, "lost.psd");
        var real = Path.Combine(directory, "real.psd");

        File.WriteAllText(real, "x");

        VirtualEntries.Place(pending, EntryKind.File, VirtualEntryState.Pending);
        VirtualEntries.Place(lost, EntryKind.File, VirtualEntryState.Lost);

        var growing = Assert.IsType<Grid>(Icons.For(new Entry(pending, EntryKind.File), 16));

        Assert.Contains(growing.Children, child => child is VirtualRing);
        Assert.Equal(0.55, growing.Children[0].Opacity, 2);

        var gone = Assert.IsType<Grid>(Icons.For(new Entry(lost, EntryKind.File), 16));

        Assert.InRange(gone.Children.Count, 1, 2);
        Assert.Equal(1, gone.Children[0].Opacity);

        if (gone.Children.Count == 2)
        {
            var hue = Assert.IsType<Image>(gone.Children[1]);

            Assert.Equal(BitmapBlendingMode.Hue, hue.BlendMode);
            Assert.IsAssignableFrom<Bitmap>(hue.Source);
        }
        else
        {
            // No picture to take a hue from: the system gave none, so the mark this plugin draws is the
            // entry's icon, and its square is already painted with the red of what is lost.
            var mark = Assert.IsType<Border>(gone.Children[0]);

            Assert.Equal(16, mark.Width);
            Assert.Equal(16, mark.Height);
        }

        Assert.IsNotType<Grid>(Icons.For(new Entry(real, EntryKind.File), 16));
    }

    /// <summary>A directory dock draws the ring smaller than the icon its grid gives it.</summary>
    [Fact]
    public void TheDirectoryDockDrawsTheRingSmallerThanItsIcon()
    {
        var directory = Scratch.New("virtual-mark");
        var path = Path.Combine(directory, "growing.psd");
        var entry = new Entry(path, EntryKind.File);

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Pending);

        var compact = Assert.IsType<Grid>(Icons.For(entry, 64, Icons.CompactMark));
        var small = Assert.IsType<VirtualRing>(compact.Children[1]);

        Assert.Equal(
            Math.Round(64 * Icons.CompactMark, MidpointRounding.AwayFromZero),
            small.Width,
            3
        );

        var row = Assert.IsType<Grid>(Icons.For(entry, 16));
        var whole = Assert.IsType<VirtualRing>(row.Children[1]);

        Assert.Equal(16, whole.Width);
    }

    /// <summary>A virtual ring is centred on the icon it lies over, at the size it was given there.</summary>
    /// <remarks>
    /// The ring's layout size is rounded to whole pixels while the icon's may not be, so a share of the
    /// icon that is not a whole number is rounded before it is handed over: halving a fractional size
    /// leaves the circle a fraction of a pixel from the middle it is meant to turn about.
    /// </remarks>
    [Fact]
    public void AVirtualRingIsCentredOnItsIcon()
    {
        var directory = Scratch.New("virtual-ring-centre");
        var path = Path.Combine(directory, "growing.psd");

        VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Pending);

        var entry = new Entry(path, EntryKind.File);
        var icon = Icons.For(entry, 64, Icons.CompactMark);
        var window = new Window { Content = icon, Width = 200, Height = 200 };

        window.Show();

        try
        {
            var ring = Until(
                window,
                () => icon.GetVisualDescendants().OfType<VirtualRing>().FirstOrDefault()
            );
            var centre = ring.TranslatePoint(
                new Point(ring.Bounds.Width / 2, ring.Bounds.Height / 2),
                icon
            );

            Assert.NotNull(centre);
            Assert.Equal(32, centre.Value.X, 3);
            Assert.Equal(32, centre.Value.Y, 3);
            Assert.Equal(RelativePoint.Center, ring.RenderTransformOrigin);
            Assert.IsType<RotateTransform>(ring.RenderTransform);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A picture the renderer cannot resize is stamped at its own size rather than crashing.</summary>
    [Fact]
    public void APictureTheRendererCannotResizeIsStillStamped()
    {
        using var picture = new WriteableBitmap(
            new PixelSize(32, 32),
            new Vector(96, 96),
            PixelFormat.Bgra8888,
            AlphaFormat.Premul
        );

        var stamp = Icons.Glyph(picture, PaletteKeys.Del, 16);

        Assert.Equal(16, stamp.Width);
    }

    /// <summary>A listing's directory and the location it is read through.</summary>
    /// <param name="directory">Where the listing is.</param>
    /// <returns>The host and the location.</returns>
    private static (IPluginHost Host, Browser Browser) Client(string directory)
    {
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);

        return (host, browser);
    }

    /// <summary>The listing a directory is read as.</summary>
    /// <param name="host">The host.</param>
    /// <param name="browser">The location.</param>
    /// <returns>The listing.</returns>
    private static Files FilesOf(IPluginHost host, Browser browser) =>
        new(host, browser, new BrowserActions(host, browser, new Clip(host.Log)), new Clip(host.Log));

    /// <summary>What a listing shows, by the last part of each path.</summary>
    /// <param name="browser">The location.</param>
    /// <returns>The names, in the order they are shown.</returns>
    private static string[] Names(Browser browser) =>
        [.. browser.Shown.Select(entry => Path.GetFileName(entry.Path))];

    /// <summary>A window holding the tree rooted where a directory is.</summary>
    /// <param name="directory">Where the tree is rooted.</param>
    /// <returns>The window, which the caller closes.</returns>
    private static Window OpenAt(string directory)
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
            Width = 600,
            Height = 500,
        };

        window.Show();
        window.Activate();

        return window;
    }

    /// <summary>The step the tree in a window is rooted at, or nothing while the tree is not drawn yet.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <returns>The root step.</returns>
    private static TreeViewItem? Root(Window window) =>
        window.GetVisualDescendants()
            .OfType<TreeView>()
            .SelectMany(tree => tree.Items.OfType<TreeViewItem>())
            .FirstOrDefault(step => step.Bounds.Height > 0);

    /// <summary>The row standing for a path, once it is drawn.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <param name="path">The path the row stands for.</param>
    /// <returns>The row, or nothing while it is not drawn.</returns>
    private static TreeViewItem? Row(Window window, string path) =>
        window.GetVisualDescendants()
            .OfType<TreeViewItem>()
            .FirstOrDefault(item => string.Equals(item.Tag as string, path, StringComparison.Ordinal));

    /// <summary>Waits for something a window draws, or fails the test when it never is.</summary>
    /// <typeparam name="T">What is waited for.</typeparam>
    /// <param name="window">The window to draw.</param>
    /// <param name="find">How it is found, or nothing while it is not drawn.</param>
    /// <returns>What was found.</returns>
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
}

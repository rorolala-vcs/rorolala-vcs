using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;
using ShapePath = Avalonia.Controls.Shapes.Path;

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
        EntryColourProviders.Clear();
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

    /// <summary>A chosen row is one rounded wash, with no second fill inside it.</summary>
    [Fact]
    public void AChosenRowIsOneRoundedWash()
    {
        var directory = Scratch.New("tree-selected");

        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        var (window, _) = OpenAt(directory, width: 400);

        try
        {
            AppTheme(window);

            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, Path.Combine(directory, "hero.psd")));

            row.IsSelected = true;
            PumpFrames(window, 20);

            var border = row.GetVisualDescendants()
                .OfType<Border>()
                .FirstOrDefault(one => one.Name == "SelectionBorder");
            var presenter = row.GetVisualDescendants()
                .OfType<ContentPresenter>()
                .FirstOrDefault(one => one.Name == "PART_HeaderPresenter");

            Assert.NotNull(border);
            Assert.NotNull(presenter);
            Assert.Equal(new CornerRadius(5), border!.CornerRadius);
            Assert.True(
                presenter!.Background is null
                    || presenter.Background is ISolidColorBrush { Color.A: 0 },
                "the header presenter paints a second fill inside the chosen row"
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A badge is centred in the step, not in the room the theme leaves under its line.</summary>
    [Fact]
    public void ABadgeIsCentredOnItsRow()
    {
        var directory = Scratch.New("tree-badge-centre");

        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        EntryMarkProviders.Register(new FixedMarks());

        var (window, _) = OpenAt(directory, width: 400);

        try
        {
            AppTheme(window);

            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, Path.Combine(directory, "hero.psd")));
            var badges = Named<StackPanel>(row, "entry-badges");
            var top = badges.TranslatePoint(new Point(0, 0), row)?.Y ?? 0;
            var bottom = row.Bounds.Height - top - badges.Bounds.Height;

            Assert.InRange(Math.Abs(top - bottom), 0, 0.75);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A coloured tree is resized without its lines taking the layout down.</summary>
    [Fact]
    public void AColouredTreeSurvivesResize()
    {
        var directory = Scratch.New("tree-resize");
        var marked = Directory.CreateDirectory(Path.Combine(directory, "marked")).FullName;
        var child = Directory.CreateDirectory(Path.Combine(marked, "child")).FullName;

        File.WriteAllText(Path.Combine(child, "hero.psd"), "x");

        EntryColourProviders.Register(new FixedColour("it.tree.resize.red", marked, Colors.Red));

        var (window, _) = OpenAt(directory, width: 500);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var markedRow = Until(window, () => Row(window, marked));

            markedRow.IsExpanded = true;
            PumpFrames(window, 20);

            for (var width = 240; width <= 640; width += 80)
            {
                window.Width = width;
                PumpFrames(window, 20);
            }
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A row draws one guide line in its indent for every level above it.</summary>
    [Fact]
    public void ARowDrawsOneGuidePerLevel()
    {
        var directory = Scratch.New("tree-guide");
        var nested = Path.Combine(directory, "one", "two");

        Directory.CreateDirectory(nested);
        File.WriteAllText(Path.Combine(nested, "hero.psd"), "x");

        var (window, _) = OpenAt(directory, width: 400);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var one = Until(window, () => Row(window, Path.Combine(directory, "one")));

            one.IsExpanded = true;
            PumpFrames(window, 20);

            var two = Until(window, () => Row(window, Path.Combine(directory, "one", "two")));

            two.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, Path.Combine(nested, "hero.psd")));
            var border = row.GetVisualDescendants()
                .OfType<Border>()
                .FirstOrDefault(one => one.Name == "SelectionBorder");

            Assert.NotNull(border);

            var header = Assert.IsType<Grid>(border!.Child);
            var guides = header.Children
                .OfType<Panel>()
                .FirstOrDefault(one => one.Name == "TreeGuides");

            Assert.NotNull(guides);
            Assert.Equal(3, guides!.Children.Count);
            Assert.Equal(row.Level, guides.Children.Count);
        }
        finally
        {
            window.Close();
        }
    }


    /// <summary>A mark that says what a folder is is worn beside its row, in the badge the others use.</summary>
    [Fact]
    public void ATreeMarkIsWornBesideTheRow()
    {
        var directory = Scratch.New("tree-park");
        var marked = Directory.CreateDirectory(Path.Combine(directory, "marked")).FullName;

        EntryMarkProviders.Register(new FixedPark(marked));

        var (window, _) = OpenAt(directory, width: 400);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var row = Until(window, () => Row(window, marked));
            var strip = Named<StackPanel>(row, "entry-badges");
            var badge = Assert.Single(strip.Children);
            var ring = badge.GetVisualDescendants()
                .OfType<Border>()
                .FirstOrDefault(border => border.BorderBrush is ISolidColorBrush);

            Assert.NotNull(ring);
            Assert.Equal(
                Colors.Red,
                Assert.IsType<SolidColorBrush>(ring!.BorderBrush).Color
            );
            Assert.Empty(
                Named<Control>(row, "entry-icon").GetVisualDescendants().OfType<BadgeArc>()
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A marked folder is one coloured line beside its own row and everything under it.</summary>
    [Fact]
    public void AMarkedFolderIsOneLineBesideItsWholeSubtree()
    {
        var directory = Scratch.New("tree-colour");
        var marked = Directory.CreateDirectory(Path.Combine(directory, "marked")).FullName;
        var child = Directory.CreateDirectory(Path.Combine(marked, "child")).FullName;
        var file = Path.Combine(child, "hero.psd");
        var plain = Directory.CreateDirectory(Path.Combine(directory, "plain")).FullName;

        File.WriteAllText(file, "x");

        EntryColourProviders.Register(new FixedColour("it.tree.row.red", marked, Colors.Red));

        var (window, _) = OpenAt(directory, width: 500);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var markedRow = Until(window, () => Row(window, marked));
            var blocks = window.GetVisualDescendants()
                .OfType<Canvas>()
                .First(canvas => canvas.Name == "TreeBlocks");

            // The folder's own row carries no fill of its own: the mark is the line beside the rows.
            Assert.Equal(0, Wash(markedRow).A);

            var line = Assert.Single(blocks.Children.OfType<Border>());

            Assert.True(line.Width > 2);
            Assert.Equal(Colors.Red.R, Assert.IsType<SolidColorBrush>(line.Background).Color.R);

            markedRow.IsExpanded = true;
            PumpFrames(window, 20);

            line = Assert.Single(blocks.Children.OfType<Border>());

            Assert.Equal(markedRow.Bounds.Height, line.Height, 1);

            var guides = markedRow.GetVisualDescendants()
                .OfType<Panel>()
                .FirstOrDefault(one => one.Name == "TreeGuides");

            Assert.NotNull(guides);
            Assert.False(Assert.IsType<Border>(Assert.Single(guides!.Children)).IsVisible);

            // A marked folder inside another draws its line at its own deeper indent: the two colours stand
            // beside one another rather than one replacing the other.
            EntryColourProviders.Register(new FixedColour("it.tree.row.blue", child, Colors.Blue));

            var childRow = Until(window, () => Row(window, child));

            childRow.IsExpanded = true;
            PumpFrames(window, 20);

            var lines = blocks.Children.OfType<Border>().ToList();

            Assert.Equal(2, lines.Count);

            var outer = Assert.Single(
                lines,
                one => Assert.IsType<SolidColorBrush>(one.Background).Color.R == Colors.Red.R
            );
            var inner = Assert.Single(
                lines,
                one => Assert.IsType<SolidColorBrush>(one.Background).Color.R == Colors.Blue.R
            );

            Assert.True(Canvas.GetLeft(inner) > Canvas.GetLeft(outer));

            var plainRow = Until(window, () => Row(window, plain));

            Assert.Equal(0, Wash(plainRow).A);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The wash a row's header carries.</summary>
    /// <param name="row">The row to ask.</param>
    /// <returns>The wash, or a colour of no strength when the row carries none.</returns>
    private static Color Wash(TreeViewItem row)
    {
        var header = Assert.IsType<Border>(row.Header);

        return header.Background is ISolidColorBrush brush ? brush.Color : default;
    }

    /// <summary>Paints a window with the program's own look, so that a row is measured as it is drawn.</summary>
    /// <param name="window">The window to dress.</param>
    private static void AppTheme(Window window)
    {
        var look = new RorolalaTheme(ThemePalette.DefaultDark, null, WindowStyle.Windows);

        foreach (var style in look.Styles)
        {
            window.Styles.Add(style);
        }

        var dark = Assert.IsAssignableFrom<ResourceDictionary>(
            look.Palette.ThemeDictionaries[ThemeVariant.Dark]
        );
        var resources = new ResourceDictionary();

        foreach (var one in dark)
        {
            resources[one.Key] = one.Value;
        }

        window.Resources = resources;
        window.RequestedThemeVariant = ThemeVariant.Dark;
    }

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

    /// <summary>A colour provider that marks one directory with one colour.</summary>
    private sealed class FixedColour : IEntryColourProvider
    {
        /// <summary>The name the provider is known by.</summary>
        private readonly string _id;

        /// <summary>The directory the provider marks.</summary>
        private readonly string _marked;

        /// <summary>The colour the directory is marked with.</summary>
        private readonly Color _colour;

        /// <summary>Makes a provider that marks one directory.</summary>
        /// <param name="id">The name the provider is known by.</param>
        /// <param name="marked">The directory to mark.</param>
        /// <param name="colour">The colour to mark it with.</param>
        public FixedColour(string id, string marked, Color colour)
        {
            _id = id;
            _marked = marked;
            _colour = colour;
        }

        /// <inheritdoc />
        public string Id => _id;

        /// <inheritdoc />
        public Color? Colour(Entry entry) =>
            entry.Kind is EntryKind.Directory
            && string.Equals(entry.Path, _marked, StringComparison.Ordinal)
                ? _colour
                : null;
    }

    /// <summary>A mark provider that puts one fixed picture on one directory's icon.</summary>
    private sealed class FixedPark : IEntryMarkProvider
    {
        /// <summary>The directory the provider marks.</summary>
        private readonly string _marked;

        /// <summary>Makes a provider that marks one directory.</summary>
        /// <param name="marked">The directory to mark.</param>
        public FixedPark(string marked) => _marked = marked;

        /// <inheritdoc />
        public string Id => "it.tree.row.park";

        /// <inheritdoc />
        public bool Applies(string directory) => true;

        /// <inheritdoc />
        public EntryMark Mark(Entry entry) =>
            entry.Kind is EntryKind.Directory
            && string.Equals(entry.Path, _marked, StringComparison.Ordinal)
                ? new EntryMark(
                    IconKey: Icons.CreateNewFolder,
                    Colour: Colors.Red,
                    Tagged: true
                )
                : new EntryMark();
    }
}

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
using RorolalaDesktop.Theming;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A grid tile's cell: the face it wears, the room between two of them, and how tall one is.
/// </summary>
/// <remarks>
/// A tile is a cell of the grid rather than a card laid on it. What is checked here is the three things that
/// follow from it: the face is the look's own tile wash and square, the cell is as tall as the tallest tile of
/// its line rather than as tall as its own caption, and two cells of one line meet at their edge rather than
/// leaving a strip of the ground between them. The window is drawn at a scaling that is not whole, because
/// that is where a cell a fraction of a pixel wide would be rounded over its neighbour.
/// </remarks>
public sealed class GridCellTests
{
    /// <summary>The scaling the window is drawn at, which is not whole on purpose.</summary>
    private const double Scale = 1.25;

    /// <summary>Begins each test from no provider and no configuration of its own.</summary>
    public GridCellTests()
    {
        DataHome.Clean();
        EntryColourProviders.Clear();
        EntryMarkProviders.Clear();
        ChosenEntry.Clear();
    }

    /// <summary>The chosen cell is one square face, the line's height, meeting the cell beside it.</summary>
    [Fact]
    public void AChosenTileIsTheWholeSquareCell()
    {
        var root = Scratch.New("grid-cell");
        var brief = Path.Combine(root, "alpha.psd");
        var named = Path.Combine(root, "zulu-a-name-that-is-far-too-long-to-stand-under-one-tile.psd");

        File.WriteAllText(brief, "x");
        File.WriteAllText(named, "x");

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
        var window = new Window { Width = 900, Height = 600, Content = control };

        AppTheme(window);

        window.Show();
        window.Activate();

        // The cells are laid on the pixels the window draws at, and a scaling that is not whole is where
        // two cells would otherwise cover one pixel of each other: a face is thin enough to see through,
        // and two of them over one pixel would read as a brighter line between them.
        window.SetRenderScaling(Scale);
        Pump();

        try
        {
            control.OpenAt(root);
            Pump();

            var view = Assert.IsType<GridBrowser<Entry>>(
                window.GetVisualDescendants().OfType<EntryView<Entry>>().First()
            );
            var list = view.GetVisualDescendants().OfType<ListBox>().First();
            var briefItem = Item(list, brief);
            var namedItem = Item(list, named);
            var briefEntry = Assert.IsType<Entry>(briefItem.DataContext);
            var namedEntry = Assert.IsType<Entry>(namedItem.DataContext);

            // The long name wraps, so its tile is the taller of the two, and the line is that height. Both
            // cells take it: a cell is the line's, not its own caption's, and the line is rounded up to
            // the pixels the window draws at so that the row below starts on the edge it ends on.
            var tall = view.HeightOf(namedEntry);

            Assert.True(
                tall > view.HeightOf(briefEntry),
                $"the long tile stands {tall} against {view.HeightOf(briefEntry)}"
            );

            Assert.Equal(briefItem.Bounds.Height, namedItem.Bounds.Height, 0.01);
            Assert.InRange(namedItem.Bounds.Height, tall, tall + (1 / Scale));

            // The room a tile keeps around its picture is inside the cell, and the cell runs to where the
            // next one begins rather than stopping short of it. The two edges are the same pixel: a face is
            // thin enough to see through, and two of them over one pixel would read as a brighter line.
            Assert.Equal(briefItem.Bounds.Right, namedItem.Bounds.X, 0.01);

            Assert.Equal(new CornerRadius(0), briefItem.CornerRadius);
            Assert.Equal(new Thickness(0), briefItem.Margin);
            Assert.Equal(new Thickness(0), briefItem.BorderThickness);

            briefItem.IsSelected = true;
            Settle();

            var presenter = Presenter(briefItem);
            var chosen = Colour(window, PaletteKeys.TileSelection);

            Assert.Equal(chosen, Wash(presenter));

            // The pointer's wash is the answer while the pointer is on a chosen tile: what the pointer is
            // on is the live answer, and a choice is the quiet one behind it.
            var at = briefItem.TranslatePoint(
                new Point(briefItem.Bounds.Width / 2, briefItem.Bounds.Height / 2),
                window
            );

            Assert.NotNull(at);
            window.MouseMove(at.Value);
            Settle();

            Assert.Equal(Colour(window, PaletteKeys.TileHover), Wash(presenter));

            // Away from the tile, the choice is what is left.
            window.MouseMove(new Point(4, 4));
            Settle();

            Assert.Equal(chosen, Wash(presenter));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The item one path is drawn in.</summary>
    /// <param name="list">The grid's list.</param>
    /// <param name="path">The entry's path.</param>
    /// <returns>The item.</returns>
    private static ListBoxItem Item(ListBox list, string path) =>
        Assert.IsType<ListBoxItem>(
            list.ContainerFromItem(
                Assert.Single(list.Items.OfType<Entry>(), item => item.Path == path)
            )
        );

    /// <summary>The surface a tile's cell is drawn on.</summary>
    /// <param name="item">The item whose cell it is.</param>
    /// <returns>The presenter the wash is painted on.</returns>
    private static ContentPresenter Presenter(ListBoxItem item) =>
        item.GetVisualDescendants()
            .OfType<ContentPresenter>()
            .First(one => one.Name == "PART_ContentPresenter");

    /// <summary>What a surface is washed with.</summary>
    /// <param name="presenter">The surface.</param>
    /// <returns>The colour it is filled with.</returns>
    private static Color Wash(ContentPresenter presenter) =>
        Assert.IsAssignableFrom<ISolidColorBrush>(presenter.Background).Color;

    /// <summary>The colour the look publishes under one of its names.</summary>
    /// <param name="window">The window the look is dressing.</param>
    /// <param name="key">The name to read.</param>
    /// <returns>The colour it is answered with.</returns>
    private static Color Colour(Window window, string key)
    {
        Assert.True(window.TryFindResource(key, ThemeVariant.Dark, out var found), $"the look states no {key}");

        return Assert.IsAssignableFrom<ISolidColorBrush>(found).Color;
    }

    /// <summary>Paints a window with the program's own look, so that a cell is drawn as it is in the program.</summary>
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

    /// <summary>Lets the layout and the drawing catch up.</summary>
    private static void Pump()
    {
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
        Thread.Sleep(10);
    }

    /// <summary>Lets a change and the fade it arrives with finish.</summary>
    private static void Settle()
    {
        for (var at = 0; at < 20; at++)
        {
            Pump();
            Thread.Sleep(15);
        }
    }
}

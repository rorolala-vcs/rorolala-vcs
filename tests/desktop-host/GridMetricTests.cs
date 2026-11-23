using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A tile's size is known before the tile is made, which is what lets a panel lay out what it has not realized.
/// </summary>
/// <remarks>
/// The fixed part comes from one container that was made for real. The part that changes is the name, and
/// it is measured with a block wearing the caption's classes, so the answer is the caption's own. What is
/// checked here is that the prediction and the container agree, for a short name and for one that wraps.
/// </remarks>
public sealed class GridMetricTests
{
    /// <summary>Begins each test from no provider at all.</summary>
    public GridMetricTests() => EntryMarkProviders.Clear();

    /// <summary>The model's height and width match the containers the grid actually made.</summary>
    [Fact]
    public void ACalibratedTilePredictsEveryOtherTilesSize()
    {
        var root = Scratch.New("grid-metric");
        var names = Enumerable
            .Range(0, 20)
            .Select(at =>
                at % 5 == 0
                    ? $"a-name-that-is-far-too-long-to-stand-under-one-tile-{at}.psd"
                    : $"file-{at}.psd"
            )
            .ToArray();

        foreach (var name in names)
        {
            File.WriteAllText(Path.Combine(root, name), "x");
        }

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var clip = new Clip(host.Log);
        var files = new Files(host, browser, new BrowserActions(host, browser, clip), clip);
        var grid = new GridBrowser<Entry>(host, files, 64);
        var window = new Window { Width = 900, Height = 600, Content = grid };

        window.Show();

        try
        {
            Pump();

            var items = grid.GetVisualDescendants().OfType<ListBoxItem>().ToArray();

            Assert.NotEmpty(items);

            grid.Calibrate(items[0], Assert.IsType<Entry>(items[0].DataContext));

            foreach (var item in items)
            {
                var entry = Assert.IsType<Entry>(item.DataContext);

                Assert.True(
                    Math.Abs(item.DesiredSize.Width - grid.TileWidth) <= 0.5,
                    $"{Path.GetFileName(entry.Path)} width {item.DesiredSize.Width} against {grid.TileWidth}"
                );
                Assert.True(
                    Math.Abs(item.DesiredSize.Height - grid.HeightOf(entry)) <= 0.5,
                    $"{Path.GetFileName(entry.Path)} height {item.DesiredSize.Height} against {grid.HeightOf(entry)}"
                );
            }
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Lets the layout and the drawing catch up.</summary>
    private static void Pump()
    {
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
        Thread.Sleep(10);
    }
}

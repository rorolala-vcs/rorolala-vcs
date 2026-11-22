using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A grid tile's name: wrapped rather than cut off, and the tile and its line grown by the lines the name takes.
/// </summary>
/// <remarks>
/// A name cut short is a name the reader has to open the entry to read, so what does not fit under the picture is
/// carried onto the next line. What that costs is height, and it is paid where it is owed: the tile is as tall as
/// its name needs, and the line it stands on is as tall as its tallest tile, which is checked here by the two
/// tiles of one line being of two heights and standing on one height.
/// </remarks>
public sealed class GridCaptionTests
{
    /// <summary>Begins each test from no provider and no configuration of its own.</summary>
    public GridCaptionTests()
    {
        DataHome.Clean();
        EntryColourProviders.Clear();
        EntryMarkProviders.Clear();
        ChosenEntry.Clear();
    }

    /// <summary>A name too long for the tile's width is wrapped, and its tile is taller for it.</summary>
    [Fact]
    public void ALongNameWrapsAndMakesItsTileTaller()
    {
        var root = Scratch.New("grid-caption");
        var brief = Path.Combine(root, "a.psd");
        var named = Path.Combine(root, "a-name-that-is-far-too-long-to-stand-under-one-tile.psd");

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

        window.Show();
        window.Activate();
        Pump();

        try
        {
            control.OpenAt(root);
            Pump();

            var view = window.GetVisualDescendants().OfType<EntryView<Entry>>().First();
            var list = view.GetVisualDescendants().OfType<ListBox>().First();

            var captions = window
                .GetVisualDescendants()
                .OfType<TextBlock>()
                .Where(text => text.Classes.Contains("entry-name") && text.Classes.Contains("caption"))
                .ToList();

            var briefCaption = Assert.Single(captions, text => text.Text == "a.psd");
            var namedCaption = Assert.Single(captions, text => text.Text == Path.GetFileName(named));

            // What does not fit on the line is carried onto the next rather than cut off, whatever it takes.
            Assert.Equal(TextWrapping.Wrap, namedCaption.TextWrapping);
            Assert.Equal(TextTrimming.None, namedCaption.TextTrimming);
            Assert.True(
                namedCaption.Bounds.Height > briefCaption.Bounds.Height + 1,
                $"the long name stands on {namedCaption.Bounds.Height} against {briefCaption.Bounds.Height}"
            );

            // The tile is as tall as its name needs, so the item it is drawn in is taller as well, and the two
            // still stand on one line, which is what makes the whole line taller rather than the one tile.
            var briefItem = Item(list, brief);
            var namedItem = Item(list, named);

            Assert.True(
                namedItem.Bounds.Height > briefItem.Bounds.Height,
                $"the long tile stands {namedItem.Bounds.Height} against {briefItem.Bounds.Height}"
            );
            Assert.Equal(briefItem.Bounds.Y, namedItem.Bounds.Y, 0.5);
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

    /// <summary>Lets the layout and the drawing catch up.</summary>
    private static void Pump()
    {
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
        Thread.Sleep(10);
    }
}

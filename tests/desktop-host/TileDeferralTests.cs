using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A grid tile builds its picture only when it comes near the viewport.
/// </summary>
/// <remarks>
/// The grid realizes a tile for every entry it lists. What a tile asks for when it is built, a preview
/// and a provider's badges, is the expensive half, and a directory of a few thousand entries would ask
/// for all of it before its first frame. What is checked here is that a tile far from the viewport
/// holds a placeholder rather than a picture, and that scrolling builds the ones that arrive.
/// </remarks>
public sealed class TileDeferralTests
{
    /// <summary>Begins each test from no provider at all.</summary>
    public TileDeferralTests() => EntryMarkProviders.Clear();

    /// <summary>A grid of four hundred entries builds only the pictures near the viewport.</summary>
    [Fact]
    public void OnlyTilesNearTheViewportBuildTheirPicture()
    {
        var root = Scratch.New("tile-deferral");
        var entries = Enumerable
            .Range(0, 400)
            .Select(at => new Entry(Path.Combine(root, $"file-{at:000}.psd"), EntryKind.File))
            .ToArray();

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var listing = new CountingListing(host, entries);
        var grid = new GridBrowser<Entry>(host, listing, 64);
        var window = new Window { Width = 900, Height = 600, Content = grid };

        window.Show();

        try
        {
            Pump();

            Assert.True(listing.MarkedCount > 0, "nothing was drawn");
            Assert.True(
                listing.MarkedCount < 240,
                $"{listing.MarkedCount} pictures were built for a first screen of {entries.Length} entries"
            );

            var scroll = grid.GetVisualDescendants().OfType<ScrollViewer>().First();
            var before = listing.MarkedCount;

            scroll.Offset = new Vector(0, Math.Max(0, scroll.Extent.Height - scroll.Viewport.Height));
            Pump();

            Assert.True(
                listing.MarkedCount > before,
                "scrolling to the end built no picture"
            );
            Assert.True(
                listing.MarkedCount < entries.Length / 2,
                $"{listing.MarkedCount} pictures were built of {entries.Length} after scrolling to the end"
            );
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

    /// <summary>A listing of entries whose pictures are counted rather than drawn.</summary>
    private sealed class CountingListing(IPluginHost host, IReadOnlyList<Entry> items)
        : Listing<Entry>(host, items)
    {
        /// <summary>How many tiles were asked for a picture.</summary>
        public int MarkedCount { get; private set; }

        /// <inheritdoc />
        public override object Key(Entry item) => item.Path;

        /// <inheritdoc />
        public override string Name(Entry item) => Path.GetFileName(item.Path);

        /// <inheritdoc />
        public override Control Icon(Entry item, int size) => Picture(size);

        /// <inheritdoc />
        public override Control Marked(Entry item, int size)
        {
            MarkedCount++;

            return Picture(size);
        }

        /// <inheritdoc />
        public override void Open(Entry item) { }

        /// <inheritdoc />
        public override void Fill(
            ContextMenu menu,
            Control from,
            IReadOnlyList<Entry> chosen,
            bool force,
            string? words = null
        ) { }

        /// <summary>An empty picture of the size a tile asks for.</summary>
        /// <param name="size">How wide and tall it is.</param>
        /// <returns>The picture.</returns>
        private static Control Picture(int size) => new Border { Width = size, Height = size };
    }
}

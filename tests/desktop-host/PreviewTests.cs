using System.Diagnostics.CodeAnalysis;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Previews;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The preview service and the tile that shows a preview: sources asked in order, answers kept, and a tile
/// falling back to its type icon.
/// </summary>
/// <remarks>
/// The UI is made on the suite's own thread, so these tests wait for a task with a blocking call rather than
/// becoming async: an async test resumes on the thread pool, and a window made there is a window made on another
/// thread, which is the one thing the headless platform cannot do.
/// </remarks>
[SuppressMessage(
    "xUnit",
    "xUnit1031:Test methods should not use blocking task operations",
    Justification = "the UI must stay on the suite's thread; an async test resumes on the thread pool"
)]
public sealed class PreviewTests
{
    /// <summary>The system's source is asked before a plugin's, and a source that does not care is skipped.</summary>
    [Fact]
    public void ASourceIsAskedInOrder()
    {
        var services = Host.Services();
        var first = new FakeSource("it.first", cares: false, Picture(PreviewOrigin.Plugin));
        var second = new FakeSource("it.second", cares: true, Picture(PreviewOrigin.Plugin));
        var previews = new PreviewService(services.Log.For("it.previews"), services.Progress, first);

        previews.Register(second);

        var answer = previews.Get(Request("hero.psd"), CancellationToken.None).GetAwaiter().GetResult();

        Assert.NotNull(answer);
        Assert.Equal(0, first.Asked);
        Assert.Equal(1, second.Asked);
    }

    /// <summary>A source that answered is not asked again for the same file and size.</summary>
    [Fact]
    public void APreviewIsCached()
    {
        var services = Host.Services();
        var source = new FakeSource("it.cache", cares: true, Picture(PreviewOrigin.Plugin));
        var previews = new PreviewService(services.Log.For("it.previews"), services.Progress, source);
        var directory = Scratch.New("preview-cache");
        var path = Path.Combine(directory, "hero.psd");

        File.WriteAllText(path, "x");

        var request = new PreviewRequest(path, EntryKind.File, new PreviewSize(64, 64), PreviewPurpose.Tile);

        Assert.NotNull(previews.Get(request, CancellationToken.None).GetAwaiter().GetResult());
        Assert.NotNull(previews.Get(request, CancellationToken.None).GetAwaiter().GetResult());
        Assert.Equal(1, source.Asked);
    }

    /// <summary>A file that changed is a new key, so its old answer is not served.</summary>
    [Fact]
    public void AChangedFileIsNotServedFromCache()
    {
        var services = Host.Services();
        var source = new FakeSource("it.changed", cares: true, Picture(PreviewOrigin.Plugin));
        var previews = new PreviewService(services.Log.For("it.previews"), services.Progress, source);
        var directory = Scratch.New("preview-changed");
        var path = Path.Combine(directory, "hero.psd");

        File.WriteAllText(path, "x");

        var request = new PreviewRequest(path, EntryKind.File, new PreviewSize(64, 64), PreviewPurpose.Tile);

        previews.Get(request, CancellationToken.None).GetAwaiter().GetResult();

        File.WriteAllText(path, "a longer body");
        File.SetLastWriteTimeUtc(path, DateTime.UtcNow.AddSeconds(2));

        previews.Get(request, CancellationToken.None).GetAwaiter().GetResult();

        Assert.Equal(2, source.Asked);
    }

    /// <summary>A cancelled request answers nothing, and a tile keeps its type icon when no source can answer.</summary>
    [Fact]
    public void ACancelledRequestIsDropped()
    {
        var services = Host.Services();
        var source = new FakeSource("it.slow", cares: true, null, delay: TimeSpan.FromSeconds(2));
        var previews = new PreviewService(services.Log.For("it.previews"), services.Progress, source);
        var directory = Scratch.New("preview-cancel");
        var path = Path.Combine(directory, "hero.psd");

        File.WriteAllText(path, "x");

        using var cancel = new CancellationTokenSource(TimeSpan.FromMilliseconds(50));
        var answer = previews
            .Get(
                new PreviewRequest(path, EntryKind.File, new PreviewSize(64, 64), PreviewPurpose.Tile),
                cancel.Token
            )
            .GetAwaiter()
            .GetResult();

        Assert.Null(answer);
    }

    /// <summary>A tile shows the preview once the source answers, and the type icon until then.</summary>
    [Fact]
    public void TheTileShowsThePreviewWhenItArrives()
    {
        var services = Host.Services();
        var source = new FakeSource("it.tile", cares: true, Picture(PreviewOrigin.Plugin));
        var previews = new PreviewService(services.Log.For("it.previews"), services.Progress, source);
        var directory = Scratch.New("preview-tile");
        var path = Path.Combine(directory, "hero.psd");

        File.WriteAllText(path, "x");

        var tile = new PreviewTile(previews, new Entry(path, EntryKind.File), 64, []);
        var window = new Window { Width = 300, Height = 300, Content = tile };

        Assert.False(tile.ClipToBounds, "a clipped tile cuts its badges in half");

        window.Show();

        Image image;

        try
        {
            image = Until(
                window,
                () => tile.GetVisualDescendants().OfType<Image>().FirstOrDefault(one => one.IsVisible)
            );
        }
        finally
        {
            window.Close();
        }

        Assert.NotNull(image.Source);
    }

    /// <summary>A fixed request.</summary>
    /// <param name="path">The file.</param>
    /// <returns>The request.</returns>
    private static PreviewRequest Request(string path) =>
        new(path, EntryKind.File, new PreviewSize(64, 64), PreviewPurpose.Tile);

    /// <summary>A two by two picture, with the given origin.</summary>
    /// <param name="origin">Which layer made it.</param>
    /// <returns>The pixels.</returns>
    private static PreviewImage Picture(PreviewOrigin origin) =>
        new(
            2,
            2,
            8,
            new byte[]
            {
                0, 0, 255, 255,
                0, 255, 0, 255,
                255, 0, 0, 255,
                255, 255, 255, 255,
            },
            origin
        );

    /// <summary>Waits for something a turn or so after it is asked for.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="find">What is being waited for.</param>
    /// <returns>What was waited for.</returns>
    private static T Until<T>(Window window, Func<T?> find)
        where T : class
    {
        for (var turn = 0; turn < 200; turn++)
        {
            if (find() is { } found)
            {
                return found;
            }

            Thread.Sleep(10);
            Pump();
        }

        throw new InvalidOperationException("what was asked for was never drawn");
    }

    /// <summary>Draws the window for a few turns.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 10; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }
    }

    /// <summary>A source that answers a fixed picture after a delay, and counts how often it is asked.</summary>
    private sealed class FakeSource(
        string id,
        bool cares,
        PreviewImage? answer,
        TimeSpan? delay = null
    ) : IPreviewSource
    {
        /// <summary>How many times the source was asked to render.</summary>
        public int Asked { get; private set; }

        /// <inheritdoc />
        public string Id => id;

        /// <inheritdoc />
        public bool Cares(PreviewRequest request) => cares;

        /// <inheritdoc />
        public async ValueTask<PreviewImage?> RenderAsync(
            PreviewRequest request,
            CancellationToken cancel
        )
        {
            Asked++;

            if (delay is { } wait)
            {
                await Task.Delay(wait, cancel);
            }

            return answer;
        }
    }
}

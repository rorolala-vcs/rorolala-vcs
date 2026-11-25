using System.Buffers.Binary;
using System.Diagnostics.CodeAnalysis;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Headless;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using SightPlugin;
using Sight = SightPlugin.SightPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Sight dock: the previewers chosen for a file, what each reads, and what the dock draws for
/// the File System's choice.
/// </summary>
/// <remarks>
/// The plugin depends on the File System plugin and nothing else, so what is exercised here is a real
/// plugin start-up over a real host and real files. The UI is made on the suite's own thread, so a
/// test waits for a task with a blocking call rather than becoming async: an async test resumes on
/// the thread pool, and a window made there is a window made on another thread.
/// </remarks>
[SuppressMessage(
    "xUnit",
    "xUnit1031:Test methods should not use blocking task operations",
    Justification = "the UI must stay on the suite's thread; an async test resumes on the thread pool"
)]
public sealed class SightTests
{
    /// <summary>The highest rank is tried first, and a tie keeps registration order.</summary>
    [Fact]
    public void TheListIsOrderedByRankAndThenByRegistration()
    {
        Ready();

        var lowFirst = new FakePreviewer("it.sight.order.a", 10, "itorder");
        var lowSecond = new FakePreviewer("it.sight.order.b", 10, "itorder");
        var high = new FakePreviewer("it.sight.order.c", 20, "itorder");

        SightPreviewers.Register(lowFirst);
        SightPreviewers.Register(lowSecond);
        SightPreviewers.Register(high);

        var ordered = SightPreviewers.Ordered(Sample("cube.itorder", "itorder"), null);

        Assert.Equal(
            new[] { "it.sight.order.c", "it.sight.order.a", "it.sight.order.b" },
            ordered.Select(previewer => previewer.Id).ToArray()
        );
    }

    /// <summary>A forced previewer is the only one asked, and Auto asks every one that cares.</summary>
    [Fact]
    public void AForcedPreviewerIsAskedAlone()
    {
        var host = Ready();

        SightPreviewers.Register(new FakePreviewer("it.sight.forced.one", 1, "itforced"));
        SightPreviewers.Register(new FakePreviewer("it.sight.forced.two", 99, "itforced"));

        var controller = new SightController(host, new SightView(host));
        var file = Sample("blob.itforced", "itforced");

        controller.Choose("it.sight.forced.one");

        var forced = controller.Candidates(file, null);

        Assert.Single(forced);
        Assert.Equal("it.sight.forced.one", forced[0].Id);

        controller.Choose(null);

        Assert.Equal(2, controller.Candidates(file, null).Count);
    }

    /// <summary>Text is read as lines, a NUL refuses it, and a file past the limit is not read.</summary>
    [Fact]
    public void TextIsReadAsLinesAndRefusesBinary()
    {
        var host = Ready();
        var directory = Scratch.New("sight-text");
        var text = Path.Combine(directory, "notes.txt");

        File.WriteAllText(text, "alpha\nbeta\n");

        var document = Assert.IsType<SightTextDocument>(Open(new SightText(), SightFile.Read(text), null, host));

        Assert.Equal(new[] { "alpha", "beta", "" }, document.Lines);

        var binary = Path.Combine(directory, "blob.txt");

        File.WriteAllBytes(binary, [0x41, 0x00, 0x42]);

        Assert.Null(Open(new SightText(), SightFile.Read(binary), null, host));

        var huge = new SightFile(
            Path.Combine(directory, "huge.txt"),
            SightSettings.TextLimitBytes + 1,
            "txt",
            "huge.txt",
            0
        );
        var tooLarge = Assert.IsType<SightTooLargeDocument>(Open(new SightText(), huge, null, host));

        Assert.Equal(SightSettings.TextLimitBytes, tooLarge.Limit);
    }

    /// <summary>A diff counts the lines it added and removed, and keeps the lines around them.</summary>
    [Fact]
    public void ADiffCountsWhatChangedAndKeepsItsContext()
    {
        var host = Ready();
        var directory = Scratch.New("sight-diff");
        var left = Path.Combine(directory, "left.txt");
        var right = Path.Combine(directory, "right.txt");

        File.WriteAllText(left, "one\ntwo\nthree\n");
        File.WriteAllText(right, "one\nTWO\nthree\n");

        var document = Assert.IsType<SightDiffDocument>(
            Open(new SightDiff(), SightFile.Read(left), SightFile.Read(right), host)
        );

        Assert.Equal(1, document.Added);
        Assert.Equal(1, document.Removed);
        Assert.Contains(document.Lines, line => line.Kind == SightDiffKind.Hunk);
        Assert.Contains(
            document.Lines,
            line => line.Kind == SightDiffKind.Added && string.Equals(line.Text, "TWO", StringComparison.Ordinal)
        );
        Assert.Contains(
            document.Lines,
            line => line.Kind == SightDiffKind.Removed && string.Equals(line.Text, "two", StringComparison.Ordinal)
        );
        Assert.Contains(
            document.Lines,
            line => line.Kind == SightDiffKind.Context && string.Equals(line.Text, "one", StringComparison.Ordinal)
        );
    }

    /// <summary>An OBJ cube is read as eight vertices and twelve triangles, and renders opaque pixels.</summary>
    [Fact]
    public void AnObjCubeIsReadAndRendersOpaquePixels()
    {
        var mesh = Assert.IsType<SightMesh>(SightModel.Read(Cube()));

        Assert.Equal(8, mesh.Vertices.Count);
        Assert.Equal(12, mesh.TriangleCount);

        var pixels = SightModelRenderer.Render(mesh, 0.7f, 0.35f, 3.2f, 96, 96, grid: true);

        Assert.Equal(96 * 96 * 4, pixels.Length);
        Assert.Contains(
            Enumerable.Range(0, pixels.Length / 4),
            at => pixels[(at * 4) + 3] == 255
        );
    }

    /// <summary>A PSD header becomes the fields the corner shows, and a composite it cannot decode leaves the picture out.</summary>
    [Fact]
    public void APsdHeaderBecomesItsMetadata()
    {
        var host = Ready();
        var directory = Scratch.New("sight-psd");
        var path = Path.Combine(directory, "hero.psd");

        File.WriteAllBytes(path, PsdHeader(width: 16, height: 8, channels: 3, depth: 8, mode: 3));

        var file = SightFile.Read(path);
        var psd = Assert.IsType<SightPsdDocument>(Open(new SightPsd(), file, null, host));

        Assert.Equal(16, psd.Width);
        Assert.Equal(8, psd.Height);
        Assert.Equal(3, psd.Channels);
        Assert.Equal(8, psd.Depth);
        Assert.Equal("RGB", psd.Mode);
        Assert.Null(psd.Picture);

        var items = SightMetadataProviders.Read(new SightMetadataContext(file, null, psd, new SightPsd()));

        Assert.Contains(
            items,
            item => string.Equals(item.LabelKey, "rorolala_sight.meta.channels", StringComparison.Ordinal)
                && string.Equals(item.Value, "3", StringComparison.Ordinal)
        );
        Assert.Contains(
            items,
            item => string.Equals(item.LabelKey, "rorolala_sight.meta.mode", StringComparison.Ordinal)
                && string.Equals(item.Value, "RGB", StringComparison.Ordinal)
        );
    }

    /// <summary>Every provider is asked, in the order it was registered.</summary>
    [Fact]
    public void MetadataProvidersAreReadInRegistrationOrder()
    {
        Ready();

        SightMetadataProviders.Register(new FakeMetadata("it.sight.meta.a", "it.sight.meta.a.label", "a"));
        SightMetadataProviders.Register(new FakeMetadata("it.sight.meta.b", "it.sight.meta.b.label", "b"));

        var items = SightMetadataProviders.Read(
            new SightMetadataContext(new SightFile("x.bin", 0, "bin", "x.bin", 0), null, null, null)
        );

        var labels = items.Select(item => item.LabelKey).ToList();

        Assert.True(labels.IndexOf("it.sight.meta.a.label") >= 0);
        Assert.True(
            labels.IndexOf("it.sight.meta.a.label") < labels.IndexOf("it.sight.meta.b.label"),
            "the providers were not read in registration order"
        );
    }

    /// <summary>
    /// The previewer button is the same plain toggle the Version Graph puts in its corner.
    /// </summary>
    /// <remarks>
    /// The base theme dresses a control by its exact type, so a subclass would not wear the toggled,
    /// rectangular, bordered look the graph's corner buttons wear. This checks the control handed to
    /// the corner is a <see cref="ToggleButton"/> itself.
    /// </remarks>
    [Fact]
    public void ThePreviewerButtonIsAPlainToggle()
    {
        var host = Ready();
        var strip = new SightPreviewerStrip(host, _ => { });

        Assert.Equal(typeof(ToggleButton), strip.Button.GetType());
        Assert.Contains("tool", strip.Button.Classes);
        Assert.Equal(6, strip.Button.Padding.Top);
    }

    /// <summary>The picture's tools turn it by quarter steps and put the view back.</summary>
    [Fact]
    public void ThePictureToolsTurnItAndPutTheViewBack()
    {
        var host = Ready();

        using var picture = new WriteableBitmap(
            new PixelSize(8, 4),
            new Vector(96, 96),
            PixelFormat.Bgra8888,
            AlphaFormat.Unpremul
        );
        var stage = new SightImageStage(host, picture);

        stage.TurnBy(90);
        Assert.Equal(90, stage.RotationDegrees);

        stage.TurnBy(90);
        Assert.Equal(180, stage.RotationDegrees);

        stage.TurnBy(-90);
        Assert.Equal(90, stage.RotationDegrees);

        stage.Upright();
        Assert.Equal(0, stage.RotationDegrees);

        stage.TurnBy(90);
        stage.ResetView();

        Assert.Equal(1, stage.Zoom);
        Assert.Equal(0, stage.RotationDegrees);
    }

    /// <summary>The list holds Auto, then every previewer offered now.</summary>
    [Fact]
    public void TheListHoldsAutoAndEveryPreviewer()
    {
        var host = Ready();
        var strip = new SightPreviewerStrip(host, _ => { });

        strip.Refresh(Sample("notes.txt", "txt"), null, null);

        var menu = strip.BuildMenu();

        Assert.Equal(SightPreviewers.All.Count + 1, menu.Children.Count);
        Assert.All(menu.Children, child => Assert.IsType<Button>(child));
    }

    /// <summary>The dock says nothing is chosen once the File System's choice is taken away.</summary>
    [Fact]
    public void TheDockDrawsTheEmptyStateWhenTheChoiceIsCleared()
    {
        var host = Ready();
        var directory = Scratch.New("sight-dock");
        var path = Path.Combine(directory, "notes.txt");

        File.WriteAllText(path, "alpha\nbeta\n");

        ChosenFiles.Clear();
        ChosenEntry.Publish(new Entry(path, EntryKind.File));

        var view = new SightView(host);
        var window = new Window { Content = view };

        try
        {
            window.Show();
            Pump();

            Assert.True(ShowsPreview(view), "the chosen file was never drawn");

            ChosenEntry.Clear();
            Pump();

            Assert.False(ShowsPreview(view), "the preview was left on screen after the choice was cleared");
            Assert.True(ShowsEmpty(view), "the dock did not say nothing is chosen");
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Starts the plugin over a fresh host, so that its previewers and metadata are offered.</summary>
    /// <returns>The host the plugin was started with.</returns>
    private static IPluginHost Ready()
    {
        DataHome.Clean();

        var services = Host.Services();
        var host = services.For(new PluginId(Sight.Identity), 0);

        new Sight().Initialize(host);

        return host;
    }

    /// <summary>Opens one file through one previewer, on the suite's thread.</summary>
    private static SightDocument? Open(
        ISightPreviewer previewer,
        SightFile primary,
        SightFile? second,
        IPluginHost host
    ) =>
        previewer
            .OpenAsync(new SightOpenRequest(primary, second, host), CancellationToken.None)
            .AsTask()
            .GetAwaiter()
            .GetResult();

    /// <summary>A file a previewer is asked about, whose path need not exist.</summary>
    private static SightFile Sample(string name, string extension) =>
        new(Path.Combine(Scratch.New("sight-file"), name), 4, extension, name, 0);

    /// <summary>Whether the dock is drawing a preview rather than a notice.</summary>
    private static bool ShowsPreview(SightView view) =>
        !ShowsEmpty(view) && view.GetVisualDescendants().OfType<TextBlock>().Any();

    /// <summary>Whether the dock is drawing the answer that nothing is chosen.</summary>
    private static bool ShowsEmpty(SightView view) =>
        view.GetVisualDescendants()
            .OfType<TextBlock>()
            .Any(block =>
                string.Equals(
                    block.Text,
                    RolaI18N.Get("rorolala_sight.empty.title"),
                    StringComparison.Ordinal
                )
            );

    /// <summary>Lets the toolkit run the work posted back to its thread.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 40; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>An OBJ cube, as six quads over eight vertices.</summary>
    private static string[] Cube() =>
        [
            "v -1 -1 -1",
            "v 1 -1 -1",
            "v 1 1 -1",
            "v -1 1 -1",
            "v -1 -1 1",
            "v 1 -1 1",
            "v 1 1 1",
            "v -1 1 1",
            "f 1 2 3 4",
            "f 5 8 7 6",
            "f 1 5 6 2",
            "f 2 6 7 3",
            "f 3 7 8 4",
            "f 5 1 4 8",
        ];

    /// <summary>A PSD header and three empty sections, holding no image data.</summary>
    private static byte[] PsdHeader(int width, int height, ushort channels, ushort depth, ushort mode)
    {
        var bytes = new byte[40];

        bytes[0] = (byte)'8';
        bytes[1] = (byte)'B';
        bytes[2] = (byte)'P';
        bytes[3] = (byte)'S';

        BinaryPrimitives.WriteUInt16BigEndian(bytes.AsSpan(4), 1);
        BinaryPrimitives.WriteUInt16BigEndian(bytes.AsSpan(12), channels);
        BinaryPrimitives.WriteUInt32BigEndian(bytes.AsSpan(14), (uint)height);
        BinaryPrimitives.WriteUInt32BigEndian(bytes.AsSpan(18), (uint)width);
        BinaryPrimitives.WriteUInt16BigEndian(bytes.AsSpan(22), depth);
        BinaryPrimitives.WriteUInt16BigEndian(bytes.AsSpan(24), mode);

        // The three section lengths and the compression are zero already, which is what makes the
        // composite absent rather than decoded.
        return bytes;
    }

    /// <summary>A previewer that cares for the extensions it was made for and answers nothing.</summary>
    private sealed class FakePreviewer : ISightPreviewer
    {
        /// <summary>The extensions it cares for.</summary>
        private readonly IReadOnlyList<string> _extensions;

        /// <summary>Makes a previewer known by one id.</summary>
        /// <param name="id">The stable value.</param>
        /// <param name="rank">How early it is tried.</param>
        /// <param name="extensions">The extensions it cares for.</param>
        public FakePreviewer(string id, int rank, params string[] extensions)
        {
            Id = id;
            Rank = rank;
            _extensions = extensions;
        }

        /// <inheritdoc />
        public string Id { get; }

        /// <inheritdoc />
        public string NameKey => "it.sight.name";

        /// <inheritdoc />
        public string? IconKey => null;

        /// <inheritdoc />
        public int Rank { get; }

        /// <inheritdoc />
        public bool Cares(SightFile primary, SightFile? second) =>
            second is null && _extensions.Contains(primary.Extension, StringComparer.OrdinalIgnoreCase);

        /// <inheritdoc />
        public ValueTask<SightDocument?> OpenAsync(SightOpenRequest request, CancellationToken cancel) =>
            new((SightDocument?)null);
    }

    /// <summary>A provider that always says one thing, so that the order it is read in is visible.</summary>
    private sealed class FakeMetadata : ISightMetadataProvider
    {
        /// <summary>The label it answers with.</summary>
        private readonly string _label;

        /// <summary>The value it answers with.</summary>
        private readonly string _value;

        /// <summary>Makes a provider known by one id.</summary>
        /// <param name="id">The stable value.</param>
        /// <param name="label">The label key.</param>
        /// <param name="value">The value.</param>
        public FakeMetadata(string id, string label, string value)
        {
            Id = id;
            _label = label;
            _value = value;
        }

        /// <inheritdoc />
        public string Id { get; }

        /// <inheritdoc />
        public IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context) =>
            [new SightMetadataItem(_label, _value)];
    }
}

using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace SightPlugin;

/// <summary>
/// The Sight dock: the preview, with a previewer button and an info button in its corner. Nothing
/// else is drawn here.
/// </summary>
/// <remarks>
/// The file name, its path and its size are the File System's, and a listing already says them. What
/// this view adds is what the file is: a picture, text, a diff or a model, and the metadata the
/// registered providers have to say about it. Both tools stand in the preview's top-right corner, as
/// the Version Graph keeps its tools: the previewer on top wears the previewer in force, and the info
/// under it shows the metadata while the pointer rests there.
/// </remarks>
internal sealed class SightView : UserControl
{
    /// <summary>The face text, diffs and model hints are drawn in.</summary>
    private const string Monospace =
        "Cascadia Mono,Consolas,Menlo,DejaVu Sans Mono,SF Mono,monospace";

    /// <summary>How far the camera stands from a model's centre.</summary>
    private const float CameraDistance = 3.2f;

    /// <summary>The host, for the pictures this view draws.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the preview sits in, with the metadata over it.</summary>
    private readonly Grid _stage = new();

    /// <summary>Where the current document is drawn.</summary>
    private readonly Grid _canvas = new();

    /// <summary>The room the corner buttons keep from the top and the right edge.</summary>
    internal const double Corner = 12;

    /// <summary>The table the corner's info button shows.</summary>
    private readonly SightInfoPanel _panel;

    /// <summary>The previewer button in the corner.</summary>
    private readonly SightPreviewerStrip _strip;

    /// <summary>The info button in the corner.</summary>
    private readonly ToggleButton _info = new();

    /// <summary>What follows the File System's choice.</summary>
    private readonly SightController _controller;

    /// <summary>What the preview says, shown while the pointer rests on the info button.</summary>
    private IReadOnlyList<SightMetadataItem> _items = [];

    /// <summary>What is written over the info table.</summary>
    private string _title = string.Empty;

    /// <summary>The model on screen, when the document is one.</summary>
    private SightMesh? _mesh;

    /// <summary>The control the model is drawn in, which is what the pointer orbits.</summary>
    private Control? _meshStage;

    /// <summary>The picture the model is drawn on.</summary>
    private Image? _meshImage;

    /// <summary>The bitmap the model was last drawn into.</summary>
    private WriteableBitmap? _meshPicture;

    /// <summary>The model render in flight, so a drag cancels the one before it.</summary>
    private CancellationTokenSource? _render;

    /// <summary>What the render on screen is of, so a late one does not draw over it.</summary>
    private int _version;

    /// <summary>How far the model is turned about the vertical axis.</summary>
    private float _yaw = 0.7f;

    /// <summary>How far the model is tilted.</summary>
    private float _pitch = 0.35f;

    /// <summary>Where the pointer started the drag, when one is under way.</summary>
    private Point? _drag;

    /// <summary>Makes the view, its corner and its info table.</summary>
    /// <param name="host">The host.</param>
    public SightView(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        _host = host;
        _controller = new SightController(host, this);
        _strip = new SightPreviewerStrip(host, _controller.Choose);
        _panel = new SightInfoPanel(host);

        _info.Classes.Add("tool");
        _info.Padding = new Thickness(6);
        _info.Content = Mark(Icons.Info, 14, PaletteKeys.Front);
        SetInfoTip();
        // A toggle for its dress: the corner tools are drawn as the Version Graph's are, and that look
        // belongs to the toggle rather than to the button. Here the check is the answer it gives, and the
        // table below the preview follows it.
        _info.IsCheckedChanged += (_, _) =>
        {
            SetInfoTip();

            if (_info.IsChecked == true)
            {
                ShowInfo();
            }
            else
            {
                _panel.Hide();
            }
        };

        // The two buttons stand in the corner the Version Graph keeps its own in, one under the
        // other: the previewer on top, the info under it.
        var corner = new StackPanel
        {
            Orientation = Orientation.Vertical,
            Spacing = 6,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, Corner, Corner, 0),
            Children = { _strip.Button, _info },
        };

        _stage.Children.Add(_canvas);
        _stage.Children.Add(corner);

        // The table is drawn after the preview and stands in its bottom-right corner, where it covers
        // the least of the drawing. It takes no pointer: it is read, and the button above is what hides
        // it again.
        _stage.Children.Add(_panel);

        Content = _stage;

        AttachedToVisualTree += (_, _) => _controller.Attach();
        DetachedFromVisualTree += (_, _) => _controller.Detach();
        SizeChanged += (_, _) =>
        {
            if (_mesh is not null)
            {
                RenderModel();
            }
        };
    }

    /// <summary>Draws the previewer button against what is chosen now.</summary>
    /// <param name="primary">The file chosen last, or nothing.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <param name="current">The previewer in force, or nothing for Auto.</param>
    public void RefreshPreviewers(SightFile? primary, SightFile? second, string? current) =>
        _strip.Refresh(primary, second, current);

    /// <summary>Says that nothing is chosen.</summary>
    public void ShowEmpty()
    {
        Reset();
        _canvas.Children.Add(Notice(Icons.Eye, "rorolala_sight.empty.title", "rorolala_sight.empty.hint"));
    }

    /// <summary>Says that the file is being read.</summary>
    public void ShowLoading()
    {
        Reset();

        var panel = new StackPanel
        {
            Spacing = 10,
            MaxWidth = 320,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var words = new TextBlock
        {
            Text = RolaI18N.Get("rorolala_sight.loading"),
            HorizontalAlignment = HorizontalAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
        };
        words[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);
        var bar = new ProgressBar { IsIndeterminate = true, Width = 220 };
        panel.Children.Add(words);
        panel.Children.Add(bar);
        _canvas.Children.Add(panel);
    }

    /// <summary>Says that no previewer could read the file.</summary>
    public void ShowUnsupported()
    {
        Reset();
        _canvas.Children.Add(
            Notice(Icons.File, "rorolala_sight.unsupported.title", "rorolala_sight.unsupported.hint")
        );
    }

    /// <summary>Says that the file could not be read.</summary>
    public void ShowError()
    {
        Reset();
        _canvas.Children.Add(
            Notice(Icons.XCircle, "rorolala_sight.error.title", "rorolala_sight.error.hint")
        );
    }

    /// <summary>Says that the file passes the size a previewer reads under.</summary>
    /// <param name="limit">The limit, in bytes.</param>
    public void ShowTooLarge(long limit)
    {
        Reset();
        _canvas.Children.Add(
            Notice(
                Icons.WarningCircle,
                "rorolala_sight.toolarge.title",
                "rorolala_sight.toolarge.hint",
                Size(limit)
            )
        );
    }

    /// <summary>Draws one document and keeps what the providers said about it for the corner's table.</summary>
    /// <param name="document">The document.</param>
    /// <param name="items">What the providers say about it.</param>
    /// <param name="previewer">The previewer that answered, or nothing when none did.</param>
    public void ShowDocument(
        SightDocument document,
        IReadOnlyList<SightMetadataItem> items,
        ISightPreviewer? previewer
    )
    {
        ArgumentNullException.ThrowIfNull(document);
        ArgumentNullException.ThrowIfNull(items);

        Reset();
        _items = items;
        _title = RolaI18N.Get(previewer?.NameKey ?? "rorolala_sight.previewer.auto");
        _strip.Drawn(previewer?.Id);

        if (_info.IsChecked == true)
        {
            ShowInfo();
        }

        switch (document)
        {
            case SightTextDocument text:
                ShowText(text);

                break;
            case SightDiffDocument diff:
                ShowDiff(diff);

                break;
            case SightImageDocument image:
                ShowPicture(image.Picture);

                break;
            case SightPsdDocument psd when psd.Picture is { } picture:
                ShowPicture(picture);

                break;
            case SightPsdDocument:
                _canvas.Children.Add(
                    Notice(Icons.File, "rorolala_sight.psd.none", "rorolala_sight.psd.note")
                );

                break;
            case SightModelDocument model:
                ShowModel(model.Mesh);

                break;
            case SightTooLargeDocument tooLarge:
                ShowTooLarge(tooLarge.Limit);

                break;
            default:
                ShowUnsupported();

                break;
        }
    }

    /// <summary>Shows the table, when the info button is on and there is something to say.</summary>
    private void ShowInfo()
    {
        if (_items.Count == 0)
        {
            _panel.Hide();

            return;
        }

        _panel.Show(_title, _items);
    }

    /// <summary>Says what pressing the info button would do, which is the other way round when it is on.</summary>
    private void SetInfoTip() =>
        ToolTip.SetTip(
            _info,
            RolaI18N.Get(
                _info.IsChecked == true
                    ? "rorolala_sight.metadata.hide"
                    : "rorolala_sight.metadata.show"
            )
        );

    /// <summary>Lets go of the document on screen and the render behind it.</summary>
    private void Reset()
    {
        _version++;
        _render?.Cancel();
        _render = null;

        if (_meshPicture is not null)
        {
            _meshPicture.Dispose();
            _meshPicture = null;
        }

        _mesh = null;
        _meshImage = null;
        _meshStage = null;
        _drag = null;
        _panel.Hide();
        _items = [];
        _title = string.Empty;
        _canvas.Children.Clear();
    }

    /// <summary>Draws a text document as numbered lines.</summary>
    /// <param name="text">The text.</param>
    private void ShowText(SightTextDocument text)
    {
        var numbers = new StringBuilder();
        var code = new StringBuilder();

        for (var at = 0; at < text.Lines.Count; at++)
        {
            numbers.Append(at + 1).Append('\n');
            code.Append(text.Lines[at]).Append('\n');
        }

        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*"),
            VerticalAlignment = VerticalAlignment.Top,
        };
        var numberAt = new TextBlock
        {
            Text = numbers.ToString(),
            FontFamily = new FontFamily(Monospace),
            FontSize = 12.5,
            LineHeight = 20,
            TextAlignment = TextAlignment.Right,
            Margin = new Thickness(12, 10, 12, 10),
            MinWidth = 44,
        };
        numberAt[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);
        var lineAt = new TextBlock
        {
            Text = code.ToString(),
            FontFamily = new FontFamily(Monospace),
            FontSize = 12.5,
            LineHeight = 20,
            TextWrapping = SightSettings.Wrap ? TextWrapping.Wrap : TextWrapping.NoWrap,
            Margin = new Thickness(0, 10, 16, 10),
        };
        lineAt[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
        grid.Children.Add(numberAt);
        Grid.SetColumn(lineAt, 1);
        grid.Children.Add(lineAt);

        _canvas.Children.Add(
            new ScrollViewer
            {
                Content = grid,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                Background = null,
            }
        );
    }

    /// <summary>Draws a diff as a list of coloured lines.</summary>
    /// <param name="diff">The diff.</param>
    private void ShowDiff(SightDiffDocument diff)
    {
        if (diff.Lines.Count == 0)
        {
            _canvas.Children.Add(
                Notice(Icons.ArrowsLeftRight, "rorolala_sight.diff.same", "rorolala_sight.diff.same_hint")
            );

            return;
        }

        var panel = new StackPanel { Spacing = 0 };

        foreach (var line in diff.Lines)
        {
            panel.Children.Add(DiffLine(line));
        }

        _canvas.Children.Add(
            new ScrollViewer
            {
                Content = panel,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            }
        );
    }

    /// <summary>Draws one diff line, with its sign and its colour.</summary>
    /// <param name="line">The line.</param>
    /// <returns>The control.</returns>
    private static Border DiffLine(SightDiffLine line)
    {
        var sign = new TextBlock
        {
            Text = line.Kind switch
            {
                SightDiffKind.Added => "+",
                SightDiffKind.Removed => "-",
                _ => string.Empty,
            },
            FontFamily = new FontFamily(Monospace),
            FontSize = 12.5,
            LineHeight = 20,
            Width = 40,
            TextAlignment = TextAlignment.Right,
            Margin = new Thickness(0, 0, 10, 0),
        };
        sign[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(
            line.Kind switch
            {
                SightDiffKind.Added => PaletteKeys.Add,
                SightDiffKind.Removed => PaletteKeys.Del,
                _ => PaletteKeys.Muted,
            }
        );
        var text = new TextBlock
        {
            Text = line.Text,
            FontFamily = new FontFamily(Monospace),
            FontSize = 12.5,
            LineHeight = 20,
        };
        text[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(
            line.Kind switch
            {
                SightDiffKind.Added => PaletteKeys.Add,
                SightDiffKind.Removed => PaletteKeys.Del,
                SightDiffKind.Hunk => PaletteKeys.Allow,
                _ => PaletteKeys.Front,
            }
        );
        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*"),
            Margin = new Thickness(12, 0, 16, 0),
        };
        row.Children.Add(sign);
        Grid.SetColumn(text, 1);
        row.Children.Add(text);

        var card = new Border { Child = row };

        if (line.Kind == SightDiffKind.Hunk)
        {
            card[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
            card.Padding = new Thickness(0, 3);
        }
        else if (line.Kind == SightDiffKind.Added)
        {
            card[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.AddBackground);
        }
        else if (line.Kind == SightDiffKind.Removed)
        {
            card[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.DelBackground);
        }

        return card;
    }

    /// <summary>Draws a decoded picture, with the tools that move, enlarge and turn it.</summary>
    /// <param name="picture">The picture.</param>
    private void ShowPicture(Bitmap picture)
    {
        var surface = new SightImageStage(_host, picture);

        _canvas.Children.Add(surface);
        _canvas.Children.Add(ImageTools(surface));
    }

    /// <summary>The tool dock under a picture: the two quarter turns, upright, and the view put back.</summary>
    /// <param name="surface">The stage the tools act on.</param>
    /// <returns>The dock, in the pane's bottom middle.</returns>
    private Control ImageTools(SightImageStage surface)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            Children =
            {
                Tool(Icons.ArrowCounterClockwise, "rorolala_sight.image.rotate_left", () =>
                    surface.TurnBy(-90)
                ),
                Tool(Icons.ArrowClockwise, "rorolala_sight.image.rotate_right", () =>
                    surface.TurnBy(90)
                ),
                Tool(Icons.Compass, "rorolala_sight.image.upright", surface.Upright),
                Tool(Icons.FrameCorners, "rorolala_sight.image.reset", surface.ResetView),
            },
        };

        var dock = new Border
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, 0, 16),
            Padding = new Thickness(4),
            CornerRadius = new CornerRadius(8),
            BorderThickness = new Thickness(1),
            Child = row,
        };
        dock[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);
        dock[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        return dock;
    }

    /// <summary>One tool in a dock: a picture, and what pressing it does.</summary>
    /// <param name="iconKey">The picture's key.</param>
    /// <param name="nameKey">What the tooltip says.</param>
    /// <param name="press">What pressing it does.</param>
    /// <returns>The tool.</returns>
    private Button Tool(string iconKey, string nameKey, Action press)
    {
        var button = new Button { Classes = { "tool" }, Padding = new Thickness(6) };

        button.Content = Mark(iconKey, 14, PaletteKeys.Front);
        ToolTip.SetTip(button, RolaI18N.Get(nameKey));
        button.Click += (_, _) => press();

        return button;
    }

    /// <summary>Draws a model, and lets a drag orbit it.</summary>
    /// <param name="mesh">The mesh.</param>
    private void ShowModel(SightMesh mesh)
    {
        _mesh = mesh;
        _yaw = 0.7f;
        _pitch = 0.35f;

        var image = new Image { Stretch = Stretch.Uniform };
        var hint = new TextBlock
        {
            Text = RolaI18N.Get("rorolala_sight.model.drag"),
            FontSize = 11,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(10),
            IsHitTestVisible = false,
        };
        hint[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        var stage = new Grid();
        stage.Children.Add(image);
        stage.Children.Add(hint);
        stage.PointerPressed += (_, e) =>
        {
            _drag = e.GetPosition(stage);
            e.Pointer.Capture(stage);
        };
        stage.PointerMoved += (_, e) =>
        {
            if (_drag is not { } from)
            {
                return;
            }

            var at = e.GetPosition(stage);
            _yaw += (float)((at.X - from.X) * 0.01);
            _pitch = Math.Clamp(
                _pitch + (float)((at.Y - from.Y) * 0.01),
                -1.3f,
                1.3f
            );
            _drag = at;
            RenderModel();
        };
        stage.PointerReleased += (_, e) =>
        {
            _drag = null;
            e.Pointer.Capture(null);
        };

        _meshStage = stage;
        _meshImage = image;
        _canvas.Children.Add(stage);
        RenderModel();
    }

    /// <summary>Renders the model at the pane's size, dropping a render a newer drag outran.</summary>
    private void RenderModel()
    {
        if (_mesh is not { } mesh || _meshImage is not { } image || _meshStage is not { } stage)
        {
            return;
        }

        var width = (int)Math.Clamp(stage.Bounds.Width, 1, 1200);
        var height = (int)Math.Clamp(stage.Bounds.Height, 1, 900);

        if (width < 8 || height < 8)
        {
            return;
        }

        _render?.Cancel();
        var reading = new CancellationTokenSource();
        _render = reading;
        var version = ++_version;

        _ = DrawModel(mesh, image, width, height, version, reading);
    }

    /// <summary>Draws one view of the model, then hands the pixels to a bitmap on the UI thread.</summary>
    private async Task DrawModel(
        SightMesh mesh,
        Image image,
        int width,
        int height,
        int version,
        CancellationTokenSource reading
    )
    {
        try
        {
            var yaw = _yaw;
            var pitch = _pitch;
            var grid = SightSettings.ModelGrid;
            var pixels = await Task
                .Run(
                    () => SightModelRenderer.Render(mesh, yaw, pitch, CameraDistance, width, height, grid),
                    reading.Token
                )
                .ConfigureAwait(true);

            if (version != _version || reading.IsCancellationRequested)
            {
                return;
            }

            var bitmap = ToBitmap(pixels, width, height);
            _meshPicture?.Dispose();
            _meshPicture = bitmap;
            image.Source = bitmap;
        }
        catch (OperationCanceledException)
        {
            // A newer drag took the pane.
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _host.Log.Warn($"the model could not be drawn: {error.Message}");
        }
        finally
        {
            if (ReferenceEquals(_render, reading))
            {
                _render = null;
            }

            reading.Dispose();
        }
    }

    /// <summary>Makes a bitmap out of BGRA bytes, on the UI thread.</summary>
    private static WriteableBitmap ToBitmap(byte[] pixels, int width, int height)
    {
        var bitmap = new WriteableBitmap(
            new PixelSize(width, height),
            new Vector(96, 96),
            PixelFormat.Bgra8888,
            AlphaFormat.Unpremul
        );

        using var frame = bitmap.Lock();
        var row = width * 4;

        for (var at = 0; at < height; at++)
        {
            Marshal.Copy(pixels, at * row, frame.Address + (at * frame.RowBytes), row);
        }

        return bitmap;
    }

    /// <summary>Builds a centred notice out of a mark, a title and a hint.</summary>
    private Control Notice(string iconKey, string titleKey, string hintKey, params object?[] values)
    {
        var panel = new StackPanel
        {
            Spacing = 6,
            MaxWidth = 380,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var mark = Mark(iconKey, 30, PaletteKeys.Muted);

        if (mark is not null)
        {
            mark.HorizontalAlignment = HorizontalAlignment.Center;
            panel.Children.Add(mark);
        }

        var title = new TextBlock
        {
            Text = RolaI18N.Get(titleKey),
            FontSize = 14,
            FontWeight = FontWeight.SemiBold,
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            HorizontalAlignment = HorizontalAlignment.Center,
        };
        title[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
        var hint = new TextBlock
        {
            Text = values.Length == 0 ? RolaI18N.Get(hintKey) : RolaI18N.Get(hintKey, values[0]),
            FontSize = 12,
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            HorizontalAlignment = HorizontalAlignment.Center,
        };
        hint[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);
        panel.Children.Add(title);
        panel.Children.Add(hint);

        return panel;
    }

    /// <summary>One picture from the library, stamped in one of the look's inks.</summary>
    /// <param name="key">The picture's key.</param>
    /// <param name="size">How wide and tall it is.</param>
    /// <param name="ink">The resource the mask is filled with.</param>
    /// <returns>The mark, or nothing when the library holds no such picture.</returns>
    private Border? Mark(string key, double size, string ink)
    {
        if (_host.Icons.Find(key) is not { } picture)
        {
            return null;
        }

        // The mask is drawn whole rather than resampled with the toolkit's default filter: the
        // pictures are drawn larger than the marks that wear them, and a thin stroke reduced this way
        // loses its stem.
        var mark = new Border
        {
            Width = size,
            Height = size,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        RenderOptions.SetBitmapInterpolationMode(mark, BitmapInterpolationMode.HighQuality);
        mark[!Border.BackgroundProperty] = new DynamicResourceExtension(ink);

        return mark;
    }

    /// <summary>A byte count as a reader reads it.</summary>
    private static string Size(long bytes)
    {
        var mib = bytes / (1024.0 * 1024.0);

        return mib.ToString("0.#", CultureInfo.InvariantCulture) + " MiB";
    }
}

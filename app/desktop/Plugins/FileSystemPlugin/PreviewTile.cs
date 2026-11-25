using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// A grid tile's picture: the entry's icon or its thumbnail inside the plate, with the type icon at the bottom
/// right once a thumbnail is there.
/// </summary>
/// <remarks>
/// The plate is the same one a place's picture stands on (see <see cref="Plate"/>), so a grid of files and a grid
/// of places read as one grid: what this adds is the thumbnail, and the entry's own type icon at four tenths of
/// the tile once the thumbnail has taken the whole of it. The contribution badges stay above both pictures, in
/// the same row they are drawn in anywhere else.
/// <para>
/// The preview is asked for when the tile is attached to a window and cancelled when it leaves: a grid recycles
/// its tiles, and a request made once for one entry would otherwise be a request a different entry inherits.
/// </para>
/// <para>
/// The pixels are copied to a bitmap here, on the window's thread, because a bitmap belongs to the renderer: a
/// source makes bytes, and this is the one place the bytes become something a window can draw.
/// </para>
/// </remarks>
internal sealed class PreviewTile : Grid
{
    /// <summary>How large the type badge is, as a share of the tile.</summary>
    private const double BadgeShare = 0.4;

    /// <summary>The entry the tile is for.</summary>
    private readonly Entry _entry;

    /// <summary>The plate the icon and the preview stand in.</summary>
    private readonly Border _plate;

    /// <summary>What the plate holds: the icon, and the preview over it when there is one.</summary>
    private readonly Grid _inner;

    /// <summary>The type icon, which is drawn at the plate's own content size until a preview arrives.</summary>
    private readonly Control _icon;

    /// <summary>The preview's own picture, inside the frame.</summary>
    private readonly Image _picture;

    /// <summary>The frame the preview is drawn in, hidden until there is one.</summary>
    private readonly Border _frame;

    /// <summary>The type icon, worn at the plate's bottom right once a preview is there.</summary>
    private readonly Border _type;

    /// <summary>How wide and tall the type badge is.</summary>
    private readonly int _badge;

    /// <summary>How wide and tall the plate's content is.</summary>
    private readonly int _content;

    /// <summary>How much of a virtual entry's icon its ring covers.</summary>
    private readonly double _mark;

    /// <summary>What the preview is asked of.</summary>
    private readonly IPreviewService _previews;

    /// <summary>The request in flight, so that a tile that leaves the tree can cancel it.</summary>
    private CancellationTokenSource? _asking;

    /// <summary>Makes a tile.</summary>
    /// <param name="previews">What the preview is asked of.</param>
    /// <param name="entry">The entry the tile is for.</param>
    /// <param name="size">How wide and tall the picture is.</param>
    /// <param name="badges">What the contributions said, drawn above the picture.</param>
    /// <param name="mark">How much of the icon a virtual entry's ring covers.</param>
    public PreviewTile(
        IPreviewService previews,
        Entry entry,
        int size,
        IReadOnlyList<EntryBadge> badges,
        double mark = 1
    )
    {
        _previews = previews;
        _entry = entry;
        _mark = mark;
        _badge = Badge(size);
        _content = Plate.Content(size);
        _icon = Icons.For(entry, _content, mark);
        _icon.HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center;
        _icon.VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center;
        _picture = new Image
        {
            Width = _content,
            Height = _content,
            Stretch = Stretch.UniformToFill,
        };
        _frame = new Border
        {
            Width = _content,
            Height = _content,
            CornerRadius = new CornerRadius(Plate.Inner(size)),
            ClipToBounds = true,
            Child = _picture,
            IsVisible = false,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
            VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center,
        };
        _inner = new Grid();
        _inner.Children.Add(_icon);
        _inner.Children.Add(_frame);
        _plate = Plate.Frame(size, _inner);
        _type = new Border
        {
            Width = _badge,
            Height = _badge,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            VerticalAlignment = Avalonia.Layout.VerticalAlignment.Bottom,
            IsVisible = false,
        };

        Width = size;
        Height = size;

        // The tile is not clipped: the plate is, and the badges stand on the plate's edge and reach a little
        // past it. The type badge sits on the plate's own bottom right corner.
        Children.Add(_plate);
        Children.Add(_type);

        if (badges.Count > 0)
        {
            var row = new BadgeRow();

            for (var at = 0; at < BadgeRow.Worn(badges.Count, size); at++)
            {
                row.Children.Add(Icons.Worn(badges[at], size));
            }

            Children.Add(row);
        }

        AttachedToVisualTree += (_, _) => Load();
        DetachedFromVisualTree += (_, _) => Cancel();
    }

    /// <summary>Asks for the preview, at the size the window will draw it.</summary>
    private void Load()
    {
        Cancel();

        var scaling = TopLevel.GetTopLevel(this)?.RenderScaling ?? 1.0;
        var pixels = (int)Math.Round(_content * scaling);
        var cancel = new CancellationTokenSource();

        _asking = cancel;

        _ = Ask(pixels, cancel.Token);
    }

    /// <summary>Asks for the preview and draws it when it arrives.</summary>
    /// <param name="pixels">How wide and tall the preview is asked to be, in real pixels.</param>
    /// <param name="cancel">Cancels the request.</param>
    private async Task Ask(int pixels, CancellationToken cancel)
    {
        try
        {
            var image = await _previews.Get(
                new PreviewRequest(
                    _entry.Path,
                    _entry.Kind,
                    new PreviewSize(pixels, pixels),
                    PreviewPurpose.Tile
                ),
                cancel
            );

            if (image is null || cancel.IsCancellationRequested || VisualRoot is null)
            {
                return;
            }

            _picture.Source = Bitmap(image);
            _frame.IsVisible = true;
            _type.Child ??= Icons.For(_entry, _badge, _mark);
            _type.IsVisible = true;
            _icon.IsVisible = false;
        }
        catch (OperationCanceledException)
        {
            // The tile left the tree, or another entry took its place.
        }
    }

    /// <summary>Copies raw BGRA into a bitmap the renderer can draw.</summary>
    /// <param name="image">The pixels.</param>
    /// <returns>The bitmap.</returns>
    private static WriteableBitmap Bitmap(PreviewImage image)
    {
        var bitmap = new WriteableBitmap(
            new PixelSize(image.Width, image.Height),
            new Vector(96, 96),
            PixelFormat.Bgra8888,
            AlphaFormat.Premul
        );

        using var frame = bitmap.Lock();
        var row = Math.Min(image.Stride, frame.RowBytes);
        var pixels = image.Pixels.ToArray();

        for (var at = 0; at < image.Height; at++)
        {
            Marshal.Copy(
                pixels,
                at * image.Stride,
                frame.Address + (at * frame.RowBytes),
                row
            );
        }

        return bitmap;
    }

    /// <summary>Cancels the request in flight, if one is.</summary>
    private void Cancel()
    {
        _asking?.Cancel();
        _asking?.Dispose();
        _asking = null;
    }

    /// <summary>How wide and tall the type badge is at a tile of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <returns>The badge's size, four tenths of the tile, at least one pixel.</returns>
    private static int Badge(int size) =>
        Math.Max(1, (int)Math.Round(size * BadgeShare, MidpointRounding.AwayFromZero));
}

using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// A grid tile's picture: the system's thumbnail in a rounded frame where there is one, and the entry's type
/// icon where there is not.
/// </summary>
/// <remarks>
/// The preview is asked for when the tile is attached to a window and cancelled when it leaves: a grid recycles its
/// tiles, and a request made once for one entry would otherwise be a request a different entry inherits. The
/// contribution badges stay above both pictures, in the same arc they are drawn on anywhere else, and the type
/// icon takes their opposite corner at four tenths of the tile once a thumbnail is there: the thumbnail is what is
/// read first, and the type is what is found at its corner when it is looked for.
/// <para>
/// The pixels are copied to a bitmap here, on the window's thread, because a bitmap belongs to the renderer: a
/// source makes bytes, and this is the one place the bytes become something a window can draw.
/// </para>
/// </remarks>
internal sealed class PreviewTile : Grid
{
    /// <summary>How round a thumbnail's corners are, as a share of the tile.</summary>
    private const double CornerShare = 0.12;

    /// <summary>How large the type badge is, as a share of the tile.</summary>
    private const double BadgeShare = 0.4;

    /// <summary>The entry the tile is for.</summary>
    private readonly Entry _entry;

    /// <summary>The type icon, which is drawn at the whole tile until a preview arrives.</summary>
    private readonly Control _icon;

    /// <summary>The preview's own picture, inside the frame.</summary>
    private readonly Image _picture;

    /// <summary>The frame the preview is drawn in, hidden until there is one.</summary>
    private readonly Border _frame;

    /// <summary>The type icon's badge, worn at the bottom right of a preview.</summary>
    private readonly Border _type;

    /// <summary>How wide and tall the type badge is.</summary>
    private readonly int _badge;

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
        _icon = Icons.For(entry, size, mark);
        _picture = new Image
        {
            Width = size,
            Height = size,
            Stretch = Stretch.UniformToFill,
        };
        _frame = new Border
        {
            Width = size,
            Height = size,
            CornerRadius = new CornerRadius(Corner(size)),
            ClipToBounds = true,
            Child = _picture,
            IsVisible = false,
        };
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

        // The tile is not clipped: its badges stand on the arc around the icon's corner and reach a little past
        // it, and a clip at the tile's own edge cuts them in half. The thumbnail is clipped by its own frame,
        // and the type badge sits inside the tile's own bottom right corner.
        Children.Add(_frame);
        Children.Add(_icon);
        Children.Add(_type);

        if (badges.Count > 0)
        {
            var arc = new BadgeArc();

            for (var at = 0; at < BadgeArc.Worn(badges.Count); at++)
            {
                arc.Children.Add(Icons.Worn(badges[at], size));
            }

            Children.Add(arc);
        }

        AttachedToVisualTree += (_, _) => Load();
        DetachedFromVisualTree += (_, _) => Cancel();
    }

    /// <summary>Asks for the preview, at the size the window will draw it.</summary>
    private void Load()
    {
        Cancel();

        var scaling = TopLevel.GetTopLevel(this)?.RenderScaling ?? 1.0;
        var pixels = (int)Math.Round(Width * scaling);
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

    /// <summary>How round a thumbnail's corners are at a tile of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <returns>The corner radius, at least two pixels.</returns>
    private static double Corner(double size) =>
        Math.Max(2, Math.Round(size * CornerShare, MidpointRounding.AwayFromZero));

    /// <summary>How wide and tall the type badge is at a tile of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <returns>The badge's size, four tenths of the tile, at least one pixel.</returns>
    private static int Badge(int size) =>
        Math.Max(1, (int)Math.Round(size * BadgeShare, MidpointRounding.AwayFromZero));
}

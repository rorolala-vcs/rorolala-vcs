using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// A grid tile's picture: the system's thumbnail where there is one, and the entry's type icon where there is not.
/// </summary>
/// <remarks>
/// The preview is asked for when the tile is attached to a window and cancelled when it leaves: a grid recycles its
/// tiles, and a request made once for one entry would otherwise be a request a different entry inherits. The badges
/// stay above both pictures, in the same arc they are drawn on anywhere else.
/// <para>
/// The pixels are copied to a bitmap here, on the window's thread, because a bitmap belongs to the renderer: a
/// source makes bytes, and this is the one place the bytes become something a window can draw.
/// </para>
/// </remarks>
internal sealed class PreviewTile : Grid
{
    /// <summary>The entry the tile is for.</summary>
    private readonly Entry _entry;

    /// <summary>The type icon, which is drawn until a preview arrives and stays if none does.</summary>
    private readonly Control _icon;

    /// <summary>The preview, hidden until there is one.</summary>
    private readonly Image _picture;

    /// <summary>What the preview is asked of.</summary>
    private readonly IPreviewService _previews;

    /// <summary>The request in flight, so that a tile that leaves the tree can cancel it.</summary>
    private CancellationTokenSource? _asking;

    /// <summary>Makes a tile.</summary>
    /// <param name="previews">What the preview is asked of.</param>
    /// <param name="entry">The entry the tile is for.</param>
    /// <param name="size">How wide and tall the picture is.</param>
    /// <param name="badges">What the contributions said, drawn above the picture.</param>
    public PreviewTile(
        IPreviewService previews,
        Entry entry,
        int size,
        IReadOnlyList<EntryBadge> badges
    )
    {
        _previews = previews;
        _entry = entry;
        _icon = Icons.For(entry, size);
        _picture = new Image
        {
            Width = size,
            Height = size,
            Stretch = Stretch.UniformToFill,
            IsVisible = false,
        };

        Width = size;
        Height = size;

        // The tile is not clipped: its badges stand on the arc around the icon's corner and reach a little past
        // it, and a clip at the tile's own edge cuts them in half. The picture is already the tile's size.
        Children.Add(_picture);
        Children.Add(_icon);

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
            _picture.IsVisible = true;
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
}

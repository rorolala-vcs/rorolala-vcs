using System.Buffers.Binary;
using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;

namespace SightPlugin;

/// <summary>The previewer that reads a PSD file's header and, when it is plain, its merged image.</summary>
/// <remarks>
/// Layer data is not decoded: a layer table is a format module of its own, and a preview of the
/// flattened file is what a reader asks for first. The composite is read only when it is stored as
/// plain 8-bit R, G, B, (A) pixels; a compressed or high-depth composite is left to the metadata,
/// which says what the header holds.
/// </remarks>
internal sealed class SightPsd : ISightPreviewer
{
    /// <summary>How many pixels of a merged image are decoded at most.</summary>
    private const long PixelCap = 32_000_000;

    /// <inheritdoc />
    public string Id => "sight.psd";

    /// <inheritdoc />
    public string NameKey => "rorolala_sight.previewer.psd";

    /// <inheritdoc />
    public string? IconKey => Icons.File;

    /// <inheritdoc />
    public int Rank => 65;

    /// <inheritdoc />
    public bool Cares(SightFile primary, SightFile? second) =>
        second is null && string.Equals(primary.Extension, "psd", StringComparison.OrdinalIgnoreCase);

    /// <inheritdoc />
    public async ValueTask<SightDocument?> OpenAsync(
        SightOpenRequest request,
        CancellationToken cancel
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        var limit = SightSettings.ImageLimitBytes;

        if (request.Primary.Length > limit)
        {
            return new SightTooLargeDocument(limit);
        }

        var bytes = await File.ReadAllBytesAsync(request.Primary.Path, cancel)
            .ConfigureAwait(false);

        cancel.ThrowIfCancellationRequested();

        return await ReadAsync(bytes).ConfigureAwait(false);
    }

    /// <summary>Reads a PSD file's header, and its composite when the bytes carry one.</summary>
    /// <remarks>
    /// The picture is made on the UI thread, and this awaits that step rather than blocking on it:
    /// a caller already on the UI thread would otherwise wait for a dispatch that cannot run until
    /// it returns.
    /// </remarks>
    /// <param name="bytes">The file.</param>
    /// <returns>The document, or nothing when the file is not a PSD this build reads.</returns>
    internal static async ValueTask<SightPsdDocument?> ReadAsync(byte[] bytes)
    {
        ArgumentNullException.ThrowIfNull(bytes);

        if (bytes.Length < 26 || bytes[0] != '8' || bytes[1] != 'B' || bytes[2] != 'P' || bytes[3] != 'S')
        {
            return null;
        }

        var version = BinaryPrimitives.ReadUInt16BigEndian(bytes.AsSpan(4));

        if (version != 1)
        {
            return null;
        }

        var channels = BinaryPrimitives.ReadUInt16BigEndian(bytes.AsSpan(12));
        var height = BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(14));
        var width = BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(18));
        var depth = BinaryPrimitives.ReadUInt16BigEndian(bytes.AsSpan(22));
        var mode = BinaryPrimitives.ReadUInt16BigEndian(bytes.AsSpan(24));

        if (width == 0 || height == 0 || width > int.MaxValue || height > int.MaxValue)
        {
            return null;
        }

        var pixels = Composite(bytes, (int)width, (int)height, channels, depth);
        Bitmap? picture = null;

        if (pixels is { } bgra)
        {
            picture = await Dispatcher.UIThread.InvokeAsync(() =>
                Picture(bgra, (int)width, (int)height)
            );
        }

        return new SightPsdDocument(
            (int)width,
            (int)height,
            channels,
            depth,
            ModeName(mode),
            picture
        );
    }

    /// <summary>Decodes the merged image when it is a plain 8-bit RGB or RGBA composite.</summary>
    /// <param name="bytes">The file.</param>
    /// <param name="width">The width from the header.</param>
    /// <param name="height">The height from the header.</param>
    /// <param name="channels">The channel count from the header.</param>
    /// <param name="depth">The depth from the header.</param>
    /// <returns>The BGRA pixels, or nothing when this build does not decode the composite.</returns>
    private static byte[]? Composite(
        byte[] bytes,
        int width,
        int height,
        int channels,
        int depth
    )
    {
        if (depth != 8 || channels is not (3 or 4) || (long)width * height > PixelCap)
        {
            return null;
        }

        var at = 26;

        for (var section = 0; section < 3; section++)
        {
            if (at + 4 > bytes.Length)
            {
                return null;
            }

            var length = BinaryPrimitives.ReadUInt32BigEndian(bytes.AsSpan(at));

            if (length > int.MaxValue || at + 4 + (long)length > bytes.Length)
            {
                return null;
            }

            at += 4 + (int)length;
        }

        if (at + 2 > bytes.Length)
        {
            return null;
        }

        var compression = BinaryPrimitives.ReadUInt16BigEndian(bytes.AsSpan(at));
        at += 2;

        var pixels = (long)width * height;

        if (compression != 0 || at + (pixels * channels) > bytes.Length)
        {
            return null;
        }

        var bgra = new byte[pixels * 4];

        for (var index = 0; index < (int)pixels; index++)
        {
            var to = index * 4;
            var from = (int)(at + (index * channels));
            bgra[to] = bytes[from + 2];
            bgra[to + 1] = bytes[from + 1];
            bgra[to + 2] = bytes[from];
            bgra[to + 3] = channels == 4 ? bytes[from + 3] : (byte)255;
        }

        return bgra;
    }

    /// <summary>Makes a bitmap out of BGRA bytes, on the UI thread.</summary>
    /// <param name="bgra">The pixels.</param>
    /// <param name="width">How wide the picture is.</param>
    /// <param name="height">How tall it is.</param>
    /// <returns>The picture.</returns>
    private static WriteableBitmap Picture(byte[] bgra, int width, int height)
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
            Marshal.Copy(bgra, at * row, frame.Address + (at * frame.RowBytes), row);
        }

        return bitmap;
    }

    /// <summary>The colour mode's name.</summary>
    /// <param name="mode">The number the header states.</param>
    /// <returns>The name, or the number when this build does not know it.</returns>
    private static string ModeName(int mode) =>
        mode switch
        {
            0 => "Bitmap",
            1 => "Grayscale",
            2 => "Indexed",
            3 => "RGB",
            4 => "CMYK",
            7 => "Multichannel",
            8 => "Duotone",
            9 => "Lab",
            _ => mode.ToString(System.Globalization.CultureInfo.InvariantCulture),
        };
}

using Avalonia.Media.Imaging;
using Avalonia.Threading;

namespace SightPlugin;

/// <summary>The previewer that decodes an image with Avalonia's own decoder.</summary>
/// <remarks>
/// The decode is Avalonia's, not the host preview service's: that service answers a square thumbnail
/// asked for at one size, which cannot state the picture's own dimensions, and the dock draws the
/// picture at the pane's size and lets the metadata say what it is. The bitmap is made on the UI
/// thread, since a bitmap belongs to the renderer; the read is on the caller's thread.
/// </remarks>
internal sealed class SightImage : ISightPreviewer
{
    /// <summary>Every extension an image is read by.</summary>
    private static readonly IReadOnlySet<string> ImageExtensions = new HashSet<string>(
        StringComparer.OrdinalIgnoreCase
    )
    {
        "png",
        "jpg",
        "jpeg",
        "bmp",
        "gif",
        "webp",
        "tif",
        "tiff",
        "ico",
    };

    /// <inheritdoc />
    public string Id => "sight.image";

    /// <inheritdoc />
    public string NameKey => "rorolala_sight.previewer.image";

    /// <inheritdoc />
    public string? IconKey => Icons.Image;

    /// <inheritdoc />
    public int Rank => 60;

    /// <inheritdoc />
    public bool Cares(SightFile primary, SightFile? second) =>
        second is null && ImageExtensions.Contains(primary.Extension);

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
        var picture = await Dispatcher.UIThread.InvokeAsync(() =>
        {
            using var stream = new MemoryStream(bytes, writable: false);

            return new Bitmap(stream);
        });

        cancel.ThrowIfCancellationRequested();

        return new SightImageDocument(picture, request.Primary.Extension.ToUpperInvariant());
    }
}

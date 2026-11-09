using System.Diagnostics.CodeAnalysis;
using System.IO.Compression;
using System.Security.Cryptography;
using System.Text;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Previews;

/// <summary>
/// The thumbnails a Linux desktop has already cached, read from the freedesktop thumbnail directory.
/// </summary>
/// <remarks>
/// This source does not generate a thumbnail, and that is deliberate: generating one means starting a helper
/// process or embedding a decoder for every format the desktop knows, with a timeout nobody controls. What it
/// does is answer from the cache the desktop already keeps, and leave a file with no cached thumbnail to the
/// plugin renderers.
/// <para>
/// A cached picture is only used when its own <c>Thumb::URI</c> and <c>Thumb::MTime</c> say it is the file that
/// was asked for: a hash alone would serve a thumbnail made for an older version of the same path.
/// </para>
/// </remarks>
public sealed class FreedesktopPreviewSource : IPreviewSource
{
    /// <summary>The directory sizes the freedesktop cache defines.</summary>
    private static readonly (string Name, int Extent)[] Sizes =
    [
        ("normal", 128),
        ("large", 256),
        ("x-large", 512),
    ];

    /// <inheritdoc />
    public string Id => "host.freedesktop";

    /// <inheritdoc />
    public bool Cares(PreviewRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        return OperatingSystem.IsLinux()
            && request.Kind == EntryKind.File
            && request.Size.Width > 0
            && request.Size.Height > 0
            && File.Exists(request.Path);
    }

    /// <inheritdoc />
    public ValueTask<PreviewImage?> RenderAsync(PreviewRequest request, CancellationToken cancel)
    {
        ArgumentNullException.ThrowIfNull(request);

        if (cancel.IsCancellationRequested)
        {
            return ValueTask.FromResult<PreviewImage?>(null);
        }

        return ValueTask.FromResult(Read(request));
    }

    /// <summary>Reads the cached thumbnail for a request, or answers nothing.</summary>
    /// <param name="request">The request.</param>
    /// <returns>The pixels, or nothing.</returns>
    [SuppressMessage(
        "Security",
        "CA5351:Do not use broken cryptographic algorithms",
        Justification = "the freedesktop cache names a thumbnail by the MD5 of its URI, so the hash is the cache's address rather than a security check"
    )]
    private static PreviewImage? Read(PreviewRequest request)
    {
        var wanted = Math.Max(request.Size.Width, request.Size.Height);
        var directory = Cache();

        if (directory is null)
        {
            return null;
        }

        var uri = new Uri(request.Path).AbsoluteUri;
        var modified = new DateTimeOffset(File.GetLastWriteTimeUtc(request.Path)).ToUnixTimeSeconds();
        var name = Convert.ToHexString(MD5.HashData(Encoding.UTF8.GetBytes(uri))).ToLowerInvariant();

        foreach (var (size, extent) in Sizes)
        {
            if (wanted > extent)
            {
                continue;
            }

            var path = Path.Combine(directory, size, $"{name}.png");

            if (!File.Exists(path))
            {
                continue;
            }

            try
            {
                var picture = Decode(File.ReadAllBytes(path), out var said);

                if (
                    picture is not null
                    && said.TryGetValue("Thumb::URI", out var named)
                    && string.Equals(named, uri, StringComparison.Ordinal)
                    && said.TryGetValue("Thumb::MTime", out var when)
                    && string.Equals(when, modified.ToString(System.Globalization.CultureInfo.InvariantCulture), StringComparison.Ordinal)
                )
                {
                    return picture;
                }
            }
            catch (Exception error) when (error is IOException or InvalidDataException or ArgumentException)
            {
                // A cache file that will not read is a file with no preview, not a failed listing.
            }
        }

        return null;
    }

    /// <summary>The freedesktop thumbnail cache directory, or nothing when the platform names none.</summary>
    /// <returns>The directory, or nothing.</returns>
    private static string? Cache()
    {
        var home = Environment.GetEnvironmentVariable("XDG_CACHE_HOME");

        if (home is { Length: > 0 })
        {
            return Path.Combine(home, "thumbnails");
        }

        var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);

        return profile is { Length: > 0 } ? Path.Combine(profile, ".cache", "thumbnails") : null;
    }

    /// <summary>Reads a PNG's pixels and the text chunks that say what it is of.</summary>
    /// <param name="file">The file's bytes.</param>
    /// <param name="said">The text chunks, by keyword.</param>
    /// <returns>The pixels, or nothing when the file is not an 8 bit PNG this reads.</returns>
    private static PreviewImage? Decode(byte[] file, out Dictionary<string, string> said)
    {
        said = new Dictionary<string, string>(StringComparer.Ordinal);

        if (
            file.Length < 8
            || file[0] != 0x89
            || file[1] != 0x50
            || file[2] != 0x4E
            || file[3] != 0x47
        )
        {
            return null;
        }

        var at = 8;
        var width = 0;
        var height = 0;
        var colour = 0;
        var alpha = false;
        using var data = new MemoryStream();

        while (at + 12 <= file.Length)
        {
            var length = BigEndian(file, at);
            var kind = Encoding.ASCII.GetString(file, at + 4, 4);
            var body = at + 8;

            if (body + length + 4 > file.Length)
            {
                return null;
            }

            switch (kind)
            {
                case "IHDR":
                    width = BigEndian(file, body);
                    height = BigEndian(file, body + 4);

                    if (file[body + 8] != 8 || file[body + 12] != 0)
                    {
                        return null;
                    }

                    colour = file[body + 9];
                    alpha = colour == 6;
                    break;

                case "tEXt":
                    var text = Encoding.Latin1.GetString(file, body, length);
                    var split = text.IndexOf('\0', StringComparison.Ordinal);

                    if (split > 0)
                    {
                        said[text[..split]] = text[(split + 1)..];
                    }

                    break;

                case "IDAT":
                    data.Write(file, body, length);
                    break;
            }

            at = body + length + 4;
        }

        if (width <= 0 || height <= 0 || colour is not (2 or 6))
        {
            return null;
        }

        data.Position = 0;

        using var inflate = new ZLibStream(data, CompressionMode.Decompress);
        using var raw = new MemoryStream();
        inflate.CopyTo(raw);

        var channels = alpha ? 4 : 3;
        var stride = width * channels;
        var source = raw.ToArray();

        if (source.Length < height * (stride + 1))
        {
            return null;
        }

        var rows = new byte[height][];
        var prior = new byte[stride];

        for (var row = 0; row < height; row++)
        {
            var start = row * (stride + 1);
            var filter = source[start];
            var line = new byte[stride];
            Array.Copy(source, start + 1, line, 0, stride);
            Unfilter(filter, line, prior, channels);
            rows[row] = line;
            prior = line;
        }

        var pixels = new byte[width * height * 4];

        for (var row = 0; row < height; row++)
        {
            var line = rows[row];

            for (var column = 0; column < width; column++)
            {
                var from = column * channels;
                var to = ((row * width) + column) * 4;
                var red = line[from];
                var green = line[from + 1];
                var blue = line[from + 2];
                var cover = alpha ? line[from + 3] : (byte)255;

                // The contract's pixels are premultiplied, and a PNG's are not.
                pixels[to] = (byte)((blue * cover) / 255);
                pixels[to + 1] = (byte)((green * cover) / 255);
                pixels[to + 2] = (byte)((red * cover) / 255);
                pixels[to + 3] = cover;
            }
        }

        return new PreviewImage(width, height, width * 4, pixels, PreviewOrigin.System);
    }

    /// <summary>Reverses one PNG row's filter.</summary>
    /// <param name="filter">The filter the row states.</param>
    /// <param name="line">The row's bytes, changed in place.</param>
    /// <param name="prior">The row before it, or zeroes for the first.</param>
    /// <param name="channels">How many bytes one pixel takes.</param>
    private static void Unfilter(byte filter, byte[] line, byte[] prior, int channels)
    {
        for (var at = 0; at < line.Length; at++)
        {
            var left = at >= channels ? line[at - channels] : (byte)0;
            var up = prior[at];

            line[at] = filter switch
            {
                0 => line[at],
                1 => (byte)(line[at] + left),
                2 => (byte)(line[at] + up),
                3 => (byte)(line[at] + ((left + up) / 2)),
                4 => (byte)(line[at] + Paeth(left, up, at >= channels ? prior[at - channels] : (byte)0)),
                _ => line[at],
            };
        }
    }

    /// <summary>The PNG Paeth predictor.</summary>
    /// <param name="left">The byte to the left.</param>
    /// <param name="up">The byte above.</param>
    /// <param name="corner">The byte above and to the left.</param>
    /// <returns>The predicted byte.</returns>
    private static byte Paeth(byte left, byte up, byte corner)
    {
        var estimate = left + up - corner;
        var toLeft = Math.Abs(estimate - left);
        var toUp = Math.Abs(estimate - up);
        var toCorner = Math.Abs(estimate - corner);

        return toLeft <= toUp && toLeft <= toCorner ? left : toUp <= toCorner ? up : corner;
    }

    /// <summary>A big endian 32 bit number from a PNG chunk.</summary>
    /// <param name="file">The file.</param>
    /// <param name="at">Where the number starts.</param>
    /// <returns>The number.</returns>
    private static int BigEndian(byte[] file, int at) =>
        (file[at] << 24) | (file[at + 1] << 16) | (file[at + 2] << 8) | file[at + 3];
}

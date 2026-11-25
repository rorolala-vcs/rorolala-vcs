using System.Text;

namespace SightPlugin;

/// <summary>The previewer that reads a file as lines of text.</summary>
internal sealed class SightText : ISightPreviewer
{
    /// <summary>How many bytes are looked at when deciding whether a file is text.</summary>
    private const int Sniff = 8192;

    /// <summary>What the previewer is known by.</summary>
    public const string PreviewerId = "sight.text";

    /// <summary>Every extension a file is read as text by.</summary>
    internal static readonly IReadOnlySet<string> TextExtensions = new HashSet<string>(
        StringComparer.OrdinalIgnoreCase
    )
    {
        "txt",
        "md",
        "markdown",
        "json",
        "yml",
        "yaml",
        "toml",
        "ini",
        "cfg",
        "conf",
        "cs",
        "rs",
        "c",
        "h",
        "cpp",
        "hpp",
        "cc",
        "py",
        "js",
        "ts",
        "tsx",
        "css",
        "html",
        "htm",
        "xml",
        "sh",
        "ps1",
        "bat",
        "log",
        "diff",
        "patch",
    };

    /// <inheritdoc />
    public string Id => PreviewerId;

    /// <inheritdoc />
    public string NameKey => "rorolala_sight.previewer.text";

    /// <inheritdoc />
    public string? IconKey => Icons.TextT;

    /// <inheritdoc />
    public int Rank => 50;

    /// <inheritdoc />
    public bool Cares(SightFile primary, SightFile? second) =>
        second is null && TextExtensions.Contains(primary.Extension);

    /// <inheritdoc />
    public async ValueTask<SightDocument?> OpenAsync(
        SightOpenRequest request,
        CancellationToken cancel
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        var limit = SightSettings.TextLimitBytes;

        if (request.Primary.Length > limit)
        {
            return new SightTooLargeDocument(limit);
        }

        var lines = await ReadLinesAsync(request.Primary, limit, cancel).ConfigureAwait(false);

        return lines is null ? null : new SightTextDocument(lines, false);
    }

    /// <summary>
    /// Reads a file as lines, or answers nothing when it is binary.
    /// </summary>
    /// <remarks>
    /// It is the one reading the diff uses as well, so the two cannot disagree about what a text
    /// file is. A file whose size passes `limit` is not read at all; the caller has already answered
    /// that as too large.
    /// </remarks>
    /// <param name="file">The file.</param>
    /// <param name="limit">How many bytes may be read.</param>
    /// <param name="cancel">Cancels the read.</param>
    /// <returns>The lines, or nothing when the file is binary.</returns>
    internal static async Task<IReadOnlyList<string>?> ReadLinesAsync(
        SightFile file,
        long limit,
        CancellationToken cancel
    )
    {
        ArgumentNullException.ThrowIfNull(file);

        if (file.Length > limit)
        {
            return null;
        }

        var bytes = await File.ReadAllBytesAsync(file.Path, cancel).ConfigureAwait(false);
        var head = bytes.AsSpan(0, Math.Min(bytes.Length, Sniff));

        if (head.Contains((byte)0))
        {
            return null;
        }

        var text = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: false)
            .GetString(bytes)
            .TrimStart('\uFEFF');

        return text.Replace("\r\n", "\n", StringComparison.Ordinal)
            .Replace('\r', '\n')
            .Split('\n');
    }
}

/// <summary>The name a previewer and the metadata call a text file's format.</summary>
internal static class SightFormatName
{
    /// <summary>The name of a text extension.</summary>
    /// <param name="extension">The extension, in lower case.</param>
    /// <returns>The format's name, or the extension in capitals.</returns>
    public static string OfText(string extension) =>
        extension switch
        {
            "md" or "markdown" => "Markdown",
            "json" => "JSON",
            "yml" or "yaml" => "YAML",
            "toml" => "TOML",
            "xml" => "XML",
            "html" or "htm" => "HTML",
            "css" => "CSS",
            "js" => "JavaScript",
            "ts" => "TypeScript",
            "tsx" => "TSX",
            "cs" => "C#",
            "rs" => "Rust",
            "py" => "Python",
            "diff" or "patch" => "Diff",
            _ => extension.Length > 0 ? extension.ToUpperInvariant() : "Text",
        };
}

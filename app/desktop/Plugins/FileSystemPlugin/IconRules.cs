using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The kind of thing an entry is, as far as an icon set is concerned.
/// </summary>
/// <remarks>
/// An icon set names a picture per kind of thing rather than per extension, and this is the taxonomy its
/// <c>[fallback]</c> table is written in. It is deliberately coarse: the five kinds are the ones a person
/// recognises at a glance in a listing, and a set that wanted to tell a PSD from a PNG would be asking a
/// fallback table to be a MIME database.
/// </remarks>
internal enum IconCategory
{
    /// <summary>A directory.</summary>
    Directory,

    /// <summary>Text a person reads, source code among it.</summary>
    Text,

    /// <summary>Sound.</summary>
    Audio,

    /// <summary>A picture or a moving picture, and the documents artists work in.</summary>
    Media,

    /// <summary>Everything else: archives, programs, documents no one edits by hand.</summary>
    Binary,
}

/// <summary>
/// The File System's own fallback rules: what kind of thing a name says an entry is.
/// </summary>
/// <remarks>
/// This is a rule of thumb rather than a content sniff, and it is deliberately cheap: it runs for every row of
/// every listing, so it may not open the file. What it cannot tell — a PSD that is really a renamed ZIP, a text
/// file with no extension — is a file drawn as a generic document, which is what a person reading a listing
/// expects of a name that says nothing.
/// <para>
/// The four lists below are the whole of the rule. A kind added to a set's <c>[fallback]</c> table is a kind
/// named here; a kind a name falls into that the set has no picture for is answered by the set's binary
/// picture, and then by the system's own icon.
/// </para>
/// </remarks>
internal static class IconRules
{
    /// <summary>Extensions that hold text a person reads, source code included.</summary>
    private static readonly HashSet<string> Text = new(StringComparer.OrdinalIgnoreCase)
    {
        "txt",
        "text",
        "md",
        "markdown",
        "rst",
        "log",
        "csv",
        "tsv",
        "json",
        "jsonc",
        "json5",
        "yaml",
        "yml",
        "toml",
        "ini",
        "cfg",
        "conf",
        "properties",
        "env",
        "xml",
        "xsl",
        "xslt",
        "html",
        "htm",
        "xhtml",
        "css",
        "scss",
        "sass",
        "less",
        "js",
        "mjs",
        "cjs",
        "jsx",
        "ts",
        "mts",
        "cts",
        "tsx",
        "vue",
        "svelte",
        "c",
        "h",
        "cc",
        "cpp",
        "cxx",
        "hpp",
        "hh",
        "cs",
        "csx",
        "java",
        "kt",
        "kts",
        "go",
        "rs",
        "py",
        "pyw",
        "rb",
        "php",
        "pl",
        "lua",
        "r",
        "sh",
        "bash",
        "zsh",
        "fish",
        "ps1",
        "psm1",
        "bat",
        "cmd",
        "sql",
        "graphql",
        "gql",
        "proto",
        "gradle",
        "cmake",
    };

    /// <summary>Extensions that hold sound.</summary>
    private static readonly HashSet<string> Audio = new(StringComparer.OrdinalIgnoreCase)
    {
        "mp3",
        "wav",
        "flac",
        "aac",
        "ogg",
        "oga",
        "opus",
        "m4a",
        "wma",
        "aiff",
        "aif",
        "alac",
        "mid",
        "midi",
        "amr",
        "ape",
        "dsf",
        "dff",
    };

    /// <summary>Extensions that hold a picture, a moving picture, or a document an artist works in.</summary>
    private static readonly HashSet<string> Media = new(StringComparer.OrdinalIgnoreCase)
    {
        "png",
        "jpg",
        "jpeg",
        "gif",
        "bmp",
        "webp",
        "svg",
        "ico",
        "tif",
        "tiff",
        "heic",
        "heif",
        "avif",
        "psd",
        "psb",
        "ai",
        "eps",
        "indd",
        "raw",
        "cr2",
        "nef",
        "dng",
        "mp4",
        "m4v",
        "mkv",
        "mov",
        "avi",
        "webm",
        "wmv",
        "flv",
        "mpg",
        "mpeg",
        "3gp",
        "ogv",
    };

    /// <summary>File names with no extension that are text all the same.</summary>
    private static readonly HashSet<string> Named = new(StringComparer.OrdinalIgnoreCase)
    {
        "dockerfile",
        "makefile",
        "gnumakefile",
        "rakefile",
        "gemfile",
        "license",
        "licence",
        "readme",
        "changelog",
        "authors",
        "notice",
        "copying",
    };

    /// <summary>The kind of thing an entry is.</summary>
    /// <param name="entry">The entry to classify.</param>
    /// <returns>The kind of thing its name says it is.</returns>
    public static IconCategory Of(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        if (entry.Kind == EntryKind.Directory)
        {
            return IconCategory.Directory;
        }

        var name = Path.GetFileName(entry.Path);

        if (Named.Contains(name))
        {
            return IconCategory.Text;
        }

        var extension = Path.GetExtension(name).TrimStart('.');

        if (Text.Contains(extension))
        {
            return IconCategory.Text;
        }

        if (Audio.Contains(extension))
        {
            return IconCategory.Audio;
        }

        return Media.Contains(extension) ? IconCategory.Media : IconCategory.Binary;
    }
}

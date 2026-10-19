using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Security.Cryptography;
using System.Text.Json;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using SkiaSharp;
using Svg.Skia;
using Tomlyn;
using Tomlyn.Model;

namespace FileSystemPlugin;

/// <summary>
/// One source file an icon set draws one kind of entry with.
/// </summary>
/// <param name="Category">The kind of thing it is the picture for.</param>
/// <param name="RelativePath">Where it sits, relative to the set's own directory.</param>
internal sealed record IconSource(IconCategory Category, string RelativePath);

/// <summary>
/// One icon set as it is packaged: what it says about itself, and what it draws.
/// </summary>
/// <param name="Id">The directory it was found in, which is the value the setting keeps.</param>
/// <param name="Name">What it calls itself, which is what the setting offers.</param>
/// <param name="Authors">Who made it.</param>
/// <param name="Description">What it is, in its own words.</param>
/// <param name="Folder">Where the set sits on disk.</param>
/// <param name="Sources">The pictures it draws, one per kind of thing.</param>
/// <param name="Sizes">The pixel sizes its pictures are cached at.</param>
internal sealed record IconSet(
    string Id,
    string Name,
    IReadOnlyList<string> Authors,
    string Description,
    string Folder,
    IReadOnlyList<IconSource> Sources,
    IReadOnlyList<int> Sizes
);

/// <summary>
/// The icon sets the program was packaged with, and the cache each one is drawn into.
/// </summary>
/// <remarks>
/// A set is a directory under <c>icon-sets/</c> beside the program, holding an <c>icon-theme.toml</c> and the
/// pictures it names. The set's pictures are vectors, and a listing asks for one size at a time, so the vectors
/// are drawn into PNGs once and the PNGs are what a row reads: drawing a vector per row would be drawing the
/// same vector for every row of every listing.
/// <para>
/// The cache is under <c>icons/.cache/&lt;set&gt;/</c> beside the program, because it is the program's own
/// working state rather than the user's — a set that was packaged with the program is drawn from files beside
/// the program, and a cache under the user's data directory would outlive the package that made it. Each set
/// carries a <c>manifest.json</c> recording the SHA-1 of every source it was drawn from, so a source that was
/// replaced — an update, a developer's edit — is drawn again rather than served from a cache that no longer
/// describes it.
/// </para>
/// <para>
/// Drawing happens off the UI thread. Until a set is ready the listing keeps the system's own icons, which is
/// what it showed before the set was chosen: a program that waits for a set to draw before it shows a directory
/// is a program that appears to have hung on its first start.
/// </para>
/// </remarks>
internal static class IconSets
{
    /// <summary>The value of the setting that means "use the system's own icons".</summary>
    public const string System = "system";

    /// <summary>The setting the chosen set is kept under.</summary>
    public const string Setting = "Icons/set";

    /// <summary>What the manifest is written as, and what a reader of it must agree with.</summary>
    private const int ManifestVersion = 1;

    /// <summary>The sizes drawn when a set does not state any.</summary>
    private static readonly int[] DefaultSizes = [16, 32, 64, 128, 256];

    /// <summary>The kinds a set's <c>[fallback]</c> table names, and the keys it names them under.</summary>
    private static readonly (string Key, IconCategory Category)[] Fallbacks =
    [
        ("directory-format", IconCategory.Directory),
        ("text-format", IconCategory.Text),
        ("audio-format", IconCategory.Audio),
        ("media-format", IconCategory.Media),
        ("binary-format", IconCategory.Binary),
    ];

    /// <summary>What the discovered sets and the chosen one are guarded by.</summary>
    private static readonly object Gate = new();

    /// <summary>The pictures read out of the cache so far, by their path.</summary>
    private static readonly Dictionary<string, Bitmap> Read = new(StringComparer.Ordinal);

    /// <summary>What each set's generation is doing.</summary>
    private static readonly Dictionary<string, Progress> States = new(StringComparer.Ordinal);

    /// <summary>Every set that was found when the program started.</summary>
    private static IReadOnlyList<IconSet> _sets = [];

    /// <summary>The set in force, or <see cref="System"/>.</summary>
    private static string _selected = System;

    /// <summary>Where a warning about a set is said.</summary>
    private static Action<string> _warn = _ => { };

    /// <summary>What is said once a set has been drawn and can be shown.</summary>
    private static Action? _ready;

    /// <summary>Where the sets are packaged, beside the program.</summary>
    public static string Root => Path.Combine(AppContext.BaseDirectory, "icon-sets");

    /// <summary>Where the cache is written, beside the program.</summary>
    public static string CacheRoot => Path.Combine(AppContext.BaseDirectory, "icons", ".cache");

    /// <summary>Every set that was found, in the order the setting offers them.</summary>
    public static IReadOnlyList<IconSet> Available
    {
        get
        {
            lock (Gate)
            {
                return _sets;
            }
        }
    }

    /// <summary>
    /// Reads every set under one directory, skipping the ones that will not read.
    /// </summary>
    /// <remarks>
    /// A set that cannot be read is one the setting does not offer rather than one that fails the program:
    /// a malformed file in an optional package should cost its own set and nothing else.
    /// </remarks>
    /// <param name="root">The directory the sets are packaged under.</param>
    /// <param name="warn">Where a set that will not read is reported.</param>
    /// <returns>The sets that did read, by directory name.</returns>
    public static IReadOnlyList<IconSet> Discover(string root, Action<string> warn)
    {
        ArgumentNullException.ThrowIfNull(warn);

        if (!Directory.Exists(root))
        {
            return [];
        }

        var found = new List<IconSet>();
        List<string> folders;

        try
        {
            folders = [.. Directory.EnumerateDirectories(root).OrderBy(path => path, StringComparer.Ordinal)];
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            warn($"the icon sets beside the program will not be read: {error.Message}");
            return [];
        }

        foreach (var folder in folders)
        {
            var id = Path.GetFileName(folder);
            var theme = Path.Combine(folder, "icon-theme.toml");

            if (!File.Exists(theme))
            {
                warn($"the icon set `{id}` has no icon-theme.toml and is not offered");
                continue;
            }

            try
            {
                if (Parse(id, folder, theme, warn) is { } set)
                {
                    found.Add(set);
                }
            }
            catch (Exception error)
                when (error
                        is IOException
                            or UnauthorizedAccessException
                            or TomlException
                            or JsonException
                            or FormatException
                            or InvalidOperationException
                            or ArgumentException
                )
            {
                warn($"the icon set `{id}` will not read: {error.Message}");
            }
        }

        return found;
    }

    /// <summary>
    /// Takes the sets that were found, and says where a set that has been drawn is announced.
    /// </summary>
    /// <param name="sets">The sets that were found.</param>
    /// <param name="warn">Where a set that will not draw is reported.</param>
    /// <param name="ready">What to say once a set can be shown.</param>
    public static void Adopt(IReadOnlyList<IconSet> sets, Action<string> warn, Action ready)
    {
        ArgumentNullException.ThrowIfNull(sets);
        ArgumentNullException.ThrowIfNull(warn);
        ArgumentNullException.ThrowIfNull(ready);

        lock (Gate)
        {
            _sets = sets;
            _warn = warn;
            _ready = ready;
            _selected = System;
            States.Clear();
            Read.Clear();
        }
    }

    /// <summary>
    /// Chooses a set, falling back to the system when it is not one this program carries.
    /// </summary>
    /// <remarks>
    /// Choosing a set that is already drawn costs a lookup and nothing else. Choosing one that is not drawn
    /// starts drawing it and answers at once, so the caller may say what it chose without waiting for the
    /// pictures: the listing keeps the system's own until the drawing is done.
    /// </remarks>
    /// <param name="id">The set to use, or <see cref="System"/>.</param>
    public static void Choose(string? id)
    {
        IconSet? chosen;

        lock (Gate)
        {
            chosen = id is null ? null : _sets.FirstOrDefault(set => string.Equals(set.Id, id, StringComparison.Ordinal));
            _selected = chosen?.Id ?? System;
        }

        if (chosen is null)
        {
            if (!string.IsNullOrEmpty(id) && !string.Equals(id, System, StringComparison.Ordinal))
            {
                Warn($"the icon set `{id}` is not there; the system's own icons are used");
            }

            return;
        }

        Generate(chosen);
    }

    /// <summary>The setting options the discovered sets are offered as, the system first.</summary>
    /// <returns>The options, for a choice setting.</returns>
    public static IReadOnlyList<SettingOption> Options()
    {
        var options = new List<SettingOption>
        {
            new(
                System,
                "rorolala_file_system.setting.icon_set_system",
                Detail: RolaI18N.Get("rorolala_file_system.setting.icon_set_system_detail")
            ),
        };

        foreach (var set in Available)
        {
            options.Add(
                new SettingOption(
                    set.Id,
                    string.Empty,
                    Label: set.Name,
                    Detail: Detail(set)
                )
            );
        }

        return options;
    }

    /// <summary>The words an option is described with: who made the set, and what it is.</summary>
    /// <param name="set">The set to describe.</param>
    /// <returns>The description, in the language the program speaks.</returns>
    public static string Detail(IconSet set)
    {
        ArgumentNullException.ThrowIfNull(set);

        var authors = string.Join(", ", set.Authors);

        if (authors.Length == 0)
        {
            return set.Description;
        }

        return set.Description.Length == 0
            ? RolaI18N.Get("rorolala_file_system.setting.icon_set_author", authors)
            : RolaI18N.Get(
                "rorolala_file_system.setting.icon_set_by",
                authors,
                set.Description
            );
    }

    /// <summary>
    /// The picture an entry is drawn with, or nothing when the system's own icon is what to draw.
    /// </summary>
    /// <param name="entry">The entry to draw.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>The picture, or nothing to keep the system's own.</returns>
    public static Bitmap? Picture(Entry entry, int size)
    {
        ArgumentNullException.ThrowIfNull(entry);

        IconSet? set;
        var category = IconRules.Of(entry);

        lock (Gate)
        {
            set = _sets.FirstOrDefault(candidate => string.Equals(candidate.Id, _selected, StringComparison.Ordinal));

            if (set is null || !States.TryGetValue(set.Id, out var progress) || !progress.Ready)
            {
                return null;
            }
        }

        var source = set.Sources.FirstOrDefault(candidate => candidate.Category == category);

        // A file of a kind the set has no picture for is drawn as the set's generic file, which is what a
        // set's binary picture is for. A directory is not asked for one: a folder drawn as a document is a
        // folder a reader would take for a file, and the system's own folder is better than that.
        if (source is null && category != IconCategory.Directory)
        {
            source = set.Sources.FirstOrDefault(candidate => candidate.Category == IconCategory.Binary);
        }

        if (source is null)
        {
            return null;
        }

        var folder = CacheDirectory(set.Id);

        // The set states the sizes it was drawn at, and a listing may ask for any other — a grid's zoom is
        // continuous. The nearest drawn size is read and the control scales it, which keeps a zoom from
        // asking the set to be drawn at every size the pointer passes through.
        foreach (var candidate in Nearest(set.Sizes, size))
        {
            if (Loaded(Path.Combine(folder, NameOf(source.RelativePath, candidate))) is { } picture)
            {
                return picture;
            }
        }

        return null;
    }

    /// <summary>The file name a source is drawn to at one size.</summary>
    /// <param name="relativePath">The source's path within the set.</param>
    /// <param name="size">The pixel size.</param>
    /// <returns>The name, under the set's cache directory.</returns>
    public static string NameOf(string relativePath, int size) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"{Path.GetFileNameWithoutExtension(relativePath)}.x{size}.png"
        );

    /// <summary>The directory one set's cache is written to, under the program's own cache root.</summary>
    /// <param name="id">The set's directory name.</param>
    /// <returns>The cache directory, which may not exist yet.</returns>
    public static string CacheDirectory(string id) => Path.Combine(CacheRoot, id);

    /// <summary>
    /// Draws a set into a cache, or agrees that the cache already describes it.
    /// </summary>
    /// <remarks>
    /// The manifest is written last, so a run that stopped halfway leaves a cache that is not described as
    /// complete and is drawn again rather than a cache that claims to be whole. A source whose SHA-1 differs
    /// from the manifest's is enough to draw the whole set again: the pictures within a set are meant to be
    /// one look, and a set half drawn from an old source and half from a new one is a set of two looks.
    /// </remarks>
    /// <param name="set">The set to draw.</param>
    /// <param name="cacheRoot">The directory the cache root sits under, for a caller that keeps one elsewhere.</param>
    /// <param name="warn">Where a picture that will not draw is reported.</param>
    /// <returns>Whether the cache now describes the set.</returns>
    public static bool Build(IconSet set, string cacheRoot, Action<string> warn)
    {
        ArgumentNullException.ThrowIfNull(set);
        ArgumentNullException.ThrowIfNull(cacheRoot);
        ArgumentNullException.ThrowIfNull(warn);

        var folder = Path.Combine(cacheRoot, set.Id);
        var sources = new SortedDictionary<string, string>(StringComparer.Ordinal);

        foreach (var source in set.Sources)
        {
            var full = Within(set.Folder, source.RelativePath);

            if (full is null)
            {
                warn($"the icon set `{set.Id}` names a source outside itself: {source.RelativePath}");
                continue;
            }

            if (!File.Exists(full))
            {
                warn($"the icon set `{set.Id}` is missing its source: {source.RelativePath}");
                continue;
            }

            sources[source.RelativePath] = full;
        }

        if (sources.Count == 0)
        {
            warn($"the icon set `{set.Id}` draws nothing and is not offered");
            return false;
        }

        var current = new Dictionary<string, string>(StringComparer.Ordinal);

        foreach (var (relative, full) in sources)
        {
            current[relative] = Sha1(full);
        }

        if (!Stale(folder, set, current))
        {
            return true;
        }

        Directory.CreateDirectory(folder);

        var manifest = new CacheManifest
        {
            Version = ManifestVersion,
            Set = set.Id,
            Sizes = [.. set.Sizes],
            Sources = new SortedDictionary<string, CacheSource>(StringComparer.Ordinal),
        };

        var produced = new HashSet<string>(StringComparer.Ordinal);
        var drawn = 0;

        foreach (var (relative, full) in sources)
        {
            var outputs = new List<string>();

            foreach (var size in set.Sizes)
            {
                var name = NameOf(relative, size);

                if (Render(full, Path.Combine(folder, name), size, warn))
                {
                    outputs.Add(name);
                    produced.Add(name);
                    drawn++;
                }
            }

            manifest.Sources[relative] = new CacheSource { Sha1 = current[relative], Outputs = outputs };
        }

        if (drawn == 0)
        {
            warn($"the icon set `{set.Id}` drew nothing and is not offered");
            return false;
        }

        WriteManifest(Path.Combine(folder, "manifest.json"), manifest);
        RemoveUnlisted(folder, produced, warn);

        return true;
    }

    /// <summary>
    /// Draws one set on a thread of its own, and says so when it is ready.
    /// </summary>
    /// <param name="set">The set to draw.</param>
    private static void Generate(IconSet set)
    {
        Progress progress;

        lock (Gate)
        {
            if (States.TryGetValue(set.Id, out var known) && (known.Running || known.Ready))
            {
                return;
            }

            progress = new Progress { Running = true };
            States[set.Id] = progress;
        }

        var warn = _warn;
        var ready = _ready;

        _ = Task.Run(() =>
        {
            var drawn = false;

            try
            {
                drawn = Build(set, CacheRoot, warn);
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                warn($"the icon set `{set.Id}` will not draw: {error.Message}");
            }

            lock (Gate)
            {
                // Only the run that is still the set's own says how it ended: a run that was replaced by
                // another choice is no longer what a reader of the set is waiting for.
                if (States.TryGetValue(set.Id, out var current) && ReferenceEquals(current, progress))
                {
                    progress.Running = false;
                    progress.Ready = drawn;
                }
            }

            if (drawn)
            {
                ready?.Invoke();
            }
        });
    }

    /// <summary>Whether a cache does not describe a set as it is now.</summary>
    /// <param name="folder">The set's cache directory.</param>
    /// <param name="set">The set as it is now.</param>
    /// <param name="current">The SHA-1 of every source as it is now.</param>
    /// <returns>Whether the set has to be drawn again.</returns>
    private static bool Stale(string folder, IconSet set, IReadOnlyDictionary<string, string> current)
    {
        var manifest = ReadManifest(Path.Combine(folder, "manifest.json"));

        if (manifest is null
            || manifest.Version != ManifestVersion
            || !string.Equals(manifest.Set, set.Id, StringComparison.Ordinal)
            || !manifest.Sizes.SequenceEqual(set.Sizes)
            || manifest.Sources.Count != current.Count)
        {
            return true;
        }

        foreach (var (relative, sha1) in current)
        {
            if (!manifest.Sources.TryGetValue(relative, out var source)
                || !string.Equals(source.Sha1, sha1, StringComparison.Ordinal)
                || source.Outputs.Count == 0)
            {
                return true;
            }

            foreach (var output in source.Outputs)
            {
                if (!File.Exists(Path.Combine(folder, output)))
                {
                    return true;
                }
            }
        }

        return false;
    }

    /// <summary>Draws one source into one PNG, writing through a temporary file so a reader never sees half of it.</summary>
    /// <param name="source">The source picture.</param>
    /// <param name="destination">Where the PNG is written.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <param name="warn">Where a picture that will not draw is reported.</param>
    /// <returns>Whether the picture was drawn.</returns>
    private static bool Render(string source, string destination, int size, Action<string> warn)
    {
        var temporary = $"{destination}.tmp";

        try
        {
            using var bitmap = new SKBitmap(new SKImageInfo(size, size, SKColorType.Rgba8888, SKAlphaType.Premul));
            using var canvas = new SKCanvas(bitmap);
            canvas.Clear(SKColors.Transparent);

            if (source.EndsWith(".svg", StringComparison.OrdinalIgnoreCase))
            {
                using var svg = new SKSvg();
                svg.Load(source);

                if (svg.Picture is not { } picture || picture.CullRect.Width <= 0 || picture.CullRect.Height <= 0)
                {
                    warn($"the icon `{source}` draws nothing");
                    return false;
                }

                var bounds = picture.CullRect;
                var scale = Math.Min(size / (double)bounds.Width, size / (double)bounds.Height);

                canvas.Translate(
                    (float)(((size - (bounds.Width * scale)) / 2) - (bounds.Left * scale)),
                    (float)(((size - (bounds.Height * scale)) / 2) - (bounds.Top * scale))
                );
                canvas.Scale((float)scale);
                canvas.DrawPicture(picture);
            }
            else
            {
                using var picture = SKBitmap.Decode(source);

                if (picture is null || picture.Width <= 0 || picture.Height <= 0)
                {
                    warn($"the icon `{source}` will not decode");
                    return false;
                }

                var scale = Math.Min(size / (double)picture.Width, size / (double)picture.Height);
                var width = (float)(picture.Width * scale);
                var height = (float)(picture.Height * scale);

                canvas.DrawBitmap(
                    picture,
                    SKRect.Create((size - width) / 2, (size - height) / 2, width, height)
                );
            }

            using var image = SKImage.FromBitmap(bitmap);
            using var data = image.Encode(SKEncodedImageFormat.Png, 100);

            if (data is null)
            {
                warn($"the icon `{source}` will not encode");
                return false;
            }

            using (var stream = File.Create(temporary))
            {
                data.SaveTo(stream);
            }

            File.Move(temporary, destination, true);

            return true;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            warn($"the icon `{source}` will not draw: {error.Message}");

            try
            {
                File.Delete(temporary);
            }
            catch (Exception leftover) when (leftover is IOException or UnauthorizedAccessException)
            {
                // A temporary file left behind is one the next draw overwrites, and a set that did not draw
                // is already being reported; failing to clean up is not a second thing worth saying.
            }

            return false;
        }
    }

    /// <summary>Reads one cached picture, or nothing when it will not read.</summary>
    /// <param name="path">The picture's path.</param>
    /// <returns>The picture, or nothing.</returns>
    private static Bitmap? Loaded(string path)
    {
        lock (Gate)
        {
            if (Read.TryGetValue(path, out var kept))
            {
                return kept;
            }
        }

        try
        {
            using var stream = File.OpenRead(path);
            var picture = new Bitmap(stream);

            lock (Gate)
            {
                Read[path] = picture;
            }

            return picture;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return null;
        }
    }

    /// <summary>The drawn sizes of a set, nearest to the size asked for first.</summary>
    /// <param name="sizes">The sizes the set states.</param>
    /// <param name="wanted">The size a control asks for.</param>
    /// <returns>The sizes to try, in order.</returns>
    private static IEnumerable<int> Nearest(IReadOnlyList<int> sizes, int wanted) =>
        sizes.OrderBy(size => Math.Abs(size - wanted)).ThenBy(size => size);

    /// <summary>The SHA-1 of a file.</summary>
    /// <remarks>
    /// SHA-1 is enough here because it is not a signature: it is asked one question — has this source changed
    /// since the cache was drawn from it — and an answer that a hostile file could collide with would cost a
    /// wrong icon, not a wrong decision about who may do what. It is also the shortest hash that answers the
    /// question, which keeps the manifest a person can read.
    /// </remarks>
    /// <param name="path">The file.</param>
    /// <returns>The hash, as lower-case hex.</returns>
    [SuppressMessage(
        "Security",
        "CA5350:Do Not Use Weak Cryptographic Algorithms",
        Justification = "the hash is a cache fingerprint, not a signature; a collision costs a stale icon and nothing else"
    )]
    private static string Sha1(string path)
    {
        using var stream = File.OpenRead(path);

        return Convert.ToHexString(SHA1.HashData(stream)).ToLowerInvariant();
    }

    /// <summary>
    /// The full path of a source, or nothing when it would leave the set's directory.
    /// </summary>
    /// <remarks>
    /// The path comes from a file that travels with the program, but a package is still a file a person may
    /// edit: a set that named <c>../../something</c> would be reading outside itself, which is not what an
    /// icon is.
    /// </remarks>
    /// <param name="folder">The set's directory.</param>
    /// <param name="relative">The path the set states.</param>
    /// <returns>The full path, or nothing when it is outside the set.</returns>
    private static string? Within(string folder, string relative)
    {
        var root = Path.GetFullPath(folder) + Path.DirectorySeparatorChar;
        var full = Path.GetFullPath(Path.Combine(folder, relative));
        var comparison = OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;

        return full.StartsWith(root, comparison) ? full : null;
    }

    /// <summary>Writes the manifest, replacing any manifest already there.</summary>
    /// <param name="path">Where it is written.</param>
    /// <param name="manifest">What it says.</param>
    private static void WriteManifest(string path, CacheManifest manifest)
    {
        var temporary = $"{path}.tmp";

        using (var stream = File.Create(temporary))
        {
            JsonSerializer.Serialize(stream, manifest, ManifestOptions);
        }

        File.Move(temporary, path, true);
    }

    /// <summary>Reads a manifest, or nothing when it will not read.</summary>
    /// <param name="path">Where it sits.</param>
    /// <returns>What it says, or nothing.</returns>
    private static CacheManifest? ReadManifest(string path)
    {
        if (!File.Exists(path))
        {
            return null;
        }

        try
        {
            using var stream = File.OpenRead(path);

            return JsonSerializer.Deserialize<CacheManifest>(stream, ManifestOptions);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            return null;
        }
    }

    /// <summary>Removes the pictures a cache holds that this drawing did not produce.</summary>
    /// <param name="folder">The set's cache directory.</param>
    /// <param name="produced">The names this drawing produced.</param>
    /// <param name="warn">Where a picture that will not go is reported.</param>
    private static void RemoveUnlisted(string folder, IReadOnlySet<string> produced, Action<string> warn)
    {
        foreach (var file in Directory.EnumerateFiles(folder, "*.png"))
        {
            if (produced.Contains(Path.GetFileName(file)))
            {
                continue;
            }

            try
            {
                File.Delete(file);
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                warn($"a cached icon will not go: {file}: {error.Message}");
            }
        }
    }

    /// <summary>Says a warning about a set, or says nothing when there is nowhere to say it.</summary>
    /// <param name="message">What to say.</param>
    private static void Warn(string message) => _warn(message);

    /// <summary>How one set's drawing is going.</summary>
    private sealed class Progress
    {
        /// <summary>Whether it is being drawn now.</summary>
        public bool Running { get; set; }

        /// <summary>Whether it has been drawn and can be read.</summary>
        public bool Ready { get; set; }
    }

    /// <summary>What one set's cache records about itself.</summary>
    private sealed class CacheManifest
    {
        /// <summary>The version of the shape this manifest is written in.</summary>
        public int Version { get; set; }

        /// <summary>The set it describes.</summary>
        public string Set { get; set; } = string.Empty;

        /// <summary>The sizes its pictures were drawn at.</summary>
        public IReadOnlyList<int> Sizes { get; set; } = [];

        /// <summary>The source files it was drawn from, by their path within the set.</summary>
        public SortedDictionary<string, CacheSource> Sources { get; set; } = new(StringComparer.Ordinal);
    }

    /// <summary>What one source file's cache is.</summary>
    private sealed class CacheSource
    {
        /// <summary>The SHA-1 of the source it was drawn from.</summary>
        public string Sha1 { get; set; } = string.Empty;

        /// <summary>The PNGs it was drawn into.</summary>
        public IReadOnlyList<string> Outputs { get; set; } = [];
    }

    /// <summary>How a manifest is written and read.</summary>
    private static readonly JsonSerializerOptions ManifestOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        WriteIndented = true,
    };

    /// <summary>Reads one set from the file that describes it.</summary>
    /// <param name="id">The directory name the set is known by.</param>
    /// <param name="folder">Where it sits.</param>
    /// <param name="theme">The file that describes it.</param>
    /// <param name="warn">Where a part that does not read is reported.</param>
    /// <returns>The set, or nothing when it draws nothing.</returns>
    private static IconSet? Parse(string id, string folder, string theme, Action<string> warn)
    {
        var model = TomlSerializer.Deserialize<TomlTable>(File.ReadAllText(theme));

        if (model is null)
        {
            warn($"the icon set `{id}` has no readable icon-theme.toml");
            return null;
        }

        var metadata = Table(model, "metadata");
        var generation = Table(model, "generation");
        var fallback = Table(model, "fallback");
        var sizes = Sizes(generation, id, warn);
        var sources = new List<IconSource>();

        foreach (var (key, category) in Fallbacks)
        {
            if (Text(fallback, key) is not { Length: > 0 } relative)
            {
                continue;
            }

            if (Within(folder, relative) is null)
            {
                warn($"the icon set `{id}` names a source outside itself: {relative}");
                continue;
            }

            sources.Add(new IconSource(category, relative));
        }

        if (sources.Count == 0)
        {
            warn($"the icon set `{id}` names no fallback picture");
            return null;
        }

        var name = Text(metadata, "name") is { Length: > 0 } called ? called.Trim() : id;
        var authors = Strings(metadata, "authors");
        var description = Text(metadata, "description")?.Trim() ?? string.Empty;

        return new IconSet(id, name, authors, description, folder, sources, sizes);
    }

    /// <summary>The sizes a set states, or the sizes one is assumed to be drawn at.</summary>
    /// <param name="generation">The set's generation table, or nothing.</param>
    /// <param name="id">The set, for a warning about a size that was not read.</param>
    /// <param name="warn">Where the warning is said.</param>
    /// <returns>The sizes, smallest first.</returns>
    private static IReadOnlyList<int> Sizes(TomlTable? generation, string id, Action<string> warn)
    {
        if (generation is null
            || !generation.TryGetValue("sizes", out var value)
            || value is not TomlArray stated)
        {
            return DefaultSizes;
        }

        var sizes = new SortedSet<int>();

        foreach (var entry in stated)
        {
            if (entry is long number && number is > 0 and <= 2048)
            {
                sizes.Add((int)number);
            }
            else
            {
                warn($"the icon set `{id}` states a size that is not a pixel size: {entry}");
            }
        }

        return sizes.Count == 0 ? DefaultSizes : [.. sizes];
    }

    /// <summary>One table under a key, or nothing.</summary>
    /// <param name="table">The table to read from.</param>
    /// <param name="key">The key it sits under.</param>
    /// <returns>The table, or nothing.</returns>
    private static TomlTable? Table(TomlTable? table, string key) =>
        table is not null && table.TryGetValue(key, out var value) ? value as TomlTable : null;

    /// <summary>One string under a key, or nothing.</summary>
    /// <param name="table">The table to read from.</param>
    /// <param name="key">The key it sits under.</param>
    /// <returns>The string, or nothing.</returns>
    private static string? Text(TomlTable? table, string key) =>
        table is not null && table.TryGetValue(key, out var value) ? value as string : null;

    /// <summary>The strings of one array under a key, or nothing.</summary>
    /// <param name="table">The table to read from.</param>
    /// <param name="key">The key it sits under.</param>
    /// <returns>The strings, in the order they are written.</returns>
    private static IReadOnlyList<string> Strings(TomlTable? table, string key)
    {
        if (table is null || !table.TryGetValue(key, out var value) || value is not TomlArray array)
        {
            return [];
        }

        var strings = new List<string>();

        foreach (var entry in array)
        {
            if (entry is string text && text.Trim() is { Length: > 0 } trimmed)
            {
                strings.Add(trimmed);
            }
        }

        return strings;
    }
}

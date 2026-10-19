using System.Globalization;

namespace RorolalaDesktop.SysIcons;

/// <summary>
/// The MIME type a freedesktop system says a file name has.
/// </summary>
/// <remarks>
/// The system's own glob table — <c>globs2</c> under the MIME directory — is the authority on which name
/// is which kind: it is the file <c>xdg-mime</c> and every file manager read, and reading it costs no
/// process per row. The user's table is read after the system's, so what a user added or corrected wins.
/// <para>
/// Only literal extensions and literal names are taken: a pattern with a wildcard of its own, or a
/// character class, is one this cannot answer without implementing the matching language as well. What is
/// left out falls back to the generic picture of the file's top-level kind, which is a picture rather than
/// no picture.
/// </para>
/// </remarks>
internal static class MimeGlobs
{
    /// <summary>The table, read once when the first name is asked about.</summary>
    private static readonly Lazy<Table> Known = new(Read, LazyThreadSafetyMode.ExecutionAndPublication);

    /// <summary>The MIME type a name has, or nothing when the system's table does not say.</summary>
    /// <param name="name">The file's name or path.</param>
    /// <returns>The MIME type, or nothing.</returns>
    public static string? TypeOf(string name) => Known.Value.Find(Path.GetFileName(name));

    /// <summary>Every name the table knows, by extension and by whole name.</summary>
    private sealed class Table
    {
        /// <summary>What each extension says, without its leading dot.</summary>
        private readonly Dictionary<string, Entry> _extensions = new(StringComparer.OrdinalIgnoreCase);

        /// <summary>What each whole name says, for a file whose name is its kind — a Makefile.</summary>
        private readonly Dictionary<string, Entry> _names = new(StringComparer.OrdinalIgnoreCase);

        /// <summary>Adds what one extension says, or corrects it when this table is the later one.</summary>
        /// <param name="extension">The extension, without its leading dot.</param>
        /// <param name="weight">How strong the system says the statement is.</param>
        /// <param name="mime">The MIME type.</param>
        public void Extension(string extension, int weight, string mime) =>
            Adding(_extensions, extension, weight, mime);

        /// <summary>Adds what one whole name says, or corrects it when this table is the later one.</summary>
        /// <param name="name">The whole name.</param>
        /// <param name="weight">How strong the system says the statement is.</param>
        /// <param name="mime">The MIME type.</param>
        public void Name(string name, int weight, string mime) =>
            Adding(_names, name, weight, mime);

        /// <summary>The MIME type a name has, or nothing.</summary>
        /// <param name="name">The file's name, with no directory part.</param>
        /// <returns>The MIME type, or nothing.</returns>
        public string? Find(string name)
        {
            var extension = Path.GetExtension(name);

            if (extension.Length > 1 && _extensions.TryGetValue(extension[1..], out var byExtension))
            {
                return byExtension.Mime;
            }

            return _names.TryGetValue(name, out var byName) ? byName.Mime : null;
        }

        /// <summary>Adds one statement, letting a later table of the same weight correct an earlier one.</summary>
        /// <param name="entries">The table to add to.</param>
        /// <param name="key">The extension or whole name.</param>
        /// <param name="weight">How strong the system says the statement is.</param>
        /// <param name="mime">The MIME type.</param>
        private static void Adding(
            Dictionary<string, Entry> entries,
            string key,
            int weight,
            string mime
        )
        {
            if (!entries.TryGetValue(key, out var known) || weight >= known.Weight)
            {
                entries[key] = new Entry(weight, mime);
            }
        }

        /// <summary>One statement: how strong it is, and what it says.</summary>
        /// <param name="Weight">The weight the system gives it.</param>
        /// <param name="Mime">The MIME type.</param>
        private readonly record struct Entry(int Weight, string Mime);
    }

    /// <summary>Reads the system's table and then the user's, which corrects it.</summary>
    /// <returns>The table.</returns>
    private static Table Read()
    {
        var table = new Table();

        foreach (var file in Files())
        {
            Read(file, table);
        }

        return table;
    }

    /// <summary>Reads one glob table, or says nothing when it is not there or will not read.</summary>
    /// <param name="file">The table's path.</param>
    /// <param name="table">The table to add to.</param>
    private static void Read(string file, Table table)
    {
        if (!File.Exists(file))
        {
            return;
        }

        try
        {
            foreach (var raw in File.ReadLines(file))
            {
                Parsed(raw.Trim(), table);
            }
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            // A table that will not read is one file fewer of the answers, which the generic pictures are
            // for; it is not a reason to stop drawing a listing.
        }
    }

    /// <summary>
    /// Reads one line of the table: <c>weight:mime:glob</c>, with the glob's flags after another colon.
    /// </summary>
    /// <param name="line">The line, trimmed.</param>
    /// <param name="table">The table to add to.</param>
    private static void Parsed(string line, Table table)
    {
        if (line.Length == 0 || line[0] == '#')
        {
            return;
        }

        var first = line.IndexOf(':', StringComparison.Ordinal);
        var second = -1;

        if (first >= 0)
        {
            var offset = line.AsSpan(first + 1).IndexOf(':');
            second = offset < 0 ? -1 : first + 1 + offset;
        }

        if (first <= 0 || second <= first + 1)
        {
            return;
        }

        if (!int.TryParse(line[..first], NumberStyles.Integer, CultureInfo.InvariantCulture, out var weight))
        {
            return;
        }

        var mime = line[(first + 1)..second];

        if (mime.Length == 0)
        {
            return;
        }

        var rest = line[(second + 1)..];
        var flags = rest.IndexOf(':', StringComparison.Ordinal);
        var glob = flags < 0 ? rest : rest[..flags];

        if (glob.Length == 0 || glob == "*")
        {
            return;
        }

        if (glob.StartsWith("*.", StringComparison.Ordinal))
        {
            var extension = glob[2..];

            if (extension.Length == 0
                || extension.Contains('*', StringComparison.Ordinal)
                || extension.Contains('?', StringComparison.Ordinal)
                || extension.Contains('[', StringComparison.Ordinal))
            {
                return;
            }

            table.Extension(extension, weight, mime);
            return;
        }

        if (glob.Contains('*', StringComparison.Ordinal)
            || glob.Contains('?', StringComparison.Ordinal)
            || glob.Contains('[', StringComparison.Ordinal))
        {
            return;
        }

        table.Name(glob, weight, mime);
    }

    /// <summary>The tables to read, the user's last so that it corrects the system's.</summary>
    /// <returns>The paths, in the order they are read.</returns>
    private static IEnumerable<string> Files()
    {
        yield return "/usr/share/mime/globs2";
        yield return "/usr/local/share/mime/globs2";

        var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        var data = Environment.GetEnvironmentVariable("XDG_DATA_HOME");

        if (string.IsNullOrEmpty(data))
        {
            data = Path.Combine(home, ".local", "share");
        }

        yield return Path.Combine(data, "mime", "globs2");
    }
}

namespace RorolalaLauncher;

/// <summary>
/// The Workspaces this machine has opened, kept in <c>workspaces.toml</c>.
/// </summary>
/// <remarks>
/// The file states one thing: a list of directories. What each one is, is read from the directory every
/// time the list is drawn rather than written down here — a Workspace can be moved, taken away, or made
/// over a directory the list still names, and a state kept in this file would be a second answer to a
/// question the file system already answers.
/// <para>
/// Paths are kept as they were chosen and compared as the file system compares them: without regard to
/// case on Windows, and with it everywhere else.
/// </para>
/// </remarks>
internal sealed class WorkspacesFile
{
    /// <summary>The key the list is written under.</summary>
    private const string WorkspacesKey = "workspaces";

    /// <summary>Every directory the file names, in the order it names them.</summary>
    public List<string> Paths { get; } = [];

    /// <summary>How two paths are compared on this machine.</summary>
    private static StringComparison Comparison =>
        OperatingSystem.IsWindows() || OperatingSystem.IsMacOS()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal;

    /// <summary>Reads the file, or answers an empty list when there is none.</summary>
    /// <param name="path">The file.</param>
    /// <returns>What it names.</returns>
    /// <exception cref="IOException">The file is there and could not be read.</exception>
    public static WorkspacesFile Read(string path)
    {
        var file = new WorkspacesFile();

        if (!File.Exists(path))
        {
            return file;
        }

        file.Paths.AddRange(ReadPaths(File.ReadAllText(path)));

        return file;
    }

    /// <summary>
    /// Adds a directory to the list, once.
    /// </summary>
    /// <remarks>
    /// What is kept is the whole path, so a directory named twice in two spellings is one entry: what the
    /// list is for is a place on the machine, not the words a run happened to write it with.
    /// </remarks>
    /// <param name="path">The directory.</param>
    /// <returns>Whether it was not there already.</returns>
    public bool Add(string path)
    {
        var whole = Whole(path);

        if (Paths.Exists(held => string.Equals(held, whole, Comparison)))
        {
            return false;
        }

        Paths.Add(whole);

        return true;
    }

    /// <summary>
    /// Takes a directory off the list.
    /// </summary>
    /// <remarks>
    /// Nothing on the disk is touched: the list is what this program remembers, and a directory taken off it
    /// is one it will not offer again — importing it puts it back, which is what makes this a list being
    /// tidied rather than a Workspace being removed.
    /// </remarks>
    /// <param name="path">The directory.</param>
    /// <returns>Whether it was on the list.</returns>
    public bool Remove(string path)
    {
        var whole = Whole(path);

        return Paths.RemoveAll(held => string.Equals(held, whole, Comparison)) > 0;
    }

    /// <summary>Writes the file, making the directory it sits in.</summary>
    /// <param name="path">The file.</param>
    /// <exception cref="IOException">The file could not be written.</exception>
    public void Write(string path)
    {
        var directory = Path.GetDirectoryName(path);

        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var text = new System.Text.StringBuilder();

        text.Append(WorkspacesKey).Append(" = [\n");

        foreach (var held in Paths)
        {
            text.Append('\t').Append(TomlLine.Quote(held)).Append(",\n");
        }

        text.Append("]\n");

        File.WriteAllText(path, text.ToString());
    }

    /// <summary>
    /// The paths a file states.
    /// </summary>
    /// <remarks>
    /// Read as an array rather than as a line, so that the list may be written one path a line — which is
    /// how a person edits one — or all on one line, which is how a program that wrote it whole would. A
    /// file this cannot make sense of is answered as no paths: the list is only ever a convenience, and a
    /// program that refused to open over it would be refusing over the wrong thing.
    /// </remarks>
    /// <param name="text">The file's text.</param>
    /// <returns>Every quoted path in the list the file states.</returns>
    private static List<string> ReadPaths(string text)
    {
        var paths = new List<string>();
        var list = ArrayBody(text);

        if (list is null)
        {
            return paths;
        }

        foreach (var element in Elements(list))
        {
            var held = TomlLine.String(element);

            if (held.Length > 0)
            {
                paths.Add(held);
            }
        }

        return paths;
    }

    /// <summary>The text between the list's brackets, comment removed, or nothing.</summary>
    /// <param name="text">The file's text.</param>
    /// <returns>What the array holds, or nothing when the file states no array.</returns>
    private static string? ArrayBody(string text)
    {
        var open = -1;
        var at = 0;

        foreach (var line in text.Split('\n'))
        {
            var (key, value) = TomlLine.Split(line);

            if (key is not null && string.Equals(key, WorkspacesKey, StringComparison.Ordinal))
            {
                if (value is not null && value.StartsWith('['))
                {
                    open = at + line.IndexOf('[', StringComparison.Ordinal);
                    break;
                }
            }

            at += line.Length + 1;
        }

        if (open < 0)
        {
            return null;
        }

        // The closing bracket is the first one outside a string: a path may hold one, and a file may hold a
        // comment after the list, and neither is the end of the array.
        var close = Closing(text, open + 1);

        return close > open ? WithoutComments(text[(open + 1)..close]) : null;
    }

    /// <summary>Where an array ends, which is the first closing bracket outside a string.</summary>
    /// <param name="text">The file's text.</param>
    /// <param name="from">Where the array's body starts.</param>
    /// <returns>Where its bracket stands, or -1 when the file states none.</returns>
    private static int Closing(string text, int from)
    {
        var quote = '\0';

        for (var at = from; at < text.Length; at++)
        {
            var character = text[at];

            if (quote != '\0')
            {
                if (character == '\\' && quote == '"')
                {
                    at++;
                }
                else if (character == quote)
                {
                    quote = '\0';
                }

                continue;
            }

            if (character is '"' or '\'')
            {
                quote = character;
                continue;
            }

            if (character == ']')
            {
                return at;
            }
        }

        return -1;
    }

    /// <summary>
    /// The text without the comments written in it, which a path named in one is not part of.
    /// </summary>
    /// <remarks>
    /// A `#` inside a quoted string is part of the string, so only one outside quotes begins a comment, and
    /// a comment runs to the end of its line.
    /// </remarks>
    /// <param name="text">The text.</param>
    /// <returns>What is left of it.</returns>
    private static string WithoutComments(string text)
    {
        var written = new System.Text.StringBuilder(text.Length);
        var quote = '\0';
        var comment = false;

        for (var at = 0; at < text.Length; at++)
        {
            var character = text[at];

            if (comment)
            {
                if (character == '\n')
                {
                    comment = false;
                    written.Append(character);
                }

                continue;
            }

            if (quote != '\0')
            {
                written.Append(character);

                if (character == '\\' && quote == '"' && at + 1 < text.Length)
                {
                    // An escape and what it escapes are both part of the string, whatever the second is.
                    written.Append(text[++at]);
                }
                else if (character == quote)
                {
                    quote = '\0';
                }

                continue;
            }

            if (character is '"' or '\'')
            {
                quote = character;
                written.Append(character);
                continue;
            }

            if (character == '#')
            {
                comment = true;
                continue;
            }

            written.Append(character);
        }

        return written.ToString();
    }

    /// <summary>
    /// The quoted strings an array's body holds, one per element.
    /// </summary>
    /// <remarks>
    /// An element is a string literal, which is all this file ever holds; brackets nested inside one are
    /// read as the string they are, because the reading is done with the quotes in mind.
    /// </remarks>
    /// <param name="body">What the array's brackets hold.</param>
    /// <returns>Each element as it was written.</returns>
    private static IEnumerable<string> Elements(string body)
    {
        var at = 0;

        while (at < body.Length)
        {
            var character = body[at];

            if (character is '"' or '\'')
            {
                var end = at + 1;

                while (end < body.Length)
                {
                    if (character == '"' && body[end] == '\\')
                    {
                        end += 2;
                        continue;
                    }

                    if (body[end] == character)
                    {
                        break;
                    }

                    end++;
                }

                if (end >= body.Length)
                {
                    yield return body[at..];
                    yield break;
                }

                yield return body[at..(end + 1)];
                at = end + 1;
                continue;
            }

            at++;
        }
    }

    /// <summary>The whole of a path, so that two spellings of one place are one entry.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The whole path, without the separator a directory may end with.</returns>
    private static string Whole(string path)
    {
        string whole;

        try
        {
            whole = Path.GetFullPath(path);
        }
        catch (Exception error) when (error is ArgumentException or NotSupportedException or PathTooLongException)
        {
            whole = path;
        }

        return whole.Length > 1 ? whole.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) : whole;
    }
}

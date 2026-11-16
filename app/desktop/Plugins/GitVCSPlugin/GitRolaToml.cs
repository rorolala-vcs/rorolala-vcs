using System.Text.Json;

namespace GitVCSPlugin;

/// <summary>
/// The little of a git-rola project's <c>rola.toml</c> this plugin reads: its Vaults and its default
/// one.
/// </summary>
/// <remarks>
/// The file is a person's, so it is read a line at a time rather than by a parser of the whole
/// language: what the view needs is which Vaults the project declares, and which of them the
/// mappings go through, and both are assignments. Comments, blank lines and every section this
/// plugin does not read are skipped, which keeps a file a person wrote from being refused for a
/// reason that is not this plugin's.
/// </remarks>
internal static class GitRolaToml
{
    /// <summary>The file a git-rola project is recognised by, beside its <c>.git</c>.</summary>
    public const string FileName = "rola.toml";

    /// <summary>The project file of the directory.</summary>
    /// <param name="directory">The directory.</param>
    /// <returns>The file's path.</returns>
    public static string PathOf(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        return System.IO.Path.Combine(directory, FileName);
    }

    /// <summary>Whether a directory is the root of a git-rola project.</summary>
    /// <remarks>
    /// Both files are required: a <c>rola.toml</c> outside a git repository is a file with nothing
    /// to place, and a git repository without one is a repository this program has no project for.
    /// The <c>.git</c> of a work tree is a file rather than a directory, which is why both are
    /// asked about.
    /// </remarks>
    /// <param name="directory">The directory.</param>
    /// <returns>Whether it is one.</returns>
    public static bool IsProject(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var git = System.IO.Path.Combine(directory, ".git");

        return File.Exists(PathOf(directory)) && (Directory.Exists(git) || File.Exists(git));
    }

    /// <summary>The nearest directory at or above the one named that is a project.</summary>
    /// <param name="directory">The directory to start from.</param>
    /// <returns>The project's root, or nothing when there is none.</returns>
    public static string? ProjectOf(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        for (var at = new DirectoryInfo(directory); at is not null; at = at.Parent)
        {
            if (IsProject(at.FullName))
            {
                return at.FullName;
            }
        }

        return null;
    }

    /// <summary>Reads the project file at <paramref name="path"/>.</summary>
    /// <param name="path">The file.</param>
    /// <returns>What it declares, or an empty project when it cannot be read.</returns>
    public static GitRolaProject ReadFile(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        try
        {
            return Read(File.ReadAllText(path));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return GitRolaProject.Empty;
        }
    }

    /// <summary>Reads the Vaults and the default one out of the file's text.</summary>
    /// <param name="text">The file's text.</param>
    /// <returns>What it declares.</returns>
    public static GitRolaProject Read(string text)
    {
        ArgumentNullException.ThrowIfNull(text);

        var vaults = new List<string>();
        string? fallback = null;
        var section = string.Empty;

        foreach (var line in text.Split('\n'))
        {
            var trimmed = line.TrimEnd('\r').Trim();

            if (trimmed.Length == 0 || trimmed.StartsWith('#'))
            {
                continue;
            }

            if (trimmed.StartsWith('['))
            {
                section = trimmed.Trim('[', ']').Trim();
                continue;
            }

            var (key, value) = Split(trimmed);

            if (key is null || value is null)
            {
                continue;
            }

            switch (section)
            {
                case "vaults":
                    vaults.Add(key);
                    break;
                case "defaults" when string.Equals(key, "vault", StringComparison.Ordinal):
                    fallback = value;
                    break;
                default:
                    break;
            }
        }

        return new GitRolaProject(vaults, fallback);
    }

    /// <summary>The key and the text of a value a line states, without the value's comment.</summary>
    /// <param name="line">The line.</param>
    /// <returns>The two, or nothing when the line states neither.</returns>
    private static (string? Key, string? Value) Split(string line)
    {
        var text = WithoutComment(line);
        var assign = OutsideQuotes(text, '=');

        if (assign < 0)
        {
            return (null, null);
        }

        var key = text[..assign].Trim().Trim('"', '\'');
        var value = Unquoted(text[(assign + 1)..].Trim());

        return key.Length == 0 || value.Length == 0 ? (null, null) : (key, value);
    }

    /// <summary>The text a written string stands for, or the text itself where it was not one.</summary>
    /// <remarks>
    /// A JSON string literal is also a TOML basic string, so one escape rule serves both and no
    /// character a Vault's address may hold is lost.
    /// </remarks>
    /// <param name="value">What a line states.</param>
    /// <returns>The string it states.</returns>
    private static string Unquoted(string value)
    {
        var text = value.Trim();

        if (text.Length >= 2 && text[0] == '\'' && text[^1] == '\'')
        {
            return text[1..^1];
        }

        if (text.Length >= 2 && text[0] == '"' && text[^1] == '"')
        {
            try
            {
                return JsonSerializer.Deserialize<string>(text) ?? text;
            }
            catch (JsonException)
            {
                return text[1..^1];
            }
        }

        return text;
    }

    /// <summary>The line without the comment it carries, if any.</summary>
    /// <param name="line">The line.</param>
    /// <returns>What is left of it.</returns>
    private static string WithoutComment(string line)
    {
        var comment = OutsideQuotes(line, '#');

        return comment < 0 ? line : line[..comment];
    }

    /// <summary>The first place a character stands outside any quoted string, or -1.</summary>
    /// <param name="text">The text.</param>
    /// <param name="wanted">The character.</param>
    /// <returns>Where it stands, or -1.</returns>
    private static int OutsideQuotes(string text, char wanted)
    {
        var quote = '\0';

        for (var at = 0; at < text.Length; at++)
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

            if (character == wanted)
            {
                return at;
            }
        }

        return -1;
    }
}

/// <summary>The Vaults a project names, and which of them its mappings go through.</summary>
/// <param name="Vaults">The Vaults, in the order the file states them.</param>
/// <param name="Default">The Vault <c>defaults.vault</c> names, or nothing when it states none.</param>
internal sealed record GitRolaProject(IReadOnlyList<string> Vaults, string? Default)
{
    /// <summary>A project that names nothing.</summary>
    public static GitRolaProject Empty { get; } = new([], null);

    /// <summary>
    /// The Vault a mapping goes through when the pin names none: the stated default, then
    /// <c>origin</c>, then the first the project declares.
    /// </summary>
    public string? DefaultVault =>
        Default ?? (Vaults.Contains("origin") ? "origin" : Vaults.Count > 0 ? Vaults[0] : null);
}

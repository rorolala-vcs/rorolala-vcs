namespace GitVCSPlugin;

/// <summary>The materials a git-rola project's <c>rola.lock</c> names.</summary>
/// <remarks>
/// A lock line is the format's own rather than a person's: the header lines open with <c>?</c>, and
/// a material is a path, a content hash and the word <c>file</c> or <c>tree</c>, separated by
/// single spaces. What is needed here is the paths alone, as the file tree is drawn against them.
/// </remarks>
internal static class GitRolaLock
{
    /// <summary>The file the placed materials are written in.</summary>
    public const string FileName = "rola.lock";

    /// <summary>Every path the lock names, written the way the project writes one.</summary>
    /// <param name="text">The lock's text.</param>
    /// <returns>The paths, without a leading slash.</returns>
    public static IReadOnlySet<string> Materials(string text)
    {
        ArgumentNullException.ThrowIfNull(text);

        var materials = new HashSet<string>(StringComparer.Ordinal);

        foreach (var line in text.Split('\n'))
        {
            var trimmed = line.TrimEnd('\r');

            if (trimmed.Length == 0 || trimmed.StartsWith('?'))
            {
                continue;
            }

            var space = trimmed.IndexOf(' ', StringComparison.Ordinal);
            var path = (space < 0 ? trimmed : trimmed[..space]).TrimStart('/');

            if (path.Length > 0)
            {
                materials.Add(path);
            }
        }

        return materials;
    }

    /// <summary>Every path the lock names as a tree rather than as a file.</summary>
    /// <param name="text">The lock's text.</param>
    /// <returns>The paths, without a leading slash.</returns>
    public static IReadOnlySet<string> Trees(string text)
    {
        ArgumentNullException.ThrowIfNull(text);

        var trees = new HashSet<string>(StringComparer.Ordinal);

        foreach (var line in text.Split('\n'))
        {
            var trimmed = line.TrimEnd('\r');

            if (trimmed.Length == 0 || trimmed.StartsWith('?'))
            {
                continue;
            }

            var first = trimmed.IndexOf(' ', StringComparison.Ordinal);
            var kind = trimmed.LastIndexOf(' ');

            if (kind < 0 || !trimmed[(kind + 1)..].Equals("tree", StringComparison.Ordinal))
            {
                continue;
            }

            var path = (first < 0 ? trimmed : trimmed[..first]).TrimStart('/');

            if (path.Length > 0)
            {
                trees.Add(path);
            }
        }

        return trees;
    }
}

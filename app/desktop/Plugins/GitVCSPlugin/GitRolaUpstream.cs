namespace GitVCSPlugin;

/// <summary>One entry a Vault's Layout names.</summary>
/// <param name="Path">The path, relative to the Layout's root.</param>
/// <param name="Folder">Whether it is a tree rather than a file.</param>
internal sealed record GitRolaUpstreamEntry(string Path, bool Folder);

/// <summary>One row of a level of a Vault's Layout.</summary>
/// <param name="Path">The path the row stands for.</param>
/// <param name="Name">What it is called, which is the last part of the path.</param>
/// <param name="Folder">Whether it is a folder to open.</param>
internal sealed record GitRolaRow(string Path, string Name, bool Folder);

/// <summary>What the `git rola upstream` run answered, and how a level is read from it.</summary>
/// <remarks>
/// The command reads the Vault's Layout from the daemon, which is a run rather than a file, so this
/// is the half a provider may not do on the window's thread. What it writes is one entry a line: the
/// path, a tab, and `file` or `tree`. A path that has entries under it is a folder, which is what
/// makes a level from the flat answer.
/// <para>
/// A tree is drawn as a file, the way `rola@` draws one: the Layout holds a tree as one entry rather
/// than as the paths inside it, so it is a thing to bring in rather than a level to open. What a level
/// folds is only the paths beside each other, and a tree's own name is one of them.
/// </para>
/// </remarks>
internal static class GitRolaUpstream
{
    /// <summary>Runs `git-rola upstream` for the Vault and reads its answer.</summary>
    /// <param name="program">The `git-rola` program.</param>
    /// <param name="directory">The project's root.</param>
    /// <param name="vault">The Vault to read.</param>
    /// <returns>The entries, and what the run said when it failed.</returns>
    public static (IReadOnlyList<GitRolaUpstreamEntry> Entries, string? Said) Read(
        string program,
        string directory,
        string vault
    )
    {
        var (code, output, said) = GitRolaCommandLine.Run(program, ["upstream", vault], directory);

        if (code != 0)
        {
            return ([], said.Length > 0 ? said : $"exit {code}");
        }

        var entries = new List<GitRolaUpstreamEntry>();

        foreach (var line in output.Split('\n'))
        {
            var trimmed = line.TrimEnd('\r');
            var tab = trimmed.LastIndexOf('\t');

            if (tab <= 0 || tab == trimmed.Length - 1)
            {
                continue;
            }

            var path = trimmed[..tab];

            if (path.Length > 0)
            {
                entries.Add(new GitRolaUpstreamEntry(path, string.Equals(trimmed[(tab + 1)..], "tree", StringComparison.Ordinal)));
            }
        }

        return (entries, null);
    }

    /// <summary>The rows standing directly under a prefix.</summary>
    /// <param name="entries">What the Layout names.</param>
    /// <param name="prefix">The prefix, empty for the top of the Layout.</param>
    /// <returns>The rows, folders first and then by name.</returns>
    public static IReadOnlyList<GitRolaRow> Level(
        IReadOnlyList<GitRolaUpstreamEntry> entries,
        string prefix
    )
    {
        ArgumentNullException.ThrowIfNull(entries);
        ArgumentNullException.ThrowIfNull(prefix);

        var names = new Dictionary<string, bool>(StringComparer.Ordinal);

        foreach (var entry in entries)
        {
            if (!Under(entry.Path, prefix, out var rest) || rest.Length == 0)
            {
                continue;
            }

            var slash = rest.IndexOf('/', StringComparison.Ordinal);
            var name = slash < 0 ? rest : rest[..slash];

            names[name] = names.TryGetValue(name, out var known) ? known || slash >= 0 : slash >= 0;
        }

        return names
            .OrderBy(pair => pair.Value ? 0 : 1)
            .ThenBy(pair => pair.Key, StringComparer.OrdinalIgnoreCase)
            .ThenBy(pair => pair.Key, StringComparer.Ordinal)
            .Select(pair => new GitRolaRow(Join(prefix, pair.Key), pair.Key, pair.Value))
            .ToList();
    }

    /// <summary>What a path is under a prefix, or nothing when it is not under it.</summary>
    /// <param name="path">The path.</param>
    /// <param name="prefix">The prefix, empty for the top.</param>
    /// <param name="rest">What is left of the path.</param>
    /// <returns>Whether it is under the prefix.</returns>
    private static bool Under(string path, string prefix, out string rest)
    {
        if (prefix.Length == 0)
        {
            rest = path;

            return true;
        }

        if (string.Equals(path, prefix, StringComparison.Ordinal))
        {
            rest = string.Empty;

            return false;
        }

        if (path.Length > prefix.Length
            && path.StartsWith(prefix, StringComparison.Ordinal)
            && path[prefix.Length] == '/')
        {
            rest = path[(prefix.Length + 1)..];

            return true;
        }

        rest = string.Empty;

        return false;
    }

    /// <summary>A name under a prefix, which is the name itself at the top.</summary>
    /// <param name="prefix">The prefix.</param>
    /// <param name="name">The name.</param>
    /// <returns>The path.</returns>
    private static string Join(string prefix, string name) =>
        prefix.Length == 0 ? name : prefix + "/" + name;
}

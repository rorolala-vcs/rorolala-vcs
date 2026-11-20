namespace RorolalaLauncher;

/// <summary>What a directory on the list is, as the file system says it.</summary>
internal enum WorkspaceState
{
    /// <summary>A Workspace: a directory holding the data directory a Workspace keeps.</summary>
    Workspace,

    /// <summary>A git repository with a rola.toml, which the Desktop's git-rola view opens on.</summary>
    GitProject,

    /// <summary>The directory is not there.</summary>
    Missing,

    /// <summary>The directory is there and holds no Workspace.</summary>
    NotWorkspace,
}

/// <summary>
/// What a directory is, read the way the command line reads one.
/// </summary>
/// <remarks>
/// A Workspace is a directory holding <c>.rola</c>, which is the one thing that makes a directory one. It
/// is read every time a card is drawn rather than kept: a Workspace may be moved, taken away, or made over
/// a directory the list still names, and a state kept beside the path would be a second answer to a
/// question the file system answers already.
/// </remarks>
internal static class Workspaces
{
    /// <summary>The directory a Workspace keeps its own files under.</summary>
    public const string DataDirectory = ".rola";

    /// <summary>The file that, beside a <c>.git</c>, makes a directory a git-rola project.</summary>
    public const string ProjectFile = "rola.toml";

    /// <summary>
    /// Whether a directory is a git-rola project: a git repository holding a <c>rola.toml</c>.
    /// </summary>
    /// <remarks>
    /// It is asked apart from <see cref="Of"/> because a directory can be both a Workspace and a git
    /// checkout of one: the card then opens as a Workspace and still wears the Git mark, rather than the
    /// mark disappearing because the data directory happened to be there first.
    /// </remarks>
    /// <param name="path">The directory.</param>
    /// <returns>Whether it is a git-rola project.</returns>
    public static bool IsGit(string path)
    {
        var git = Path.Combine(path, ".git");

        return File.Exists(Path.Combine(path, ProjectFile))
            && (Directory.Exists(git) || File.Exists(git));
    }

    /// <summary>
    /// What a directory is.
    /// </summary>
    /// <remarks>
    /// A git-rola project is a git repository with a <c>rola.toml</c>. It is not a Workspace, and it is worth
    /// opening on anyway: the Desktop's git-rola view reads the project's Vaults and its lock, and the file tree
    /// shows the materials the lock places. A <c>.git</c> that is a file rather than a directory is what a work
    /// tree and a submodule have, which is why both are asked about.
    /// </remarks>
    /// <param name="path">The directory.</param>
    /// <returns>What it is.</returns>
    public static WorkspaceState Of(string path)
    {
        if (!Directory.Exists(path))
        {
            return WorkspaceState.Missing;
        }

        if (Directory.Exists(Path.Combine(path, DataDirectory)))
        {
            return WorkspaceState.Workspace;
        }

        return IsGit(path) ? WorkspaceState.GitProject : WorkspaceState.NotWorkspace;
    }

    /// <summary>
    /// What a Workspace is called on a card: the name of the directory it is rooted at.
    /// </summary>
    /// <remarks>
    /// A path that ends with a separator still names the directory the separator is under, so the name is
    /// read from what is left after it is taken away.
    /// </remarks>
    /// <param name="path">The directory.</param>
    /// <returns>The name it is known by.</returns>
    public static string Named(string path)
    {
        // Both separators, because the name is read off a path a person chose: on Windows a path may
        // be written with either, and a path written the other way is still the path it names.
        var trimmed = path.TrimEnd('\\', '/');
        var index = trimmed.LastIndexOfAny(['\\', '/']);
        var name = index >= 0 ? trimmed[(index + 1)..] : trimmed;

        return name.Length > 0 ? name : trimmed;
    }
}

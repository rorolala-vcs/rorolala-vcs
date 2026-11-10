namespace RorolalaVCSPlugin;

/// <summary>
/// What a path inside a rola-tree names: the tree's root rather than the file it happens to be.
/// </summary>
/// <remarks>
/// A tree is one entry of the Layout, so the version graph can only stand on the tree's directory. A file
/// chosen inside one is a way into that tree, and what is read for it is the tree's own history; a file
/// that is not inside one is its own entry and is answered as it stands.
/// </remarks>
internal static class RolaTrees
{
    /// <summary>
    /// The root of the nearest tree holding a path, with no Workspace to stop at.
    /// </summary>
    /// <param name="path">The path, which may be a file or a directory.</param>
    /// <returns>The tree's path, or nothing.</returns>
    public static string? RootOf(string path) => Walk(path, workspace: null);

    /// <summary>
    /// The root of the nearest tree holding a path, or nothing when no tree holds it.
    /// </summary>
    /// <param name="path">The path, which may be a file or a directory.</param>
    /// <param name="workspace">Where the Workspace is rooted.</param>
    /// <returns>The tree's path, or nothing.</returns>
    public static string? RootOf(string path, string workspace) => Walk(path, workspace);

    /// <summary>Walks up from a path to the nearest tree, stopping at its Workspace when one is named.</summary>
    /// <param name="path">The path, which may be a file or a directory.</param>
    /// <param name="workspace">Where the Workspace is rooted, or nothing to walk to the volume's root.</param>
    /// <returns>The tree's path, or nothing.</returns>
    private static string? Walk(string path, string? workspace)
    {
        ArgumentNullException.ThrowIfNull(path);

        // The computer is a place with no path, and the process's own directory is not it.
        if (path.Length == 0)
        {
            return null;
        }

        var root = workspace is null
            ? null
            : Path.TrimEndingDirectorySeparator(Path.GetFullPath(workspace));
        string? at;

        try
        {
            at = Directory.Exists(path)
                ? Path.GetFullPath(path)
                : Path.GetDirectoryName(Path.GetFullPath(path));
        }
        catch (Exception error)
            when (error is ArgumentException or NotSupportedException or PathTooLongException)
        {
            // A place a browser opens is not always a filesystem path: an address has its own answerer,
            // and a path the platform refuses is not inside any tree.
            return null;
        }

        var comparison = OperatingSystem.IsWindows()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal;
        var below = root is null
            ? null
            : root.EndsWith(Path.DirectorySeparatorChar)
                ? root
                : root + Path.DirectorySeparatorChar;

        while (!string.IsNullOrEmpty(at))
        {
            if (
                root is not null
                && below is not null
                && !string.Equals(at, root, comparison)
                && !at.StartsWith(below, comparison)
            )
            {
                return null;
            }

            if (Directory.Exists(Path.Combine(at, ".rola-tree")))
            {
                return at;
            }

            if (root is not null && string.Equals(at, root, comparison))
            {
                break;
            }

            at = Path.GetDirectoryName(at);
        }

        return null;
    }
}

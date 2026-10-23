namespace RorolalaLauncher;

/// <summary>What a directory on the list is, as the file system says it.</summary>
internal enum WorkspaceState
{
    /// <summary>A Workspace: a directory holding the data directory a Workspace keeps.</summary>
    Workspace,

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

    /// <summary>
    /// What a directory is.
    /// </summary>
    /// <param name="path">The directory.</param>
    /// <returns>What it is.</returns>
    public static WorkspaceState Of(string path)
    {
        if (!Directory.Exists(path))
        {
            return WorkspaceState.Missing;
        }

        return Directory.Exists(Path.Combine(path, DataDirectory))
            ? WorkspaceState.Workspace
            : WorkspaceState.NotWorkspace;
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

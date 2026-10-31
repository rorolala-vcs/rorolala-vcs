namespace RorolalaVCSPlugin;

/// <summary>
/// The empty folders a place suggested, read against the copy of the work that stands beside them.
/// </summary>
/// <remarks>
/// A folder is only a path, so a suggestion and a folder of the work's own are the same string as soon as the work
/// comes to name what was suggested. The copy is what tells them apart: a suggestion is one the copy does not name,
/// and the moment it names it the suggestion has been used up and gives way to the path itself. Reading that here
/// rather than inside the place is what lets the answer be checked without a window, a Vault or a Workspace.
/// <para>
/// What a copy names is either an entry at the path or something under it: a Layout has no directories, so a folder
/// that holds entries is one the paths under it make.
/// </para>
/// </remarks>
internal static class UpstreamDrafts
{
    /// <summary>Whether the copy names a path, with an entry at it or with something under it.</summary>
    /// <param name="tree">The copy.</param>
    /// <param name="path">The path to ask about.</param>
    /// <returns>Whether the work names it.</returns>
    public static bool Covered(UpstreamEntriesTree tree, string path) =>
        tree.Find(path) is { Entry: not null } or { HasChildren: true };

    /// <summary>
    /// Whether a path is a suggestion this side made rather than one the work names.
    /// </summary>
    /// <remarks>
    /// A copy that has not been read names nothing and so takes nothing away: the suggestion holds until a read says
    /// otherwise, which is the state a place built without a Workspace is in.
    /// </remarks>
    /// <param name="tree">The copy, or nothing when none has been read.</param>
    /// <param name="drafts">The suggestions this side holds.</param>
    /// <param name="path">The path to ask about.</param>
    /// <returns>Whether it is still a suggestion.</returns>
    public static bool Ours(UpstreamEntriesTree? tree, IReadOnlySet<string> drafts, string path) =>
        drafts.Contains(path) && (tree is null || !Covered(tree, path));

    /// <summary>The suggestions the copy has named, which it has used up.</summary>
    /// <param name="tree">The copy.</param>
    /// <param name="drafts">The suggestions this side holds.</param>
    /// <returns>The paths that have given way, in no particular order.</returns>
    public static IReadOnlyList<string> Used(UpstreamEntriesTree tree, IReadOnlySet<string> drafts) =>
        [.. drafts.Where(path => Covered(tree, path))];
}

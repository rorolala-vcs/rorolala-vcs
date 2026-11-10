using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>What a path in a tree is: a new thing, or a thing that changed.</summary>
internal enum TreeWorkStatus
{
    /// <summary>Nothing the staging area holds has it, and the work does.</summary>
    Added,

    /// <summary>It is in both places, and what it is or holds changed.</summary>
    Modified,
}

/// <summary>How much of a path's work the tree's staging area holds.</summary>
internal enum TreeWorkStage
{
    /// <summary>Everything the path changed is staged.</summary>
    Staged,

    /// <summary>Some of it is staged and some is not.</summary>
    Partial,

    /// <summary>Nothing the path changed is staged.</summary>
    Unstaged,
}

/// <summary>What a tree says about one path inside it.</summary>
/// <param name="Status">Whether the path is new or changed.</param>
/// <param name="Stage">How much of its work is staged.</param>
internal sealed record TreeWork(TreeWorkStatus Status, TreeWorkStage Stage);

/// <summary>
/// What a tree's status says about one path inside it, gathered over everything under a directory.
/// </summary>
/// <remarks>
/// A tree names the paths it holds rather than the entries a Layout names, so a subdirectory is not a
/// change of its own: what it is, is what the changes under it add up to. A directory whose changes are all
/// additions is a new directory, and one with any change to what it already held is a changed directory.
/// </remarks>
internal static class RolaTreeWork
{
    /// <summary>What a tree says about a path, or nothing when the path has no unrecorded work.</summary>
    /// <param name="status">The tree's status.</param>
    /// <param name="tree">The tree's directory.</param>
    /// <param name="entry">The path inside the tree.</param>
    /// <returns>What it is and how much of it is staged, or nothing when nothing changed.</returns>
    public static TreeWork? Of(RolaTreeStatus status, string tree, Entry entry)
    {
        ArgumentNullException.ThrowIfNull(status);
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(entry);

        var relative = Path.GetRelativePath(tree, entry.Path).Replace('\\', '/');

        if (relative is ".")
        {
            relative = string.Empty;
        }

        var staged = 0;
        var unstaged = 0;
        var added = false;
        var modified = false;

        foreach (var change in status.Staged)
        {
            if (!Holds(relative, entry.Kind, change.Path, change.OldPath))
            {
                continue;
            }

            staged++;

            if (change.Change == "added")
            {
                added = true;
            }
            else
            {
                modified = true;
            }
        }

        foreach (var change in status.Unstaged)
        {
            if (!Holds(relative, entry.Kind, change.Path, change.OldPath))
            {
                continue;
            }

            unstaged++;

            if (change.Change == "added")
            {
                added = true;
            }
            else
            {
                modified = true;
            }
        }

        foreach (var untracked in status.Untracked)
        {
            if (!Holds(relative, entry.Kind, untracked.Path, null))
            {
                continue;
            }

            unstaged++;
            added = true;
        }

        foreach (var conflict in status.Conflicts)
        {
            if (!Holds(relative, entry.Kind, conflict.Path, null))
            {
                continue;
            }

            unstaged++;
            modified = true;
        }

        if (staged == 0 && unstaged == 0)
        {
            return null;
        }

        var stage = staged switch
        {
            0 => TreeWorkStage.Unstaged,
            _ when unstaged > 0 => TreeWorkStage.Partial,
            _ => TreeWorkStage.Staged,
        };

        return new TreeWork(
            added && !modified ? TreeWorkStatus.Added : TreeWorkStatus.Modified,
            stage
        );
    }

    /// <summary>Whether a change at a tree path is about an entry, or is under it.</summary>
    /// <param name="relative">The entry's path inside the tree, with forward slashes.</param>
    /// <param name="kind">What the entry is.</param>
    /// <param name="path">The changed path.</param>
    /// <param name="old">Where a rename came from, when it is one.</param>
    /// <returns>Whether the change belongs to the entry.</returns>
    private static bool Holds(
        string relative,
        EntryKind kind,
        string path,
        string? old
    )
    {
        if (kind == EntryKind.File)
        {
            return string.Equals(path, relative, StringComparison.Ordinal)
                || string.Equals(old, relative, StringComparison.Ordinal);
        }

        return Under(relative, path) || Under(relative, old);
    }

    /// <summary>Whether a path is a directory itself or something under it.</summary>
    /// <param name="directory">The directory's path, with forward slashes.</param>
    /// <param name="path">The path to test.</param>
    /// <returns>Whether it is the directory or below it.</returns>
    private static bool Under(string directory, string? path) =>
        path is { Length: > 0 }
        && (
            string.Equals(path, directory, StringComparison.Ordinal)
            || (
                path.Length > directory.Length
                && path.StartsWith(directory, StringComparison.Ordinal)
                && path[directory.Length] == '/'
            )
        );
}

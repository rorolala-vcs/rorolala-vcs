using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>What a path in a tree is: a new thing, or a thing that changed.</summary>
/// <remarks>
/// It is what the Status column writes and what a badge is pictured by, so it stays what the record and the work
/// disagree about rather than what the row is coloured: a path that was moved is a change like any other here, and
/// the lamp says which kind of change it was (see <see cref="TreeWorkShape"/>).
/// </remarks>
enum TreeWorkStatus
{
    /// <summary>Nothing the staging area holds has it, and the work does.</summary>
    Added,

    /// <summary>It is in both places, and what it is or holds changed.</summary>
    Modified,

    /// <summary>It changed, but the tree is not this account's to record, or its base is behind upstream.</summary>
    Wrong,
}

/// <summary>What happened to a path, in the roundest terms a colour can say.</summary>
/// <remarks>
/// A path of a tree is one of four things, and the lamp at the left of its row is read for which before any word
/// of the row is: something new, something changed, something the record names elsewhere, or something the record
/// names and the work no longer holds.
/// </remarks>
enum TreeWorkShape
{
    /// <summary>The record does not hold it at all.</summary>
    Added,

    /// <summary>The record holds it, and what it is or holds changed.</summary>
    Modified,

    /// <summary>The record names it at another path, which is what a rename is to a reader.</summary>
    Moved,

    /// <summary>The record names it and the work does not hold it.</summary>
    Removed,
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
/// <param name="Shape">What happened to it, as the lamp at the left of the row says it.</param>
internal sealed record TreeWork(TreeWorkStatus Status, TreeWorkStage Stage, TreeWorkShape Shape);

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
    public static TreeWork? Of(TreeStatus status, string tree, Entry entry)
    {
        ArgumentNullException.ThrowIfNull(status);
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(entry);

        if (HeldOf(status, tree, entry) is not { } held)
        {
            return null;
        }

        var stage = held.Staged switch
        {
            0 => TreeWorkStage.Unstaged,
            _ when held.Unstaged > 0 => TreeWorkStage.Partial,
            _ => TreeWorkStage.Staged,
        };

        return new TreeWork(
            status.Wrong
                ? TreeWorkStatus.Wrong
                : held.Added && !held.Modified
                    ? TreeWorkStatus.Added
                    : TreeWorkStatus.Modified,
            stage,
            held.Shape()
        );
    }

    /// <summary>What the changes a tree reports say about one path, or nothing when none of them is about it.</summary>
    /// <param name="status">The tree's status.</param>
    /// <param name="tree">The tree's directory.</param>
    /// <param name="entry">The path inside the tree.</param>
    /// <returns>What was gathered over the path, or nothing when nothing changed.</returns>
    private static Held? HeldOf(TreeStatus status, string tree, Entry entry)
    {
        var relative = Path.GetRelativePath(tree, entry.Path).Replace('\\', '/');

        if (relative is ".")
        {
            relative = string.Empty;
        }

        var staged = 0;
        var unstaged = 0;
        var added = false;
        var modified = false;
        var deleted = false;
        var moved = false;

        void Gather(TreeChange change, bool isStaged)
        {
            if (!Holds(relative, entry.Kind, change.Path, change.OldPath))
            {
                return;
            }

            if (isStaged)
            {
                staged++;
            }
            else
            {
                unstaged++;
            }

            switch (change.Change)
            {
                case "added":
                    added = true;

                    break;
                case "deleted":
                    deleted = true;

                    break;
                case "renamed":
                    moved = true;

                    break;
                default:
                    modified = true;

                    break;
            }
        }

        foreach (var change in status.Staged)
        {
            Gather(change, isStaged: true);
        }

        foreach (var change in status.Unstaged)
        {
            Gather(change, isStaged: false);
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

        return new Held(staged, unstaged, added, modified, deleted, moved);
    }

    /// <summary>What the changes gathered over one path add up to.</summary>
    /// <param name="Staged">How many of them the staging area holds.</param>
    /// <param name="Unstaged">How many of them it does not.</param>
    /// <param name="Added">Whether one of them is a path the record does not hold at all.</param>
    /// <param name="Modified">Whether one of them is a change to what the record holds.</param>
    /// <param name="Deleted">Whether one of them is a path the work no longer holds.</param>
    /// <param name="Moved">Whether one of them is a path the record names elsewhere.</param>
    private readonly record struct Held(
        int Staged,
        int Unstaged,
        bool Added,
        bool Modified,
        bool Deleted,
        bool Moved
    )
    {
        /// <summary>What the changes are, in the roundest terms a colour can say.</summary>
        /// <remarks>
        /// A removal comes first because a row that is not there is the first thing to act on, and a move next
        /// because it is the one shape a reader cannot see for themselves: a path that changed, or one that is new,
        /// both look like what they are.
        /// </remarks>
        /// <returns>What happened to the path.</returns>
        public TreeWorkShape Shape() =>
            Deleted ? TreeWorkShape.Removed
            : Moved ? TreeWorkShape.Moved
            : Added && !Modified ? TreeWorkShape.Added
            : TreeWorkShape.Modified;
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
            directory.Length == 0
            || string.Equals(path, directory, StringComparison.Ordinal)
            || (
                path.Length > directory.Length
                && path.StartsWith(directory, StringComparison.Ordinal)
                && path[directory.Length] == '/'
            )
        );
}

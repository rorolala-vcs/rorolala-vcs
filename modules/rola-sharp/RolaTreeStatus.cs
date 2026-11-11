using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>One path that differs between two versions of a rola-tree.</summary>
/// <param name="Path">The path in the later version, or the path it was at for a deletion.</param>
/// <param name="Kind">What the path is, or was: a file or a directory.</param>
/// <param name="Change">What happened to it: added, modified, metadata, deleted, type_changed or renamed.</param>
/// <param name="OldPath">Where it was, for a rename, and nothing otherwise.</param>
public sealed record RolaTreeChange(string Path, string Kind, string Change, string? OldPath);

/// <summary>One path a rola-tree holds and its staging area does not.</summary>
/// <param name="Path">The path.</param>
/// <param name="Kind">What it is: a file or a directory.</param>
public sealed record RolaTreeUntracked(string Path, string Kind);

/// <summary>One path a merge left in conflict in a rola-tree.</summary>
/// <param name="Path">The path the two sides disagree about.</param>
/// <param name="Theirs">The conflict copy that holds the other side.</param>
public sealed record RolaTreeConflict(string Path, string Theirs);

/// <summary>
/// How the rola-tree holding a directory stands: what it staged, what the work changed, and what is in
/// conflict.
/// </summary>
/// <remarks>
/// The reading is the tree's own, and it is a value rather than a handle: the native side answers with one
/// JSON document and this reads the paths in it. The paths are relative to the tree's root and use a
/// forward slash, which is the shape the tree itself writes rather than the host's path separator.
/// </remarks>
public sealed class RolaTreeStatus
{
    /// <summary>Paths that differ between the recorded version and the staging area.</summary>
    public IReadOnlyList<RolaTreeChange> Staged { get; }

    /// <summary>Paths that differ between the staging area and the work on disk.</summary>
    public IReadOnlyList<RolaTreeChange> Unstaged { get; }

    /// <summary>Paths the work holds and the staging area does not.</summary>
    public IReadOnlyList<RolaTreeUntracked> Untracked { get; }

    /// <summary>Paths a merge left in conflict.</summary>
    public IReadOnlyList<RolaTreeConflict> Conflicts { get; }

    /// <summary>Whether the tree's own Layout entry is a change that must not be recorded.</summary>
    public bool Wrong { get; }

    /// <summary>Whether the tree has nothing staged, changed or in conflict.</summary>
    public bool Clean =>
        !HasWork && !Wrong;

    /// <summary>Whether the tree has work a version would record.</summary>
    /// <remarks>
    /// A wrong tree may have no work at all: its own Layout entry is a change that must not be recorded,
    /// which is a fact about the entry rather than work of the tree. What asks whether there is a change
    /// to draw or a version to make asks this, and what says whether the tree is settled asks
    /// <see cref="Clean"/>.
    /// </remarks>
    public bool HasWork =>
        Staged.Count > 0
        || Unstaged.Count > 0
        || Untracked.Count > 0
        || Conflicts.Count > 0;

    /// <summary>Makes the value from the lists the native side answered.</summary>
    /// <param name="staged">What the staging area changed from the record.</param>
    /// <param name="unstaged">What the work changed from the staging area.</param>
    /// <param name="untracked">What the work holds and the staging area does not.</param>
    /// <param name="conflicts">What a merge left in conflict.</param>
    /// <param name="wrong">Whether the tree's own Layout entry is a wrong change.</param>
    private RolaTreeStatus(
        IReadOnlyList<RolaTreeChange> staged,
        IReadOnlyList<RolaTreeChange> unstaged,
        IReadOnlyList<RolaTreeUntracked> untracked,
        IReadOnlyList<RolaTreeConflict> conflicts,
        bool wrong
    )
    {
        Staged = staged;
        Unstaged = unstaged;
        Untracked = untracked;
        Conflicts = conflicts;
        Wrong = wrong;
    }

    /// <summary>
    /// Reads the status of the tree holding a directory.
    /// </summary>
    /// <param name="directory">The directory, or a file inside the tree.</param>
    /// <returns>The reading, or nothing when the directory is not inside a tree that can be read.</returns>
    public static RolaTreeStatus? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var json = NativeText.Taken(RorolalaBinding.rola_tree_status_json(path));

            if (json.Length == 0 || json == "null")
            {
                return null;
            }

            using var reading = JsonDocument.Parse(json);
            var root = reading.RootElement;

            return new RolaTreeStatus(
                Changes(root, "staged"),
                Changes(root, "unstaged"),
                ReadUntracked(root),
                ReadConflicts(root),
                root.TryGetProperty("wrong", out var wrong)
                    && wrong.ValueKind == JsonValueKind.True
            );
        }
        catch (JsonException)
        {
            // A reading that does not parse is not one to act on: what the graph asks is whether there is a
            // change, and an answer it cannot read is no answer rather than a change invented for it.
            return null;
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>One of a status's two lists of changes.</summary>
    /// <param name="root">The status.</param>
    /// <param name="name">The list's name.</param>
    /// <returns>Its entries, or none when the reading names no such list.</returns>
    private static List<RolaTreeChange> Changes(JsonElement root, string name)
    {
        var changes = new List<RolaTreeChange>();

        if (root.TryGetProperty(name, out var list) && list.ValueKind == JsonValueKind.Array)
        {
            foreach (var change in list.EnumerateArray())
            {
                changes.Add(
                    new RolaTreeChange(
                        Text(change, "path"),
                        Text(change, "kind"),
                        Text(change, "change"),
                        Text(change, "old_path") is { Length: > 0 } old ? old : null
                    )
                );
            }
        }

        return changes;
    }

    /// <summary>The paths the work holds and the staging area does not.</summary>
    /// <param name="root">The status.</param>
    /// <returns>Its entries, or none when the reading names no such list.</returns>
    private static List<RolaTreeUntracked> ReadUntracked(JsonElement root)
    {
        var untracked = new List<RolaTreeUntracked>();

        if (root.TryGetProperty("untracked", out var list) && list.ValueKind == JsonValueKind.Array)
        {
            foreach (var entry in list.EnumerateArray())
            {
                untracked.Add(new RolaTreeUntracked(Text(entry, "path"), Text(entry, "kind")));
            }
        }

        return untracked;
    }

    /// <summary>The paths a merge left in conflict.</summary>
    /// <param name="root">The status.</param>
    /// <returns>Its entries, or none when the reading names no such list.</returns>
    private static List<RolaTreeConflict> ReadConflicts(JsonElement root)
    {
        var conflicts = new List<RolaTreeConflict>();

        if (root.TryGetProperty("conflicts", out var list) && list.ValueKind == JsonValueKind.Array)
        {
            foreach (var entry in list.EnumerateArray())
            {
                conflicts.Add(
                    new RolaTreeConflict(Text(entry, "path"), Text(entry, "theirs"))
                );
            }
        }

        return conflicts;
    }

    /// <summary>Reads one string of a status entry.</summary>
    /// <param name="entry">The entry.</param>
    /// <param name="name">The field's name.</param>
    /// <returns>What it says, or an empty string when it names nothing.</returns>
    private static string Text(JsonElement entry, string name) =>
        entry.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? string.Empty
            : string.Empty;
}

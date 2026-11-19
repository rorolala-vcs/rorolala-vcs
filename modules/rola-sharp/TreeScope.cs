using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>One path that differs between two versions of a tree.</summary>
public sealed record TreeChange(string Path, string Kind, string Change, string? OldPath);

/// <summary>One path a tree holds and its staging area does not.</summary>
public sealed record TreeUntracked(string Path, string Kind);

/// <summary>One path a merge left in conflict in a tree.</summary>
public sealed record TreeConflict(string Path, string Theirs);

/// <summary>A tree's mark: the Layout entry id it keeps and the description staged for its next version.</summary>
/// <param name="Id">The Layout entry id the tree keeps, empty when the mark says nothing.</param>
/// <param name="Described">Whether a description is staged.</param>
/// <param name="Description">The staged description, empty when none is.</param>
public sealed record TreeMark(string Id, bool Described, string Description);

/// <summary>How a tree stands: what it staged, what the work changed, and what is in conflict.</summary>
public sealed class TreeStatus
{
    /// <summary>Makes a tree reading.</summary>
    /// <param name="staged">What the staging area changed from the record.</param>
    /// <param name="unstaged">What the work changed from the staging area.</param>
    /// <param name="untracked">What the work holds and the staging area does not.</param>
    /// <param name="conflicts">What a merge left in conflict.</param>
    /// <param name="wrong">Whether the tree's own Layout entry is a wrong change.</param>
    internal TreeStatus(
        IReadOnlyList<TreeChange> staged,
        IReadOnlyList<TreeChange> unstaged,
        IReadOnlyList<TreeUntracked> untracked,
        IReadOnlyList<TreeConflict> conflicts,
        bool wrong
    )
    {
        Staged = staged;
        Unstaged = unstaged;
        Untracked = untracked;
        Conflicts = conflicts;
        Wrong = wrong;
    }

    /// <summary>Paths that differ between the recorded version and the staging area.</summary>
    public IReadOnlyList<TreeChange> Staged { get; }

    /// <summary>Paths that differ between the staging area and the work on disk.</summary>
    public IReadOnlyList<TreeChange> Unstaged { get; }

    /// <summary>Paths the work holds and the staging area does not.</summary>
    public IReadOnlyList<TreeUntracked> Untracked { get; }

    /// <summary>Paths a merge left in conflict.</summary>
    public IReadOnlyList<TreeConflict> Conflicts { get; }

    /// <summary>Whether the tree's own Layout entry is a change that must not be recorded.</summary>
    public bool Wrong { get; }

    /// <summary>Whether the tree has nothing staged, changed or in conflict.</summary>
    public bool Clean => !HasWork && !Wrong;

    /// <summary>Whether the tree has work a version would record.</summary>
    public bool HasWork =>
        Staged.Count > 0
        || Unstaged.Count > 0
        || Untracked.Count > 0
        || Conflicts.Count > 0;
}

/// <summary>A tree: the directory a tree version is kept under, and the work inside it.</summary>
public sealed class TreeScope : Scope
{
    private readonly string _root;

    /// <summary>Makes a scope over the tree holding a directory.</summary>
    /// <param name="directory">The tree's root, or a directory inside it.</param>
    public TreeScope(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The tree's root.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>How the tree stands, once it is read.</summary>
    public TreeStatus? Status { get; private set; }

    /// <summary>The tree's mark, once it is read. Nothing when the directory keeps none.</summary>
    public TreeMark? Mark { get; private set; }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _root,
                "the tree's directory is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();

        var path = Marshal.StringToCoTaskMemUTF8(_root);
        try
        {
            var json = NativeText.Taken(RorolalaBinding.rola_tree_status_json(path));
            if (string.IsNullOrEmpty(json) || json == "null")
            {
                throw new RolaReadError(
                    RolaReadErrorKind.Unavailable,
                    _root,
                    "no tree status could be read"
                );
            }

            using var document = JsonDocument.Parse(json);
            var root = document.RootElement;
            Status = new TreeStatus(
                Changes(root, "staged"),
                Changes(root, "unstaged"),
                Untracked(root),
                Conflicts(root),
                RolaJson.Flag(root, "wrong")
            );
            Mark = MarkOf(_root);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Whether a directory is the root of a tree.</summary>
    /// <param name="directory">The directory to ask about.</param>
    /// <returns>Whether it holds a tree's mark.</returns>
    public static bool IsTree(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);
        try
        {
            return RorolalaBinding.rola_is_tree(path);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Reads the mark of the tree rooted at a directory, without reading the tree's work.</summary>
    /// <param name="directory">The tree's root.</param>
    /// <returns>The mark, or nothing when the directory keeps none.</returns>
    public static TreeMark? MarkOf(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);
        try
        {
            return ParseMark(NativeText.Taken(RorolalaBinding.rola_tree_mark_json(path)));
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    private static TreeMark? ParseMark(string json)
    {
        if (string.IsNullOrEmpty(json) || json == "null")
        {
            return null;
        }

        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        if (root.ValueKind is not JsonValueKind.Object)
        {
            return null;
        }

        return new TreeMark(
            RolaJson.Text(root, "id") ?? string.Empty,
            RolaJson.Flag(root, "described"),
            RolaJson.Text(root, "description") ?? string.Empty
        );
    }

    private static IReadOnlyList<TreeChange> Changes(JsonElement root, string name)
    {
        var items = RolaJson.Items(root, name);
        if (items is null)
        {
            return [];
        }

        return items
            .Value.EnumerateArray()
            .Select(item => new TreeChange(
                RolaJson.Text(item, "path") ?? string.Empty,
                RolaJson.Text(item, "kind") ?? string.Empty,
                RolaJson.Text(item, "change") ?? string.Empty,
                RolaJson.Text(item, "old_path") is { Length: > 0 } old ? old : null
            ))
            .ToArray();
    }

    private static IReadOnlyList<TreeUntracked> Untracked(JsonElement root)
    {
        var items = RolaJson.Items(root, "untracked");
        if (items is null)
        {
            return [];
        }

        return items
            .Value.EnumerateArray()
            .Select(item => new TreeUntracked(
                RolaJson.Text(item, "path") ?? string.Empty,
                RolaJson.Text(item, "kind") ?? string.Empty
            ))
            .ToArray();
    }

    private static IReadOnlyList<TreeConflict> Conflicts(JsonElement root)
    {
        var items = RolaJson.Items(root, "conflicts");
        if (items is null)
        {
            return [];
        }

        return items
            .Value.EnumerateArray()
            .Select(item => new TreeConflict(
                RolaJson.Text(item, "path") ?? string.Empty,
                RolaJson.Text(item, "theirs") ?? string.Empty
            ))
            .ToArray();
    }
}

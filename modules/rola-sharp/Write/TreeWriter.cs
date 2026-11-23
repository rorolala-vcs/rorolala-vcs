using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>One path a staging changed.</summary>
/// <param name="Path">The path, as the tree writes it.</param>
/// <param name="Kind">What happened: <c>added</c>, <c>updated</c> or <c>deleted</c>.</param>
public sealed record TreeAddChange(string Path, string Kind);

/// <summary>What making a tree did.</summary>
/// <param name="Path">The tree's path, or <c>.</c> for the Workspace root.</param>
/// <param name="Collapsed">How many entries stopped being files.</param>
/// <param name="IsRoot">Whether the tree is the Workspace root.</param>
public sealed record TreeInit(string Path, int Collapsed, bool IsRoot);

/// <summary>What a reset did.</summary>
/// <param name="Whole">Whether the whole staging area was cleared.</param>
/// <param name="Paths">The paths reset, or none for the whole area.</param>
public sealed record TreeReset(bool Whole, IReadOnlyList<string> Paths);

/// <summary>What a tree removal staged.</summary>
/// <param name="Paths">The paths that left the staged structure.</param>
/// <param name="Cached">Whether the work was left where it was.</param>
public sealed record TreeRemoval(IReadOnlyList<string> Paths, bool Cached);

/// <summary>Writes a tree's description, which the next version carries.</summary>
public sealed class TreeWriter : Writer
{
    private readonly WorkspaceScope _workspace;

    /// <summary>Makes a writer over a Workspace.</summary>
    /// <param name="workspace">The Workspace, which must have been read.</param>
    public TreeWriter(WorkspaceScope workspace) =>
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));

    /// <summary>Writes the description of the tree at a path.</summary>
    /// <param name="text">The description; empty clears it.</param>
    /// <param name="target">The tree's path, or nothing for the Workspace root.</param>
    /// <returns>How many bytes the description is.</returns>
    public int WriteDescription(string text, string? target = null)
    {
        ArgumentNullException.ThrowIfNull(text);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(target ?? string.Empty);
        var words = Utf8(text);
        try
        {
            var answer = RorolalaBinding.rola_write_tree_description(directory, at, words);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return (int)RorolalaBinding.rola_write_tree_description_bytes(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_write_tree_description(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(words);
        }
    }

    /// <summary>Makes a directory a tree.</summary>
    /// <param name="target">The tree's path, or nothing for the Workspace root.</param>
    /// <param name="force">Whether to agree to the recorded files collapsing into the tree.</param>
    /// <returns>What was made.</returns>
    public TreeInit Init(string? target = null, bool force = false)
    {
        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(target ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_tree_init(directory, at, force);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new TreeInit(
                    NativeText.Taken(RorolalaBinding.rola_write_tree_init_path(handle)),
                    (int)RorolalaBinding.rola_write_tree_init_collapsed(handle),
                    RorolalaBinding.rola_write_tree_init_is_root(handle)
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_tree_init(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
        }
    }

    /// <summary>Resets the staged paths, or the whole staging area when none are named.</summary>
    /// <param name="paths">The paths to reset.</param>
    /// <param name="target">The tree's path, or nothing for the Workspace root.</param>
    /// <returns>What was reset.</returns>
    public TreeReset Reset(IEnumerable<string> paths, string? target = null)
    {
        ArgumentNullException.ThrowIfNull(paths);

        using var request = new NativeRequest();
        var directory = request.Text(_workspace.Root.FullName);
        var at = request.Text(target);
        var list = request.Span(paths.ToArray());
        var answer = RorolalaBinding.rola_write_tree_reset(directory, at, list);
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.From(answer.payload);
        }

        var handle = answer.payload;
        try
        {
            var count = RorolalaBinding.rola_write_tree_reset_path_count(handle);
            var reset = new List<string>((int)count);
            for (nuint index = 0; index < count; index++)
            {
                reset.Add(
                    NativeText.Taken(RorolalaBinding.rola_write_tree_reset_path_at(handle, index))
                );
            }

            return new TreeReset(RorolalaBinding.rola_write_tree_reset_whole(handle), reset);
        }
        finally
        {
            RorolalaBinding.free_rola_write_tree_reset(handle);
        }
    }

    /// <summary>Stages work into the tree at a target.</summary>
    /// <param name="paths">The paths to stage.</param>
    /// <param name="target">The tree's path, or nothing for the Workspace root.</param>
    /// <param name="includeIgnored">Stage work the rules would ignore.</param>
    /// <returns>What the staged structure changed.</returns>
    public IReadOnlyList<TreeAddChange> Add(
        IEnumerable<string> paths,
        string? target = null,
        bool includeIgnored = false
    )
    {
        ArgumentNullException.ThrowIfNull(paths);

        using var request = new NativeRequest();
        var directory = request.Text(_workspace.Root.FullName);
        var at = request.Text(target);
        var list = request.Span(paths.ToArray());
        var answer = RorolalaBinding.rola_write_tree_add(directory, at, list, includeIgnored);
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.From(answer.payload);
        }

        var handle = answer.payload;
        try
        {
            var count = RorolalaBinding.rola_write_tree_add_change_count(handle);
            var changes = new List<TreeAddChange>((int)count);
            for (nuint index = 0; index < count; index++)
            {
                var change = RorolalaBinding.rola_write_tree_add_change_at(handle, index);
                if (change == 0)
                {
                    continue;
                }

                try
                {
                    changes.Add(
                        new TreeAddChange(
                            NativeText.Taken(
                                RorolalaBinding.rola_write_tree_add_change_path(change)
                            ),
                            NativeText.Taken(
                                RorolalaBinding.rola_write_tree_add_change_kind(change)
                            )
                        )
                    );
                }
                finally
                {
                    RorolalaBinding.free_rola_write_tree_add_change(change);
                }
            }

            return changes;
        }
        finally
        {
            RorolalaBinding.free_rola_write_tree_add(handle);
        }
    }

    /// <summary>Stages the removal of paths from the tree at a target.</summary>
    /// <param name="paths">The paths to remove.</param>
    /// <param name="target">The tree's path, or nothing for the Workspace root.</param>
    /// <param name="cached">Leave the work in place.</param>
    /// <param name="recursive">Take a directory with what is under it.</param>
    /// <param name="force">Drop work that was never staged.</param>
    /// <returns>What was staged.</returns>
    public TreeRemoval Remove(
        IEnumerable<string> paths,
        string? target = null,
        bool cached = false,
        bool recursive = false,
        bool force = false
    )
    {
        ArgumentNullException.ThrowIfNull(paths);

        using var request = new NativeRequest();
        var directory = request.Text(_workspace.Root.FullName);
        var at = request.Text(target);
        var list = request.Span(paths.ToArray());
        var answer = RorolalaBinding.rola_write_tree_remove_paths(
            directory,
            at,
            list,
            cached,
            recursive,
            force
        );
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.From(answer.payload);
        }

        var handle = answer.payload;
        try
        {
            var count = RorolalaBinding.rola_write_tree_remove_path_count(handle);
            var removed = new List<string>((int)count);
            for (nuint index = 0; index < count; index++)
            {
                removed.Add(
                    NativeText.Taken(RorolalaBinding.rola_write_tree_remove_path_at(handle, index))
                );
            }

            return new TreeRemoval(
                removed,
                RorolalaBinding.rola_write_tree_remove_cached(handle)
            );
        }
        finally
        {
            RorolalaBinding.free_rola_write_tree_remove(handle);
        }
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

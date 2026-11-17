using System.Runtime.InteropServices;
using System.Text.Json;

using RolaSharp.V2;

namespace RolaSharp.V2.Write;

/// <summary>What a tree removal staged.</summary>
/// <param name="Paths">The paths that left the staged structure.</param>
/// <param name="Cached">Whether the work was left where it was.</param>
public sealed record TreeRemoval(IReadOnlyList<string> Paths, bool Cached);

/// <summary>Writes a tree's description, which the next version carries.</summary>
public sealed class TreeWriter : Writer
{
    private readonly WorkspaceScoop _workspace;

    /// <summary>Makes a writer over a Workspace.</summary>
    /// <param name="workspace">The Workspace, which must have been read.</param>
    public TreeWriter(WorkspaceScoop workspace) =>
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

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return (int)(RolaJson.Number(document.RootElement, "bytes") ?? 0);
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(words);
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

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(target ?? string.Empty);
        var list = Utf8(JsonSerializer.Serialize(paths));
        try
        {
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

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));
            var removed =
                RolaJson
                    .Items(document.RootElement, "paths")
                    ?.EnumerateArray()
                    .Select(item => item.GetString() ?? string.Empty)
                    .ToArray()
                ?? [];

            return new TreeRemoval(removed, RolaJson.Flag(document.RootElement, "cached"));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(list);
        }
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

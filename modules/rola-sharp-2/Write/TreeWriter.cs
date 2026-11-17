using System.Runtime.InteropServices;
using System.Text.Json;

using RolaSharp.V2;

namespace RolaSharp.V2.Write;

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

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

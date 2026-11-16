using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>
/// The id of a tree and the description staged for its next version.
/// </summary>
/// <remarks>
/// A tree's id is what tells a tree that moved from one that was lost and another that appeared; the
/// description is what the next version is to say. Both are read together because both live in the
/// tree's own directory, and a caller that read that directory itself would be keeping a second copy
/// of the format.
/// </remarks>
public sealed class RolaTreeMark
{
    /// <summary>Makes a reading of a tree's mark.</summary>
    /// <param name="id">The Layout entry id the tree keeps.</param>
    /// <param name="described">Whether a description is staged.</param>
    /// <param name="description">The description itself.</param>
    private RolaTreeMark(string id, bool described, string description)
    {
        Id = id;
        Described = described;
        Description = description;
    }

    /// <summary>The Layout entry id the tree keeps, empty when the file says nothing.</summary>
    public string Id { get; }

    /// <summary>Whether a description is staged for the next version.</summary>
    public bool Described { get; }

    /// <summary>The staged description, empty when none is.</summary>
    public string Description { get; }

    /// <summary>Whether a directory is the root of a tree.</summary>
    /// <remarks>
    /// The answer is the mark alone, so it costs one check of the file system and needs no Workspace,
    /// Layout, index or store. It is what a listing asks about every directory it draws.
    /// </remarks>
    /// <param name="directory">The directory to ask about.</param>
    /// <returns>Whether it is the root of a tree.</returns>
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

    /// <summary>Reads the mark of the tree rooted at a directory.</summary>
    /// <param name="directory">The tree's root.</param>
    /// <returns>The mark, or nothing when the directory is not the root of a tree.</returns>
    public static RolaTreeMark? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var json = NativeText.Taken(RorolalaBinding.rola_tree_mark_json(path));

            return Parse(json);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Reads the mark the native side wrote.</summary>
    /// <param name="json">The answer, which may be the JSON <c>null</c>.</param>
    /// <returns>The mark, or nothing when the answer says there is none.</returns>
    private static RolaTreeMark? Parse(string json)
    {
        if (json.Length == 0)
        {
            return null;
        }

        using var document = JsonDocument.Parse(json);

        if (document.RootElement.ValueKind is not JsonValueKind.Object)
        {
            return null;
        }

        return new RolaTreeMark(
            RolaJson.Text(document.RootElement, "id") ?? string.Empty,
            RolaJson.Flag(document.RootElement, "described"),
            RolaJson.Text(document.RootElement, "description") ?? string.Empty
        );
    }
}

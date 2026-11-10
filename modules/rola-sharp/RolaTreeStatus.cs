using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>
/// How the rola-tree holding a directory stands: what it staged, what the work changed, and what is in
/// conflict.
/// </summary>
/// <remarks>
/// The reading is the tree's own, and it is a value rather than a handle: the native side answers with one
/// JSON document and this reads what the graph asks of it, the size of each list and whether there is
/// anything at all. The paths in those lists are the tree's own and are not kept here, since a reader that
/// wants them reads the tree's status command.
/// </remarks>
/// <param name="Staged">How many paths differ between the recorded version and the staging area.</param>
/// <param name="Unstaged">How many paths differ between the staging area and the work.</param>
/// <param name="Untracked">How many paths the work holds and the staging area does not.</param>
/// <param name="Conflicts">How many paths a merge left in conflict.</param>
public sealed record RolaTreeStatus(int Staged, int Unstaged, int Untracked, int Conflicts)
{
    /// <summary>Whether the tree has nothing staged, changed or in conflict.</summary>
    public bool Clean => Staged == 0 && Unstaged == 0 && Untracked == 0 && Conflicts == 0;

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
                Count(root, "staged"),
                Count(root, "unstaged"),
                Count(root, "untracked"),
                Count(root, "conflicts")
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

    /// <summary>How many entries the named list of a tree status holds.</summary>
    /// <param name="root">The status.</param>
    /// <param name="name">The list's name.</param>
    /// <returns>Its length, or zero when the reading names no such list.</returns>
    private static int Count(JsonElement root, string name) =>
        root.TryGetProperty(name, out var list) && list.ValueKind == JsonValueKind.Array
            ? list.GetArrayLength()
            : 0;
}

using System.Runtime.InteropServices;
using System.Text.Json;

using RolaSharp.V2;

namespace RolaSharp.V2.Write;

/// <summary>Changes one Layout, one batch at a time.</summary>
/// <remarks>
/// These are the low-level changes: a path is moved, an entry is inserted, and the caller owns the
/// order. The checks that a workflow makes, ownership and the version a path may move to, are in
/// <see cref="WorkspaceWriter"/>'s workflows rather than here.
/// </remarks>
public sealed class LayoutWriter : Writer
{
    private readonly LayoutScoop _layout;

    /// <summary>Makes a writer over a Layout a caller has named.</summary>
    /// <param name="layout">The Layout.</param>
    public LayoutWriter(LayoutScoop layout) =>
        _layout = layout ?? throw new ArgumentNullException(nameof(layout));

    /// <summary>Applies a batch of changes named as JSON.</summary>
    /// <param name="ops">The changes, as the native request shape.</param>
    public void Apply(string ops)
    {
        ArgumentNullException.ThrowIfNull(ops);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var changes = Utf8(ops);
        try
        {
            var answer = RorolalaBinding.rola_write_layout_apply(directory, layout, changes);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(changes);
        }
    }

    /// <summary>Moves the entry at one path to another.</summary>
    /// <param name="from">The path the entry is at.</param>
    /// <param name="to">The path it is to be at.</param>
    public void MovePath(string from, string to) =>
        Apply(JsonSerializer.Serialize(new[] { new { op = "move_path", from, to } }));

    /// <summary>Removes a path, leaving its entry.</summary>
    /// <param name="path">The path to remove.</param>
    public void RemovePath(string path) =>
        Apply(JsonSerializer.Serialize(new[] { new { op = "remove_path", path } }));

    /// <summary>Binds a path to an entry.</summary>
    /// <param name="path">The path to bind.</param>
    /// <param name="uuid">The entry it names.</param>
    public void CreatePath(string path, Guid uuid) =>
        Apply(
            JsonSerializer.Serialize(
                new[] { new { op = "create_path", path, uuid = uuid.ToString() } }
            )
        );

    /// <summary>Makes an entry, and binds a path to it.</summary>
    /// <param name="uuid">The Uuid the entry is known by.</param>
    /// <param name="path">The path to bind.</param>
    /// <param name="version">The version it is at, as the hex a key is written by.</param>
    /// <param name="owner">The account that holds it, or nothing.</param>
    /// <param name="description">What it says about itself.</param>
    /// <param name="tree">Whether it names a tree rather than a file.</param>
    public void InsertEntry(
        Guid uuid,
        string path,
        string version,
        string? owner,
        string description,
        bool tree = false
    ) =>
        Apply(
            JsonSerializer.Serialize(
                new[]
                {
                    new
                    {
                        op = "create_entry",
                        uuid = uuid.ToString(),
                        path,
                        version,
                        owner,
                        description,
                        kind = tree ? "tree" : "file",
                    },
                }
            )
        );

    /// <summary>Changes who holds an entry, keeping what it holds.</summary>
    /// <param name="uuid">The entry.</param>
    /// <param name="owner">The account that now holds it, or nothing.</param>
    public void ChangeOwnership(Guid uuid, string? owner) =>
        Apply(
            JsonSerializer.Serialize(
                new[] { new { op = "change_ownership", uuid = uuid.ToString(), owner } }
            )
        );

    /// <summary>Moves an entry to a new Uuid.</summary>
    /// <param name="from">The Uuid it has.</param>
    /// <param name="to">The Uuid it is to have.</param>
    public void ChangeUuid(Guid from, Guid to) =>
        Apply(
            JsonSerializer.Serialize(
                new[]
                {
                    new { op = "change_uuid", from = from.ToString(), to = to.ToString() },
                }
            )
        );

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

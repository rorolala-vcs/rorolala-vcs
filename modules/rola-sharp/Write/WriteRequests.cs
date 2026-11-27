using System.Text.Json;
using System.Text.Json.Serialization;

namespace RolaSharp.Write;

/// <summary>The fields every one-shot write request carries.</summary>
/// <remarks>
/// The native side flattens these beside a request's own fields, so a record that derives from this
/// one is serialized as one object rather than as one object holding another. The Workspace is
/// replaced by the writer with the root of the Workspace it was made over: a request cannot send
/// work to a Workspace other than the one it names.
/// </remarks>
public abstract record WriteRequest
{
    /// <summary>The Workspace the run works in, which the writer sets to its own root.</summary>
    [JsonPropertyName("workspace")]
    public string Workspace { get; init; } = string.Empty;

    /// <summary>The account the run acts as, or nothing when the run needs none.</summary>
    [JsonPropertyName("account")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Account { get; init; }

    /// <summary>The Vault the run reaches for, the tracked one when nothing is said.</summary>
    [JsonPropertyName("target")]
    public WriteTarget Target { get; init; } = WriteTarget.Tracked;

    /// <summary>Whether the run may reach a Vault at all.</summary>
    [JsonPropertyName("offline")]
    public bool Offline { get; init; }

    /// <summary>Whether the run is told to go past a gate.</summary>
    [JsonPropertyName("force")]
    public bool Force { get; init; }

    /// <summary>How many reads a transfer keeps in flight, or zero for the native side's own choice.</summary>
    [JsonPropertyName("concurrency")]
    public int Concurrency { get; init; }

    /// <summary>What the write calls the work while it does it, or nothing for the native words.</summary>
    [JsonPropertyName("labels")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public WriteLabels? Labels { get; init; }
}

/// <summary>Which Vault a write reaches for.</summary>
public enum WriteTargetKind
{
    /// <summary>The Vault the Layout being worked in tracks.</summary>
    Tracked = 0,

    /// <summary>A Vault the Workspace knows, under a name.</summary>
    Named = 1,

    /// <summary>An address, which is a Vault the Workspace never named.</summary>
    Address = 2,
}

/// <summary>The Vault a request reaches for, as the native JSON names it.</summary>
public sealed record WriteTarget
{
    /// <summary>Which kind of Vault the target names.</summary>
    [JsonPropertyName("kind")]
    [JsonConverter(typeof(SnakeCaseEnumConverter<WriteTargetKind>))]
    public WriteTargetKind Kind { get; init; } = WriteTargetKind.Tracked;

    /// <summary>The name the Workspace bound it to, when the target names one.</summary>
    [JsonPropertyName("name")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Name { get; init; }

    /// <summary>Where it answers, when the target is an address.</summary>
    [JsonPropertyName("address")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Address { get; init; }

    /// <summary>The Vault the Layout tracks, which is what a request that says nothing reaches.</summary>
    public static WriteTarget Tracked { get; } = new() { Kind = WriteTargetKind.Tracked };

    /// <summary>A Vault the Workspace knows, under <paramref name="name"/>.</summary>
    /// <param name="name">The name the Workspace bound it to.</param>
    /// <returns>The target.</returns>
    public static WriteTarget Named(string name)
    {
        ArgumentNullException.ThrowIfNull(name);

        return new() { Kind = WriteTargetKind.Named, Name = name };
    }

    /// <summary>A Vault at <paramref name="address"/>, which the Workspace never named.</summary>
    /// <param name="address">Where the Vault answers.</param>
    /// <returns>The target.</returns>
    public static WriteTarget At(string address)
    {
        ArgumentNullException.ThrowIfNull(address);

        return new() { Kind = WriteTargetKind.Address, Address = address };
    }
}

/// <summary>The words a write calls the work while it does it.</summary>
/// <remarks>
/// A field left out keeps the native side's own word, so a caller translates only what it shows.
/// </remarks>
public sealed record WriteLabels
{
    /// <summary>Reading the tree, a Layout or an index.</summary>
    [JsonPropertyName("reading")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Reading { get; init; }

    /// <summary>Writing content into the store.</summary>
    [JsonPropertyName("writing")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Writing { get; init; }

    /// <summary>Reaching the Vault for a Layout or an object.</summary>
    [JsonPropertyName("fetching")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Fetching { get; init; }

    /// <summary>Carrying content to the Vault.</summary>
    [JsonPropertyName("uploading")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Uploading { get; init; }

    /// <summary>Carrying content from the Vault.</summary>
    [JsonPropertyName("downloading")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Downloading { get; init; }

    /// <summary>Checking the work before writing it.</summary>
    [JsonPropertyName("checking")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Checking { get; init; }

    /// <summary>Carrying out a plan.</summary>
    [JsonPropertyName("applying")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Applying { get; init; }
}

/// <summary>Brings what a Vault holds and this Layout does not into it.</summary>
public sealed record BringRequest : WriteRequest
{
    /// <summary>
    /// The references to bring in: paths of the Vault's Layout, globs over those paths, or Uuids.
    /// </summary>
    [JsonPropertyName("refs")]
    public IReadOnlyList<string> Refs { get; init; } = [];

    /// <summary>Where the references land, or nothing to land each at the path the Vault names it by.</summary>
    [JsonPropertyName("to")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? To { get; init; }

    /// <summary>Whether each reference keeps only its own name under a destination.</summary>
    [JsonPropertyName("by_name")]
    public bool ByName { get; init; }

    /// <summary>
    /// The field the native side dispatches on to read this document as the variant merge, or
    /// nothing for a bring of entries.
    /// </summary>
    /// <remarks>
    /// Only its presence tells the two brings apart, and a bring of entries leaves it out. A caller
    /// that wants the merge uses <see cref="BringVariantRequest"/>, which fills every field the
    /// merge reads rather than this one alone.
    /// </remarks>
    [JsonPropertyName("join")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Join { get; init; }
}

/// <summary>Brings a variant in to be joined into a file this Layout names.</summary>
/// <remarks>
/// Both brings are one document and the native side dispatches on the presence of `join`, so
/// <see cref="Join"/> is computed from <see cref="Variant"/> rather than set beside it. The two
/// things the merge itself reads are `variant`, the variant to bring in, and `file`, the file it is
/// to be joined into.
/// </remarks>
public sealed record BringVariantRequest : WriteRequest
{
    /// <summary>The variant to bring in, as a whole hash or as the head of one.</summary>
    [JsonPropertyName("variant")]
    public required string Variant { get; init; }

    /// <summary>The file it is to be joined into, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("file")]
    public required string File { get; init; }

    /// <summary>
    /// Where the variant file is to lie, or nothing for the place the merge convention gives it.
    /// </summary>
    [JsonPropertyName("to")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? To { get; init; }

    /// <summary>Whether a tree variant is merged into its tree rather than put beside the work.</summary>
    [JsonPropertyName("tree_merge")]
    public bool TreeMerge { get; init; }

    /// <summary>The field the native side dispatches on, which is always the variant named here.</summary>
    [JsonPropertyName("join")]
    public string Join => Variant;
}

/// <summary>Records a batch of paths, or the changes under them, as versions.</summary>
public sealed record TrackRequest : WriteRequest
{
    /// <summary>The names to record, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("files")]
    public IReadOnlyList<string> Files { get; init; } = [];

    /// <summary>Whether each file's version is replaced rather than added after it.</summary>
    [JsonPropertyName("replace")]
    public bool Replace { get; init; }

    /// <summary>The cut every file this run writes is made with, or nothing for the store's own choice.</summary>
    [JsonPropertyName("policy")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Policy { get; init; }

    /// <summary>Whether a file the ignore rules govern may be named anyway.</summary>
    [JsonPropertyName("include_ignored")]
    public bool IncludeIgnored { get; init; }
}

/// <summary>Moves a path's record to a version, or one version back when none is named.</summary>
public sealed record RetrackRequest : WriteRequest
{
    /// <summary>The one file or tree to retrack, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("file")]
    public required string File { get; init; }

    /// <summary>
    /// What the path is to record: `~N`, a number, a hash, a Uuid or a path of the Vault's Layout, or
    /// empty for one version back.
    /// </summary>
    [JsonPropertyName("record")]
    public string Record { get; init; } = string.Empty;

    /// <summary>Whether the record moves and the work on disk is left alone.</summary>
    [JsonPropertyName("only_index")]
    public bool OnlyIndex { get; init; }

    /// <summary>Whether a version or an entry off the way to what the path records is refused.</summary>
    [JsonPropertyName("deny_jump")]
    public bool DenyJump { get; init; }

    /// <summary>Whether the path is given a fresh Uuid holding what it records now.</summary>
    [JsonPropertyName("fork")]
    public bool Fork { get; init; }
}

/// <summary>What an alignment is to do with the one path a run named.</summary>
public enum AlignModeKind
{
    /// <summary>Confirm that a path the Layout names but the tree does not hold is gone.</summary>
    Delete = 0,

    /// <summary>Confirm the move the path is a side of.</summary>
    Rename = 1,

    /// <summary>Take back the move the path is a side of, leaving a loss and a gain.</summary>
    Break = 2,

    /// <summary>Assert that the path moved to the path named here.</summary>
    MoveTo = 3,

    /// <summary>Put a moved file back where the Layout names it.</summary>
    RestoreMove = 4,

    /// <summary>Put a changed file back to the version the Layout names.</summary>
    RestoreModify = 5,

    /// <summary>Put a file inside a tree back to what the tree has staged for it.</summary>
    RestoreStaged = 6,

    /// <summary>Put a deleted file back, taking the version the Layout names out of the store.</summary>
    RestoreDelete = 7,

    /// <summary>Do whichever of the three a path put back needs.</summary>
    Restore = 8,
}

/// <summary>What an alignment is to do with the one path a run named, as its JSON names it.</summary>
public sealed record AlignMode
{
    /// <summary>What the run does with the path.</summary>
    [JsonPropertyName("kind")]
    [JsonConverter(typeof(SnakeCaseEnumConverter<AlignModeKind>))]
    public AlignModeKind Kind { get; init; } = AlignModeKind.Delete;

    /// <summary>Where the path moved to, when the mode is <see cref="AlignModeKind.MoveTo"/>.</summary>
    [JsonPropertyName("path")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Path { get; init; }

    /// <summary>Confirms that a lost path is gone.</summary>
    public static AlignMode Delete { get; } = new() { Kind = AlignModeKind.Delete };

    /// <summary>Confirms the move the path is a side of.</summary>
    public static AlignMode Rename { get; } = new() { Kind = AlignModeKind.Rename };

    /// <summary>Takes back the move the path is a side of.</summary>
    public static AlignMode Break { get; } = new() { Kind = AlignModeKind.Break };

    /// <summary>Puts a moved file back where the Layout names it.</summary>
    public static AlignMode RestoreMove { get; } = new() { Kind = AlignModeKind.RestoreMove };

    /// <summary>Puts a changed file back to the version the Layout names.</summary>
    public static AlignMode RestoreModify { get; } = new() { Kind = AlignModeKind.RestoreModify };

    /// <summary>Puts a file inside a tree back to what the tree has staged for it.</summary>
    public static AlignMode RestoreStaged { get; } = new() { Kind = AlignModeKind.RestoreStaged };

    /// <summary>Puts a deleted file back, taking the version the Layout names out of the store.</summary>
    public static AlignMode RestoreDelete { get; } = new() { Kind = AlignModeKind.RestoreDelete };

    /// <summary>Does whichever of the three a path put back needs.</summary>
    public static AlignMode Restore { get; } = new() { Kind = AlignModeKind.Restore };

    /// <summary>Asserts that the path moved to <paramref name="path"/>.</summary>
    /// <param name="path">Where the path moved to, relative to the Workspace root or absolute.</param>
    /// <returns>The mode.</returns>
    public static AlignMode MoveTo(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return new() { Kind = AlignModeKind.MoveTo, Path = path };
    }
}

/// <summary>Settles one path against the Layout it works from.</summary>
public sealed record AlignRequest : WriteRequest
{
    /// <summary>The path the run acts on, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("path")]
    public string Path { get; init; } = string.Empty;

    /// <summary>What the run does with the path.</summary>
    [JsonPropertyName("mode")]
    public required AlignMode Mode { get; init; }

    /// <summary>Whether a move is put back in content as well as in place.</summary>
    [JsonPropertyName("completely")]
    public bool Completely { get; init; }

    /// <summary>Whether an entry the ignore rules govern may be selected anyway.</summary>
    [JsonPropertyName("include_ignored")]
    public bool IncludeIgnored { get; init; }
}

/// <summary>Which way an entry's ownership is to move.</summary>
public enum OwnershipKind
{
    /// <summary>Take the entry for this account.</summary>
    Hold = 0,

    /// <summary>Let the entry go.</summary>
    Giveup = 1,
}

/// <summary>Moves the ownership of the entries a run names.</summary>
public sealed record OwnershipPlanRequest : WriteRequest
{
    /// <summary>Which way the ownership moves.</summary>
    [JsonPropertyName("kind")]
    [JsonConverter(typeof(SnakeCaseEnumConverter<OwnershipKind>))]
    public OwnershipKind Kind { get; init; } = OwnershipKind.Hold;

    /// <summary>
    /// The names to move the ownership for, read from the Workspace root: a file is itself and a
    /// directory is every file under it.
    /// </summary>
    [JsonPropertyName("paths")]
    public IReadOnlyList<string> Paths { get; init; } = [];

    /// <summary>Whether what can be moved is moved when not every name passes its gates.</summary>
    [JsonPropertyName("allow_partial")]
    public bool AllowPartial { get; init; }
}

/// <summary>Brings a Layout and the Vault it tracks together.</summary>
/// <remarks>
/// The same document asks for the plan and for the run that carries it out, so a caller that
/// previewed a plan applies the plan it previewed. A relative path is never named here: the writer
/// fills the Workspace with the root of the Workspace it was made over.
/// <see cref="WriteRequest.Force"/> is not read by these entry points; <see cref="Forced"/> is the
/// sync's own gate, and it is what lets a version whose chain moved apart go up.
/// </remarks>
public sealed record SyncRequest : WriteRequest
{
    /// <summary>Which way content may move, both when nothing is said.</summary>
    [JsonPropertyName("direction")]
    [JsonConverter(typeof(SnakeCaseEnumConverter<SyncDirection>))]
    public SyncDirection Direction { get; init; } = SyncDirection.Both;

    /// <summary>Whether a version whose chain moved apart may go up anyway.</summary>
    [JsonPropertyName("forced")]
    public bool Forced { get; init; }

    /// <summary>Whether the store moves, which it does when nothing is said.</summary>
    [JsonPropertyName("storage")]
    public bool Storage { get; init; } = true;

    /// <summary>Whether the index moves, which it does when nothing is said.</summary>
    [JsonPropertyName("index")]
    public bool Index { get; init; } = true;

    /// <summary>Whether the Vault's Layout is read and written, which it is when nothing is said.</summary>
    [JsonPropertyName("layout")]
    public bool Layout { get; init; } = true;
}

/// <summary>Reads and writes an enum as its snake case name, which is the native wire spelling.</summary>
/// <typeparam name="T">The enum whose members are named in C# and spelled on the wire.</typeparam>
internal sealed class SnakeCaseEnumConverter<T> : JsonConverter<T>
    where T : struct, Enum
{
    /// <summary>Reads the snake case name back to the member it names.</summary>
    /// <param name="reader">The reader the name arrives on.</param>
    /// <param name="typeToConvert">The enum being read.</param>
    /// <param name="options">The serializer's options, which this does not read.</param>
    /// <returns>The member.</returns>
    /// <exception cref="JsonException">The text names no member of the enum.</exception>
    public override T Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options
    )
    {
        var text = reader.GetString();
        foreach (var value in Enum.GetValues<T>())
        {
            if (
                string.Equals(
                    JsonNamingPolicy.SnakeCaseLower.ConvertName(value.ToString()),
                    text,
                    StringComparison.Ordinal
                )
            )
            {
                return value;
            }
        }

        throw new JsonException($"`{text}` names no {typeof(T).Name}");
    }

    /// <summary>Writes the member as its snake case name.</summary>
    /// <param name="writer">The writer the name goes to.</param>
    /// <param name="value">The member being written.</param>
    /// <param name="options">The serializer's options, which this does not read.</param>
    public override void Write(Utf8JsonWriter writer, T value, JsonSerializerOptions options) =>
        writer.WriteStringValue(
            JsonNamingPolicy.SnakeCaseLower.ConvertName(value.ToString())
        );
}

/// <summary>Folds the local versions above the tracked Vault's version into one.</summary>
public sealed record CollapseRequest : WriteRequest
{
    /// <summary>The one path whose chain is folded, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("file")]
    public required string File { get; init; }

    /// <summary>The first version folded, or empty for the version above the origin.</summary>
    [JsonPropertyName("target")]
    public string TargetVersion { get; init; } = string.Empty;

    /// <summary>The versions kept apart from the fold, each a target the command would take.</summary>
    [JsonPropertyName("keep")]
    public IReadOnlyList<string> Keep { get; init; } = [];

    /// <summary>Whether a merge version in the range is kept as a checkpoint.</summary>
    [JsonPropertyName("keep_join")]
    public bool KeepJoin { get; init; }
}

/// <summary>Cuts a file's old history away and starts it again at a kept version.</summary>
public sealed record CutRequest : WriteRequest
{
    /// <summary>The one path whose old history is cut away, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("file")]
    public required string File { get; init; }

    /// <summary>The first version kept in the new chain, or empty for the version above the origin.</summary>
    [JsonPropertyName("target")]
    public string TargetVersion { get; init; } = string.Empty;

    /// <summary>Whether the entry keeps its `Uuid`, which forks it from the Vault.</summary>
    [JsonPropertyName("keep_uuid")]
    public bool KeepUuid { get; init; }

    /// <summary>Whether every merge in the kept range is dropped.</summary>
    [JsonPropertyName("strip_join")]
    public bool StripJoin { get; init; }
}

/// <summary>Puts back what the last recorded operations changed, newest first.</summary>
public sealed record UndoRequest : WriteRequest
{
    /// <summary>How many operations to undo, or zero for one.</summary>
    [JsonPropertyName("count")]
    public int Count { get; init; }

    /// <summary>Whether only the Layout moves back and every file is left as it is.</summary>
    [JsonPropertyName("soft")]
    public bool Soft { get; init; }

    /// <summary>What to do with an operation that stands only in part.</summary>
    [JsonPropertyName("on_partial")]
    public UndoPartial OnPartial { get; init; }
}

/// <summary>Applies a sheet to a path's chain, rebuilding what follows the point adjusted.</summary>
public sealed record TweakRequest : WriteRequest
{
    /// <summary>The one path whose chain is adjusted, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("file")]
    public required string File { get; init; }

    /// <summary>The sheet, as the editor writes one.</summary>
    [JsonPropertyName("sheet")]
    public required string Sheet { get; init; }
}

/// <summary>Drops the plan waiting for one path.</summary>
public sealed record TweakCancelRequest : WriteRequest
{
    /// <summary>The one path whose pending plan is dropped, relative to the Workspace root or absolute.</summary>
    [JsonPropertyName("file")]
    public required string File { get; init; }
}

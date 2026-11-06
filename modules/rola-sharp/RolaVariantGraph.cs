using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace RolaSharp;

/// <summary>One entry a variant graph is centred on: the side it is named on, and its path.</summary>
/// <param name="Side">
/// The side the entry is named on: <c>local</c> for the Layout being worked in, or the name of a
/// Vault whose copy was fetched.
/// </param>
/// <param name="Path">The path the entry is at, as the Layout names it.</param>
public sealed record VariantGraphFocus(string Side, string Path);

/// <summary>What a variant graph is asked for.</summary>
/// <remarks>
/// The request is a window, not a file: <see cref="Focus"/> names the entries to stand on and how far
/// to look is what the rest of the request says. A reading that reaches the edge of a window asks
/// again from the node it reached, so a large history is read a part at a time.
/// </remarks>
public sealed record VariantGraphRequest
{
    /// <summary>
    /// The entries the graph is centred on, or empty to stand on the work's own entries.
    /// </summary>
    /// <remarks>
    /// Empty stands on every entry of the Layout being worked in that records a version, which is the
    /// reading a reader that has not chosen a file yet is given.
    /// </remarks>
    public IReadOnlyList<VariantGraphFocus>? Focus { get; init; }

    /// <summary>How many variants back towards the root to walk from each focus.</summary>
    public uint Back { get; init; } = 64;

    /// <summary>How many variants forward from each focus to walk.</summary>
    public uint Forward { get; init; }

    /// <summary>Whether to walk into what a variant merges in.</summary>
    public bool Joins { get; init; } = true;

    /// <summary>Whether to take in the variants based on a version in the window, following each branch on.</summary>
    public bool Branches { get; init; } = true;

    /// <summary>How many variants the window holds at most.</summary>
    public uint MaxNodes { get; init; } = 400;
}

/// <summary>What an entry the reading stood on resolved to.</summary>
/// <param name="Side">The side it is named on.</param>
/// <param name="Path">The path it is at.</param>
/// <param name="Version">The version it records, as hex, or nothing when it records none.</param>
/// <param name="Variant">
/// The variant that version fixes, as hex, or nothing when the reading did not resolve one.
/// </param>
public sealed record VariantGraphFocusResolution(
    string Side,
    string Path,
    string? Version,
    string? Variant
);

/// <summary>Where a node's walks were not taken in.</summary>
/// <param name="Parents">The base variant, or the version it is based on, is not in the window.</param>
/// <param name="Children">A variant based on one of the node's versions is not in the window.</param>
/// <param name="Joined">A variant the node merges in is not in the window.</param>
public sealed record VariantGraphFrontier(bool Parents, bool Children, bool Joined);

/// <summary>One variant in the window.</summary>
/// <param name="Hash">The variant's own key, as hex.</param>
/// <param name="BaseVersion">The version it is based on, as hex.</param>
/// <param name="BaseVariant">
/// The variant that version fixes, as hex, or nothing when the index does not hold it.
/// </param>
/// <param name="Join">The variant it merges in, as hex, or nothing when it is no merge.</param>
/// <param name="Storage">The content it was made of, as the store's key in hex.</param>
/// <param name="Creator">Who made it, as the hash of the Creator index object.</param>
/// <param name="Message">What it says was done, as the hash of the Message index object.</param>
/// <param name="Frontier">Where the window stops short of what this node names.</param>
public sealed record VariantGraphNode(
    string Hash,
    string BaseVersion,
    string? BaseVariant,
    string? Join,
    string Storage,
    string Creator,
    string Message,
    VariantGraphFrontier Frontier
);

/// <summary>One version fixing a variant in the window.</summary>
/// <param name="Hash">The version's own key, as hex.</param>
/// <param name="Variant">The variant it fixes, as hex; the zero hash for the root.</param>
/// <param name="Number">
/// Its number in its chain, or nothing for the root and for a chain cut off from it.
/// </param>
/// <param name="IsRoot">Whether it is the root, numbered before the first.</param>
public sealed record VariantGraphVersion(string Hash, string Variant, ulong? Number, bool IsRoot);

/// <summary>One edge between two variants in the window.</summary>
/// <param name="From">The variant the edge is walked from.</param>
/// <param name="To">The variant it names.</param>
/// <param name="Kind">What the edge is: <c>base</c> or <c>join</c>.</param>
public sealed record VariantGraphEdge(string From, string To, string Kind);

/// <summary>One key the window names but the index does not hold.</summary>
/// <param name="Hash">The key, as hex.</param>
/// <param name="Kind">What it would have been: <c>version</c> or <c>variant</c>.</param>
public sealed record VariantGraphMissing(string Hash, string Kind);

/// <summary>One entry of one side, named beside the graph.</summary>
/// <param name="Side">The side it is named on.</param>
/// <param name="Path">The path it is at.</param>
/// <param name="Uuid">The <c>Uuid</c> it is held by.</param>
/// <param name="Version">The version it records, as hex, or nothing when it records none.</param>
/// <param name="Owner">The account that holds it, or nothing when none does.</param>
/// <param name="Deprecated">Whether it was moved under the deprecated marker.</param>
public sealed record VariantGraphEntry(
    string Side,
    string Path,
    string Uuid,
    string? Version,
    string? Owner,
    bool Deprecated
);

/// <summary>One merge in progress.</summary>
/// <param name="Variant">The variant that was checked in, as the hex it is named by.</param>
/// <param name="Target">The <c>Uuid</c> of the file it is to be joined into.</param>
/// <param name="Path">The path that file is at, when the Layout still names it.</param>
/// <param name="File">Where the variant file lies.</param>
public sealed record VariantGraphMerging(string Variant, string Target, string Path, string File);

/// <summary>The words of what the window's variants name.</summary>
/// <param name="Creators">Each creator's name, by the hash the variant names it by.</param>
/// <param name="Messages">Each message, by the hash the variant names it by.</param>
public sealed record VariantGraphTexts(
    IReadOnlyDictionary<string, string> Creators,
    IReadOnlyDictionary<string, string> Messages
);

/// <summary>
/// The variant graph around a set of entries, as the Desktop's Version Graph draws it.
/// </summary>
/// <remarks>
/// The graph is the index's: which variants there are, how they are based on one another, and what
/// they merge in. What only the workspace knows is read beside it: the entries of the Layout being
/// worked in and of every fetched copy, the merges in progress, and the text of each creator and
/// message the window's variants name. A key the index does not hold is reported in
/// <see cref="Missing"/> rather than fetched: a reading never reaches the network.
/// <para>
/// The answer is a snapshot read out of the native side whole, so asking it again cannot answer
/// anything newer. A reading is a value rather than a resource: there is no native handle to give
/// back, and every list and text is in managed memory by the time <see cref="Locate"/> returns.
/// </para>
/// </remarks>
public sealed class RolaVariantGraph
{
    /// <summary>How requests and answers are named: the native side spells its fields in snake case.</summary>
    private static readonly JsonSerializerOptions Format = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    /// <summary>Makes a reading from the answer the native side gave.</summary>
    /// <param name="payload">The answer, which must not be null.</param>
    private RolaVariantGraph(Payload payload)
    {
        Focus = payload.Focus;
        Nodes = payload.Nodes;
        Versions = payload.Versions;
        Edges = payload.Edges;
        Missing = payload.Missing;
        Truncated = payload.Truncated;
        Entries = payload.Entries;
        Merging = payload.Merging;
        Texts = payload.Texts;
    }

    /// <summary>What each entry the request stood on resolved to, in the order asked.</summary>
    public IReadOnlyList<VariantGraphFocusResolution> Focus { get; }

    /// <summary>The variants in the window, in key order.</summary>
    public IReadOnlyList<VariantGraphNode> Nodes { get; }

    /// <summary>The versions fixing them, in key order.</summary>
    public IReadOnlyList<VariantGraphVersion> Versions { get; }

    /// <summary>The edges between variants in the window.</summary>
    public IReadOnlyList<VariantGraphEdge> Edges { get; }

    /// <summary>The keys the window names but the index does not hold.</summary>
    public IReadOnlyList<VariantGraphMissing> Missing { get; }

    /// <summary>Whether the window was cut short by its cap rather than walked out.</summary>
    public bool Truncated { get; }

    /// <summary>The entries whose version the window holds, the work's own first.</summary>
    public IReadOnlyList<VariantGraphEntry> Entries { get; }

    /// <summary>The merges in progress.</summary>
    public IReadOnlyList<VariantGraphMerging> Merging { get; }

    /// <summary>The words of what the window's variants name.</summary>
    public VariantGraphTexts Texts { get; }

    /// <summary>
    /// Reads the variant graph around the entries <paramref name="request"/> names.
    /// </summary>
    /// <remarks>
    /// The inverse index the graph is walked over is rebuilt first when it does not describe the
    /// index, which is derived data and never the network: what is drawn is what the index holds now.
    /// </remarks>
    /// <param name="directory">The directory the Workspace is searched upwards from.</param>
    /// <param name="request">What to read, or nothing for the work's own entries and their chains.</param>
    /// <returns>
    /// The reading, or nothing when there is nothing to draw, no Workspace holds the directory, the
    /// Workspace works in no Layout, or it holds no version control index.
    /// </returns>
    /// <exception cref="InvalidOperationException">
    /// The native side could not read the request, the index, or the records beside it.
    /// </exception>
    public static RolaVariantGraph? Locate(string directory, VariantGraphRequest? request = null)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var at = Marshal.StringToCoTaskMemUTF8(directory);
        var asked = Marshal.StringToCoTaskMemUTF8(
            JsonSerializer.Serialize(request ?? new VariantGraphRequest(), Format)
        );

        try
        {
            var answer = RorolalaBinding.locate_rola_variant_graph_json(at, asked);

            if (answer.tag != RorolalaResultTag.RorolalaResult_Ok)
            {
                var (nothing, reason) = Failure(answer.payload);

                return nothing ? null : throw new InvalidOperationException(reason);
            }

            var json = NativeText.Taken(answer.payload);

            return new RolaVariantGraph(
                JsonSerializer.Deserialize<Payload>(json, Format)
                    ?? throw new InvalidOperationException("the variant graph read as nothing")
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(asked);
        }
    }

    /// <summary>Reads what the native side failed with, and gives the failure back.</summary>
    /// <param name="error">The failure, which this takes ownership of.</param>
    /// <returns>Whether it is only that there is nothing to draw, and what it said.</returns>
    private static (bool Nothing, string Reason) Failure(nint error)
    {
        var read = Marshal.PtrToStructure<RolaVariantGraphError>(error);

        // The reasons are copied before the failure is given back: they are the failure's to release,
        // and a pointer into it read afterwards would be a pointer into what was given back.
        var reason = read.tag switch
        {
            RolaVariantGraphErrorTag.RolaVariantGraphError_BadRequest => NativeText.Copied(
                read.payload.BadRequest.reason
            ),
            RolaVariantGraphErrorTag.RolaVariantGraphError_Read => NativeText.Copied(
                read.payload.Read.reason
            ),
            _ => string.Empty,
        };

        RorolalaBinding.free_rola_variant_graph_error(error);

        var nothing = read.tag
            is RolaVariantGraphErrorTag.RolaVariantGraphError_NoWorkspace
                or RolaVariantGraphErrorTag.RolaVariantGraphError_NoLayout
                or RolaVariantGraphErrorTag.RolaVariantGraphError_NoIndex;

        return (nothing, reason);
    }

    /// <summary>The answer the native side wrote, read as fields.</summary>
    /// <param name="Focus">What each entry the request stood on resolved to.</param>
    /// <param name="Nodes">The variants in the window.</param>
    /// <param name="Versions">The versions fixing them.</param>
    /// <param name="Edges">The edges between variants in the window.</param>
    /// <param name="Missing">The keys the window names but the index does not hold.</param>
    /// <param name="Truncated">Whether the window was cut short by its cap.</param>
    /// <param name="Entries">Every entry of every side.</param>
    /// <param name="Merging">The merges in progress.</param>
    /// <param name="Texts">The words of what the window's variants name.</param>
    private sealed record Payload(
        IReadOnlyList<VariantGraphFocusResolution> Focus,
        IReadOnlyList<VariantGraphNode> Nodes,
        IReadOnlyList<VariantGraphVersion> Versions,
        IReadOnlyList<VariantGraphEdge> Edges,
        IReadOnlyList<VariantGraphMissing> Missing,
        bool Truncated,
        IReadOnlyList<VariantGraphEntry> Entries,
        IReadOnlyList<VariantGraphMerging> Merging,
        VariantGraphTexts Texts
    );
}

using System.Runtime.InteropServices;

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
/// <param name="Offline">
/// Whether every object the content needs is held here, so the variant can be used without a Vault:
/// its content, every chunk of it, and every file a tree names.
/// </param>
/// <param name="Creator">Who made it, as the hash of the Creator index object.</param>
/// <param name="Message">What it says was done, as the hash of the Message index object.</param>
/// <param name="Frontier">Where the window stops short of what this node names.</param>
public sealed record VariantGraphNode(
    string Hash,
    string BaseVersion,
    string? BaseVariant,
    string? Join,
    string Storage,
    bool Offline,
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
/// <param name="Variant">The variant that was brought in, as the hex it is named by.</param>
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
public sealed class VariantGraphReading
{
    /// <summary>Makes a reading from what the native side answered.</summary>
    private VariantGraphReading(
        IReadOnlyList<VariantGraphFocusResolution> focus,
        IReadOnlyList<VariantGraphNode> nodes,
        IReadOnlyList<VariantGraphVersion> versions,
        IReadOnlyList<VariantGraphEdge> edges,
        IReadOnlyList<VariantGraphMissing> missing,
        bool truncated,
        IReadOnlyList<VariantGraphEntry> entries,
        IReadOnlyList<VariantGraphMerging> merging,
        VariantGraphTexts texts
    )
    {
        Focus = focus;
        Nodes = nodes;
        Versions = versions;
        Edges = edges;
        Missing = missing;
        Truncated = truncated;
        Entries = entries;
        Merging = merging;
        Texts = texts;
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
    /// The native side could not read the index, or the records beside it.
    /// </exception>
    public static VariantGraphReading? Locate(string directory, VariantGraphRequest? request = null)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var asked = request ?? new VariantGraphRequest();
        var at = Marshal.StringToCoTaskMemUTF8(directory);
        var borrowed = new List<nint>();
        nint window = 0;
        nint focus = 0;

        try
        {
            var focuses = asked.Focus ?? [];
            var size = Marshal.SizeOf<RolaVariantGraphFocus>();

            if (focuses.Count > 0)
            {
                focus = Marshal.AllocCoTaskMem(size * focuses.Count);

                for (var index = 0; index < focuses.Count; index++)
                {
                    var one = focuses[index];
                    var path = Hold(borrowed, one.Path);
                    var name = one.Side == "local" ? 0 : Hold(borrowed, one.Side);

                    Marshal.StructureToPtr(
                        new RolaVariantGraphFocus { side = Side(name), path = path },
                        focus + (index * size),
                        false
                    );
                }
            }

            window = Marshal.AllocCoTaskMem(Marshal.SizeOf<RolaVariantGraphWindow>());
            Marshal.StructureToPtr(
                new RolaVariantGraphWindow
                {
                    back = asked.Back,
                    forward = asked.Forward,
                    joins = (byte)(asked.Joins ? 1 : 0),
                    branches = (byte)(asked.Branches ? 1 : 0),
                    max_nodes = asked.MaxNodes,
                },
                window,
                false
            );

            var answer = RorolalaBinding.rola_variant_graph_locate(
                at,
                window,
                focus,
                (nuint)focuses.Count
            );

            if (answer.tag != RorolalaResultTag.RorolalaResult_Ok)
            {
                var (nothing, reason) = Failure(answer.payload);

                return nothing ? null : throw new InvalidOperationException(reason);
            }

            var handle = answer.payload;
            if (handle == 0)
            {
                throw new InvalidOperationException("the variant graph read as nothing");
            }

            try
            {
                return Read(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(at);

            if (window != 0)
            {
                Marshal.FreeCoTaskMem(window);
            }

            if (focus != 0)
            {
                Marshal.FreeCoTaskMem(focus);
            }

            foreach (var text in borrowed)
            {
                Marshal.FreeCoTaskMem(text);
            }
        }
    }

    /// <summary>Keeps a string in unmanaged memory for the call, answering where it lies.</summary>
    private static nint Hold(List<nint> borrowed, string text)
    {
        var held = Marshal.StringToCoTaskMemUTF8(text);
        borrowed.Add(held);

        return held;
    }

    /// <summary>The side a focus is named on, with the Vault's name when it names one.</summary>
    private static RolaVariantGraphSide Side(nint name) =>
        name == 0
            ? new RolaVariantGraphSide { tag = RolaVariantGraphSideTag.RolaVariantGraphSide_Local }
            : new RolaVariantGraphSide
            {
                tag = RolaVariantGraphSideTag.RolaVariantGraphSide_Vault,
                payload = new RolaVariantGraphSidePayload
                {
                    Vault = new RolaVariantGraphSideVault { name = name },
                },
            };

    /// <summary>Reads what the native side failed with, and gives the failure back.</summary>
    /// <param name="error">The failure, which this takes ownership of.</param>
    /// <returns>Whether it is only that there is nothing to draw, and what it said.</returns>
    private static (bool Nothing, string Reason) Failure(nint error)
    {
        var read = Marshal.PtrToStructure<RolaVariantGraphError>(error);

        // The reason is copied before the failure is given back: it is the failure's to release, and
        // a pointer into it read afterwards would be a pointer into what was given back.
        var reason =
            read.tag is RolaVariantGraphErrorTag.RolaVariantGraphError_Read
                ? NativeText.Copied(read.payload.Read.reason)
                : string.Empty;

        RorolalaBinding.free_rola_variant_graph_error(error);

        var nothing = read.tag
            is RolaVariantGraphErrorTag.RolaVariantGraphError_NoWorkspace
                or RolaVariantGraphErrorTag.RolaVariantGraphError_NoLayout
                or RolaVariantGraphErrorTag.RolaVariantGraphError_NoIndex;

        return (nothing, reason);
    }

    /// <summary>Reads the answer the native side wrote, releasing the handle's items one by one.</summary>
    private static VariantGraphReading Read(nint handle) =>
        new(
            ReadFocus(handle),
            ReadNodes(handle),
            ReadVersions(handle),
            ReadEdges(handle),
            ReadMissing(handle),
            RorolalaBinding.rola_variant_graph_truncated(handle),
            ReadEntries(handle),
            ReadMerging(handle),
            new VariantGraphTexts(ReadTexts(handle, creators: true), ReadTexts(handle, creators: false))
        );

    /// <summary>What each entry the request stood on resolved to.</summary>
    private static IReadOnlyList<VariantGraphFocusResolution> ReadFocus(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_focus_count(handle);
        var focus = new List<VariantGraphFocusResolution>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_focus_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                focus.Add(
                    new VariantGraphFocusResolution(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_focus_resolution_side(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_focus_resolution_path(item)),
                        Optional(
                            NativeText.Taken(
                                RorolalaBinding.rola_variant_graph_focus_resolution_version(item)
                            )
                        ),
                        Optional(
                            NativeText.Taken(
                                RorolalaBinding.rola_variant_graph_focus_resolution_variant(item)
                            )
                        )
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_focus_resolution(item);
            }
        }

        return focus;
    }

    /// <summary>The variants in the window.</summary>
    private static IReadOnlyList<VariantGraphNode> ReadNodes(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_node_count(handle);
        var nodes = new List<VariantGraphNode>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_node_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                nodes.Add(
                    new VariantGraphNode(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_node_hash(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_node_base_version(item)),
                        Optional(
                            NativeText.Taken(
                                RorolalaBinding.rola_variant_graph_node_base_variant(item)
                            )
                        ),
                        Optional(NativeText.Taken(RorolalaBinding.rola_variant_graph_node_join(item))),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_node_storage(item)),
                        RorolalaBinding.rola_variant_graph_node_offline(item),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_node_creator(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_node_message(item)),
                        new VariantGraphFrontier(
                            RorolalaBinding.rola_variant_graph_node_frontier_parents(item),
                            RorolalaBinding.rola_variant_graph_node_frontier_children(item),
                            RorolalaBinding.rola_variant_graph_node_frontier_joined(item)
                        )
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_node(item);
            }
        }

        return nodes;
    }

    /// <summary>The versions fixing the window's variants.</summary>
    private static IReadOnlyList<VariantGraphVersion> ReadVersions(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_version_count(handle);
        var versions = new List<VariantGraphVersion>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_version_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                var number = RorolalaBinding.rola_variant_graph_version_number_present(item)
                    ? RorolalaBinding.rola_variant_graph_version_number(item)
                    : (ulong?)null;

                versions.Add(
                    new VariantGraphVersion(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_version_hash(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_version_variant(item)),
                        number,
                        RorolalaBinding.rola_variant_graph_version_is_root(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_version(item);
            }
        }

        return versions;
    }

    /// <summary>The edges between the window's variants.</summary>
    private static IReadOnlyList<VariantGraphEdge> ReadEdges(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_edge_count(handle);
        var edges = new List<VariantGraphEdge>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_edge_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                edges.Add(
                    new VariantGraphEdge(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_edge_from(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_edge_to(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_edge_kind(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_edge(item);
            }
        }

        return edges;
    }

    /// <summary>The keys the window names that the index does not hold.</summary>
    private static IReadOnlyList<VariantGraphMissing> ReadMissing(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_missing_count(handle);
        var missing = new List<VariantGraphMissing>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_missing_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                missing.Add(
                    new VariantGraphMissing(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_missing_hash(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_missing_kind(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_missing(item);
            }
        }

        return missing;
    }

    /// <summary>The entries whose version the window holds.</summary>
    private static IReadOnlyList<VariantGraphEntry> ReadEntries(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_entry_count(handle);
        var entries = new List<VariantGraphEntry>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_entry_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                entries.Add(
                    new VariantGraphEntry(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_entry_side(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_entry_path(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_entry_uuid(item)),
                        Optional(
                            NativeText.Taken(RorolalaBinding.rola_variant_graph_entry_version(item))
                        ),
                        Optional(
                            NativeText.Taken(RorolalaBinding.rola_variant_graph_entry_owner(item))
                        ),
                        RorolalaBinding.rola_variant_graph_entry_deprecated(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_entry(item);
            }
        }

        return entries;
    }

    /// <summary>The merges in progress.</summary>
    private static IReadOnlyList<VariantGraphMerging> ReadMerging(nint handle)
    {
        var count = RorolalaBinding.rola_variant_graph_merging_count(handle);
        var merging = new List<VariantGraphMerging>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_variant_graph_merging_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                merging.Add(
                    new VariantGraphMerging(
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_merging_variant(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_merging_target(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_merging_path(item)),
                        NativeText.Taken(RorolalaBinding.rola_variant_graph_merging_file(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_merging(item);
            }
        }

        return merging;
    }

    /// <summary>The text of each creator or message the window's variants name.</summary>
    private static IReadOnlyDictionary<string, string> ReadTexts(nint handle, bool creators)
    {
        var count = creators
            ? RorolalaBinding.rola_variant_graph_creator_count(handle)
            : RorolalaBinding.rola_variant_graph_message_count(handle);
        var texts = new Dictionary<string, string>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = creators
                ? RorolalaBinding.rola_variant_graph_creator_at(handle, index)
                : RorolalaBinding.rola_variant_graph_message_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                texts[NativeText.Taken(RorolalaBinding.rola_variant_graph_text_hash(item))] =
                    NativeText.Taken(RorolalaBinding.rola_variant_graph_text_text(item));
            }
            finally
            {
                RorolalaBinding.free_rola_variant_graph_text(item);
            }
        }

        return texts;
    }

    /// <summary>The text, or nothing when the native side answered an empty string.</summary>
    private static string? Optional(string text) => string.IsNullOrEmpty(text) ? null : text;
}

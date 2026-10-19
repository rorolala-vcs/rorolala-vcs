using RolaSharp;

namespace RorolalaVCSPlugin;

/// <summary>
/// The version chain as geometry: versions on one vertical axis, variants on another to their left,
/// and the variants merged in further left still.
/// </summary>
/// <remarks>
/// This is the desktop's answer to the command line's character drawing. The unit is a content unit:
/// the renderer multiplies every position by the zoom, so the shape holds at every scale.
/// </remarks>
internal sealed class VersionGraphDrawing
{
    /// <summary>How far the version axis stands from the variant axis.</summary>
    internal const double AxisGap = 100;

    /// <summary>How far apart the variants of one level are laid out.</summary>
    internal const double MergedGap = 92;

    /// <summary>Where the furthest merged variant's axis starts.</summary>
    internal const double VariantStart = 60;

    /// <summary>Where the highest version's centre stands.</summary>
    internal const double TopY = 56;

    /// <summary>The radius of a version node.</summary>
    internal const double Radius = 17;

    /// <summary>The height of a variant capsule.</summary>
    internal const double CapsuleHeight = 24;

    /// <summary>How far apart two version nodes are.</summary>
    internal const double VersionStep = 92;

    /// <summary>How far below its version a variant node stands.</summary>
    internal const double VersionToVariant = 46;

    /// <summary>How far to the right of its node a version's bubble starts.</summary>
    internal const double BubbleGap = 24;

    /// <summary>How far the tail reaches from the bubble's left edge.</summary>
    /// <remarks>
    /// Shorter than the gap on purpose: the tip stands <c>BubbleGap - BubbleTail</c> short of the
    /// node's edge, so the arrow points at the node rather than touching it.
    /// </remarks>
    internal const double BubbleTail = 14;

    /// <summary>Half the height of the tail's base on the bubble's edge.</summary>
    internal const double BubbleTailHalf = 8;

    /// <summary>The room under the oldest version.</summary>
    internal const double BottomMargin = 56;

    /// <summary>Makes a drawing from its pieces.</summary>
    private VersionGraphDrawing(
        IReadOnlyList<VersionNode> versions,
        IReadOnlyList<VariantNode> variants,
        IReadOnlyDictionary<ulong, VersionNode> versionByLevel,
        IReadOnlyDictionary<ulong, VariantNode> spineByLevel,
        double versionX,
        double variantX,
        IReadOnlyList<UpstreamNode> upstream,
        double upstreamX,
        ulong? branchFrom,
        bool diverged,
        string upstreamVault,
        double height,
        ulong top
    )
    {
        Versions = versions;
        Variants = variants;
        VersionByLevel = versionByLevel;
        SpineByLevel = spineByLevel;
        VersionX = versionX;
        VariantX = variantX;
        Upstream = upstream;
        UpstreamX = upstreamX;
        BranchFrom = branchFrom;
        Diverged = diverged;
        UpstreamVault = upstreamVault;
        Height = height;
        Top = top;
    }

    /// <summary>The versions, the highest first.</summary>
    public IReadOnlyList<VersionNode> Versions { get; }

    /// <summary>The variants in the chain's order.</summary>
    public IReadOnlyList<VariantNode> Variants { get; }

    /// <summary>The version at each level.</summary>
    public IReadOnlyDictionary<ulong, VersionNode> VersionByLevel { get; }

    /// <summary>The spine variant at each level.</summary>
    public IReadOnlyDictionary<ulong, VariantNode> SpineByLevel { get; }

    /// <summary>The column the version nodes stand on.</summary>
    public double VersionX { get; }

    /// <summary>The column the spine variants stand on.</summary>
    public double VariantX { get; }

    /// <summary>
    /// The tracked Vault's versions that are not part of the local chain, the highest first.
    /// </summary>
    /// <remarks>
    /// Only what is local is drawn: a version the index here holds is walked, and one it does not is
    /// the one placeholder a reader gets instead — see <see cref="Upstream"/> and
    /// <see cref="UpstreamVault"/>. Versions both chains hold are drawn once, on the local axis.
    /// </remarks>
    public IReadOnlyList<UpstreamNode> Upstream { get; }

    /// <summary>The column the upstream nodes stand on.</summary>
    /// <remarks>
    /// It is the local versions' own column while the Vault is only ahead, and a second column to its
    /// right once the chains have parted.
    /// </remarks>
    public double UpstreamX { get; }

    /// <summary>The local level the upstream branch starts from, or nothing when there is no upstream node.</summary>
    /// <remarks>
    /// It is the highest level both chains hold the same version at — the common ancestor — or the
    /// local head when nothing is shared.
    /// </remarks>
    public ulong? BranchFrom { get; }

    /// <summary>
    /// Whether the two chains have each moved on from the common ancestor, which is what puts the Vault's
    /// versions in a column of their own and joins them with an arc.
    /// </summary>
    /// <remarks>
    /// A Vault that is merely ahead is the local chain continued: its versions stand in the same column,
    /// above the work's head. Only a divergence is a branch.
    /// </remarks>
    public bool Diverged { get; }

    /// <summary>The Vault whose versions <see cref="Upstream"/> is, or an empty string when none is tracked.</summary>
    public string UpstreamVault { get; }

    /// <summary>How tall the drawing is, in content units.</summary>
    public double Height { get; }

    /// <summary>The number of the highest version.</summary>
    public ulong Top { get; }

    /// <summary>Lays a chain out.</summary>
    /// <param name="graph">The chain read from the index.</param>
    /// <param name="editing">
    /// How the working file stands beside what was recorded, when it has changes that were never
    /// recorded; nothing when it stands as recorded.
    /// </param>
    /// <param name="unjoined">
    /// The variants the chain draws nowhere — the ones the work may still be on its way through —
    /// or nothing when the graph is not showing them.
    /// </param>
    /// <param name="merging">
    /// The variant waiting to be joined into the file, as its hash, or nothing when no merge is in
    /// progress. It is the one a checkin left behind, and it is drawn in the colour of work under way.
    /// </param>
    /// <returns>The drawing.</returns>
    /// <remarks>
    /// An edit is drawn as a variant of its own, based on the highest version: it is the variant that
    /// would make the version the index does not have yet, so it stands where every variant stands —
    /// in the row above the version it is based on. The versions move down a row to make it room, since
    /// there is no version above it for that row to sit under.
    /// <para>
    /// An unjoined variant is drawn where a merged one is, in the column to the left of the spine and
    /// on the row of the version it is based on, but with no edge: nothing joins it into the chain, and
    /// the absence of an edge is what says so. A variant based on the highest version needs the same row
    /// above it that an edit does, so the versions move down for it as well.
    /// </para>
    /// </remarks>
    public static VersionGraphDrawing From(
        RolaVersionGraph graph,
        (string Words, bool Wrong)? editing,
        IReadOnlyList<UnjoinedVariant>? unjoined = null,
        string? merging = null
    )
    {
        ArgumentNullException.ThrowIfNull(graph);

        var versions = graph.Versions;
        var localTop = versions.Count == 0 ? 0UL : versions.Max(version => version.Level);
        var localByLevel = versions.ToDictionary(version => version.Level);

        // The highest level both chains hold the same version at. The versions under it are the
        // common ancestors and are already drawn on the local axis; only what is above it belongs to
        // the branch.
        var shared = -1L;

        foreach (var one in graph.Upstream)
        {
            if (
                localByLevel.TryGetValue(one.Level, out var local)
                && string.Equals(local.Hash, one.Hash, StringComparison.Ordinal)
            )
            {
                shared = Math.Max(shared, (long)one.Level);
            }
        }

        var upstream = new List<UpstreamNode>();

        foreach (var one in graph.Upstream)
        {
            if ((long)one.Level > shared)
            {
                upstream.Add(
                    new UpstreamNode(
                        one.Level,
                        $"V{one.Level}",
                        0,
                        0,
                        one.Hash,
                        one.Creator,
                        one.Message,
                        false
                    )
                );
            }
        }

        // A Vault's version the index does not hold — the ordinary state after a fetch — is one
        // node with the hash the Layout names it by, set above the local chain. It is not a level
        // anything here knows, so its label says that rather than naming a version number.
        if (upstream.Count == 0 && graph.Upstream.Count == 0 && graph.UpstreamHead.Length > 0)
        {
            upstream.Add(
                new UpstreamNode(
                    localTop + 1,
                    "?",
                    0,
                    0,
                    graph.UpstreamHead,
                    string.Empty,
                    string.Empty,
                    true
                )
            );
        }

        var upstreamTop = upstream.Count == 0 ? localTop : upstream.Max(one => one.Level);
        var top = Math.Max(localTop, upstreamTop);
        var extras = unjoined is null
            ? Array.Empty<UnjoinedVariant>()
            : unjoined.Where(one => one.VersionNumber <= localTop).ToArray();
        var mergedCounts = graph
            .Variants.Where(variant => !variant.Spine)
            .GroupBy(variant => variant.Level)
            .ToDictionary(group => group.Key, group => group.Count());

        foreach (var group in extras.GroupBy(one => one.VersionNumber))
        {
            mergedCounts[group.Key] = mergedCounts.GetValueOrDefault(group.Key) + group.Count();
        }

        var merged = mergedCounts.Values.DefaultIfEmpty(0).Max();
        var hasTopExtra = extras.Any(one => one.VersionNumber == localTop);
        var shift = editing is null && !hasTopExtra ? 0 : VersionToVariant;
        var localTopY = TopY + shift + (double)(top - localTop) * VersionStep;

        // Both sides have moved on from the common ancestor only then is there a branch to draw: a
        // Vault that is merely ahead continues the work's own chain in one column. A version only the
        // Layout names — the `?` node — is not a chain, so it is drawn in that one column as well.
        var diverged =
            upstream.Any(one => !one.Unknown)
            && (shared < 0 || localTop > (ulong)shared);
        var variantX = VariantStart + MergedGap * merged;
        var versionX = variantX + AxisGap;
        var upstreamX = diverged ? versionX + AxisGap : versionX;
        var versionNodes = new List<VersionNode>(versions.Count);
        var versionByLevel = new Dictionary<ulong, VersionNode>();

        foreach (var version in versions)
        {
            var origin = graph.OriginLevel == version.Level;
            var message = version.Message;

            if (origin)
            {
                message = message.Length > 0
                    ? $"{message} ({graph.OriginVault})"
                    : $"({graph.OriginVault})";
            }

            var node = new VersionNode(
                version.Level,
                $"V{version.Level}",
                versionX,
                TopY + shift + (double)(top - version.Level) * VersionStep,
                version.Hash,
                version.Creator,
                message,
                origin
            );

            versionNodes.Add(node);
            versionByLevel[version.Level] = node;
        }

        var upstreamNodes = new List<UpstreamNode>(upstream.Count);

        foreach (var one in upstream)
        {
            upstreamNodes.Add(
                one with
                {
                    X = upstreamX,
                    Y = TopY + shift + (double)(top - one.Level) * VersionStep,
                }
            );
        }

        var variantNodes = new List<VariantNode>(graph.Variants.Count + extras.Length);
        var spineByLevel = new Dictionary<ulong, VariantNode>();
        var mergedOrder = new Dictionary<ulong, int>();

        // Where a variant of a level stands: under the version it produced, or in the row above the
        // highest version when what it is based on is that version itself.
        double VariantY(ulong level) =>
            versionByLevel.TryGetValue(level + 1, out var above)
                ? above.Y + VersionToVariant
                : top > localTop || editing is not null || hasTopExtra
                    ? localTopY - VersionToVariant
                    : TopY;

        for (var index = 0; index < graph.Variants.Count; index++)
        {
            var variant = graph.Variants[index];
            var order = 0;

            if (!variant.Spine)
            {
                mergedOrder.TryGetValue(variant.Level, out order);
                mergedOrder[variant.Level] = order + 1;
            }

            var x = variant.Spine
                ? variantX
                : variantX - MergedGap * (order + 1);
            var node = new VariantNode(
                index,
                variant.Label,
                variant.Hash,
                variant.Level,
                variant.Spine,
                variant.Join,
                x,
                VariantY(variant.Level),
                variant.Creator,
                variant.Message,
                null,
                false,
                Merging: IsMerging(variant.Hash, merging)
            );

            variantNodes.Add(node);

            if (variant.Spine)
            {
                spineByLevel[variant.Level] = node;
            }
        }

        foreach (var extra in extras)
        {
            mergedOrder.TryGetValue(extra.VersionNumber, out var order);
            mergedOrder[extra.VersionNumber] = order + 1;

            variantNodes.Add(
                new VariantNode(
                    variantNodes.Count,
                    extra.Hash[..Math.Min(7, extra.Hash.Length)],
                    extra.Hash,
                    extra.VersionNumber,
                    false,
                    null,
                    variantX - MergedGap * (order + 1),
                    VariantY(extra.VersionNumber),
                    extra.Creator,
                    extra.Message,
                    null,
                    false,
                    Unjoined: true,
                    Merging: IsMerging(extra.Hash, merging)
                )
            );
        }

        if (editing is { } edit)
        {
            variantNodes.Add(
                new VariantNode(
                    variantNodes.Count,
                    "???",
                    string.Empty,
                    localTop,
                    false,
                    null,
                    variantX,
                    localTopY - VersionToVariant,
                    string.Empty,
                    string.Empty,
                    edit.Words,
                    edit.Wrong
                )
            );
        }

        return new VersionGraphDrawing(
            versionNodes,
            variantNodes,
            versionByLevel,
            spineByLevel,
            versionX,
            variantX,
            upstreamNodes,
            upstreamX,
            upstreamNodes.Count == 0
                ? null
                : shared >= 0
                    ? (ulong)shared
                    : localTop,
            diverged,
            graph.UpstreamVault,
            TopY + shift + (double)top * VersionStep + BottomMargin,
            top
        );
    }

    /// <summary>Whether a variant's hash is the one waiting to be joined into the file.</summary>
    /// <param name="hash">The variant's hash, as the index names it.</param>
    /// <param name="merging">The pending variant's hash, or nothing when no merge is in progress.</param>
    /// <returns>Whether the two name the same variant.</returns>
    private static bool IsMerging(string hash, string? merging) =>
        merging is { Length: > 0 }
        && string.Equals(hash, merging, StringComparison.OrdinalIgnoreCase);

    /// <summary>One version, where it stands, and what it says.</summary>
    /// <param name="Level">The version's number, zero for the oldest.</param>
    /// <param name="Label">The word drawn in its node.</param>
    /// <param name="X">The column of its centre.</param>
    /// <param name="Y">The row of its centre.</param>
    /// <param name="Hash">The full hash it is named by.</param>
    /// <param name="Creator">Who made the variant that produced it.</param>
    /// <param name="Message">What that variant says, with the Vault's name when it is the marked one.</param>
    /// <param name="Origin">Whether it is the version the tracked Vault records.</param>
    internal sealed record VersionNode(
        ulong Level,
        string Label,
        double X,
        double Y,
        string Hash,
        string Creator,
        string Message,
        bool Origin
    );

    /// <summary>One version of the tracked Vault's chain that the local chain does not hold.</summary>
    /// <param name="Level">The version's number, numbered from the same root as the local chain.</param>
    /// <param name="Label">The word drawn in its node, or <c>?</c> for a version the index here does not hold.</param>
    /// <param name="X">The column of its centre.</param>
    /// <param name="Y">The row of its centre.</param>
    /// <param name="Hash">The full hash the Vault's Layout names it by.</param>
    /// <param name="Creator">Who made the variant that produced it, or an empty string when unknown.</param>
    /// <param name="Message">What that variant says, or an empty string when unknown.</param>
    /// <param name="Unknown">Whether the version is one only the Layout names, so that no level is known.</param>
    internal sealed record UpstreamNode(
        ulong Level,
        string Label,
        double X,
        double Y,
        string Hash,
        string Creator,
        string Message,
        bool Unknown
    );

    /// <summary>One variant, where it stands, and what it says.</summary>
    /// <param name="Index">Its index in the chain's variants.</param>
    /// <param name="Label">The label it is drawn by.</param>
    /// <param name="Hash">The full hash the index names it by, or an empty string for a placeholder.</param>
    /// <param name="Level">The version it is based on.</param>
    /// <param name="Spine">Whether it is the variant the version above points at.</param>
    /// <param name="Join">The index of the variant it merges into, when it merges one.</param>
    /// <param name="X">The column of its centre.</param>
    /// <param name="Y">The row of its centre.</param>
    /// <param name="Creator">Who made it.</param>
    /// <param name="Message">What it says.</param>
    /// <param name="State">
    /// What the working file is, when this node stands for an edit that was never recorded rather
    /// than for a variant the index holds; nothing for a recorded variant.
    /// </param>
    /// <param name="Wrong">Whether that edit must not be recorded — it is not this account's, or the
    /// version it is based on is not the one upstream is at.</param>
    /// <param name="Unjoined">
    /// Whether the variant is one the chain draws nowhere: it is based on a version of the chain, so it
    /// has the edge down to that version, but no version points at it and nothing joins it in.
    /// </param>
    /// <param name="Merging">
    /// Whether it is the variant waiting to be joined into the file, which is the one a checkin left
    /// behind and a track will record.
    /// </param>
    internal sealed record VariantNode(
        int Index,
        string Label,
        string Hash,
        ulong Level,
        bool Spine,
        int? Join,
        double X,
        double Y,
        string Creator,
        string Message,
        string? State,
        bool Wrong,
        bool Unjoined = false,
        bool Merging = false
    );
}

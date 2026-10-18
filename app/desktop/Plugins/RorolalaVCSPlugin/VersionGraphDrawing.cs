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
    /// <returns>The drawing.</returns>
    /// <remarks>
    /// An edit is drawn as a variant of its own, based on the highest version: it is the variant that
    /// would make the version the index does not have yet, so it stands where every variant stands —
    /// in the row above the version it is based on. The versions move down a row to make it room, since
    /// there is no version above it for that row to sit under.
    /// </remarks>
    public static VersionGraphDrawing From(
        RolaVersionGraph graph,
        (string Words, bool Wrong)? editing
    )
    {
        ArgumentNullException.ThrowIfNull(graph);

        var versions = graph.Versions;
        var top = versions.Count == 0 ? 0UL : versions.Max(version => version.Level);
        var shift = editing is null ? 0 : VersionToVariant;
        var merged = graph
            .Variants.Where(variant => !variant.Spine)
            .GroupBy(variant => variant.Level)
            .Select(group => group.Count())
            .DefaultIfEmpty(0)
            .Max();

        var variantX = VariantStart + MergedGap * merged;
        var versionX = variantX + AxisGap;
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

        var variantNodes = new List<VariantNode>(graph.Variants.Count);
        var spineByLevel = new Dictionary<ulong, VariantNode>();
        var mergedOrder = new Dictionary<ulong, int>();

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
            var y = versionByLevel.TryGetValue(variant.Level + 1, out var above)
                ? above.Y + VersionToVariant
                : TopY;
            var node = new VariantNode(
                index,
                variant.Label,
                variant.Hash,
                variant.Level,
                variant.Spine,
                variant.Join,
                x,
                y,
                variant.Creator,
                variant.Message,
                null,
                false
            );

            variantNodes.Add(node);

            if (variant.Spine)
            {
                spineByLevel[variant.Level] = node;
            }
        }

        if (editing is { } edit)
        {
            variantNodes.Add(
                new VariantNode(
                    variantNodes.Count,
                    "???",
                    string.Empty,
                    top,
                    false,
                    null,
                    variantX,
                    TopY,
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
            TopY + shift + (double)top * VersionStep + BottomMargin,
            top
        );
    }

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
        bool Wrong
    );
}

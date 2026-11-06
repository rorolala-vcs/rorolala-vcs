using System.Globalization;
using System.Text;

using Avalonia.Media;
using RolaSharp;

namespace RorolalaVCSPlugin;

/// <summary>The faces the graph's text is drawn in, at the size it is measured at.</summary>
internal enum GraphFont
{
    /// <summary>A variant's message, one line in its description card.</summary>
    Message,

    /// <summary>The name of the account that made a version.</summary>
    Creator,

    /// <summary>The side a pill's entry is on: <c>LOCAL</c>, a Vault's name.</summary>
    Tag,

    /// <summary>The name of a file an entry is at, or a badge's words.</summary>
    Name,

    /// <summary>A variant's hash in its pill.</summary>
    Hash,

    /// <summary>The big, faint version number behind a card's content.</summary>
    Big,
}

/// <summary>
/// Measures the graph's text in content units, so a card's pills are laid out once per reading.
/// </summary>
/// <remarks>
/// The measurement is kept apart from the drawing because it is what the layout needs: a pill's width
/// decides where the next one starts and how tall a lane has to be, and both are the same at every
/// zoom. What is measured at a zoom is the renderer's, which is why it has a measure of its own.
/// <para>
/// It is not thread-safe and is made per binding of the view: measuring hands a string to the toolkit,
/// which is the window's to do rather than a worker's.
/// </para>
/// </remarks>
internal sealed class GraphMeasure
{
    /// <summary>The face names and hashes are set in, matching the drawing's own.</summary>
    internal static readonly FontFamily Mono = new(
        "JetBrains Mono, Cascadia Mono, Consolas, DejaVu Sans Mono, Liberation Mono, monospace"
    );

    /// <summary>The face everything else is set in.</summary>
    private static readonly Typeface FaceNormal = new(FontFamily.Default);
    private static readonly Typeface FaceSemiBold = new(
        FontFamily.Default,
        FontStyle.Normal,
        FontWeight.SemiBold
    );
    private static readonly Typeface FaceExtraBold = new(
        FontFamily.Default,
        FontStyle.Normal,
        FontWeight.ExtraBold
    );
    private static readonly Typeface FaceMono = new(Mono);

    /// <summary>What has been measured, by the face and the text.</summary>
    private readonly Dictionary<(GraphFont Face, string Text), double> _widths = [];

    /// <summary>The typeface and size a face is drawn at, in content units.</summary>
    /// <param name="font">The face asked for.</param>
    /// <returns>The typeface, and the size it stands at before the zoom.</returns>
    internal static (Typeface Typeface, double Size) Face(GraphFont font) =>
        font switch
        {
            GraphFont.Message => (FaceNormal, 13),
            GraphFont.Creator => (FaceSemiBold, 10.5),
            GraphFont.Tag => (FaceSemiBold, 9),
            GraphFont.Name => (FaceNormal, 10.5),
            GraphFont.Hash => (FaceMono, 10),
            _ => (FaceExtraBold, 34),
        };

    /// <summary>How wide `text` is when set in a face, in content units.</summary>
    /// <param name="text">The text to measure.</param>
    /// <param name="font">The face it is set in.</param>
    /// <returns>Its width.</returns>
    internal double Width(string text, GraphFont font)
    {
        if (_widths.TryGetValue((font, text), out var known))
        {
            return known;
        }

        var (typeface, size) = Face(font);

        // Only the width is wanted, so the brush is the one every measurement is made with rather than
        // the colour the text is drawn in.
        var formatted = new FormattedText(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            typeface,
            size,
            Brushes.Transparent
        );

        _widths[(font, text)] = formatted.Width;

        return formatted.Width;
    }
}

/// <summary>What a card stands for.</summary>
internal enum GraphCardKind
{
    /// <summary>A variant the index holds: one that names a version, or one waiting to be recorded.</summary>
    Variant,

    /// <summary>
    /// An edit the working file holds that was never recorded, which is not a variant: it has no hash
    /// and no version number.
    /// </summary>
    Draft,
}

/// <summary>What one pill under a card says.</summary>
internal enum GraphChipKind
{
    /// <summary>The variant's own hash, with a mark when it merges one in.</summary>
    Hash,

    /// <summary>An entry pointing at the variant, with the side it is named on.</summary>
    Entry,

    /// <summary>Something about the card that is not an entry: a state, or that nothing points at it.</summary>
    Badge,

    /// <summary>The file a draft belongs to.</summary>
    File,
}

/// <summary>What two cards are joined by.</summary>
internal enum GraphLinkKind
{
    /// <summary>The chain: the older variant a variant was based on.</summary>
    Base,

    /// <summary>A recorded merge: the variant this one was recorded as merging in.</summary>
    Join,

    /// <summary>A merge in progress: the variant is checked in and waits to be joined into its file.</summary>
    Merging,

    /// <summary>A draft based on the version the file records, which a track would continue.</summary>
    Draft,

    /// <summary>A draft based on a version that does not match the file's record, which a track would fork.</summary>
    Wrong,

    /// <summary>Two variants whose content is the same.</summary>
    Same,
}

/// <summary>One pill under a card.</summary>
/// <param name="Kind">What the pill says.</param>
/// <param name="Text">What a hash, badge or file pill reads.</param>
/// <param name="Tag">The side of an entry or file pill, as its tag reads.</param>
/// <param name="Name">The file name an entry or file pill shows.</param>
/// <param name="Width">How wide it is, in content units.</param>
/// <param name="TagWidth">How wide its tag is, which is where its name starts.</param>
/// <param name="Entry">The entry it stands for, when it is an entry.</param>
/// <param name="Warn">Whether a badge is a warning.</param>
/// <param name="Wrong">Whether a badge is a wrong change.</param>
/// <param name="Strike">Whether the entry was moved under the deprecated marker.</param>
/// <param name="Chosen">Whether it names a file the user has chosen, which is drawn in the look's ink.</param>
/// <param name="Tracked">
/// Whether it is the entry of another side at a chosen file's path, which is the one that file tracks.
/// </param>
internal sealed record GraphChip(
    GraphChipKind Kind,
    string Text,
    string? Tag,
    string Name,
    double Width,
    double TagWidth,
    VariantGraphEntry? Entry,
    bool Warn,
    bool Wrong,
    bool Strike,
    bool Chosen,
    bool Tracked
);

/// <summary>What a draft card says about the edit it stands for.</summary>
/// <param name="Path">The layout path of the file being edited.</param>
/// <param name="Correct">Whether a track would continue the file's record rather than fork it.</param>
/// <param name="Unowned">Whether the change is another account's, which is not this one's to record.</param>
/// <param name="Badge">What its state pill says.</param>
/// <param name="BaseLabel">The version the file records, as its big label reads.</param>
/// <param name="ShouldLabel">The version the file should record, when the Vault has moved past it.</param>
internal sealed record DraftInfo(
    string Path,
    bool Correct,
    bool Unowned,
    string Badge,
    string BaseLabel,
    string? ShouldLabel
);

/// <summary>One card of the graph: a variant, or an edit that has no variant yet.</summary>
/// <param name="Index">Its place in the drawing's cards.</param>
/// <param name="Kind">What it stands for.</param>
/// <param name="Hash">The variant's key, empty for a draft.</param>
/// <param name="VersionHash">The version fixing it, or nothing when no version does.</param>
/// <param name="Number">Its number in its chain, or nothing when it has none.</param>
/// <param name="Label">What its big faint label reads.</param>
/// <param name="Creator">Who made it, as a name; <c>?</c> when no name is held.</param>
/// <param name="Message">What it says was done, in full; the card draws one truncated line of it.</param>
/// <param name="Storage">The content it was made of, as the store's key in hex.</param>
/// <param name="BaseVariant">The variant it is based on, when the index holds it.</param>
/// <param name="Join">The variant it merges in, when it is a merge.</param>
/// <param name="MissingBase">Whether its base version is named but not held.</param>
/// <param name="Reachable">Whether it stands on some entry's chain rather than on a history branch.</param>
/// <param name="Pending">Whether it is the variant a merge in progress is waiting to join into a file.</param>
/// <param name="PendingPath">The layout path of the file the merge waits on, when it names one.</param>
/// <param name="PendingFile">Where the variant file of the merge lies, when the reading names it.</param>
/// <param name="Draft">What a draft card says, when it is one.</param>
/// <param name="Lane">The file band it stands in.</param>
/// <param name="Column">The version column it stands in; 1 for one whose depth is unknown.</param>
/// <param name="X">The column of its description card's centre.</param>
/// <param name="Y">The row of its description card's centre.</param>
/// <param name="Chips">Its pills, in the order they are drawn.</param>
/// <param name="Rows">Its pills, wrapped into the rows they are drawn in.</param>
/// <param name="PillsHeight">How tall all its pill rows are.</param>
/// <param name="Entries">The entries that point at it.</param>
/// <param name="IncomingJoins">How many variants merge it in.</param>
/// <param name="CreatorWidth">How wide its creator's name is, in content units.</param>
/// <param name="LaneColour">The colour of the file whose lane it stands in.</param>
/// <param name="CreatorHash">The Creator object it names, as hex; empty for a draft.</param>
/// <param name="MessageHash">The Message object it names, as hex; empty for a draft.</param>
internal sealed record GraphCard(
    int Index,
    GraphCardKind Kind,
    string Hash,
    string? VersionHash,
    ulong? Number,
    string Label,
    string Creator,
    string Message,
    string Storage,
    string? BaseVariant,
    string? Join,
    bool MissingBase,
    bool Reachable,
    bool Pending,
    string? PendingPath,
    string? PendingFile,
    DraftInfo? Draft,
    int Lane,
    int Column,
    double X,
    double Y,
    IReadOnlyList<GraphChip> Chips,
    IReadOnlyList<IReadOnlyList<GraphChip>> Rows,
    double PillsHeight,
    IReadOnlyList<VariantGraphEntry> Entries,
    int IncomingJoins,
    double CreatorWidth,
    Color LaneColour,
    string CreatorHash,
    string MessageHash
)
{
    /// <summary>The left edge of its description card, in content units.</summary>
    internal double CardLeft => X - VariantGraphDrawing.CardWidth / 2;

    /// <summary>The top edge of its description card, in content units.</summary>
    internal double CardTop => Y - VariantGraphDrawing.CardHeight / 2;

    /// <summary>The right edge of its description card, in content units.</summary>
    internal double CardRight => X + VariantGraphDrawing.CardWidth / 2;

    /// <summary>The bottom edge of its pills, in content units.</summary>
    internal double ContentBottom =>
        Y + VariantGraphDrawing.CardHeight / 2 + VariantGraphDrawing.PillGap + PillsHeight;

    /// <summary>Whether a version fixes it, which is what a retrack may go to.</summary>
    internal bool HasVersion => VersionHash is { Length: > 0 };
}

/// <summary>One edge of the graph, between two cards.</summary>
/// <param name="Kind">What the edge means.</param>
/// <param name="From">The card the edge is walked from.</param>
/// <param name="To">The card it reaches.</param>
internal sealed record GraphLink(GraphLinkKind Kind, int From, int To);

/// <summary>A base version the index does not hold, drawn as a broken node beside the card naming it.</summary>
/// <param name="X">The column of its centre.</param>
/// <param name="Y">The row of its centre.</param>
/// <param name="Target">The card it is the missing base of.</param>
internal sealed record GraphGhost(double X, double Y, int Target);

/// <summary>A draft the view read from the work: a file changed but not tracked.</summary>
/// <param name="Path">The layout path of the file being edited.</param>
/// <param name="VersionHash">The version the file records, which the draft is based on.</param>
/// <param name="Wrong">Whether a track would fork rather than continue the file's record.</param>
/// <param name="Unowned">Whether the change is another account's.</param>
/// <param name="ShouldBeVersionHash">The version the file should record, when the Vault has moved past it.</param>
internal sealed record DraftFact(
    string Path,
    string VersionHash,
    bool Wrong,
    bool Unowned,
    string? ShouldBeVersionHash
);

/// <summary>The words the layout writes, taken from the UI's translations.</summary>
/// <param name="DraftLabel">What a draft's big faint label reads.</param>
/// <param name="DraftMessage">What a draft based on the record says.</param>
/// <param name="DraftWrongMessage">What a draft that would fork says.</param>
/// <param name="DraftUnownedMessage">What a draft another account holds says.</param>
/// <param name="DraftBadge">What a recordable draft's state pill says.</param>
/// <param name="DraftWrongBadge">What a forking draft's state pill says.</param>
/// <param name="DraftUnownedBadge">What an unowned draft's state pill says.</param>
/// <param name="PendingInto">The words a merge in progress puts in place of a message, with the file's name.</param>
/// <param name="NoEntries">What a variant no entry points at is marked with.</param>
/// <param name="UnknownBase">What a base version the index does not hold is labelled.</param>
internal sealed record GraphVocabulary(
    string DraftLabel,
    string DraftMessage,
    string DraftWrongMessage,
    string DraftUnownedMessage,
    string DraftBadge,
    string DraftWrongBadge,
    string DraftUnownedBadge,
    string PendingInto,
    string NoEntries,
    string UnknownBase
);

/// <summary>
/// The variant graph as geometry: a column per version depth, a lane per file, and a card per variant.
/// </summary>
/// <remarks>
/// The prototype this is drawn from put a file's versions on one horizontal line and a merge across
/// files on a curve below them. The reading the native side gives has no coordinates at all: it names
/// variants, the versions fixing them and the entries pointing at them, so the layout here is what turns
/// that into lanes and columns. Three facts are derived rather than read:
/// <list type="bullet">
/// <item>a variant's <em>depth</em> is its version number, the index numbers a chain from the root, and
/// a variant without a version sits one past the version it is based on;</item>
/// <item>a variant's <em>lane</em> is the file whose chain it stands on, which is found by walking every
/// entry's chain up and giving the ancestors its path;</item>
/// <item>a <em>draft</em> is not in the reading: the view tells the layout which local entries the work
/// says are changed, and the layout hangs one card off the version each is recorded at.</item>
/// </list>
/// </remarks>
internal sealed class VariantGraphDrawing
{
    /// <summary>The width of a description card, in content units.</summary>
    internal const double CardWidth = 248;

    /// <summary>The height of a description card, in content units.</summary>
    internal const double CardHeight = 44;

    /// <summary>The room between a description card and its first row of pills.</summary>
    internal const double PillGap = 6;

    /// <summary>The height of one pill.</summary>
    internal const double ChipHeight = 20;

    /// <summary>The room between two pills, and between two rows of them.</summary>
    internal const double ChipGap = 6;

    /// <summary>The room one column keeps from the next.</summary>
    internal const double ColumnPitch = 300;

    /// <summary>The room between two lanes' bands.</summary>
    internal const double LaneGap = 26;

    /// <summary>The room kept around one row of a lane, under its cards.</summary>
    internal const double RowGap = 16;

    /// <summary>The shortest a base or draft curve is allowed to become.</summary>
    internal const double CurveLeast = 42;

    /// <summary>The shortest a merge curve is allowed to become.</summary>
    internal const double MergeCurveLeast = 60;

    /// <summary>How far a variant's base may be looked for to find its lane by inheritance.</summary>
    private const int LaneInheritance = 8;

    /// <summary>How many characters of a hash a pill shows.</summary>
    private const int HashLength = 7;

    /// <summary>Makes a drawing from its pieces.</summary>
    /// <param name="cards">The cards, in column order.</param>
    /// <param name="byHash">The card each variant hash stands at.</param>
    /// <param name="links">The edges.</param>
    /// <param name="ghosts">The missing base versions.</param>
    /// <param name="focusChain">The cards the focused entries' own chains are.</param>
    /// <param name="focusRelated">The cards beside those: merges in, drafts and other sides.</param>
    /// <param name="truncated">Whether the window was cut short by its cap.</param>
    /// <param name="left">The leftmost content point.</param>
    /// <param name="right">The rightmost content point.</param>
    /// <param name="top">The highest content point.</param>
    /// <param name="bottom">The lowest content point.</param>
    /// <param name="variantCount">How many variants the cards hold.</param>
    /// <param name="entryCount">How many entries point at them.</param>
    /// <param name="joinCount">How many of them merge a variant in.</param>
    /// <param name="samePairCount">How many pairs of them were made of the same content.</param>
    /// <param name="pendingCount">How many merges in progress are drawn.</param>
    /// <param name="draftCount">How many drafts are drawn.</param>
    private VariantGraphDrawing(
        IReadOnlyList<GraphCard> cards,
        IReadOnlyDictionary<string, GraphCard> byHash,
        IReadOnlyList<GraphLink> links,
        IReadOnlyList<GraphGhost> ghosts,
        IReadOnlySet<int> focusChain,
        IReadOnlySet<int> focusRelated,
        bool truncated,
        double left,
        double right,
        double top,
        double bottom,
        int variantCount,
        int entryCount,
        int joinCount,
        int samePairCount,
        int pendingCount,
        int draftCount
    )
    {
        Cards = cards;
        ByHash = byHash;
        Links = links;
        Ghosts = ghosts;
        FocusChain = focusChain;
        FocusRelated = focusRelated;
        Truncated = truncated;
        Left = left;
        Right = right;
        Top = top;
        Bottom = bottom;
        VariantCount = variantCount;
        EntryCount = entryCount;
        JoinCount = joinCount;
        SamePairCount = samePairCount;
        PendingCount = pendingCount;
        DraftCount = draftCount;
    }

    /// <summary>The cards, in the order the reading named their variants, with the drafts after them.</summary>
    internal IReadOnlyList<GraphCard> Cards { get; }

    /// <summary>The card each variant hash stands at.</summary>
    internal IReadOnlyDictionary<string, GraphCard> ByHash { get; }

    /// <summary>The edges between the cards.</summary>
    internal IReadOnlyList<GraphLink> Links { get; }

    /// <summary>The base versions the index does not hold.</summary>
    internal IReadOnlyList<GraphGhost> Ghosts { get; }

    /// <summary>The cards on the focused entries' own chains.</summary>
    internal IReadOnlySet<int> FocusChain { get; }

    /// <summary>The cards beside those chains: the variants they merge in and the drafts of the same files.</summary>
    internal IReadOnlySet<int> FocusRelated { get; }

    /// <summary>Whether the window was cut short by its cap rather than walked out.</summary>
    internal bool Truncated { get; }

    /// <summary>The leftmost content point, pills and all.</summary>
    internal double Left { get; }

    /// <summary>The rightmost content point, pills and all.</summary>
    internal double Right { get; }

    /// <summary>The highest content point.</summary>
    internal double Top { get; }

    /// <summary>The lowest content point.</summary>
    internal double Bottom { get; }

    /// <summary>How many variants the cards hold, drafts left out.</summary>
    internal int VariantCount { get; }

    /// <summary>How many entries point at them.</summary>
    internal int EntryCount { get; }

    /// <summary>How many of them merge a variant in.</summary>
    internal int JoinCount { get; }

    /// <summary>How many pairs of them were made of the same content.</summary>
    internal int SamePairCount { get; }

    /// <summary>How many merges in progress are drawn.</summary>
    internal int PendingCount { get; }

    /// <summary>How many drafts are drawn.</summary>
    internal int DraftCount { get; }

    /// <summary>What a card's big faint label reads: its version number, or <c>#?</c>.</summary>
    /// <param name="card">The card asked about.</param>
    /// <returns>The label.</returns>
    internal static string VersionLabel(GraphCard card) => card.Kind == GraphCardKind.Draft
        ? card.Label
        : card.Number is { } number
            ? string.Create(CultureInfo.InvariantCulture, $"#{number}")
            : "#?";

    /// <summary>Lays the graph out from a reading of it, and from what the work says about its files.</summary>
    /// <param name="graph">The reading: the variants, versions, edges, entries, merges and texts.</param>
    /// <param name="drafts">The local entries whose files hold a change that was never recorded.</param>
    /// <param name="words">The words the layout writes, from the UI's translations.</param>
    /// <param name="measure">The measurer the pills are sized with.</param>
    /// <param name="emphasized">
    /// Whether the reading stands on entries the user chose, which is what makes its chains the ones to
    /// colour and the rest of the graph the context around them. A reading that stands on the whole work
    /// is no one's focus, so nothing is emphasised.
    /// </param>
    /// <returns>The drawing.</returns>
    internal static VariantGraphDrawing From(
        RolaVariantGraph graph,
        IReadOnlyList<DraftFact> drafts,
        GraphVocabulary words,
        GraphMeasure measure,
        bool emphasized
    )
    {
        ArgumentNullException.ThrowIfNull(graph);
        ArgumentNullException.ThrowIfNull(drafts);
        ArgumentNullException.ThrowIfNull(words);
        ArgumentNullException.ThrowIfNull(measure);

        var versions = new Dictionary<string, VariantGraphVersion>(StringComparer.Ordinal);
        var numbers = new Dictionary<string, ulong>(StringComparer.Ordinal);
        var variantOfVersion = new Dictionary<string, string>(StringComparer.Ordinal);
        var versionOfVariant = new Dictionary<string, string>(StringComparer.Ordinal);
        var roots = new HashSet<string>(StringComparer.Ordinal);

        foreach (var version in graph.Versions)
        {
            versions[version.Hash] = version;

            if (version.IsRoot)
            {
                roots.Add(version.Hash);

                continue;
            }

            variantOfVersion[version.Hash] = version.Variant;

            if (version.Number is { } number)
            {
                numbers[version.Variant] = number;
            }

            _ = versionOfVariant.TryAdd(version.Variant, version.Hash);
        }

        var entriesByVariant = new Dictionary<string, List<VariantGraphEntry>>(StringComparer.Ordinal);

        foreach (var entry in graph.Entries)
        {
            if (entry.Version is not { } recorded || !variantOfVersion.TryGetValue(recorded, out var variant))
            {
                continue;
            }

            if (!entriesByVariant.TryGetValue(variant, out var list))
            {
                list = [];
                entriesByVariant[variant] = list;
            }

            list.Add(entry);
        }

        var lanePaths = new List<string>();
        var laneOfPath = new Dictionary<string, int>(StringComparer.Ordinal);

        foreach (var entry in graph.Entries)
        {
            var path = Normalize(entry.Path);

            if (laneOfPath.TryAdd(path, lanePaths.Count))
            {
                lanePaths.Add(path);
            }
        }

        var extraLane = lanePaths.Count;
        var unknownLane = extraLane + 1;
        var cards = new List<Work>(graph.Nodes.Count + drafts.Count);
        var at = new Dictionary<string, int>(StringComparer.Ordinal);

        foreach (var node in graph.Nodes)
        {
            var missingBase =
                node.BaseVariant is null && !roots.Contains(node.BaseVersion);

            var creator = graph.Texts.Creators.TryGetValue(node.Creator, out var made)
                ? made
                : Shorten(node.Creator);

            var message = graph.Texts.Messages.TryGetValue(node.Message, out var said) ? said : string.Empty;

            at[node.Hash] = cards.Count;
            cards.Add(
                new Work
                {
                    Kind = GraphCardKind.Variant,
                    Hash = node.Hash,
                    VersionHash = versionOfVariant.GetValueOrDefault(node.Hash),
                    Number = numbers.TryGetValue(node.Hash, out var number) ? number : null,
                    Creator = creator,
                    Message = message,
                    Storage = node.Storage,
                    BaseVariant = node.BaseVariant,
                    BaseVersion = node.BaseVersion,
                    Join = node.Join,
                    MissingBase = missingBase,
                    CreatorHash = node.Creator,
                    MessageHash = node.Message,
                    Entries = entriesByVariant.GetValueOrDefault(node.Hash) ?? [],
                }
            );
        }

        var lanes = new HashSet<int>[cards.Count];

        for (var index = 0; index < cards.Count; index++)
        {
            lanes[index] = [];
        }

        foreach (var entry in graph.Entries)
        {
            if (
                entry.Version is not { } recorded
                || !variantOfVersion.TryGetValue(recorded, out var variant)
                || !at.TryGetValue(variant, out var start)
            )
            {
                continue;
            }

            var lane = laneOfPath[Normalize(entry.Path)];
            var seen = new HashSet<int>();
            var here = start;

            while (seen.Add(here))
            {
                lanes[here].Add(lane);
                cards[here].Reachable = true;

                if (
                    cards[here].BaseVariant is not { } baseVariant
                    || !at.TryGetValue(baseVariant, out here)
                )
                {
                    break;
                }
            }
        }

        var depths = new int?[cards.Count];
        var depthState = new bool[cards.Count];
        var lanesOf = new int?[cards.Count];
        var laneState = new bool[cards.Count];

        var pending = new Dictionary<string, VariantGraphMerging>(StringComparer.Ordinal);

        foreach (var merge in graph.Merging)
        {
            pending[merge.Variant] = merge;
        }

        for (var index = 0; index < cards.Count; index++)
        {
            var card = cards[index];

            if (!pending.TryGetValue(card.Hash, out var merge))
            {
                continue;
            }

            card.Pending = true;
            card.PendingPath = merge.Path;
            card.PendingFile = merge.File;
            card.Message = PendingMessage(merge, card, words);
        }

        var draftCards = new List<Work>(drafts.Count);

        foreach (var draft in drafts)
        {
            if (
                !variantOfVersion.TryGetValue(draft.VersionHash, out var variant)
                || !at.TryGetValue(variant, out var baseIndex)
                || !cards[baseIndex].HasVersion
            )
            {
                continue;
            }

            var shouldLabel = ShouldLabel(draft.ShouldBeVersionHash, variantOfVersion, numbers);
            var baseLabel = VersionLabelOf(cards[baseIndex]);

            var badge = draft.Unowned
                ? words.DraftUnownedBadge
                : draft.Wrong
                    ? words.DraftWrongBadge
                    : words.DraftBadge;

            var message = draft.Unowned
                ? words.DraftUnownedMessage
                : draft.Wrong
                    ? words.DraftWrongMessage
                    : words.DraftMessage;

            draftCards.Add(
                new Work
                {
                    Kind = GraphCardKind.Draft,
                    Hash = string.Empty,
                    VersionHash = null,
                    Number = null,
                    Label = words.DraftLabel,
                    Creator = "?",
                    Message = message,
                    Storage = string.Empty,
                    BaseVariant = cards[baseIndex].Hash,
                    MissingBase = false,
                    Draft = new DraftInfo(
                        draft.Path,
                        !draft.Wrong,
                        draft.Unowned,
                        badge,
                        baseLabel,
                        shouldLabel
                    ),
                    Entries = [],
                    Pending = false,
                    Reachable = true,
                    BaseIndex = baseIndex,
                }
            );
        }

        foreach (var draft in draftCards)
        {
            cards.Add(draft);
        }

        // The cards a file names are all in place before anything is laid out, since a draft's lane and
        // column are the ones of the version it hangs off.
        depths = new int?[cards.Count];
        depthState = new bool[cards.Count];
        lanesOf = new int?[cards.Count];
        laneState = new bool[cards.Count];

        for (var index = 0; index < cards.Count; index++)
        {
            _ = Depth(index);
            _ = Lane(index, LaneInheritance);
        }

        for (var index = 0; index < cards.Count; index++)
        {
            var lane = lanesOf[index] ?? extraLane;

            cards[index].LaneColour = lane < lanePaths.Count
                ? GraphHue.For(lanePaths[lane], 0.62, 0.55)
                : Color.FromRgb(0x94, 0xA3, 0xB8);
        }

        // A draft stands one column past the version it hangs off, which is the version its track
        // would continue: where the Vault's copy has moved on is about the record the file keeps, not
        // about which column its unrecorded change stands in.
        foreach (var card in cards)
        {
            if (card.Kind != GraphCardKind.Draft || card.BaseIndex is not { } baseIndex)
            {
                continue;
            }

            card.ColumnOverride = (depths[baseIndex] ?? 0) + 1;
        }

        foreach (var card in cards)
        {
            if (card.Kind == GraphCardKind.Draft)
            {
                continue;
            }

            card.Label = VersionLabelOf(card);
        }

        // Every card's pills are measured here rather than at draw time: a pill's width is what the next
        // one is placed after and how tall its lane has to be, and both are the same at every zoom.
        var chosen = emphasized
            ? graph.Focus.Select(focus => (focus.Side, Path: Normalize(focus.Path))).ToHashSet()
            : [];

        // A chosen file records its path on the work's side, and the entry at that path on another side
        // is the one the file tracks: that entry's Uuid is the record the track names, so it is the pill
        // the reader has to be able to find. The path is the relation the reading carries; the versions
        // may differ while the file is behind or ahead of the Vault.
        var tracked = emphasized
            ? graph
                .Entries.Where(entry =>
                    entry.Side != "local"
                    && chosen.Contains(("local", Normalize(entry.Path)))
                )
                .Select(entry => (entry.Side, Path: Normalize(entry.Path)))
                .ToHashSet()
            : [];

        // The local entry a draft is based on is the same file as the draft: it has moved to the card
        // that stands for the edit, and leaving its pill on the baseline card would draw one file twice.
        var drafted = draftCards
            .Where(card => card.Draft is not null)
            .Select(card => Normalize(card.Draft!.Path))
            .ToHashSet(StringComparer.Ordinal);

        foreach (var card in cards)
        {
            var chips = ChipsOf(card, words, measure, chosen, tracked, drafted);
            card.Rows = RowsOf(chips);
            card.PillsHeight = PillsHeightOf(card.Rows);
            card.CreatorWidth = measure.Width(card.Creator, GraphFont.Creator);
        }

        Place(cards, depths, lanesOf, extraLane);

        var byHash = new Dictionary<string, GraphCard>(StringComparer.Ordinal);
        var frozen = new List<GraphCard>(cards.Count);
        var incoming = new int[cards.Count];

        // The incoming merges are counted before the cards are frozen, since an edge names both ends.
        foreach (var card in cards)
        {
            if (
                card.Kind == GraphCardKind.Variant
                && card.Join is { } join
                && at.TryGetValue(join, out var source)
            )
            {
                incoming[source] += 1;
            }
        }

        foreach (var card in cards)
        {
            card.IncomingJoins = incoming[card.Index];

            var frozenCard = card.Freeze();
            frozen.Add(frozenCard);

            if (card.Kind == GraphCardKind.Variant)
            {
                byHash[card.Hash] = frozenCard;
            }
        }

        var links = LinksOf(cards, at);
        var written = new List<GraphCard>(frozen);
        var ghosts = new List<GraphGhost>();

        foreach (var card in frozen)
        {
            if (!card.MissingBase)
            {
                continue;
            }

            ghosts.Add(
                new GraphGhost(
                    (card.Column - 1) * ColumnPitch,
                    card.Y,
                    card.Index
                )
            );
        }

        var chain = new HashSet<int>();
        var related = new HashSet<int>();

        if (emphasized)
        {
            foreach (var focus in graph.Focus)
            {
                if (focus.Variant is not { } variant || !at.TryGetValue(variant, out var start))
                {
                    continue;
                }

                var seen = new HashSet<int>();
                var here = start;

                while (seen.Add(here))
                {
                    chain.Add(here);

                    if (
                        written[here].BaseVariant is not { } baseVariant
                        || !at.TryGetValue(baseVariant, out here)
                    )
                    {
                        break;
                    }
                }

                foreach (var entry in graph.Entries)
                {
                    if (!string.Equals(Normalize(entry.Path), Normalize(focus.Path), StringComparison.Ordinal))
                    {
                        continue;
                    }

                    if (
                        entry.Version is not { } recorded
                        || !variantOfVersion.TryGetValue(recorded, out var other)
                        || !at.TryGetValue(other, out var beside)
                    )
                    {
                        continue;
                    }

                    var seenBeside = new HashSet<int>();
                    var walk = beside;

                    while (seenBeside.Add(walk))
                    {
                        related.Add(walk);

                        if (
                            written[walk].BaseVariant is not { } baseVariant
                            || !at.TryGetValue(baseVariant, out walk)
                        )
                        {
                            break;
                        }
                    }
                }
            }

            foreach (var index in chain.Concat(related).ToArray())
            {
                if (written[index].Join is { } join && at.TryGetValue(join, out var source))
                {
                    related.Add(source);
                }
            }

            foreach (var card in written)
            {
                if (card.Kind == GraphCardKind.Draft && card.Draft is { } draft)
                {
                    foreach (var focus in graph.Focus)
                    {
                        if (string.Equals(Normalize(focus.Path), draft.Path, StringComparison.Ordinal))
                        {
                            related.Add(card.Index);
                        }
                    }
                }
            }
        }

        var samePairs = links.Count(link => link.Kind == GraphLinkKind.Same);

        var left = written.Count == 0 ? 0 : written.Min(card => card.CardLeft);
        var right = written.Count == 0 ? 0 : written.Max(card => card.CardRight);
        var top = written.Count == 0 ? 0 : written.Min(card => card.CardTop);
        var bottom = written.Count == 0 ? 0 : written.Max(card => card.ContentBottom);

        foreach (var ghost in ghosts)
        {
            left = Math.Min(left, ghost.X - 40);
            right = Math.Max(right, ghost.X + 40);
            top = Math.Min(top, ghost.Y - 40);
            bottom = Math.Max(bottom, ghost.Y + 40);
        }

        return new VariantGraphDrawing(
            frozen,
            byHash,
            links,
            ghosts,
            chain,
            related,
            graph.Truncated,
            left - 8,
            right + 8,
            top - 18,
            bottom + 8,
            written.Count(card => card.Kind == GraphCardKind.Variant),
            graph.Entries.Count,
            written.Count(card => card.Kind == GraphCardKind.Variant && card.Join is not null),
            samePairs,
            written.Count(card => card.Pending),
            written.Count(card => card.Kind == GraphCardKind.Draft)
        );

        // The depth of one card: its version number when it has one, otherwise one past the card it is
        // based on. A base version naming a variant the window does not hold is not a depth, which is
        // what leaves the card with no column.
        int? Depth(int index)
        {
            if (depthState[index])
            {
                return depths[index];
            }

            depthState[index] = true;
            var card = cards[index];

            if (card.Number is { } number)
            {
                depths[index] = (int)number;

                return depths[index];
            }

            if (card.BaseVariant is not { } baseVariant)
            {
                depths[index] = roots.Contains(card.BaseVersion ?? string.Empty) ? 0 : null;

                return depths[index];
            }

            if (at.TryGetValue(baseVariant, out var parent) && Depth(parent) is { } parentDepth)
            {
                depths[index] = parentDepth + 1;

                return depths[index];
            }

            depths[index] =
                card.BaseVersion is { } baseVersion
                && versions.TryGetValue(baseVersion, out var version)
                && version.Number is { } baseNumber
                    ? (int)baseNumber + 1
                    : null;

            return depths[index];
        }

        // The lane of one card: the first file whose chain it stands on, or the lane of the card it is
        // based on when no entry names its own file, or a lane of its own when neither says anything.
        int? Lane(int index, int depthLeft)
        {
            if (lanesOf[index] is { } known)
            {
                return known;
            }

            if (laneState[index] || depthLeft <= 0)
            {
                return lanesOf[index];
            }

            laneState[index] = true;
            var card = cards[index];

            if (card.Kind == GraphCardKind.Draft)
            {
                lanesOf[index] = card.BaseIndex is { } baseIndex ? Lane(baseIndex, depthLeft - 1) : null;

                return lanesOf[index];
            }

            if (Depth(index) is null)
            {
                lanesOf[index] = unknownLane;

                return unknownLane;
            }

            var lane = lanes[index].Count == 0 ? (int?)null : lanes[index].Min();

            if (
                lane is null
                && card.BaseVariant is { } baseVariant
                && at.TryGetValue(baseVariant, out var parent)
            )
            {
                lane = Lane(parent, depthLeft - 1);
            }

            lanesOf[index] = lane ?? extraLane;

            return lanesOf[index];
        }
    }

    /// <summary>What the graph is normalised to: a layout path without the deprecated marker.</summary>
    /// <param name="path">The path as the Layout names it.</param>
    /// <returns>The path the file is known by.</returns>
    internal static string Normalize(string path) =>
        path.StartsWith("@/removed/", StringComparison.Ordinal) ? path["@/removed/".Length..] : path;

    /// <summary>The file name part of a layout path.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The name it ends with.</returns>
    internal static string NameOf(string path) =>
        path[(path.LastIndexOf('/') + 1)..];

    /// <summary>How a card's number is written in a sentence.</summary>
    /// <param name="card">The card.</param>
    /// <returns>The number, or <c>#?</c>.</returns>
    private static string VersionLabelOf(Work card) => card.Number is { } number
        ? string.Create(CultureInfo.InvariantCulture, $"#{number}")
        : "#?";

    /// <summary>What a card's incoming merge says in place of its message.</summary>
    /// <param name="merge">The merge in progress.</param>
    /// <param name="card">The card it stands at.</param>
    /// <param name="words">The words the layout writes.</param>
    /// <returns>The message the card draws.</returns>
    private static string PendingMessage(VariantGraphMerging merge, Work card, GraphVocabulary words)
    {
        var path = merge.Path.Length > 0 ? merge.Path : merge.File;
        var name = path.Length > 0 ? NameOf(path) : Shorten(card.Hash);

        // The words are the UI's form, whose place is written `%{file}` rather than in the format
        // string's own syntax: what fills it is this layout's, so the form is filled here.
        return Fill(words.PendingInto, name);
    }

    /// <summary>Fills the first place a form leaves with a value.</summary>
    /// <param name="form">The form, whose place is written <c>%{name}</c>.</param>
    /// <param name="value">The value to put there.</param>
    /// <returns>The form with the value in it, or the form itself when it leaves no place.</returns>
    private static string Fill(string form, string value)
    {
        var open = form.IndexOf("%{", StringComparison.Ordinal);

        if (open < 0)
        {
            return form;
        }

        var close = form.IndexOf('}', open + 2);

        return close < 0 ? form : string.Concat(form[..open], value, form[(close + 1)..]);
    }

    /// <summary>A key as a pill or a sentence names it when it has no words of its own.</summary>
    /// <param name="hash">The key, in hex.</param>
    /// <returns>The first characters of it, marked as a hash.</returns>
    private static string Shorten(string hash) => HashText(hash);

    /// <summary>A key's first characters, marked as one.</summary>
    /// <param name="hash">The key, in hex.</param>
    /// <returns>The mark and the characters.</returns>
    private static string HashText(string hash) =>
        hash.Length == 0 ? "#?" : "#" + hash[..Math.Min(HashLength, hash.Length)];

    /// <summary>The version label a draft should be based on, when the reading names one.</summary>
    /// <param name="versionHash">The version the draft should be based on, when it is known.</param>
    /// <param name="variantOfVersion">The variant each version fixes.</param>
    /// <param name="numbers">The number each variant has.</param>
    /// <returns>The label, or nothing.</returns>
    private static string? ShouldLabel(
        string? versionHash,
        IReadOnlyDictionary<string, string> variantOfVersion,
        IReadOnlyDictionary<string, ulong> numbers
    )
    {
        if (versionHash is null || !variantOfVersion.TryGetValue(versionHash, out var variant))
        {
            return null;
        }

        return numbers.TryGetValue(variant, out var number)
            ? string.Create(CultureInfo.InvariantCulture, $"#{number}")
            : null;
    }

    /// <summary>The pills one card shows, in the order they are drawn.</summary>
    /// <param name="card">The card.</param>
    /// <param name="words">The words the layout writes.</param>
    /// <param name="measure">The measurer.</param>
    /// <param name="chosen">The side and path of every file the user has chosen.</param>
    /// <param name="tracked">The other sides' entries at those files' paths, which those files track.</param>
    /// <param name="drafted">The local paths a draft card stands on, whose entry pill the draft owns.</param>
    /// <returns>The pills.</returns>
    private static List<GraphChip> ChipsOf(
        Work card,
        GraphVocabulary words,
        GraphMeasure measure,
        IReadOnlySet<(string Side, string Path)> chosen,
        IReadOnlySet<(string Side, string Path)> tracked,
        IReadOnlySet<string> drafted
    )
    {
        var chips = new List<GraphChip>(card.Entries.Count + 2);

        if (card.Kind == GraphCardKind.Draft && card.Draft is { } draft)
        {
            chips.Add(
                new GraphChip(
                    GraphChipKind.Badge,
                    draft.Badge,
                    null,
                    string.Empty,
                    measure.Width(draft.Badge, GraphFont.Name) + 18,
                    0,
                    null,
                    false,
                    !draft.Correct,
                    false,
                    chosen.Contains(("local", Normalize(draft.Path))),
                    false
                )
            );

            var name = NameOf(draft.Path);
            var tagWidth = measure.Width("LOCAL", GraphFont.Tag);
            var nameWidth = measure.Width(name, GraphFont.Name);

            chips.Add(
                new GraphChip(
                    GraphChipKind.File,
                    string.Empty,
                    "LOCAL",
                    name,
                    8 + tagWidth + 7 + nameWidth + 9,
                    tagWidth,
                    null,
                    false,
                    false,
                    false,
                    chosen.Contains(("local", Normalize(draft.Path))),
                    false
                )
            );

            return chips;
        }

        var hash = (card.Join is null ? string.Empty : "⑃ ") + HashText(card.Hash);

        chips.Add(
            new GraphChip(
                GraphChipKind.Hash,
                hash,
                null,
                string.Empty,
                measure.Width(hash, GraphFont.Hash) + 16,
                0,
                null,
                false,
                false,
                false,
                false,
                false
            )
        );

        foreach (var entry in card.Entries)
        {
            if (entry.Side == "local" && drafted.Contains(Normalize(entry.Path)))
            {
                continue;
            }

            var tag = entry.Side == "local" ? "LOCAL" : entry.Side.ToUpperInvariant();
            var name = entry.Path.Length == 0 ? "?" : NameOf(entry.Path);
            var tagWidth = measure.Width(tag, GraphFont.Tag);
            var nameWidth = measure.Width(name, GraphFont.Name);

            chips.Add(
                new GraphChip(
                    GraphChipKind.Entry,
                    string.Empty,
                    tag,
                    name,
                    8 + tagWidth + 7 + nameWidth + 9,
                    tagWidth,
                    entry,
                    false,
                    false,
                    entry.Deprecated,
                    chosen.Contains((entry.Side, Normalize(entry.Path))),
                    tracked.Contains((entry.Side, Normalize(entry.Path)))
                )
            );
        }

        if (!card.Reachable && !card.Pending)
        {
            var text = words.NoEntries;

            chips.Add(
                new GraphChip(
                    GraphChipKind.Badge,
                    text,
                    null,
                    string.Empty,
                    measure.Width(text, GraphFont.Name) + 18,
                    0,
                    null,
                    false,
                    false,
                    false,
                    false,
                    false
                )
            );
        }

        return chips;
    }

    /// <summary>Wraps a card's pills into the rows they are drawn in.</summary>
    /// <param name="chips">The pills, in order.</param>
    /// <returns>The rows.</returns>
    private static List<IReadOnlyList<GraphChip>> RowsOf(IReadOnlyList<GraphChip> chips)
    {
        var rows = new List<IReadOnlyList<GraphChip>>();
        var row = new List<GraphChip>();
        var width = 0.0;

        foreach (var chip in chips)
        {
            var gap = row.Count == 0 ? 0 : ChipGap;

            if (row.Count > 0 && width + gap + chip.Width > CardWidth)
            {
                rows.Add(row);
                row = [];
                width = 0;
                gap = 0;
            }

            width += gap + chip.Width;
            row.Add(chip);
        }

        if (row.Count > 0)
        {
            rows.Add(row);
        }

        return rows;
    }

    /// <summary>How tall a card's rows of pills are.</summary>
    /// <param name="rows">The rows.</param>
    /// <returns>The height, in content units.</returns>
    private static double PillsHeightOf(IReadOnlyList<IReadOnlyList<GraphChip>> rows) =>
        (rows.Count * ChipHeight) + (Math.Max(0, rows.Count - 1) * ChipGap);

    /// <summary>Places every card at the column and row its version and file say.</summary>
    /// <param name="cards">The cards, with their pills measured.</param>
    /// <param name="depths">The depth of each card, for those that have one.</param>
    /// <param name="lanes">The lane of each card, for those that have one.</param>
    /// <param name="extraLane">The lane of the variants no entry names.</param>
    private static void Place(
        List<Work> cards,
        int?[] depths,
        int?[] lanes,
        int extraLane
    )
    {
        var byLane = new Dictionary<int, List<Work>>();
        var lanesUsed = new List<int>();

        for (var index = 0; index < cards.Count; index++)
        {
            var card = cards[index];
            card.Index = index;
            card.Column = card.ColumnOverride ?? depths[index] ?? 1;
            card.X = card.Column * ColumnPitch;
            card.Lane = lanes[index] ?? extraLane;

            if (!byLane.TryGetValue(card.Lane, out var list))
            {
                list = [];
                byLane[card.Lane] = list;
                lanesUsed.Add(card.Lane);
            }

            list.Add(card);
        }

        var bands = new Dictionary<int, (int Rows, double Pitch, double Span, double Center)>();

        foreach (var lane in lanesUsed)
        {
            var list = byLane[lane];
            var counts = new Dictionary<int, int>();
            var mostPills = 0.0;

            foreach (var card in list)
            {
                counts[card.Column] = counts.GetValueOrDefault(card.Column) + 1;
                mostPills = Math.Max(mostPills, card.PillsHeight);
            }

            var rows = Math.Max(1, counts.Values.Max());
            var pitch = CardHeight + PillGap + mostPills + RowGap;
            var span = (rows * pitch) - RowGap;

            bands[lane] = (rows, pitch, span, 0);
        }

        var ordered = lanesUsed.Order().ToList();
        var total = ordered.Sum(lane => bands[lane].Span) + (LaneGap * Math.Max(0, ordered.Count - 1));
        var cursor = -total / 2;

        foreach (var lane in ordered)
        {
            var band = bands[lane];
            bands[lane] = (band.Rows, band.Pitch, band.Span, cursor + (band.Span / 2));
            cursor += band.Span + LaneGap;
        }

        foreach (var card in cards)
        {
            card.Y = bands[card.Lane].Center;
        }

        var groups = new Dictionary<(int Lane, int Column), List<Work>>();

        foreach (var card in cards)
        {
            var key = (card.Lane, card.Column);

            if (!groups.TryGetValue(key, out var list))
            {
                list = [];
                groups[key] = list;
            }

            list.Add(card);
        }

        foreach (var group in groups.Values)
        {
            if (group.Count < 2)
            {
                continue;
            }

            var band = bands[group[0].Lane];
            group.Sort(
                (left, right) =>
                {
                    var byNumber = (left.Number ?? ulong.MaxValue).CompareTo(right.Number ?? ulong.MaxValue);

                    return byNumber != 0
                        ? byNumber
                        : string.CompareOrdinal(left.Hash, right.Hash);
                }
            );

            for (var index = 0; index < group.Count; index++)
            {
                group[index].Y = band.Center + ((index - ((group.Count - 1) / 2.0)) * band.Pitch);
            }
        }
    }

    /// <summary>The edges of the drawing, by the cards they join.</summary>
    /// <param name="cards">The cards.</param>
    /// <param name="at">Where each variant hash stands.</param>
    /// <returns>The edges.</returns>
    private static List<GraphLink> LinksOf(
        List<Work> cards,
        IReadOnlyDictionary<string, int> at
    )
    {
        var links = new List<GraphLink>();

        // Same content is drawn first, under everything else, because it is the least of the relations:
        // two files that happen to hold the same bytes are not the same chain.
        foreach (var group in cards.Where(card => card.Kind == GraphCardKind.Variant).GroupBy(card => card.Storage))
        {
            var list = group.Where(card => card.Storage.Length > 0).OrderBy(card => card.Column).ToList();

            if (list.Count < 2)
            {
                continue;
            }

            for (var index = 1; index < list.Count; index++)
            {
                links.Add(new GraphLink(GraphLinkKind.Same, list[index - 1].Index, list[index].Index));
            }
        }

        foreach (var card in cards)
        {
            if (card.Kind == GraphCardKind.Draft && card.BaseIndex is { } draftBase)
            {
                links.Add(
                    new GraphLink(
                        card.Draft is { Correct: true } ? GraphLinkKind.Draft : GraphLinkKind.Wrong,
                        draftBase,
                        card.Index
                    )
                );

                continue;
            }

            if (
                card.Kind == GraphCardKind.Variant
                && card.BaseVariant is { } baseVariant
                && at.TryGetValue(baseVariant, out var parent)
            )
            {
                links.Add(new GraphLink(GraphLinkKind.Base, parent, card.Index));
            }
        }

        foreach (var card in cards)
        {
            if (card.Kind != GraphCardKind.Variant || card.Join is not { } join || !at.TryGetValue(join, out var source))
            {
                continue;
            }

            links.Add(
                new GraphLink(
                    card.Pending ? GraphLinkKind.Merging : GraphLinkKind.Join,
                    source,
                    card.Index
                )
            );
        }

        return links;
    }

    /// <summary>A card while it is being placed, before its pills and edges are frozen into records.</summary>
    private sealed class Work
    {
        /// <summary>Its place among the cards.</summary>
        public int Index { get; set; }

        /// <summary>What it stands for.</summary>
        public GraphCardKind Kind { get; init; }

        /// <summary>The variant's key, empty for a draft.</summary>
        public string Hash { get; init; } = string.Empty;

        /// <summary>The version fixing it, when one does.</summary>
        public string? VersionHash { get; init; }

        /// <summary>Its number in its chain, when it has one.</summary>
        public ulong? Number { get; init; }

        /// <summary>What its big faint label reads.</summary>
        public string Label { get; set; } = "#?";

        /// <summary>Who made it, as a name.</summary>
        public string Creator { get; init; } = "?";

        /// <summary>What it says was done.</summary>
        public string Message { get; set; } = string.Empty;

        /// <summary>The Creator object it names, as hex.</summary>
        public string CreatorHash { get; init; } = string.Empty;

        /// <summary>The Message object it names, as hex.</summary>
        public string MessageHash { get; init; } = string.Empty;

        /// <summary>The content it was made of.</summary>
        public string Storage { get; init; } = string.Empty;

        /// <summary>The variant it is based on, when the index holds it.</summary>
        public string? BaseVariant { get; init; }

        /// <summary>The version it is based on, for a card no variant names.</summary>
        public string? BaseVersion { get; init; }

        /// <summary>The card it is based on, for a draft.</summary>
        public int? BaseIndex { get; init; }

        /// <summary>The variant it merges in, when it is a merge.</summary>
        public string? Join { get; init; }

        /// <summary>Whether its base version is named but not held.</summary>
        public bool MissingBase { get; init; }

        /// <summary>Whether it stands on an entry's chain.</summary>
        public bool Reachable { get; set; }

        /// <summary>Whether a merge in progress waits to join it into a file.</summary>
        public bool Pending { get; set; }

        /// <summary>The file the merge waits on.</summary>
        public string? PendingPath { get; set; }

        /// <summary>Where the merge's variant file lies.</summary>
        public string? PendingFile { get; set; }

        /// <summary>What a draft card says, when it is one.</summary>
        public DraftInfo? Draft { get; init; }

        /// <summary>The entries pointing at it.</summary>
        public IReadOnlyList<VariantGraphEntry> Entries { get; init; } = [];

        /// <summary>Its lane.</summary>
        public int Lane { get; set; }

        /// <summary>Its column.</summary>
        public int Column { get; set; }

        /// <summary>The column it stands in when its depth does not say it, which is a draft's.</summary>
        public int? ColumnOverride { get; set; }

        /// <summary>The column of its card's centre.</summary>
        public double X { get; set; }

        /// <summary>The row of its card's centre.</summary>
        public double Y { get; set; }

        /// <summary>How wide its creator's name is.</summary>
        public double CreatorWidth { get; set; }

        /// <summary>The colour of the file whose lane it stands in.</summary>
        public Color LaneColour { get; set; } = Colors.Gray;

        /// <summary>Its pills.</summary>
        public IReadOnlyList<IReadOnlyList<GraphChip>> Rows { get; set; } = [];

        /// <summary>How tall its rows of pills are.</summary>
        public double PillsHeight { get; set; }

        /// <summary>How many variants merge it in.</summary>
        public int IncomingJoins { get; set; }

        /// <summary>Whether a version fixes it.</summary>
        public bool HasVersion => VersionHash is { Length: > 0 };

        /// <summary>Makes the record the drawing hands out.</summary>
        /// <returns>The card.</returns>
        public GraphCard Freeze()
        {
            IReadOnlyList<GraphChip> chips = Rows.SelectMany(row => row).ToArray();

            return new GraphCard(
                Index,
                Kind,
                Hash,
                VersionHash,
                Number,
                Label,
                Creator,
                Message,
                Storage,
                BaseVariant,
                Join,
                MissingBase,
                Reachable,
                Pending,
                PendingPath,
                PendingFile,
                Draft,
                Lane,
                Column,
                X,
                Y,
                chips,
                Rows,
                PillsHeight,
                Entries,
                IncomingJoins,
                CreatorWidth,
                LaneColour,
                CreatorHash,
                MessageHash
            );
        }
    }
}

/// <summary>Gives a name a colour, which is what a creator's name and a file's lane are drawn with.</summary>
/// <remarks>
/// The same rule the launcher's workspace badges use: the name's bytes are hashed, the hash's three
/// low bytes are read as a colour, and that colour's hue is kept with one fixed saturation and
/// lightness. A name that never appeared before therefore has a colour at once, and the same name has
/// the same colour everywhere, with no table to keep in step.
/// </remarks>
internal static class GraphHue
{
    /// <summary>The colour a name is drawn with.</summary>
    /// <param name="text">The name.</param>
    /// <param name="saturation">How saturated the colour is, from 0 to 1.</param>
    /// <param name="lightness">How light it is, from 0 to 1.</param>
    /// <returns>The colour.</returns>
    internal static Color For(string text, double saturation, double lightness) =>
        FromHsl(HueOf(text), saturation, lightness);

    /// <summary>The hue a name hashes to.</summary>
    /// <param name="text">The name.</param>
    /// <returns>The hue, in degrees.</returns>
    private static double HueOf(string text)
    {
        var hash = 2_166_136_261u;

        foreach (var octet in Encoding.UTF8.GetBytes(text))
        {
            hash ^= octet;
            hash *= 16_777_619u;
        }

        var red = ((hash >> 16) & 0xFF) / 255.0;
        var green = ((hash >> 8) & 0xFF) / 255.0;
        var blue = (hash & 0xFF) / 255.0;
        var most = Math.Max(red, Math.Max(green, blue));
        var least = Math.Min(red, Math.Min(green, blue));
        var span = most - least;

        if (span == 0)
        {
            return 0;
        }

        var hue = most == red
            ? 60 * (((green - blue) / span) % 6)
            : most == green
                ? 60 * (((blue - red) / span) + 2)
                : 60 * (((red - green) / span) + 4);

        return hue < 0 ? hue + 360 : hue;
    }

    /// <summary>A colour from a hue, a saturation and a lightness.</summary>
    /// <param name="hue">The hue, in degrees.</param>
    /// <param name="saturation">How saturated it is, from 0 to 1.</param>
    /// <param name="lightness">How light it is, from 0 to 1.</param>
    /// <returns>The colour, opaque.</returns>
    private static Color FromHsl(double hue, double saturation, double lightness)
    {
        var chroma = (1 - Math.Abs((2 * lightness) - 1)) * saturation;
        var part = hue / 60;
        var second = chroma * (1 - Math.Abs((part % 2) - 1));
        var (red, green, blue) = part switch
        {
            < 1 => (chroma, second, 0.0),
            < 2 => (second, chroma, 0.0),
            < 3 => (0.0, chroma, second),
            < 4 => (0.0, second, chroma),
            < 5 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        var lift = lightness - (chroma / 2);

        return Color.FromRgb(
            (byte)Math.Round((red + lift) * 255),
            (byte)Math.Round((green + lift) * 255),
            (byte)Math.Round((blue + lift) * 255)
        );
    }
}

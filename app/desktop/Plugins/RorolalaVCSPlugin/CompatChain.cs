namespace RorolalaVCSPlugin;

/// <summary>What one row of the compat list stands for.</summary>
internal enum CompatRowKind
{
    /// <summary>A change the working file holds that was never recorded.</summary>
    Draft,

    /// <summary>The variant a merge in progress is waiting to join into the file.</summary>
    Pending,

    /// <summary>A version on the line the list follows.</summary>
    Spine,

    /// <summary>A version on a chain that left the line and comes back into it.</summary>
    Branch,
}

/// <summary>The ink a piece of the rail is drawn in.</summary>
internal enum CompatRailKind
{
    /// <summary>A settled version.</summary>
    Primary,

    /// <summary>An edit or a merge in progress.</summary>
    Modify,

    /// <summary>An edit that must not be recorded.</summary>
    Error,

    /// <summary>History older than the version the two sides parted at, which recedes rather than leads.</summary>
    Behind,
}

/// <summary>One lane's line through one row.</summary>
/// <param name="Lane">The lane it stands on; zero is the line itself.</param>
/// <param name="Kind">The ink it is drawn in.</param>
/// <param name="Dashed">Whether it is drawn broken.</param>
/// <param name="Node">Whether this row's node is on it.</param>
/// <param name="Above">Whether it runs from the row's top edge to the node.</param>
/// <param name="Below">Whether it runs from the node to the row's bottom edge.</param>
/// <param name="Open">Whether it leaves the line's node for this lane below.</param>
/// <param name="Close">Whether it comes from this lane above into the line's node.</param>
/// <param name="BelowKind">The ink its lower half is drawn in, when that is not its own.</param>
/// <param name="CloseLane">The lane the closing curve comes into, which is the node it joins.</param>
internal sealed record CompatLaneLine(
    int Lane,
    CompatRailKind Kind,
    bool Dashed,
    bool Node,
    bool Above,
    bool Below,
    bool Open,
    bool Close,
    CompatRailKind? BelowKind = null,
    int CloseLane = 0
);

/// <summary>One version of the chosen file's chain, as the compat list draws it.</summary>
/// <param name="Card">The card the row stands for.</param>
/// <param name="Kind">What the row is.</param>
/// <param name="Number">Its version number, or nothing when it has none.</param>
/// <param name="Lane">The lane its node stands on.</param>
/// <param name="Rail">The ink its own node and lane are drawn in.</param>
/// <param name="Dashed">Whether its node's ring is drawn broken.</param>
/// <param name="Behind">
/// Whether it is history older than the version the two sides parted at, which is drawn straight and
/// faded rather than in the line's own ink.
/// </param>
/// <param name="Lanes">Every lane's line through the row.</param>
/// <param name="Pills">The entry pills it shows, at most two.</param>
/// <param name="More">The entry pills the two stand in place of.</param>
/// <param name="Current">Whether it carries the chosen file's own local entry.</param>
internal sealed record CompatRow(
    GraphCard Card,
    CompatRowKind Kind,
    ulong? Number,
    int Lane,
    CompatRailKind Rail,
    bool Dashed,
    bool Behind,
    IReadOnlyList<CompatLaneLine> Lanes,
    IReadOnlyList<GraphChip> Pills,
    IReadOnlyList<GraphChip> More,
    bool Current
)
{
    /// <summary>Whether the two pills stand in place of more entries.</summary>
    internal bool MoreShown => More.Count > 0;
}

/// <summary>
/// The chain of one file's versions, in the order the compat list shows them.
/// </summary>
/// <remarks>
/// The chain is read from the drawing the two-dimensional graph is already made of, so the two views say
/// the same thing about the same snapshot. Every variant the file's chain gives rise to is read, not just
/// the versions an entry names: a future version upstream recorded, and a history branch nothing names,
/// are both rows the reader can act on.
/// <para>
/// The rail follows the rule a straight line is always preferred. The line is the chain of first children
/// from the newest version down through the file's record and its past, and a lane is opened only for a
/// chain that leaves it and comes back into it. A version with more than one successor continues the line
/// through its first successor, which is the one upstream names where it names one, and its other
/// successors are branches above it; a branch's own successors are branches of that branch. The history
/// older than the version the file's chain and upstream's parted at is drawn straight and faded.
/// </para>
/// <para>
/// Nothing here touches the toolkit: the rows, their lanes and their pills are values a test can read.
/// </para>
/// </remarks>
internal sealed class CompatChain
{
    /// <summary>The width of the column holding the entry pills.</summary>
    internal const double ReferencesWidth = 155;

    /// <summary>The width of the column the rail is drawn in.</summary>
    internal const double GraphWidth = 100;

    /// <summary>The width of the message column, which is the creator column's own start.</summary>
    internal const double MessageWidth = 280;

    /// <summary>The width of the local column.</summary>
    internal const double LocalWidth = 40;

    /// <summary>The width of the version hash column.</summary>
    internal const double VersionHashWidth = 72;

    /// <summary>The width of the variant hash column.</summary>
    internal const double VariantHashWidth = 72;

    /// <summary>The width of the version column.</summary>
    internal const double VersionWidth = 64;

    /// <summary>How tall a row is, which is the height of one pill plus its room.</summary>
    internal const double RowHeight = 32;

    /// <summary>How tall the band naming the columns is.</summary>
    internal const double HeaderHeight = 30;

    /// <summary>The width of an entry pill, which ORIGIN and LOCAL share.</summary>
    internal const double PillWidth = 52;

    /// <summary>The room between two pills.</summary>
    internal const double PillGap = 6;

    /// <summary>The width of the pill standing in place of the entries left out.</summary>
    internal const double MoreWidth = 22;

    /// <summary>How many entry pills a row shows before the rest are left out.</summary>
    internal const int ShownPills = 2;

    /// <summary>How many characters of a hash the hash columns show.</summary>
    internal const int HashLength = 7;

    /// <summary>The marks a hash that is not held is written as.</summary>
    internal const string UnknownHash = "???????";

    /// <summary>Makes a chain.</summary>
    /// <param name="rows">The rows, newest first.</param>
    /// <param name="truncated">Whether the reading stopped at the window's limit.</param>
    /// <param name="found">Whether the drawing holds an entry naming the file the chain is of.</param>
    private CompatChain(IReadOnlyList<CompatRow> rows, bool truncated, bool found)
    {
        Rows = rows;
        Truncated = truncated;
        Found = found;
    }

    /// <summary>The rows, newest first.</summary>
    internal IReadOnlyList<CompatRow> Rows { get; }

    /// <summary>Whether the reading stopped at the window's limit rather than walking out.</summary>
    internal bool Truncated { get; }

    /// <summary>The rows the drawing holds for the file the chain is of.</summary>
    internal bool Found { get; }

    /// <summary>Reads the chain of one file's versions out of a drawing.</summary>
    /// <param name="drawing">The drawing the two-dimensional graph is made of.</param>
    /// <param name="path">The chosen file's path as the Layout names it.</param>
    /// <returns>The chain, which is empty when no entry of the drawing names the path.</returns>
    internal static CompatChain Build(VariantGraphDrawing drawing, string path)
    {
        ArgumentNullException.ThrowIfNull(drawing);

        var focus = VariantGraphDrawing.Normalize(path);

        var head = drawing.Cards.FirstOrDefault(card =>
            card.Kind == GraphCardKind.Variant
            && card.Entries.Any(entry =>
                entry.Side == "local"
                && string.Equals(
                    VariantGraphDrawing.Normalize(entry.Path),
                    focus,
                    StringComparison.Ordinal
                )
            )
        );

        if (head is null)
        {
            return new CompatChain([], drawing.Truncated, found: false);
        }

        var spine = new List<GraphCard>();
        var onSpine = new HashSet<string>(StringComparer.Ordinal);

        for (
            var card = head;
            card is not null && onSpine.Add(card.Hash);
            card = ParentOf(drawing, card)
        )
        {
            spine.Add(card);
        }

        var drafts = drawing
            .Cards.Where(card =>
                card.Kind == GraphCardKind.Draft
                && card.Draft is { } draft
                && string.Equals(
                    VariantGraphDrawing.Normalize(draft.Path),
                    focus,
                    StringComparison.Ordinal
                )
            )
            .ToList();

        var relevant = Relevant(drawing, onSpine);
        var children = Children(drawing, relevant);

        // The line is the chain the reader is following: upstream's when it holds a version the file's
        // own chain does not, and the file's own chain otherwise. It runs from the newest version of that
        // chain, through the file's record, and down its past.
        var upstream = UpstreamCardOf(drawing, focus);

        var lineHead =
            upstream is not null && relevant.Contains(upstream.Hash) && !onSpine.Contains(upstream.Hash)
                ? upstream
                : head;

        var line = Line(drawing, lineHead, children, relevant, focus);
        var lineSet = line.Select(card => card.Hash).ToHashSet(StringComparer.Ordinal);

        // Where the line meets the file's own chain is the version the two sides parted at, and what is
        // below it is shared history rather than this file's work.
        var baseIndex = -1;

        if (!ReferenceEquals(lineHead, head))
        {
            for (var index = 0; index < line.Count; index++)
            {
                if (onSpine.Contains(line[index].Hash))
                {
                    baseIndex = index;

                    break;
                }
            }
        }

        var order = new List<RowWork>();
        var groups = new List<Group>();
        var emitted = new HashSet<string>(StringComparer.Ordinal);

        foreach (var draft in drafts)
        {
            order.Add(new RowWork(draft, CompatRowKind.Draft, null, Behind: false));
        }

        for (var index = 0; index < line.Count; index++)
        {
            var card = line[index];

            foreach (
                var child in Sorted(children.GetValueOrDefault(card.Hash), focus)
                    .Where(child =>
                        !lineSet.Contains(child.Hash) && !emitted.Contains(child.Hash)
                    )
            )
            {
                EmitSubtree(child, card, parent: null);
            }

            _ = emitted.Add(card.Hash);
            order.Add(
                new RowWork(
                    card,
                    card.Pending ? CompatRowKind.Pending : CompatRowKind.Spine,
                    null,
                    Behind: baseIndex >= 0 && index > baseIndex
                )
            );
        }

        void EmitSubtree(GraphCard root, GraphCard join, Group? parent)
        {
            if (emitted.Contains(root.Hash))
            {
                return;
            }

            var group = new Group(join, parent) { BlockStart = order.Count };

            groups.Add(group);

            // A chain keeps one lane for its whole length: it runs from the version it is based on up
            // through each version's first successor, which is what makes a straight chain straight.
            var chain = new List<GraphCard> { root };
            var seen = new HashSet<string>(StringComparer.Ordinal) { root.Hash };

            while (
                Sorted(children.GetValueOrDefault(chain[^1].Hash), focus)
                    .FirstOrDefault(child =>
                        !seen.Contains(child.Hash)
                        && !emitted.Contains(child.Hash)
                        && !lineSet.Contains(child.Hash)
                    )
                is { } next
            )
            {
                chain.Add(next);
                _ = seen.Add(next.Hash);
            }

            for (var index = chain.Count - 1; index >= 0; index--)
            {
                var card = chain[index];

                foreach (
                    var side in Sorted(children.GetValueOrDefault(card.Hash), focus)
                        .Where(child =>
                            !emitted.Contains(child.Hash) && !seen.Contains(child.Hash)
                        )
                )
                {
                    EmitSubtree(side, card, group);
                }

                if (group.Start < 0)
                {
                    group.Start = order.Count;
                }

                _ = emitted.Add(card.Hash);
                order.Add(
                    new RowWork(
                        card,
                        card.Pending ? CompatRowKind.Pending : CompatRowKind.Branch,
                        group,
                        Behind: false
                    )
                );
            }
        }

        if (order.Count == 0)
        {
            return new CompatChain([], drawing.Truncated, found: true);
        }

        var rowOfHash = new Dictionary<string, int>(StringComparer.Ordinal);

        for (var index = 0; index < order.Count; index++)
        {
            _ = rowOfHash.TryAdd(order[index].Card.Hash, index);
        }

        foreach (var group in groups)
        {
            group.JoinRow = rowOfHash.TryGetValue(group.Join.Hash, out var at) ? at : -1;
        }

        var used = new HashSet<int> { 0 };
        var active = new List<Group>();
        var rows = new List<CompatRow>(order.Count);

        for (var index = 0; index < order.Count; index++)
        {
            // A lane is taken from where its block begins, which is above its own newest version, so
            // the branches above a version do not take the lane that version is about to stand on.
            foreach (var group in groups.Where(group => group.BlockStart == index))
            {
                var lane = (group.Parent?.Lane ?? 0) + 1;

                while (used.Contains(lane))
                {
                    lane++;
                }

                used.Add(lane);
                group.Lane = lane;
                active.Add(group);
            }

            var work = order[index];
            var rail = work.Behind ? CompatRailKind.Behind : RailOf(work.Card, work.Kind);
            var behindBelow =
                !work.Behind
                && index + 1 < order.Count
                && order[index + 1].Behind;

            var lanes = new List<CompatLaneLine>
            {
                new(
                    Lane: 0,
                    Kind: rail,
                    Dashed: false,
                    Node: work.Group is null,
                    Above: index > 0,
                    Below: index < order.Count - 1,
                    Open: false,
                    Close: false,
                    BelowKind: behindBelow ? CompatRailKind.Behind : null
                ),
            };

            foreach (var group in active)
            {
                if (index < group.Start)
                {
                    // The lane is kept for the branches above its newest version, but nothing of it is
                    // drawn until that version is reached.
                    continue;
                }

                if (ReferenceEquals(group, work.Group))
                {
                    lanes.Add(
                        new CompatLaneLine(
                            group.Lane,
                            CompatRailKind.Primary,
                            Dashed: false,
                            Node: true,
                            Above: index > group.Start,
                            Below: group.JoinRow != index,
                            Open: false,
                            Close: false
                        )
                    );
                }
                else if (group.JoinRow == index)
                {
                    lanes.Add(
                        new CompatLaneLine(
                            group.Lane,
                            CompatRailKind.Primary,
                            Dashed: false,
                            Node: false,
                            Above: false,
                            Below: false,
                            Open: false,
                            Close: true,
                            CloseLane: work.Group?.Lane ?? 0
                        )
                    );
                }
                else if (index >= group.Start && (group.JoinRow < 0 || index <= group.JoinRow))
                {
                    lanes.Add(
                        new CompatLaneLine(
                            group.Lane,
                            CompatRailKind.Primary,
                            Dashed: false,
                            Node: false,
                            Above: true,
                            Below: true,
                            Open: false,
                            Close: false
                        )
                    );
                }
            }

            var (shown, hidden) = PillsOf(work.Card);

            rows.Add(
                new CompatRow(
                    Card: work.Card,
                    Kind: work.Kind,
                    Number: work.Kind == CompatRowKind.Draft
                        ? DraftNumber(drawing, work.Card)
                        : work.Card.Number,
                    Lane: work.Group?.Lane ?? 0,
                    Rail: rail,
                    Dashed: work.Kind is CompatRowKind.Draft or CompatRowKind.Pending,
                    Behind: work.Behind,
                    Lanes: lanes,
                    Pills: shown,
                    More: hidden,
                    Current: shown.Concat(hidden).Any(chip => chip.Chosen)
                )
            );

            foreach (var group in active.Where(group => group.JoinRow == index).ToList())
            {
                _ = used.Remove(group.Lane);
                _ = active.Remove(group);
            }
        }

        return new CompatChain(rows, drawing.Truncated, found: true);
    }

    /// <summary>The entry pills a card shows, split into those on screen and those left out.</summary>
    /// <remarks>
    /// The entries at the chosen file's path and the entry it tracks come first, so a row showing two
    /// pills shows the ones the reader is working with. A draft card has no entry of its own in the
    /// reading: the pill naming its file is what stands in their place.
    /// </remarks>
    /// <param name="card">The card.</param>
    /// <returns>The pills shown, and the pills left out.</returns>
    internal static (IReadOnlyList<GraphChip> Shown, IReadOnlyList<GraphChip> Hidden) PillsOf(
        GraphCard card
    )
    {
        ArgumentNullException.ThrowIfNull(card);

        var entries = card.Kind == GraphCardKind.Draft
            ? card.Chips.Where(chip => chip.Kind == GraphChipKind.File)
            : card.Chips.Where(chip => chip.Kind == GraphChipKind.Entry);

        var ordered = entries
            .OrderBy(chip => chip.Chosen || chip.Tracked ? 0 : 1)
            .ToList();

        return (ordered.Take(ShownPills).ToList(), ordered.Skip(ShownPills).ToList());
    }

    /// <summary>A hash as the hash columns write it.</summary>
    /// <param name="hash">The key, or nothing when no key is held.</param>
    /// <returns>Its first characters, or the marks standing for a key that is not held.</returns>
    internal static string ShortHash(string? hash) =>
        hash is { Length: > 0 } ? hash[..Math.Min(HashLength, hash.Length)] : UnknownHash;

    /// <summary>The variants the file's chain gives rise to, by hash.</summary>
    /// <param name="drawing">The drawing.</param>
    /// <param name="onSpine">The file's own chain, by hash.</param>
    /// <returns>The hashes of the chain and every variant based on it, at any depth.</returns>
    private static HashSet<string> Relevant(
        VariantGraphDrawing drawing,
        IReadOnlySet<string> onSpine
    )
    {
        var relevant = onSpine.ToHashSet(StringComparer.Ordinal);

        foreach (var card in drawing.Cards)
        {
            if (card.Kind != GraphCardKind.Variant || relevant.Contains(card.Hash))
            {
                continue;
            }

            var seen = new HashSet<string>(StringComparer.Ordinal);

            for (
                var one = card;
                one is not null && seen.Add(one.Hash);
                one = ParentOf(drawing, one)
            )
            {
                if (onSpine.Contains(one.Hash))
                {
                    _ = relevant.Add(card.Hash);

                    break;
                }
            }
        }

        return relevant;
    }

    /// <summary>The variants each variant in the window is the base of.</summary>
    /// <param name="drawing">The drawing.</param>
    /// <param name="relevant">The variants the file's chain gives rise to.</param>
    /// <returns>The children of each hash.</returns>
    private static Dictionary<string, List<GraphCard>> Children(
        VariantGraphDrawing drawing,
        IReadOnlySet<string> relevant
    )
    {
        var children = new Dictionary<string, List<GraphCard>>(StringComparer.Ordinal);

        foreach (var card in drawing.Cards)
        {
            if (
                card.Kind != GraphCardKind.Variant
                || !relevant.Contains(card.Hash)
                || card.BaseVariant is not { } parent
            )
            {
                continue;
            }

            if (!children.TryGetValue(parent, out var list))
            {
                list = [];
                children[parent] = list;
            }

            list.Add(card);
        }

        return children;
    }

    /// <summary>The chain the reader is following, newest first.</summary>
    /// <remarks>
    /// The line runs from the newest version the chain gives rise to, through the version the reader is
    /// on, and down its past. The newest version is found by walking up through the first successor at
    /// each step, which is the one upstream names where it names one, so a single future version keeps the
    /// line straight and a fork gives the line to upstream's side.
    /// </remarks>
    /// <param name="drawing">The drawing.</param>
    /// <param name="head">The version the line starts at: upstream's, or the file's own record.</param>
    /// <param name="children">The children of each variant.</param>
    /// <param name="relevant">The variants the file's chain gives rise to.</param>
    /// <param name="focus">The file's path, normalised.</param>
    /// <returns>The line, newest first.</returns>
    private static List<GraphCard> Line(
        VariantGraphDrawing drawing,
        GraphCard head,
        IReadOnlyDictionary<string, List<GraphCard>> children,
        IReadOnlySet<string> relevant,
        string focus
    )
    {
        var up = new List<GraphCard> { head };
        var seen = new HashSet<string>(StringComparer.Ordinal) { head.Hash };

        while (
            Sorted(children.GetValueOrDefault(up[0].Hash), focus)
                .FirstOrDefault(card => !seen.Contains(card.Hash) && relevant.Contains(card.Hash))
                is { } next
        )
        {
            up.Insert(0, next);
            _ = seen.Add(next.Hash);
        }

        var line = new List<GraphCard>(up);

        for (var card = ParentOf(drawing, head); card is not null; card = ParentOf(drawing, card))
        {
            line.Add(card);
        }

        return line;
    }

    /// <summary>The children of a variant in the order the list reads them.</summary>
    /// <param name="cards">The children, or nothing when it has none.</param>
    /// <param name="focus">The file's path, normalised.</param>
    /// <returns>The children, the one upstream names first, then the newest.</returns>
    private static IEnumerable<GraphCard> Sorted(
        IReadOnlyList<GraphCard>? cards,
        string focus
    )
    {
        if (cards is null)
        {
            return [];
        }

        return cards
            .OrderBy(card => UpstreamRank(card, focus))
            .ThenByDescending(card => card.Number ?? 0)
            .ThenBy(card => card.Hash, StringComparer.Ordinal);
    }

    /// <summary>How much a variant is the one upstream names at the file's path.</summary>
    /// <param name="card">The variant.</param>
    /// <param name="focus">The file's path, normalised.</param>
    /// <returns>Nothing for `origin`, one for another upstream, two for none.</returns>
    private static int UpstreamRank(GraphCard card, string focus)
    {
        var rank = 2;

        foreach (var entry in card.Entries)
        {
            if (
                entry.Side == "local"
                || entry.Deprecated
                || entry.Version is not { Length: > 0 }
                || !string.Equals(
                    VariantGraphDrawing.Normalize(entry.Path),
                    focus,
                    StringComparison.Ordinal
                )
            )
            {
                continue;
            }

            if (entry.Side == "origin")
            {
                return 0;
            }

            rank = 1;
        }

        return rank;
    }

    /// <summary>The card upstream names at the file's path, or nothing when it names none there.</summary>
    /// <remarks>
    /// The reading carries an entry per side, so the upstream entry at the path is what says which version
    /// upstream holds. `origin` is preferred where a work has several upstreams, since that is the side a
    /// reader thinks of first; another Vault's entry stands in when there is no `origin`.
    /// </remarks>
    /// <param name="drawing">The drawing.</param>
    /// <param name="focus">The file's path, normalised.</param>
    /// <returns>The card upstream's version stands at, or nothing.</returns>
    private static GraphCard? UpstreamCardOf(VariantGraphDrawing drawing, string focus)
    {
        GraphCard? other = null;

        foreach (var card in drawing.Cards)
        {
            foreach (var entry in card.Entries)
            {
                if (
                    entry.Side == "local"
                    || entry.Deprecated
                    || entry.Version is not { Length: > 0 }
                    || !string.Equals(
                        VariantGraphDrawing.Normalize(entry.Path),
                        focus,
                        StringComparison.Ordinal
                    )
                )
                {
                    continue;
                }

                if (entry.Side == "origin")
                {
                    return card;
                }

                other ??= card;
            }
        }

        return other;
    }

    /// <summary>The parent card of a version, or nothing when the window does not hold it.</summary>
    /// <param name="drawing">The drawing.</param>
    /// <param name="card">The card walking up from.</param>
    /// <returns>The parent, or nothing.</returns>
    private static GraphCard? ParentOf(VariantGraphDrawing drawing, GraphCard card) =>
        card.BaseVariant is { } baseVariant && drawing.ByHash.TryGetValue(baseVariant, out var parent)
            ? parent
            : null;

    /// <summary>The version number a draft would be recorded as: one past the version it is based on.</summary>
    /// <param name="drawing">The drawing.</param>
    /// <param name="card">The draft card.</param>
    /// <returns>The number, or nothing when the version it is based on has none.</returns>
    private static ulong? DraftNumber(VariantGraphDrawing drawing, GraphCard card) =>
        ParentOf(drawing, card) is { Number: { } number } ? number + 1 : null;

    /// <summary>The ink a row is drawn in.</summary>
    /// <param name="card">The card.</param>
    /// <param name="kind">What the row is.</param>
    /// <returns>The ink.</returns>
    private static CompatRailKind RailOf(GraphCard card, CompatRowKind kind) => kind switch
    {
        CompatRowKind.Draft => card.Draft is { Correct: false }
            ? CompatRailKind.Error
            : CompatRailKind.Modify,
        CompatRowKind.Pending => CompatRailKind.Modify,
        _ => CompatRailKind.Primary,
    };

    /// <summary>One row of the list before its lanes are known.</summary>
    /// <param name="Card">The card it stands for.</param>
    /// <param name="Kind">What it is.</param>
    /// <param name="Group">The lane it opens, or nothing for the line.</param>
    /// <param name="Behind">Whether it is shared history older than the version the two sides parted at.</param>
    private sealed record RowWork(GraphCard Card, CompatRowKind Kind, Group? Group, bool Behind);

    /// <summary>One lane a chain opens and closes into the version it left.</summary>
    private sealed class Group
    {
        /// <summary>Makes a lane that closes into a version.</summary>
        /// <param name="join">The version it comes back into.</param>
        /// <param name="parent">The lane it sits above, or nothing when it is a branch of the line.</param>
        internal Group(GraphCard join, Group? parent)
        {
            Join = join;
            Parent = parent;
        }

        /// <summary>The version it comes back into.</summary>
        internal GraphCard Join { get; }

        /// <summary>The lane it sits above, or nothing when it is a branch of the line.</summary>
        internal Group? Parent { get; }

        /// <summary>The first row of its block, which includes the branches above its newest version.</summary>
        internal int BlockStart { get; set; }

        /// <summary>The row of its own newest version, from which its lane is drawn.</summary>
        internal int Start { get; set; } = -1;

        /// <summary>The row of the version it comes back into.</summary>
        internal int JoinRow { get; set; } = -1;

        /// <summary>The lane it stands on.</summary>
        internal int Lane { get; set; }
    }
}

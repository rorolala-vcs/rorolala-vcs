using System.Globalization;

using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Media.TextFormatting;
using RolaSharp;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>What part of a card the pointer is on.</summary>
internal enum HoverPart
{
    /// <summary>The card itself, away from its content.</summary>
    Card,

    /// <summary>The message the card shows.</summary>
    Message,

    /// <summary>The creator's pill.</summary>
    Creator,

    /// <summary>The big faint version number.</summary>
    Number,

    /// <summary>The pill naming the variant's hash.</summary>
    HashPill,

    /// <summary>A pill naming an entry.</summary>
    EntryPill,

    /// <summary>A pill naming a state.</summary>
    BadgePill,

    /// <summary>The pill naming the file a draft belongs to.</summary>
    FilePill,

    /// <summary>A base version the index does not hold.</summary>
    Ghost,
}

/// <summary>One thing the pointer can be on.</summary>
/// <param name="Card">The card, when what is under the pointer is part of one.</param>
/// <param name="Part">Which part of it.</param>
/// <param name="Chip">The pill, when the part is one.</param>
/// <param name="Ghost">The missing base version, when that is what is under the pointer.</param>
/// <param name="GhostIndex">Its place among the ghosts, which is what makes two hovers the same.</param>
internal sealed record Hover(
    GraphCard? Card,
    HoverPart Part,
    GraphChip? Chip,
    GraphGhost? Ghost,
    int GhostIndex
);

/// <summary>The drawing half of the view: what is drawn over the graph, and what the pointer is on.</summary>
internal sealed partial class VersionGraphView
{
    /// <inheritdoc />
    public override void Render(DrawingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        var ink = CurrentInk();
        context.FillRectangle(new SolidColorBrush(ink.Ground), new Rect(Bounds.Size));

        if (Bounds.Width <= 0 || Bounds.Height <= 0)
        {
            return;
        }

        if (_drawing is null)
        {
            DrawEmpty(context, ink);
            DrawZoom(context, ink);
            DrawTruncated(context, ink);

            return;
        }

        ClampPan();

        var lineage = _hover?.Card is { } hovered ? LineageOf(hovered) : null;
        var stats = Stats();
        var frame = new VariantGraphFrame(
            _zoom,
            new Point(_panX, _panY),
            _hover?.Card?.Index,
            lineage,
            stats,
            _drawing.Truncated,
            _showFocus
        );

        Renderer.Render(context, Bounds.Size, _drawing, ink, frame);
        DrawTruncated(context, ink);
        DrawZoom(context, ink);
        DrawStats(context, ink, stats);
    }

    /// <summary>The control that draws the hover table over the switch in the corner.</summary>
    /// <remarks>
    /// It is made here rather than by the dock so that the drawing and the table stay one reading: the
    /// overlay asks this view what the pointer is on, and the dock only has to put it above the switch.
    /// </remarks>
    internal Control InfoTable => _infoOverlay ??= new InfoOverlay(this);

    /// <summary>The switch in the graph's corner and the table it shows, kept apart from the drawing.</summary>
    /// <remarks>
    /// The table wants to cover the switch, and a control's own drawing cannot cover a sibling control
    /// drawn after it. This is the sibling that does: it is added after the switch and draws the table
    /// over it, and it takes no pointer of its own so that the switch underneath is still the thing the
    /// pointer meets when the table has gone.
    /// </remarks>
    private sealed class InfoOverlay(VersionGraphView graph) : Control
    {
        /// <inheritdoc />
        public override void Render(DrawingContext context)
        {
            ArgumentNullException.ThrowIfNull(context);

            if (graph.InfoOnScreen() is { } table)
            {
                DrawInfoPanel(context, table.Ink, table.Info, Bounds.Size);
            }
        }
    }

    /// <summary>What the table would show now, or nothing when there is no table.</summary>
    /// <returns>The colours and the table.</returns>
    private (GraphInk Ink, Info Info)? InfoOnScreen()
    {
        if (!_showInfo || _hover is not { } hover)
        {
            return null;
        }

        var ink = CurrentInk();
        var info = InfoFor(hover, ink);

        return info.Title.Length == 0 && info.Rows.Count == 0 ? null : (ink, info);
    }

    /// <summary>What a hover table says about a thing, as its title and the rows under it.</summary>
    /// <param name="Title">What is written over the table.</param>
    /// <param name="Rows">The label and value of each row.</param>
    /// <param name="Accent">The colour the title is drawn in.</param>
    private sealed record Info(
        string Title,
        IReadOnlyList<(string Label, string Value)> Rows,
        Color Accent
    );

    /// <summary>The colours in force, read again when the look or the palette changes.</summary>
    /// <returns>The colours.</returns>
    private GraphInk CurrentInk()
    {
        var signature = (
            _themeVersion,
            _palette.Mid,
            _palette.Primary,
            _palette.Accent,
            _palette.Front
        );

        if (_ink is null || !_inkFor.Equals(signature))
        {
            _ink = GraphInk.From(_palette);
            _inkFor = signature;
            _renderer?.Invalidate();
        }

        return _ink;
    }

    /// <summary>Draws what is said in place of a graph.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawEmpty(DrawingContext context, GraphInk ink)
    {
        if (_emptyText is not { Length: > 0 } said)
        {
            return;
        }

        var text = new FormattedText(
            said,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(FontFamily.Default),
            13,
            new SolidColorBrush(ink.Muted)
        );

        context.DrawText(
            text,
            new Point(
                Math.Max(0, (Bounds.Width - text.Width) / 2),
                Math.Max(0, (Bounds.Height - text.Height) / 2)
            )
        );
    }

    /// <summary>Draws the scale in the corner.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawZoom(DrawingContext context, GraphInk ink)
    {
        if (_drawing is null)
        {
            return;
        }

        var text = new FormattedText(
            _zoomText,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(GraphMeasure.Mono),
            11,
            new SolidColorBrush(ink.Muted)
        );
        var width = text.Width + 16;
        var height = text.Height + 8;
        var at = new Rect(Inset, Bounds.Height - Inset - height, width, height);

        context.DrawRectangle(
            new SolidColorBrush(GraphTint.Alpha(ink.Card, 220)),
            new Pen(new SolidColorBrush(ink.CardEdge), 1),
            at,
            6,
            6
        );
        context.DrawText(text, new Point(at.X + 8, at.Y + 4));
    }

    /// <summary>Draws how much the graph holds, beside its corner.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="stats">What to say.</param>
    private void DrawStats(DrawingContext context, GraphInk ink, string stats)
    {
        if (_drawing is null || stats.Length == 0)
        {
            return;
        }

        var text = new FormattedText(
            stats,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(GraphMeasure.Mono),
            10.5,
            new SolidColorBrush(ink.Faint)
        );

        context.DrawText(
            text,
            new Point(
                Math.Max(Inset, Bounds.Width - Inset - text.Width),
                Bounds.Height - Inset - text.Height
            )
        );
    }

    /// <summary>Draws the warning that the reading stopped at the window's limit.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawTruncated(DrawingContext context, GraphInk ink)
    {
        if (_drawing is not { Truncated: true })
        {
            return;
        }

        var said = RolaI18N.Get("rorolala_vcs.version_graph.truncated");
        var text = new FormattedText(
            said,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(FontFamily.Default),
            11,
            new SolidColorBrush(ink.Pending)
        );
        var at = new Rect(Inset, Inset, text.Width + 16, text.Height + 8);

        context.DrawRectangle(
            new SolidColorBrush(GraphTint.Alpha(ink.Card, 220)),
            new Pen(new SolidColorBrush(ink.Pending), 1),
            at,
            6,
            6
        );
        context.DrawText(text, new Point(at.X + 8, at.Y + 4));
    }

    /// <summary>The cards and entries the graph is counted in.</summary>
    /// <returns>The line, or an empty string when there is no drawing.</returns>
    private string Stats()
    {
        if (_drawing is not { } drawing)
        {
            return string.Empty;
        }

        var parts = new List<string>
        {
            RolaI18N.Get("rorolala_vcs.version_graph.stats_variants", drawing.VariantCount),
            RolaI18N.Get("rorolala_vcs.version_graph.stats_entries", drawing.EntryCount),
            RolaI18N.Get("rorolala_vcs.version_graph.stats_merges", drawing.JoinCount),
            RolaI18N.Get("rorolala_vcs.version_graph.stats_same", drawing.SamePairCount),
        };

        if (drawing.PendingCount > 0)
        {
            parts.Add(
                RolaI18N.Get("rorolala_vcs.version_graph.stats_pending", drawing.PendingCount)
            );
        }

        if (drawing.DraftCount > 0)
        {
            parts.Add(
                RolaI18N.Get("rorolala_vcs.version_graph.stats_drafts", drawing.DraftCount)
            );
        }

        return string.Join(" · ", parts);
    }

    /// <summary>The thing under a point, or nothing.</summary>
    /// <param name="at">The point, in the control's coordinates.</param>
    /// <returns>The hover answer.</returns>
    private Hover? HitTest(Point at)
    {
        if (_drawing is null)
        {
            return null;
        }

        var x = (at.X - _panX) / _zoom;
        var y = (at.Y - _panY) / _zoom;

        for (var index = 0; index < _drawing.Ghosts.Count; index++)
        {
            var ghost = _drawing.Ghosts[index];
            var dx = x - ghost.X;
            var dy = y - ghost.Y;

            if (dx * dx + dy * dy <= 24 * 24)
            {
                return new Hover(null, HoverPart.Ghost, null, ghost, index);
            }
        }

        foreach (var card in _drawing.Cards)
        {
            if (
                x < card.CardLeft
                || x > card.CardRight
                || y < card.CardTop
                || y > card.ContentBottom
            )
            {
                continue;
            }

            if (y <= card.CardTop + VariantGraphDrawing.CardHeight)
            {
                var middle = y - card.Y;

                if (
                    x >= card.CardLeft + 11
                    && x <= card.CardLeft + 11 + 34 + card.CreatorWidth
                    && middle >= -10
                    && middle <= 10
                )
                {
                    return new Hover(card, HoverPart.Creator, null, null, -1);
                }

                if (x >= card.CardLeft + 57 + card.CreatorWidth)
                {
                    return new Hover(card, HoverPart.Message, null, null, -1);
                }

                return new Hover(card, HoverPart.Card, null, null, -1);
            }

            var rowTop = card.CardTop + VariantGraphDrawing.CardHeight + VariantGraphDrawing.PillGap;

            foreach (var row in card.Rows)
            {
                var left = card.CardLeft;

                foreach (var chip in row)
                {
                    if (
                        x >= left
                        && x <= left + chip.Width
                        && y >= rowTop
                        && y <= rowTop + VariantGraphDrawing.ChipHeight
                    )
                    {
                        return new Hover(card, PartOf(chip), chip, null, -1);
                    }

                    left += chip.Width + VariantGraphDrawing.ChipGap;
                }

                rowTop += VariantGraphDrawing.ChipHeight + VariantGraphDrawing.ChipGap;
            }

            return new Hover(card, HoverPart.Card, null, null, -1);
        }

        return null;
    }

    /// <summary>Which part a pill is.</summary>
    /// <param name="chip">The pill.</param>
    /// <returns>The part.</returns>
    private static HoverPart PartOf(GraphChip chip) => chip.Kind switch
    {
        GraphChipKind.Hash => HoverPart.HashPill,
        GraphChipKind.Entry => HoverPart.EntryPill,
        GraphChipKind.Badge => HoverPart.BadgePill,
        _ => HoverPart.FilePill,
    };

    /// <summary>The cards that stand around one: what it came from, what came from it and what it merges with.</summary>
    /// <param name="card">The card the pointer is on.</param>
    /// <returns>The cards to keep in full.</returns>
    private IReadOnlySet<int> LineageOf(GraphCard card)
    {
        var lineage = new HashSet<int>();
        var all = _drawing?.Cards ?? [];

        if (_drawing is null)
        {
            return lineage;
        }

        lineage.Add(card.Index);
        AddAncestors(card, lineage);

        if (card.Join is { } join && _drawing.ByHash.TryGetValue(join, out var merged))
        {
            lineage.Add(merged.Index);
            AddAncestors(merged, lineage);
        }

        var grew = true;

        while (grew)
        {
            grew = false;

            foreach (var one in all)
            {
                if (
                    one.BaseVariant is { } baseVariant
                    && _drawing.ByHash.TryGetValue(baseVariant, out var parent)
                    && lineage.Contains(parent.Index)
                    && lineage.Add(one.Index)
                )
                {
                    grew = true;
                }

                if (
                    one.Join is { } mergedIn
                    && _drawing.ByHash.TryGetValue(mergedIn, out var source)
                    && lineage.Contains(one.Index)
                    && lineage.Add(source.Index)
                )
                {
                    grew = true;
                }
            }
        }

        // The same file on another side is a chain beside this one rather than a descendant, so the
        // entries naming this card's file are what says which chains belong together.
        var paths = PathsOf(card);

        if (paths.Count > 0)
        {
            foreach (var one in all)
            {
                if (one.Entries.Any(entry => paths.Contains(VariantGraphDrawing.Normalize(entry.Path))))
                {
                    lineage.Add(one.Index);
                    AddAncestors(one, lineage);
                }

                if (one.Draft is { } draft && paths.Contains(draft.Path))
                {
                    lineage.Add(one.Index);
                }
            }
        }

        if (card.Draft is { } own)
        {
            foreach (var one in all)
            {
                if (one.Draft?.Path == own.Path)
                {
                    lineage.Add(one.Index);
                }

                if (one.Entries.Any(entry => VariantGraphDrawing.Normalize(entry.Path) == own.Path))
                {
                    lineage.Add(one.Index);
                    AddAncestors(one, lineage);
                }
            }
        }

        return lineage;
    }

    /// <summary>The files a card belongs to, as the paths its entries name.</summary>
    /// <param name="card">The card.</param>
    /// <returns>The paths, without the deprecated marker.</returns>
    private static HashSet<string> PathsOf(GraphCard card)
    {
        var paths = new HashSet<string>(StringComparer.Ordinal);

        foreach (var entry in card.Entries)
        {
            paths.Add(VariantGraphDrawing.Normalize(entry.Path));
        }

        if (card.Draft is { } draft)
        {
            paths.Add(draft.Path);
        }

        return paths;
    }

    /// <summary>Adds a card's ancestors to a lineage.</summary>
    /// <param name="card">The card to walk up from.</param>
    /// <param name="lineage">The set to add to.</param>
    private void AddAncestors(GraphCard card, HashSet<int> lineage)
    {
        var seen = new HashSet<int> { card.Index };
        var here = card;

        while (
            _drawing is not null
            && here.BaseVariant is { } baseVariant
            && _drawing.ByHash.TryGetValue(baseVariant, out var parent)
            && seen.Add(parent.Index)
        )
        {
            lineage.Add(parent.Index);
            here = parent;
        }
    }

    /// <summary>What hovering a thing says about it: a table rather than a line of prose.</summary>
    /// <param name="hover">What the pointer is on.</param>
    /// <param name="ink">The colours in force.</param>
    /// <returns>The table's title, rows and the colour the title is drawn in.</returns>
    private Info InfoFor(Hover hover, GraphInk ink)
    {
        if (hover.Ghost is not null)
        {
            return new Info(
                Words.UnknownBase,
                [(string.Empty, RolaI18N.Get("rorolala_vcs.version_graph.info_unknown_base"))],
                ink.Muted
            );
        }

        if (hover.Card is not { } card)
        {
            return new Info(string.Empty, [], ink.Muted);
        }

        return hover.Part switch
        {
            HoverPart.EntryPill => EntryInfo(card, hover.Chip, ink),
            HoverPart.HashPill => HashInfo(card, hover.Chip, ink),
            HoverPart.FilePill => FileInfo(card, hover.Chip),
            HoverPart.BadgePill => BadgeInfo(card, hover.Chip, ink),
            HoverPart.Creator => new Info(
                card.Creator,
                [(Label("info_creator_hash"), card.CreatorHash.Length > 0 ? card.CreatorHash : "?")],
                ink.Primary
            ),
            HoverPart.Message => new Info(
                card.Creator,
                [(Label("info_message"), Said(card.Message))],
                ink.Primary
            ),
            _ => CardInfo(card, ink),
        };
    }

    /// <summary>What a whole card says about itself.</summary>
    /// <param name="card">The card.</param>
    /// <param name="ink">The colours in force.</param>
    /// <returns>The table.</returns>
    private Info CardInfo(GraphCard card, GraphInk ink)
    {
        var rows = new List<(string Label, string Value)>
        {
            (Label("info_creator"), card.Creator),
            (Label("info_message"), Said(card.Message)),
        };

        if (card.Entries.Count > 0)
        {
            rows.Add((Label("info_entries"), card.Entries.Count.ToString(CultureInfo.InvariantCulture)));
        }

        if (
            card.Join is { } join
            && _drawing is { } drawing
            && drawing.ByHash.TryGetValue(join, out var merged)
        )
        {
            rows.Add((Label("info_join"), VariantGraphDrawing.VersionLabel(merged)));
        }

        if (card.Kind == GraphCardKind.Draft && card.Draft is { } draft)
        {
            rows.Add((Label("info_path"), draft.Path));
            rows.Add((Label("info_base"), draft.BaseLabel));

            if (draft.ShouldLabel is { } should)
            {
                rows.Add((Label("info_should"), should));
            }

            rows.Add(
                (
                    Label("info_state"),
                    RolaI18N.Get(
                        draft.Correct
                            ? "rorolala_vcs.version_graph.info_draft_correct"
                            : "rorolala_vcs.version_graph.info_draft_wrong"
                    )
                )
            );

            return new Info(
                Words.DraftLabel,
                rows,
                draft.Correct ? ink.Local : ink.Danger
            );
        }

        if (card.Pending && card.PendingPath is { Length: > 0 } pending)
        {
            rows.Add((Label("info_merging"), pending));
        }

        return new Info(
            VariantGraphDrawing.VersionLabel(card),
            rows,
            card.Pending ? ink.Pending : ink.Primary
        );
    }

    /// <summary>What a pill naming an entry says.</summary>
    /// <param name="card">The card it hangs under.</param>
    /// <param name="chip">The pill.</param>
    /// <param name="ink">The colours in force.</param>
    /// <returns>The table.</returns>
    private Info EntryInfo(GraphCard card, GraphChip? chip, GraphInk ink)
    {
        if (chip?.Entry is not { } entry)
        {
            return CardInfo(card, ink);
        }

        var rows = new List<(string Label, string Value)>
        {
            (Label("info_path"), entry.Path),
            (Label("info_uuid"), entry.Uuid),
            (Label("info_version"), VariantGraphDrawing.VersionLabel(card)),
        };

        if (entry.Owner is { Length: > 0 } owner)
        {
            rows.Add((Label("info_owner"), owner));
        }

        if (entry.Deprecated)
        {
            rows.Add((string.Empty, RolaI18N.Get("rorolala_vcs.version_graph.info_deprecated")));
        }

        return new Info($"{chip.Tag} · {chip.Name}", rows, ink.Side(entry.Side));
    }

    /// <summary>What a pill naming a variant's hash says.</summary>
    /// <param name="card">The card it hangs under.</param>
    /// <param name="chip">The pill.</param>
    /// <param name="ink">The colours in force.</param>
    /// <returns>The table.</returns>
    private Info HashInfo(GraphCard card, GraphChip? chip, GraphInk ink)
    {
        var rows = new List<(string Label, string Value)>
        {
            (Label("info_variant"), card.Hash),
        };

        if (card.HasVersion)
        {
            rows.Add((Label("info_version"), card.VersionHash!));
        }

        rows.Add((Label("info_content"), card.Storage));

        if (
            card.Join is { } join
            && _drawing is { } drawing
            && drawing.ByHash.TryGetValue(join, out var merged)
        )
        {
            rows.Add((Label("info_join"), VariantGraphDrawing.VersionLabel(merged)));
        }

        return new Info(chip?.Text ?? string.Empty, rows, ink.Primary);
    }

    /// <summary>What the pill naming a draft's file says.</summary>
    /// <param name="card">The card it hangs under.</param>
    /// <param name="chip">The pill.</param>
    /// <returns>The table.</returns>
    private Info FileInfo(GraphCard card, GraphChip? chip) =>
        new(
            chip?.Name ?? string.Empty,
            card.Draft is { } draft
                ? [(Label("info_path"), draft.Path)]
                : [],
            CurrentInk().Local
        );

    /// <summary>What a badge says, which is the reason it is worn.</summary>
    /// <param name="card">The card it hangs under.</param>
    /// <param name="chip">The pill.</param>
    /// <param name="ink">The colours in force.</param>
    /// <returns>The table.</returns>
    private static Info BadgeInfo(GraphCard card, GraphChip? chip, GraphInk ink)
    {
        IReadOnlyList<(string Label, string Value)> rows = card.Draft switch
        {
            { } draft =>
            [
                (
                    Label("info_state"),
                    RolaI18N.Get(
                        draft.Correct
                            ? "rorolala_vcs.version_graph.info_draft_correct"
                            : "rorolala_vcs.version_graph.info_draft_wrong"
                    )
                ),
            ],
            _ => [],
        };

        return new Info(
            chip?.Text ?? string.Empty,
            rows,
            card.Draft is { Correct: false } ? ink.Danger : ink.Local
        );
    }

    /// <summary>One row's label, in the language the program speaks.</summary>
    /// <param name="key">The label's key under <c>rorolala_vcs.version_graph</c>.</param>
    /// <returns>The words.</returns>
    private static string Label(string key) => RolaI18N.Get($"rorolala_vcs.version_graph.{key}");

    /// <summary>What a message is said as, when it may be nothing.</summary>
    /// <param name="message">The message.</param>
    /// <returns>The message, or what nothing is said as.</returns>
    private static string Said(string message) =>
        message.Length > 0
            ? message
            : RolaI18N.Get("rorolala_vcs.version_graph.info_no_message");

    /// <summary>Draws the table the pointer's thing is read through, in the graph's corner.</summary>
    /// <remarks>
    /// It stands still while the graph moves under it: a table that followed the pointer would cover
    /// what is being read, and one placed in the corner is read once rather than chased. A value wider
    /// than its column ends in an ellipsis; the whole of it is what a right-click copies.
    /// </remarks>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="info">What the table says.</param>
    /// <param name="size">The room the table stands in, which is the graph's whole stage.</param>
    private static void DrawInfoPanel(DrawingContext context, GraphInk ink, Info info, Size size)
    {
        const double width = 300;
        const double padding = 12;
        const double labelWidth = 64;
        const double rowHeight = 19;

        if (info.Title.Length == 0 && info.Rows.Count == 0)
        {
            return;
        }

        var title = new FormattedText(
            info.Title,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(FontFamily.Default, FontStyle.Normal, FontWeight.SemiBold),
            12.5,
            new SolidColorBrush(info.Accent)
        );

        // The right and top are the switch's own: the table covers it exactly, and its first row starts
        // where the switch did rather than one padding step below it.
        var titleTop = Corner;
        var rowsTop = titleTop + title.Height + 9;
        var height = (rowsTop - titleTop) + (info.Rows.Count * rowHeight) + 10;
        var x = size.Width - Corner - width;
        var y = titleTop;

        context.DrawRectangle(
            new SolidColorBrush(ink.Shadow),
            null,
            new Rect(x, y + 4, width, height),
            8,
            8
        );
        context.DrawRectangle(
            new SolidColorBrush(GraphTint.Alpha(ink.Card, 242)),
            new Pen(new SolidColorBrush(ink.CardEdge), 1),
            new Rect(x, y, width, height),
            8,
            8
        );
        context.DrawText(title, new Point(x + padding, y + 9));

        for (var index = 0; index < info.Rows.Count; index++)
        {
            var (label, value) = info.Rows[index];
            var top = rowsTop + (index * rowHeight);
            var valueLeft = label.Length > 0 ? x + padding + labelWidth : x + padding;
            var valueWidth = label.Length > 0
                ? width - (padding * 2) - labelWidth
                : width - (padding * 2);

            if (label.Length > 0)
            {
                var said = new FormattedText(
                    label,
                    CultureInfo.InvariantCulture,
                    FlowDirection.LeftToRight,
                    new Typeface(FontFamily.Default),
                    10.5,
                    new SolidColorBrush(ink.Faint)
                );

                context.DrawText(
                    said,
                    new Point(x + padding, top + ((rowHeight - said.Height) / 2))
                );
            }

            var text = new FormattedText(
                value,
                CultureInfo.InvariantCulture,
                FlowDirection.LeftToRight,
                new Typeface(FontFamily.Default),
                11,
                new SolidColorBrush(label.Length > 0 ? ink.Text : ink.Muted)
            )
            {
                MaxTextWidth = Math.Max(1, valueWidth),
                MaxLineCount = 1,
                Trimming = TextTrimming.CharacterEllipsis,
            };

            context.DrawText(text, new Point(valueLeft, top + ((rowHeight - text.Height) / 2)));
        }
    }
}

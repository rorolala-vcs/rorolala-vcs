using System.Globalization;

using Avalonia;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The compat half of the view: one file's chain as a list of versions rather than a field of cards.
/// </summary>
/// <remarks>
/// It shares the reading the two-dimensional drawing is made of, so the two views cannot disagree about
/// the same snapshot, and it keeps its own pointer, scroll and selection because a list is read by walking
/// rather than by panning. The rail beside the list is the desktop's own: a lane is a column of content
/// units, and the colours are the look's roles, never a colour of its own.
/// </remarks>
internal sealed partial class VersionGraphView
{
    /// <summary>Where the chain's own lane stands in the graph column.</summary>
    private const double RailMain = 29;

    /// <summary>The room between two lanes.</summary>
    private const double RailPitch = 18;

    /// <summary>The width of a lane's line.</summary>
    private const double RailLine = 2;

    /// <summary>The outer radius of a node's ring.</summary>
    private const double RailRing = 4.5;

    /// <summary>The width of a node's ring.</summary>
    private const double RailStroke = 1.5;

    /// <summary>The room between a node's ring and the line meeting it.</summary>
    private const double RailGap = RailRing + 2;

    /// <summary>The broken line a draft, a merge in progress or an unmerged variant is drawn with.</summary>
    private static readonly DashStyle RailDash = new([1.5, 1.5], 0);

    /// <summary>The broken ring the same rows are drawn with.</summary>
    private static readonly DashStyle RailRingDash = new([1.5, 1.0], 0);

    /// <summary>The frame timer that eases the hover table to the place it is read from.</summary>
    private readonly DispatcherTimer _compatInfoGlide = new()
    {
        Interval = TimeSpan.FromMilliseconds(15),
    };

    /// <summary>How much of the way to its target the hover table takes a frame.</summary>
    private const double CompatInfoPart = 0.35;

    /// <summary>Whether the list is shown rather than the two-dimensional drawing.</summary>
    private bool _compat;

    /// <summary>The table the pointer's row is read through, or nothing when none stands.</summary>
    private Info? _compatInfo;

    /// <summary>Where the table stands now, which is what a change of row moves.</summary>
    private Point _compatInfoAt;

    /// <summary>Where the table is going.</summary>
    private Point _compatInfoTarget;

    /// <summary>How far the table has arrived, from nothing to all of it.</summary>
    private double _compatInfoWeight;

    /// <summary>How far it is going, which is nothing while it is leaving.</summary>
    private double _compatInfoTargetWeight;

    /// <summary>The chain on screen, or nothing while a read is going or when there is none.</summary>
    private CompatChain? _compatChain;

    /// <summary>The path the chain on screen is of, so a window that stops holding it is told apart.</summary>
    private string? _compatPath;

    /// <summary>Whether the last read of the chain failed.</summary>
    private bool _compatFailed;

    /// <summary>The chosen file's path as the Layout names it, which is what the chain is of.</summary>
    private string? _focusPath;

    /// <summary>How far the list has been scrolled, in screen units.</summary>
    private double _compatScroll;

    /// <summary>The row the keyboard or a click picked out, by its place in the chain.</summary>
    private int? _compatSelected;

    /// <summary>The row the pointer is on, by its place in the chain.</summary>
    private int? _compatHoverRow;

    /// <summary>Whether the pointer is on the pill standing in place of the entries left out.</summary>
    private bool _compatHoverMore;

    /// <summary>Whether the list is shown rather than the two-dimensional drawing.</summary>
    internal bool CompatOn => _compat;

    /// <summary>Shows the list, or the drawing again.</summary>
    /// <param name="on">Whether the list is shown.</param>
    internal void ShowCompat(bool on)
    {
        if (on == _compat)
        {
            return;
        }

        _compat = on;
        _compatSelected = null;
        _compatHoverRow = null;
        _compatHoverMore = false;
        _compatScroll = 0;
        _hover = null;
        _hoverGlide.Stop();
        _hoverWeight = 0;
        DropCompatInfo();
        BuildCompat();
        UpdateCursor();
        RedrawInfo();
        InvalidateVisual();
    }

    /// <summary>Reads the chain of the chosen file out of the drawing on screen, or forgets it.</summary>
    private void BuildCompat()
    {
        if (!_compat || _focusPath is not { Length: > 0 } path || _drawing is not { } drawing)
        {
            _compatChain = null;
            _compatPath = null;
            _compatSelected = null;

            return;
        }

        var built = CompatChain.Build(drawing, path);

        // A reading that no longer holds the same file's entry is the window moving on rather than the
        // file losing its chain, and the list keeps what it was showing, as the drawing does.
        if (
            !built.Found
            && _compatChain is not null
            && string.Equals(_compatPath, path, StringComparison.Ordinal)
        )
        {
            return;
        }

        _compatChain = built;
        _compatPath = built.Found ? path : null;
        _compatSelected = _compatSelected is { } picked && picked < built.Rows.Count
            ? picked
            : null;
        _compatScroll = Math.Clamp(_compatScroll, 0, CompatMaxScroll(built));
    }

    /// <summary>How far the list can be scrolled before its last row is at the foot of the dock.</summary>
    /// <param name="chain">The chain.</param>
    /// <returns>The distance.</returns>
    private double CompatMaxScroll(CompatChain chain) =>
        Math.Max(0, (chain.Rows.Count * CompatChain.RowHeight) - CompatListHeight);

    /// <summary>How much room the rows have under the band naming the columns.</summary>
    private double CompatListHeight => Math.Max(0, Bounds.Height - CompatChain.HeaderHeight);

    /// <summary>Draws the list.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawCompat(DrawingContext context, GraphInk ink)
    {
        var columns = CompatColumns.Of(Bounds.Size);

        // The list keeps the file it was last about even when the choice is taken away or becomes
        // several files: the reader is looking at where the work stood, and a screen that emptied
        // itself would be a larger change than the one they made. What is not being worked on is said
        // by the entry pills losing their ink rather than by the rows going away.
        if (_compatChain is not { } chain)
        {
            DropCompatInfo();

            if (_compatFailed)
            {
                DrawCompatFailed(context, ink);
            }
            else if (_reading)
            {
                DrawCompatLoading(context, ink);
            }
            else
            {
                DrawCompatState(
                    context,
                    ink,
                    RolaI18N.Get("rorolala_vcs.version_graph.compat_unavailable")
                );
            }

            return;
        }

        if (chain.Rows.Count == 0)
        {
            DropCompatInfo();
            DrawCompatState(context, ink, RolaI18N.Get("rorolala_vcs.version_graph.empty"));

            return;
        }

        DrawCompatHeader(context, ink, columns);

        var list = new Rect(
            0,
            CompatChain.HeaderHeight,
            Bounds.Width,
            CompatListHeight
        );
        var content = chain.Rows.Count * CompatChain.RowHeight;

        _compatScroll = Math.Clamp(_compatScroll, 0, Math.Max(0, content - list.Height));

        var first = (int)Math.Floor(_compatScroll / CompatChain.RowHeight);
        var last = Math.Min(
            chain.Rows.Count - 1,
            (int)Math.Ceiling((_compatScroll + list.Height) / CompatChain.RowHeight)
        );

        using (context.PushClip(list))
        {
            for (var index = Math.Max(0, first); index <= last; index++)
            {
                DrawCompatRow(
                    context,
                    ink,
                    columns,
                    chain.Rows[index],
                    index,
                    list.Top + (index * CompatChain.RowHeight) - _compatScroll
                );
            }
        }

        DrawCompatScrollbar(context, ink, content, list);
        DrawCompatTruncated(context, ink, chain);
        DrawCompatHover(context, ink);
    }

    /// <summary>Draws the band naming the columns.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    private void DrawCompatHeader(DrawingContext context, GraphInk ink, CompatColumns columns)
    {
        var labels = new (string Key, double Left, bool Centred)[]
        {
            ("compat_column_references", columns.References, false),
            ("compat_column_graph", columns.Graph + 20, false),
            ("compat_column_message", columns.Message + 12, false),
            ("compat_column_creator", columns.Creator + 12, false),
            ("compat_column_local", columns.Local, true),
            ("compat_column_version_hash", columns.VersionHash + 8, false),
            ("compat_column_variant_hash", columns.VariantHash + 8, false),
            ("compat_column_version", columns.Version, false),
        };

        foreach (var (key, left, centred) in labels)
        {
            var text = CompatText(
                RolaI18N.Get($"rorolala_vcs.version_graph.{key}"),
                11,
                ink.Faint
            );
            var x = centred ? left + ((CompatChain.LocalWidth - text.Width) / 2) : left;

            context.DrawText(
                text,
                new Point(x, (CompatChain.HeaderHeight - text.Height) / 2)
            );
        }

        context.DrawLine(
            new Pen(new SolidColorBrush(ink.CardEdge), 1),
            new Point(0, CompatChain.HeaderHeight - 0.5),
            new Point(Bounds.Width, CompatChain.HeaderHeight - 0.5)
        );
    }

    /// <summary>Draws one row.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="index">Its place in the chain.</param>
    /// <param name="top">Its top edge.</param>
    private void DrawCompatRow(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        int index,
        double top
    )
    {
        var hovered = _compatHoverRow == index;
        var selected = _compatSelected == index;

        // A row keeps its own shape whatever the choice is; only the entry pills say whether the file
        // they name is the one being worked on.
        var dimmed = ChosenFiles.Current.Count != 1;

        if ((row.Current && !dimmed) || hovered)
        {
            context.FillRectangle(
                new SolidColorBrush(GraphTint.Alpha(ink.Text, 18)),
                new Rect(0, top, Bounds.Width, CompatChain.RowHeight)
            );
        }

        if (selected)
        {
            context.FillRectangle(
                new SolidColorBrush(GraphTint.Alpha(ink.Primary, 45)),
                new Rect(0, top, Bounds.Width, CompatChain.RowHeight)
            );
            context.FillRectangle(
                new SolidColorBrush(ink.Primary),
                new Rect(0, top + 4, 2, CompatChain.RowHeight - 8)
            );
        }

        DrawCompatRail(context, ink, columns, row, top);
        DrawCompatPills(context, ink, columns, row, top, dimmed);
        DrawCompatMessage(context, ink, columns, row, top);
        DrawCompatCreator(context, ink, columns, row, top);
        DrawCompatLocal(context, ink, columns, row, top);
        DrawCompatHashes(context, ink, columns, row, top);
        DrawCompatNumber(context, ink, columns, row, top);
    }

    /// <summary>Draws the lanes and the node of one row.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    private void DrawCompatRail(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top
    )
    {
        var mid = top + (CompatChain.RowHeight / 2);
        var bottom = top + CompatChain.RowHeight;
        var main = columns.Graph + RailMain;

        foreach (var line in row.Lanes)
        {
            var x = main + (line.Lane * RailPitch);
            var above = new Pen(new SolidColorBrush(CompatRailInk(ink, line.Kind)), RailLine)
            {
                LineCap = PenLineCap.Round,
                DashStyle = line.Dashed ? RailDash : null,
            };
            var below = line.BelowKind is { } under
                ? new Pen(new SolidColorBrush(CompatRailInk(ink, under)), RailLine)
                {
                    LineCap = PenLineCap.Round,
                    DashStyle = line.Dashed ? RailDash : null,
                }
                : above;

            if (line.Open)
            {
                var curve = new StreamGeometry();

                using (var shape = curve.Open())
                {
                    shape.BeginFigure(new Point(main + RailGap, mid), false);
                    shape.QuadraticBezierTo(new Point(x, mid), new Point(x, mid + 10));
                    shape.EndFigure(false);
                }

                context.DrawGeometry(null, above, curve);
                context.DrawLine(above, new Point(x, mid + 10), new Point(x, bottom));
            }
            else if (line.Close)
            {
                context.DrawLine(above, new Point(x, top), new Point(x, mid - 10));

                var into = main + (line.CloseLane * RailPitch) + RailGap;
                var curve = new StreamGeometry();

                using (var shape = curve.Open())
                {
                    shape.BeginFigure(new Point(x, mid - 10), false);
                    shape.QuadraticBezierTo(new Point(x, mid), new Point(into, mid));
                    shape.EndFigure(false);
                }

                context.DrawGeometry(null, above, curve);
            }
            else if (line.Node)
            {
                if (line.Above)
                {
                    context.DrawLine(above, new Point(x, top), new Point(x, mid - RailGap));
                }

                if (line.Below)
                {
                    context.DrawLine(below, new Point(x, mid + RailGap), new Point(x, bottom));
                }
            }
            else
            {
                context.DrawLine(above, new Point(x, top), new Point(x, bottom));
            }
        }

        // The rings are drawn after every line so that a curve leaving one tucks under it rather than
        // crossing it, which is what makes a merge read as one node rather than two.
        foreach (var line in row.Lanes)
        {
            if (!line.Node)
            {
                continue;
            }

            var x = main + (line.Lane * RailPitch);
            var ring = new Pen(new SolidColorBrush(CompatRailInk(ink, line.Kind)), RailStroke)
            {
                DashStyle = line.Dashed ? RailRingDash : null,
            };

            context.DrawEllipse(
                null,
                ring,
                new Point(x, mid),
                RailRing - (RailStroke / 2),
                RailRing - (RailStroke / 2)
            );
        }
    }

    /// <summary>Draws the entry pills of one row and the pill standing in place of the rest.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    /// <param name="dimmed">Whether the file the entries name is not being worked on.</param>
    private void DrawCompatPills(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top,
        bool dimmed
    )
    {
        var pillTop = top + ((CompatChain.RowHeight - 20) / 2);
        var left = columns.References;

        foreach (var chip in row.Pills)
        {
            DrawCompatPill(context, ink, chip, left, pillTop, dimmed);
            left += CompatChain.PillWidth + CompatChain.PillGap;
        }

        if (row.MoreShown)
        {
            var rect = new Rect(left, pillTop, CompatChain.MoreWidth, 20);

            context.DrawRectangle(
                Brushes.Transparent,
                new Pen(new SolidColorBrush(ink.ChipEdge), 1) { DashStyle = RailDash },
                rect,
                5,
                5
            );

            var plus = CompatText("+", 11, ink.Muted);

            context.DrawText(
                plus,
                new Point(
                    left + ((CompatChain.MoreWidth - plus.Width) / 2),
                    pillTop + ((20 - plus.Height) / 2)
                )
            );
        }
    }

    /// <summary>Draws one entry pill, in the ink its entry stands in.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="chip">The pill.</param>
    /// <param name="left">Its left edge.</param>
    /// <param name="top">Its top edge.</param>
    /// <param name="dimmed">Whether the entry it names is not being worked on.</param>
    private void DrawCompatPill(
        DrawingContext context,
        GraphInk ink,
        GraphChip chip,
        double left,
        double top,
        bool dimmed
    )
    {
        var colour = dimmed
            ? _palette.Semantic.Disabled
            : chip.Chosen
                ? ink.Local
                : chip.Tracked
                    ? ink.Origin
                    : _palette.Semantic.Disabled;
        var rect = new Rect(left, top, CompatChain.PillWidth, 20);

        context.DrawRectangle(
            new SolidColorBrush(ink.ChipFill),
            new Pen(new SolidColorBrush(GraphTint.Mix(colour, ink.ChipEdge, 0.55)), 1),
            rect,
            5,
            5
        );

        var tag = CompatText(
            chip.Kind == GraphChipKind.File || chip.Entry?.Side == "local" ? "LOCAL" : chip.Tag ?? "",
            GraphMeasure.Face(GraphFont.Tag).Size,
            colour,
            semibold: true
        );

        context.DrawText(
            tag,
            new Point(
                left + ((CompatChain.PillWidth - tag.Width) / 2),
                top + ((20 - tag.Height) / 2)
            )
        );
    }

    /// <summary>Draws the row's message, which is what the version says was done.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    private static void DrawCompatMessage(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top
    )
    {
        var said = row.Card.Message.Length > 0 ? $"\"{row.Card.Message}\"" : string.Empty;
        var text = CompatText(
            said,
            12.5,
            row.Behind ? ink.Muted : ink.Text,
            italic: true,
            room: CompatChain.MessageWidth - 24
        );

        context.DrawText(
            text,
            new Point(columns.Message + 12, top + ((CompatChain.RowHeight - text.Height) / 2))
        );
    }

    /// <summary>Draws the row's creator, right where its message ends.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    private static void DrawCompatCreator(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top
    )
    {
        var text = CompatText(
            row.Card.Creator,
            12.5,
            row.Behind ? ink.Muted : ink.Text,
            bold: true,
            room: Math.Max(1, columns.CreatorWidth - 24)
        );

        context.DrawText(
            text,
            new Point(columns.Creator + 12, top + ((CompatChain.RowHeight - text.Height) / 2))
        );
    }

    /// <summary>Draws whether the row's content is held here, as a tick or a cross.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    private void DrawCompatLocal(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top
    )
    {
        var centre = new Point(
            columns.Local + (CompatChain.LocalWidth / 2),
            top + (CompatChain.RowHeight / 2)
        );

        if (row.Card.Offline && row.Card.Kind == GraphCardKind.Variant && row.Card.HasVersion)
        {
            context.DrawEllipse(
                new SolidColorBrush(ink.Success),
                null,
                centre,
                6,
                6
            );

            var check = new StreamGeometry();

            using (var shape = check.Open())
            {
                shape.BeginFigure(new Point(centre.X - 3, centre.Y + 0.2), false);
                shape.LineTo(new Point(centre.X - 1, centre.Y + 2.2));
                shape.LineTo(new Point(centre.X + 3, centre.Y - 1.8));
                shape.EndFigure(false);
            }

            context.DrawGeometry(
                null,
                new Pen(new SolidColorBrush(ink.Ground), 1.7)
                {
                    LineCap = PenLineCap.Round,
                    LineJoin = PenLineJoin.Round,
                },
                check
            );

            return;
        }

        var cross = new Pen(new SolidColorBrush(_palette.Semantic.Disabled), 1.8)
        {
            LineCap = PenLineCap.Round,
        };

        context.DrawLine(
            cross,
            new Point(centre.X - 3.5, centre.Y - 3.5),
            new Point(centre.X + 3.5, centre.Y + 3.5)
        );
        context.DrawLine(
            cross,
            new Point(centre.X + 3.5, centre.Y - 3.5),
            new Point(centre.X - 3.5, centre.Y + 3.5)
        );
    }

    /// <summary>Draws the version's hash and the variant's hash.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    private static void DrawCompatHashes(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top
    )
    {
        var version = CompatText(
            CompatChain.ShortHash(row.Card.VersionHash),
            GraphMeasure.Face(GraphFont.Hash).Size,
            row.Behind ? ink.Faint : ink.HashText,
            mono: true
        );
        var variant = CompatText(
            CompatChain.ShortHash(row.Card.Hash),
            GraphMeasure.Face(GraphFont.Hash).Size,
            row.Behind ? ink.Faint : ink.HashText,
            mono: true
        );

        context.DrawText(
            version,
            new Point(columns.VersionHash + 8, top + ((CompatChain.RowHeight - version.Height) / 2))
        );
        context.DrawText(
            variant,
            new Point(columns.VariantHash + 8, top + ((CompatChain.RowHeight - variant.Height) / 2))
        );
    }

    /// <summary>Draws the version's number, large and faint, at the row's foot.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="columns">Where the columns stand.</param>
    /// <param name="row">The row.</param>
    /// <param name="top">Its top edge.</param>
    private static void DrawCompatNumber(
        DrawingContext context,
        GraphInk ink,
        CompatColumns columns,
        CompatRow row,
        double top
    )
    {
        if (row.Number is not { } number)
        {
            return;
        }

        var text = CompatText(
            string.Create(CultureInfo.InvariantCulture, $"#{number}"),
            GraphMeasure.Face(GraphFont.Big).Size,
            GraphTint.Alpha(ink.Muted, row.Behind ? (byte)22 : (byte)36),
            extraBold: true
        );

        context.DrawText(
            text,
            new Point(columns.Version, top + ((CompatChain.RowHeight - text.Height) / 2))
        );
    }

    /// <summary>Draws how far the list can still be scrolled, when it cannot all be seen.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="content">How tall the rows are together.</param>
    /// <param name="list">The room the rows are drawn in.</param>
    private void DrawCompatScrollbar(
        DrawingContext context,
        GraphInk ink,
        double content,
        Rect list
    )
    {
        if (content <= list.Height || list.Height <= 0)
        {
            return;
        }

        var height = Math.Max(24, list.Height * (list.Height / content));
        var travel = list.Height - height;
        var range = content - list.Height;
        var top = list.Top + (range > 0 ? travel * (_compatScroll / range) : 0);

        context.DrawRectangle(
            new SolidColorBrush(GraphTint.Alpha(ink.Text, 60)),
            null,
            new Rect(Bounds.Width - 8, top, 4, height),
            2,
            2
        );
    }

    /// <summary>Draws the note that the reading stopped at the window's limit.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="chain">The chain.</param>
    private void DrawCompatTruncated(DrawingContext context, GraphInk ink, CompatChain chain)
    {
        if (!chain.Truncated)
        {
            return;
        }

        var text = CompatText(
            RolaI18N.Get("rorolala_vcs.version_graph.truncated"),
            11,
            ink.Pending
        );

        context.DrawText(
            text,
            new Point(14, Bounds.Height - text.Height - 6)
        );
    }

    /// <summary>Draws the table the pointer's row is read through, below the row it belongs to.</summary>
    /// <remarks>
    /// The table hangs under the row the pointer is on rather than in a corner of the dock, and its left
    /// edge follows the pointer, so a reader looking at a pill reads what the pill says beside the hand
    /// that points at it. It fades and moves rather than snapping: both are what makes the table read as
    /// the same table following the pointer instead of a new one appearing at every row.
    /// </remarks>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawCompatHover(DrawingContext context, GraphInk ink)
    {
        if (_compatInfo is not { } info || _compatInfoWeight <= 0.004)
        {
            return;
        }

        using (context.PushOpacity(Math.Clamp(_compatInfoWeight, 0, 1)))
        {
            DrawInfoPanel(context, ink, info, Bounds.Size, _compatInfoAt);
        }
    }

    /// <summary>Says what the pointer's row or pill is read through, and where the table stands for it.</summary>
    /// <param name="index">The row's place.</param>
    /// <param name="pointerX">Where the pointer stands across the dock.</param>
    private void UpdateCompatInfo(int index, double pointerX)
    {
        if (!_showInfo || _compatChain is not { } chain || index < 0 || index >= chain.Rows.Count)
        {
            HideCompatInfo();

            return;
        }

        var row = chain.Rows[index];
        Info? info = _compatHoverMore
            ? MoreInfo(row)
            : _hover is { } hover
                ? InfoFor(hover, CurrentInk())
                : null;

        if (info is null || (info.Title.Length == 0 && info.Rows.Count == 0))
        {
            HideCompatInfo();

            return;
        }

        // The table hangs under the row, and stands over it when the dock has no room below: a table
        // pushed back inside the dock would cover the row it explains.
        var rowTop = CompatChain.HeaderHeight + (index * CompatChain.RowHeight) - _compatScroll;
        var height = 22 + (info.Rows.Count * 19);
        var below = rowTop + CompatChain.RowHeight + 6;
        var y = below + height <= Bounds.Height - 8 ? below : Math.Max(8, rowTop - 6 - height);

        _compatInfo = info;
        _compatInfoTarget = new Point(pointerX - (InfoWidth / 2), y);
        _compatInfoTargetWeight = 1;

        if (_compatInfoWeight <= 0.004)
        {
            // A table that is not on screen comes in where it is going, so it fades in in place rather
            // than sliding in from wherever the last one left.
            _compatInfoAt = _compatInfoTarget;
        }

        StartCompatInfo();
    }

    /// <summary>Says that the table is leaving.</summary>
    private void HideCompatInfo()
    {
        _compatInfoTargetWeight = 0;
        StartCompatInfo();
    }

    /// <summary>Takes the table away at once, for a state that has none to show.</summary>
    private void DropCompatInfo()
    {
        _compatInfoGlide.Stop();
        _compatInfo = null;
        _compatInfoWeight = 0;
        _compatInfoTargetWeight = 0;
    }

    /// <summary>Starts the table's fade and travel, or puts it where it is going when motion is off.</summary>
    /// <remarks>
    /// The travel is the one surface kept in reduced motion as well: a table that jumped between rows is
    /// harder to follow than one that moves, and the reader asked for both levels to move it. Turning
    /// motion off is what makes it arrive at once.
    /// </remarks>
    private void StartCompatInfo()
    {
        if (_motion.FadeTime(_motion.Feedback) <= TimeSpan.Zero)
        {
            _compatInfoWeight = _compatInfoTargetWeight;
            _compatInfoAt = _compatInfoTarget;

            if (_compatInfoWeight <= 0)
            {
                _compatInfo = null;
            }

            InvalidateVisual();

            return;
        }

        if (!_compatInfoGlide.IsEnabled)
        {
            _compatInfoGlide.Start();
        }
    }

    /// <summary>Takes one frame of the table's fade and travel.</summary>
    /// <param name="sender">What raised the frame.</param>
    /// <param name="e">The frame's arguments.</param>
    private void CompatInfoGlided(object? sender, EventArgs e)
    {
        _compatInfoWeight += (_compatInfoTargetWeight - _compatInfoWeight) * CompatInfoPart;

        var dx = _compatInfoTarget.X - _compatInfoAt.X;
        var dy = _compatInfoTarget.Y - _compatInfoAt.Y;

        _compatInfoAt = new Point(
            _compatInfoAt.X + (dx * CompatInfoPart),
            _compatInfoAt.Y + (dy * CompatInfoPart)
        );

        if (
            Math.Abs(_compatInfoTargetWeight - _compatInfoWeight) < 0.01
            && Math.Abs(dx) < 0.4
            && Math.Abs(dy) < 0.4
        )
        {
            _compatInfoWeight = _compatInfoTargetWeight;
            _compatInfoAt = _compatInfoTarget;
            _compatInfoGlide.Stop();

            if (_compatInfoWeight <= 0)
            {
                _compatInfo = null;
                _compatInfoWeight = 0;
            }
        }

        InvalidateVisual();
    }

    /// <summary>What the pill standing in place of the entries left out says.</summary>
    /// <param name="row">The row.</param>
    /// <returns>The table.</returns>
    private Info MoreInfo(CompatRow row) =>
        new(
            RolaI18N.Get("rorolala_vcs.version_graph.compat_more"),
            [
                .. row.More.Select(chip => (
                    chip.Tag ?? string.Empty,
                    chip.Entry?.Path ?? chip.Name
                )),
            ],
            CurrentInk().Muted
        );

    /// <summary>Draws what is said in place of the list, centred.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="said">What to say.</param>
    private void DrawCompatState(DrawingContext context, GraphInk ink, string said)
    {
        var text = CompatText(
            said,
            13,
            ink.Muted,
            room: Math.Max(1, Bounds.Width - 80)
        );

        context.DrawText(
            text,
            new Point(
                Math.Max(0, (Bounds.Width - text.Width) / 2),
                Math.Max(0, (Bounds.Height - text.Height) / 2)
            )
        );
    }

    /// <summary>Draws the rows that stand in while the chain is read.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawCompatLoading(DrawingContext context, GraphInk ink)
    {
        var said = CompatText(
            RolaI18N.Get("rorolala_vcs.version_graph.compat_loading"),
            12.5,
            ink.Muted
        );
        var top = Math.Max(24, (CompatListHeight - (5 * CompatChain.RowHeight)) / 2);

        context.DrawText(
            said,
            new Point((Bounds.Width - said.Width) / 2, top)
        );

        var bar = new SolidColorBrush(GraphTint.Alpha(ink.Text, 14));

        for (var line = 0; line < 5; line++)
        {
            var y =
                top
                + said.Height
                + 18
                + (line * CompatChain.RowHeight)
                + ((CompatChain.RowHeight - 10) / 2);

            context.DrawRectangle(bar, null, new Rect(14, y, 120, 10), 5, 5);
            context.DrawRectangle(bar, null, new Rect(186, y, 200, 10), 5, 5);
            context.DrawRectangle(bar, null, new Rect(398, y, 60, 10), 5, 5);
        }
    }

    /// <summary>Draws that the chain could not be read, and the way to read it again.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    private void DrawCompatFailed(DrawingContext context, GraphInk ink)
    {
        var said = CompatText(
            RolaI18N.Get("rorolala_vcs.version_graph.compat_failed"),
            13,
            ink.Muted
        );
        var button = CompatRetryRect();
        var label = CompatText(
            RolaI18N.Get("rorolala_vcs.status.refresh"),
            12,
            ink.Text
        );

        context.DrawText(
            said,
            new Point(
                Math.Max(0, (Bounds.Width - said.Width) / 2),
                Math.Max(0, (Bounds.Height - said.Height) / 2) - 22
            )
        );
        context.DrawRectangle(
            new SolidColorBrush(ink.ChipFill),
            new Pen(new SolidColorBrush(ink.ChipEdge), 1),
            button,
            6,
            6
        );
        context.DrawText(
            label,
            new Point(
                button.X + ((button.Width - label.Width) / 2),
                button.Y + ((button.Height - label.Height) / 2)
            )
        );
    }

    /// <summary>Where the button that reads the chain again stands.</summary>
    /// <returns>The button's rectangle.</returns>
    private Rect CompatRetryRect()
    {
        const double width = 96;
        const double height = 28;

        return new Rect(
            (Bounds.Width - width) / 2,
            (Bounds.Height / 2) + 6,
            width,
            height
        );
    }

    /// <summary>What a lane or a node of a kind is drawn in.</summary>
    /// <param name="ink">The colours in force.</param>
    /// <param name="kind">The kind.</param>
    /// <returns>The colour.</returns>
    private Color CompatRailInk(GraphInk ink, CompatRailKind kind) => kind switch
    {
        CompatRailKind.Modify => _palette.Semantic.Modify,
        CompatRailKind.Error => _palette.Semantic.Error,
        CompatRailKind.Behind => GraphTint.Mix(ink.Primary, ink.Ground, 0.62),
        _ => ink.Primary,
    };

    /// <summary>Shapes a piece of the list's text.</summary>
    /// <param name="said">What it says.</param>
    /// <param name="size">Its size.</param>
    /// <param name="colour">Its colour.</param>
    /// <param name="italic">Whether it is set in italics.</param>
    /// <param name="bold">Whether it is set bold.</param>
    /// <param name="extraBold">Whether it is set extra bold.</param>
    /// <param name="mono">Whether it is set in the hash face.</param>
    /// <param name="semibold">Whether it is set semi bold.</param>
    /// <param name="room">How wide it may be before it is cut short.</param>
    /// <returns>The shaped text.</returns>
    private static FormattedText CompatText(
        string said,
        double size,
        Color colour,
        bool italic = false,
        bool bold = false,
        bool extraBold = false,
        bool mono = false,
        bool semibold = false,
        double? room = null
    )
    {
        var face = mono
            ? new Typeface(GraphMeasure.Mono)
            : new Typeface(
                FontFamily.Default,
                italic ? FontStyle.Italic : FontStyle.Normal,
                extraBold
                    ? FontWeight.ExtraBold
                    : bold
                        ? FontWeight.Bold
                        : semibold
                            ? FontWeight.SemiBold
                            : FontWeight.Normal
            );

        var text = new FormattedText(
            said,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            face,
            size,
            new SolidColorBrush(colour)
        );

        if (room is { } width)
        {
            text.MaxTextWidth = Math.Max(1, width);
            text.MaxLineCount = 1;
            text.Trimming = TextTrimming.CharacterEllipsis;
        }

        return text;
    }

    /// <summary>What the pointer is on in the list, or nothing.</summary>
    /// <param name="at">The point.</param>
    /// <returns>The row, the pill under the pointer, whether it is the more pill, and the part of the row.</returns>
    private (int Row, int Pill, bool More, HoverPart Part)? CompatHit(Point at)
    {
        if (_compatChain is not { } chain || at.Y < CompatChain.HeaderHeight)
        {
            return null;
        }

        var index = (int)Math.Floor((at.Y - CompatChain.HeaderHeight + _compatScroll) / CompatChain.RowHeight);

        if (index < 0 || index >= chain.Rows.Count)
        {
            return null;
        }

        var columns = CompatColumns.Of(Bounds.Size);
        var row = chain.Rows[index];

        if (at.X >= columns.References && at.X < columns.Graph)
        {
            var left = columns.References;

            for (var pill = 0; pill < row.Pills.Count; pill++)
            {
                if (at.X >= left && at.X <= left + CompatChain.PillWidth)
                {
                    return (index, pill, false, HoverPart.EntryPill);
                }

                left += CompatChain.PillWidth + CompatChain.PillGap;
            }

            return row.MoreShown && at.X >= left && at.X <= left + CompatChain.MoreWidth
                ? (index, -1, true, HoverPart.EntryPill)
                : (index, -1, false, HoverPart.Card);
        }

        if (at.X < columns.Message)
        {
            return (index, -1, false, HoverPart.Card);
        }

        if (at.X < columns.Creator)
        {
            return (index, -1, false, HoverPart.Message);
        }

        if (at.X < columns.Local)
        {
            return (index, -1, false, HoverPart.Creator);
        }

        if (at.X < columns.VersionHash)
        {
            return (index, -1, false, HoverPart.Card);
        }

        return (index, -1, false, HoverPart.Number);
    }

    /// <summary>Reads what the pointer is on in the list into the hover the rest of the view reads.</summary>
    /// <param name="at">The point.</param>
    private void CompatPoint(Point at)
    {
        if (
            _compatChain is not { } chain
            || CompatHit(at) is not { } hit
            || hit.Row >= chain.Rows.Count
        )
        {
            _compatHoverRow = null;
            _compatHoverMore = false;
            _hover = null;
            HideCompatInfo();

            return;
        }

        var row = chain.Rows[hit.Row];

        _compatHoverRow = hit.Row;
        _compatHoverMore = hit.More;

        if (hit.Pill >= 0 && hit.Pill < row.Pills.Count)
        {
            var chip = row.Pills[hit.Pill];

            _hover = new Hover(
                row.Card,
                chip.Kind == GraphChipKind.File ? HoverPart.FilePill : HoverPart.EntryPill,
                chip,
                null,
                -1
            );
        }
        else
        {
            _hover = new Hover(row.Card, hit.Part, null, null, -1);
        }

        UpdateCompatInfo(hit.Row, at.X);
    }

    /// <summary>Selects a row and brings it into view, or takes the selection away.</summary>
    /// <param name="index">The row's place, or nothing to take the selection away.</param>
    private void CompatSelect(int? index)
    {
        _compatSelected = index;

        if (_compatChain is { } chain && index is { } picked)
        {
            var top = picked * CompatChain.RowHeight;

            if (top < _compatScroll)
            {
                _compatScroll = top;
            }
            else if (top + CompatChain.RowHeight > _compatScroll + CompatListHeight)
            {
                _compatScroll = Math.Clamp(
                    top - CompatListHeight + CompatChain.RowHeight,
                    0,
                    CompatMaxScroll(chain)
                );
            }
        }

        InvalidateVisual();
    }

    /// <summary>Takes a key that walks the list, or leaves it to the base class.</summary>
    /// <param name="e">The key.</param>
    /// <returns>Whether the list read it.</returns>
    private bool CompatKey(KeyEventArgs e)
    {
        if (_compatChain is not { } chain || chain.Rows.Count == 0)
        {
            return false;
        }

        var count = chain.Rows.Count;
        var page = Math.Max(1, (int)(CompatListHeight / CompatChain.RowHeight));

        switch (e.Key)
        {
            case Key.Up:
                CompatSelect(Math.Max(0, (_compatSelected ?? count) - 1));

                return true;

            case Key.Down:
                CompatSelect(Math.Min(count - 1, (_compatSelected ?? -1) + 1));

                return true;

            case Key.Home:
                CompatSelect(0);

                return true;

            case Key.End:
                CompatSelect(count - 1);

                return true;

            case Key.PageUp:
                CompatSelect(Math.Max(0, (_compatSelected ?? 0) - page));

                return true;

            case Key.PageDown:
                CompatSelect(Math.Min(count - 1, (_compatSelected ?? -1) + page));

                return true;

            case Key.Escape when _compatSelected is not null:
                CompatSelect(null);

                return true;

            case Key.Enter when _compatSelected is { } picked && picked < count:
                ShowMenu(new Hover(chain.Rows[picked].Card, HoverPart.Card, null, null, -1));

                return true;

            default:
                return false;
        }
    }

    /// <summary>Takes a press in the list: a row is selected, a menu is opened, or the reading is asked for again.</summary>
    /// <param name="e">The press.</param>
    /// <param name="point">What was pressed.</param>
    /// <param name="at">Where it was pressed.</param>
    private void CompatPressed(PointerPressedEventArgs e, PointerPoint point, Point at)
    {
        if (_compatFailed && CompatRetryRect().Contains(at))
        {
            RefreshWithFetch();
            e.Handled = true;

            return;
        }

        if (CompatHit(at) is not { } hit)
        {
            if (point.Properties.IsLeftButtonPressed)
            {
                CompatSelect(null);
            }

            HideCompatInfo();

            return;
        }

        CompatPoint(at);
        CompatSelect(hit.Row);

        if (point.Properties.IsRightButtonPressed)
        {
            if (_hover is { } hover)
            {
                ShowMenu(hover);
            }

            e.Handled = true;

            return;
        }

        // A left press is a choice of the row. A pill naming the file is also the way to that file: the
        // press makes it the choice, as the same pill on a card does, so the list follows the reader into
        // it and the drawing stands on it.
        if (point.Properties.IsLeftButtonPressed)
        {
            if (_hover is { Part: HoverPart.EntryPill or HoverPart.FilePill } pressed)
            {
                RevealOnClick(pressed);
            }

            e.Handled = true;
        }
    }

    /// <summary>Where the columns of the list stand.</summary>
    private sealed record CompatColumns(
        double References,
        double Graph,
        double Message,
        double Creator,
        double CreatorWidth,
        double Local,
        double VersionHash,
        double VariantHash,
        double Version
    )
    {
        /// <summary>Reads the columns for a width.</summary>
        /// <param name="size">The room the list stands in.</param>
        /// <returns>The columns.</returns>
        internal static CompatColumns Of(Size size)
        {
            const double left = 14;
            const double right = 18;
            const double fixedWidths =
                CompatChain.ReferencesWidth
                + CompatChain.GraphWidth
                + CompatChain.MessageWidth
                + CompatChain.LocalWidth
                + CompatChain.VersionHashWidth
                + CompatChain.VariantHashWidth
                + CompatChain.VersionWidth;

            var creator = Math.Max(0, size.Width - left - right - fixedWidths);
            var references = left;
            var graph = references + CompatChain.ReferencesWidth;
            var message = graph + CompatChain.GraphWidth;
            var creatorColumn = message + CompatChain.MessageWidth;
            var local = creatorColumn + creator;
            var versionHash = local + CompatChain.LocalWidth;
            var variantHash = versionHash + CompatChain.VersionHashWidth;

            return new CompatColumns(
                References: references,
                Graph: graph,
                Message: message,
                Creator: creatorColumn,
                CreatorWidth: creator,
                Local: local,
                VersionHash: versionHash,
                VariantHash: variantHash,
                Version: variantHash + CompatChain.VariantHashWidth
            );
        }
    }
}

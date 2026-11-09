using System.Globalization;

using Avalonia;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>Gives one colour from two, and one with an alpha.</summary>
internal static class GraphTint
{
    /// <summary>A colour part way from one to another.</summary>
    /// <param name="from">The colour at <paramref name="part"/> zero.</param>
    /// <param name="to">The colour at <paramref name="part"/> one.</param>
    /// <param name="part">How far to move from one to the other, from 0 to 1.</param>
    /// <returns>The mixed colour, opaque.</returns>
    internal static Color Mix(Color from, Color to, double part) => Color.FromRgb(
        (byte)Math.Round(from.R + ((to.R - from.R) * part)),
        (byte)Math.Round(from.G + ((to.G - from.G) * part)),
        (byte)Math.Round(from.B + ((to.B - from.B) * part))
    );

    /// <summary>The same colour, with another alpha.</summary>
    /// <param name="colour">The colour.</param>
    /// <param name="alpha">The alpha it is to have.</param>
    /// <returns>The colour.</returns>
    internal static Color Alpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);

    /// <summary>A colour as the one number a text cache is keyed by.</summary>
    /// <param name="colour">The colour.</param>
    /// <returns>Its four channels, packed.</returns>
    internal static uint Key(Color colour) =>
        ((uint)colour.A << 24) | ((uint)colour.R << 16) | ((uint)colour.G << 8) | colour.B;
}

/// <summary>The colours the graph is drawn in, read from the look in force.</summary>
/// <param name="Ground">The surface the graph stands on.</param>
/// <param name="Card">A card's fill.</param>
/// <param name="CardEdge">A card's border.</param>
/// <param name="Text">A card's message.</param>
/// <param name="Muted">What is said beside the content.</param>
/// <param name="Faint">What is said furthest beside it.</param>
/// <param name="GridMajor">The grid's heavier lines.</param>
/// <param name="GridMinor">The grid's lighter lines.</param>
/// <param name="BaseEdge">The colour of a chain edge.</param>
/// <param name="JoinEdge">The colour of a recorded merge.</param>
/// <param name="Local">The side being worked in.</param>
/// <param name="Origin">A Vault the work came from.</param>
/// <param name="Studio">A second Vault's side, whose name the drawing keeps for itself.</param>
/// <param name="Pending">A merge in progress.</param>
/// <param name="Danger">A change that must not be recorded.</param>
/// <param name="Same">Two variants made of the same content.</param>
/// <param name="Primary">The look's primary.</param>
/// <param name="ChipFill">A chip's fill.</param>
/// <param name="ChipEdge">A chip's border.</param>
/// <param name="ChipHashFill">The hash chip's fill.</param>
/// <param name="ChipHashEdge">The hash chip's border.</param>
/// <param name="HashText">The words of a hash chip.</param>
/// <param name="BubbleFill">The fill of a hover bubble.</param>
/// <param name="BubbleEdge">Its border.</param>
/// <param name="GhostFill">The fill of a missing base version.</param>
/// <param name="GhostEdge">Its border.</param>
/// <param name="Shadow">The shadow under a card or a bubble.</param>
/// <param name="Light">Whether the look is a light one.</param>
internal sealed record GraphInk(
    Color Ground,
    Color Card,
    Color CardEdge,
    Color Text,
    Color Muted,
    Color Faint,
    Color GridMajor,
    Color GridMinor,
    Color BaseEdge,
    Color JoinEdge,
    Color Local,
    Color Origin,
    Color Studio,
    Color Pending,
    Color Danger,
    Color Same,
    Color Primary,
    Color ChipFill,
    Color ChipEdge,
    Color ChipHashFill,
    Color ChipHashEdge,
    Color HashText,
    Color BubbleFill,
    Color BubbleEdge,
    Color GhostFill,
    Color GhostEdge,
    Color Shadow,
    bool Light
)
{
    /// <summary>The colours a look draws the graph in.</summary>
    /// <param name="palette">The look's own colours, read when this is called so a changed one is used.</param>
    /// <returns>The colours.</returns>
    internal static GraphInk From(IPalette palette)
    {
        ArgumentNullException.ThrowIfNull(palette);

        var ground = palette.Dark;
        var card = palette.Bright;
        var front = palette.Front;
        var splitter = palette.Splitter;
        var light = (ground.R + ground.G + ground.B) > (3 * 0x80);

        return new GraphInk(
            Ground: ground,
            Card: card,
            CardEdge: GraphTint.Mix(splitter, front, 0.22),
            Text: front,
            Muted: GraphTint.Mix(front, ground, 0.45),
            Faint: GraphTint.Mix(front, ground, 0.28),
            // A neutral grid at the strength of a hairline, because the graph is a well in the dock
            // rather than a second program: a coloured grid made the canvas the one surface in the
            // window drawn in a colour of its own. It is still there, and only just.
            GridMajor: GraphTint.Alpha(splitter, light ? (byte)14 : (byte)18),
            GridMinor: GraphTint.Alpha(splitter, light ? (byte)7 : (byte)9),
            BaseEdge: GraphTint.Mix(front, ground, light ? 0.34 : 0.18),
            JoinEdge: GraphTint.Mix(front, ground, light ? 0.44 : 0.30),
            Local: palette.Primary,
            Origin: Color.FromRgb(0x2A, 0xA6, 0xA6),
            Studio: Color.FromRgb(0xA7, 0x8B, 0xFA),
            Pending: Color.FromRgb(0xF5, 0x9E, 0x0B),
            Danger: Color.FromRgb(0xF8, 0x71, 0x71),
            Same: Color.FromRgb(0x22, 0xD3, 0xEE),
            Primary: palette.Primary,
            ChipFill: GraphTint.Mix(card, front, light ? 0.05 : 0.09),
            ChipEdge: GraphTint.Mix(splitter, front, 0.18),
            ChipHashFill: GraphTint.Mix(card, front, light ? 0.08 : 0.13),
            ChipHashEdge: GraphTint.Mix(splitter, front, 0.26),
            HashText: GraphTint.Mix(front, ground, light ? 0.45 : 0.32),
            BubbleFill: card,
            BubbleEdge: GraphTint.Mix(splitter, front, 0.22),
            GhostFill: GraphTint.Mix(card, ground, 0.55),
            GhostEdge: GraphTint.Mix(splitter, front, 0.32),
            Shadow: Color.FromArgb(light ? (byte)26 : (byte)110, 0, 0, 0),
            Light: light
        );
    }

    /// <summary>The colour a side is drawn with.</summary>
    /// <param name="side">The side's name: <c>local</c>, or a Vault's name.</param>
    /// <returns>The colour.</returns>
    internal Color Side(string side) => side switch
    {
        "local" => Local,
        "origin" => Origin,
        "studio" => Studio,
        _ => GraphHue.For(side, 0.58, Light ? 0.45 : 0.62),
    };
}

/// <summary>Where the graph is drawn: how far it is moved and scaled, and what the pointer is on.</summary>
/// <param name="Zoom">How large the content units are drawn.</param>
/// <param name="Pan">Where the content origin stands, in the control's coordinates.</param>
/// <param name="Hover">The card the pointer is on, when it is on one.</param>
/// <param name="Lineage">
/// The cards to keep in full when the pointer is on one: what it came from, what came from it, and what
/// it merges with. Everything else is drawn faint.
/// </param>
/// <param name="Stats">The line the graph is counted in.</param>
/// <param name="Truncated">Whether the reading was cut short, and so is what is drawn.</param>
/// <param name="Emphasized">
/// Whether the graph's focused chains are to be picked out. A graph whose choice has been taken away keeps
/// its focus chains in the drawing, so the same cards stay on screen, but stops emphasising them.
/// </param>
/// <param name="Edge">The edge the pointer is on, when it is on one, which is drawn thicker and on top.</param>
internal sealed record VariantGraphFrame(
    double Zoom,
    Point Pan,
    int? Hover,
    IReadOnlySet<int>? Lineage,
    string Stats,
    bool Truncated,
    bool Emphasized,
    int? Edge
);

/// <summary>
/// Draws a laid-out variant graph, drawing only what the viewport holds.
/// </summary>
/// <remarks>
/// The reading behind a graph is a window of a larger history, and a window can hold hundreds of
/// variants, so nothing outside the visible part of the drawing is measured or drawn: the cards,
/// their pills, the edges between them and the column labels are each culled against the viewport,
/// and the text of a card is only shaped once it is both visible and large enough to read. What has
/// been shaped is kept until the zoom or the look changes, which is what makes a drag cost nothing.
/// <para>
/// The drawing is in content units and the renderer is the one place the zoom is applied, so a
/// change of scale never changes the layout.
/// </para>
/// </remarks>
internal sealed class VariantGraphRenderer
{
    /// <summary>Below this scale card text is not drawn: a pill's words would be a smear of pixels.</summary>
    private const double TextZoom = 0.5;

    /// <summary>Below this scale a card's camera is not drawn, and its pills are bars of colour.</summary>
    private const double DetailZoom = 0.34;

    /// <summary>How near the viewport a thing has to reach to still be drawn.</summary>
    private const double CullMargin = 120;

    /// <summary>The words the layout's edges and cards are named with.</summary>
    private readonly GraphVocabulary _words;

    /// <summary>The text shaped at the zoom in force, until it changes.</summary>
    private readonly Dictionary<(GraphFont Face, string Text, int Size, uint Colour), FormattedText> _texts =
        [];

    /// <summary>The zoom <see cref="_texts"/> was shaped at.</summary>
    private double _shapedAt = double.NaN;

    /// <summary>Makes a renderer.</summary>
    /// <param name="words">The words the layout's edges and cards are named with.</param>
    internal VariantGraphRenderer(GraphVocabulary words)
    {
        ArgumentNullException.ThrowIfNull(words);
        _words = words;
    }

    /// <summary>Lets go of the text shaped for a look that has changed.</summary>
    internal void Invalidate() => _texts.Clear();

    /// <summary>Draws the graph.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="size">How large the control is.</param>
    /// <param name="drawing">The laid-out graph.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn, and what the pointer is on.</param>
    internal void Render(
        DrawingContext context,
        Size size,
        VariantGraphDrawing drawing,
        GraphInk ink,
        VariantGraphFrame frame
    )
    {
        ArgumentNullException.ThrowIfNull(context);
        ArgumentNullException.ThrowIfNull(drawing);
        ArgumentNullException.ThrowIfNull(ink);
        ArgumentNullException.ThrowIfNull(frame);

        if (
            drawing.Cards.Count == 0
            || size.Width <= 0
            || size.Height <= 0
            || frame.Zoom <= 0
        )
        {
            return;
        }

        if (Math.Abs(_shapedAt - frame.Zoom) > 0.0001)
        {
            _texts.Clear();
            _shapedAt = frame.Zoom;
        }

        var visible = new Rect(
            (-frame.Pan.X / frame.Zoom) - CullMargin,
            (-frame.Pan.Y / frame.Zoom) - CullMargin,
            (size.Width / frame.Zoom) + (CullMargin * 2),
            (size.Height / frame.Zoom) + (CullMargin * 2)
        );
        var screen = new Rect(0, 0, size.Width, size.Height);

        var active = Active(drawing, frame);
        var focused = frame.Emphasized && drawing.FocusChain.Count > 0 && frame.Hover is null;

        DrawGrid(context, size, ink, frame);
        DrawEdges(context, ink, frame, drawing, screen, active, frame.Edge);
        DrawGhosts(context, ink, frame, drawing);
        DrawCards(context, ink, frame, drawing, visible, active, focused, frame.Edge);
    }

    /// <summary>What the pointer's lineage is, or what the focused chains are when it is on nothing.</summary>
    /// <param name="drawing">The graph.</param>
    /// <param name="frame">What the frame says.</param>
    /// <returns>The cards to keep in full, or nothing when nothing is emphasised.</returns>
    private static IReadOnlySet<int>? Active(VariantGraphDrawing drawing, VariantGraphFrame frame)
    {
        if (frame.Lineage is { } lineage)
        {
            return lineage;
        }

        if (!frame.Emphasized || drawing.FocusChain.Count == 0)
        {
            return null;
        }

        var active = new HashSet<int>(drawing.FocusChain);
        active.UnionWith(drawing.FocusRelated);

        return active;
    }

    /// <summary>Draws the grid the graph stands on: twenty-unit squares, every other one heavier.</summary>
    /// <remarks>
    /// The squares are content units, so they move and scale with the graph and a card is read against
    /// the same ground at every zoom. Below the scale where they would be a smear of lines the ground
    /// is left plain.
    /// </remarks>
    /// <param name="context">Where to draw.</param>
    /// <param name="size">How large the control is.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    private static void DrawGrid(DrawingContext context, Size size, GraphInk ink, VariantGraphFrame frame)
    {
        var step = 20 * frame.Zoom;

        if (step < 6)
        {
            return;
        }

        var showMinor = step >= 8;
        var minor = new Pen(new SolidColorBrush(ink.GridMinor), 1);
        var major = new Pen(new SolidColorBrush(ink.GridMajor), 1);
        var right = size.Width;
        var bottom = size.Height;

        for (var index = (long)Math.Ceiling(-frame.Pan.X / step); ; index++)
        {
            var x = frame.Pan.X + (index * step);

            if (x > right)
            {
                break;
            }

            if (index % 2 == 0)
            {
                context.DrawLine(major, new Point(x, 0), new Point(x, bottom));
            }
            else if (showMinor)
            {
                context.DrawLine(minor, new Point(x, 0), new Point(x, bottom));
            }
        }

        for (var index = (long)Math.Ceiling(-frame.Pan.Y / step); ; index++)
        {
            var y = frame.Pan.Y + (index * step);

            if (y > bottom)
            {
                break;
            }

            if (index % 2 == 0)
            {
                context.DrawLine(major, new Point(0, y), new Point(right, y));
            }
            else if (showMinor)
            {
                context.DrawLine(minor, new Point(0, y), new Point(right, y));
            }
        }
    }

    /// <summary>Draws the edges: what came of the same content, then the chains, then the merges.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="drawing">The graph.</param>
    /// <param name="screen">The control's own rectangle, which the edges are drawn in.</param>
    /// <param name="active">The cards to keep in full, or nothing when nothing is emphasised.</param>
    /// <param name="hovered">The edge the pointer is on, when it is on one.</param>
    private static void DrawEdges(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        VariantGraphDrawing drawing,
        Rect screen,
        IReadOnlySet<int>? active,
        int? hovered
    )
    {
        for (var index = 0; index < drawing.Links.Count; index++)
        {
            var link = drawing.Links[index];
            var from = drawing.Cards[link.From];
            var to = drawing.Cards[link.To];
            var (start, end, control1, control2, arrow) = GeometryOf(from, to, link.Kind, frame);
            var bounds = new Rect(start, end);

            // An edge is a curve, and the curve lies inside the hull of its four points: culling on the
            // two ends alone would drop one that leaves the screen and comes back, which is what a merge
            // across lanes does.
            bounds = bounds.Union(new Rect(control1, control1));
            bounds = bounds.Union(new Rect(control2, control2));

            if (!bounds.Inflate(24).Intersects(screen))
            {
                continue;
            }

            var onPointer = hovered == index;
            var faint = !onPointer && active is not null && (!active.Contains(link.From) || !active.Contains(link.To));
            var (colour, dash, width) = EdgeStyle(link.Kind, ink);

            if (onPointer)
            {
                colour = ink.Primary;
                width = Math.Max(2.4, width * 1.9);
            }

            IDisposable? undo = faint ? context.PushOpacity(0.14) : null;

            var pen = new Pen(new SolidColorBrush(colour), width)
            {
                LineCap = PenLineCap.Round,
                LineJoin = PenLineJoin.Round,
            };

            if (dash is not null)
            {
                pen.DashStyle = dash;
            }

            var path = Curve(start, control1, control2, end);
            context.DrawGeometry(null, pen, path);

            if (arrow)
            {
                context.DrawGeometry(new SolidColorBrush(colour), null, Arrow(end, control2, frame.Zoom));
            }

            undo?.Dispose();
        }
    }

    /// <summary>What an edge is drawn with: its colour, its brokenness and its thickness.</summary>
    /// <param name="kind">What the edge means.</param>
    /// <param name="ink">The colours in force.</param>
    /// <returns>The colour, the dash style when it is broken, and the width.</returns>
    private static (Color Colour, DashStyle? Dash, double Width) EdgeStyle(GraphLinkKind kind, GraphInk ink) =>
        kind switch
        {
            GraphLinkKind.Same => (
                ink.Same,
                new DashStyle([2, 4], 0),
                Math.Max(0.8, 1.1)
            ),
            GraphLinkKind.Base => (ink.BaseEdge, null, Math.Max(0.9, 1.3)),
            GraphLinkKind.Join => (ink.JoinEdge, null, Math.Max(0.9, 1.3)),
            GraphLinkKind.Merging => (ink.Pending, new DashStyle([3.5, 3], 0), Math.Max(1, 1.3)),
            GraphLinkKind.Draft => (
                GraphTint.Mix(ink.Local, ink.Muted, 0.28),
                new DashStyle([3, 3], 0),
                Math.Max(0.9, 1.2)
            ),
            _ => (ink.Danger, new DashStyle([2.5, 2.5], 0), Math.Max(0.9, 1.2)),
        };

    /// <summary>The two ends and the two control points of an edge, in screen units.</summary>
    /// <param name="from">The card the edge is walked from.</param>
    /// <param name="to">The card it reaches.</param>
    /// <param name="kind">What the edge means.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <returns>The start and end points, the two controls, and whether it is arrowed.</returns>
    internal static (Point Start, Point End, Point Control1, Point Control2, bool Arrow) GeometryOf(
        GraphCard from,
        GraphCard to,
        GraphLinkKind kind,
        VariantGraphFrame frame
    )
    {
        var zoom = frame.Zoom;

        if (kind is GraphLinkKind.Join or GraphLinkKind.Merging)
        {
            // A merge leaves where a version's content leaves and arrives where the other's content
            // arrives: the same points every other chain uses, so the line sits in the reading rather
            // than hanging off a card's top or bottom.
            var start = Screen(frame, from.CardRight, from.Y);
            var end = Screen(frame, to.CardLeft, to.Y);
            var pull = Math.Max(
                VariantGraphDrawing.MergeCurveLeast,
                Math.Abs(end.X - start.X) / zoom * 0.3
            ) * zoom;

            return (
                start,
                end,
                new Point(start.X + pull, start.Y),
                new Point(end.X - pull, end.Y),
                true
            );
        }

        if (kind == GraphLinkKind.Same)
        {
            var a = Screen(frame, from.X, from.Y);
            var b = Screen(frame, to.X, to.Y);
            var middle = new Point(
                (a.X + b.X) / 2,
                ((a.Y + b.Y) / 2) - (34 * zoom)
            );

            return (a, b, middle, middle, false);
        }

        var left = Screen(frame, from.CardRight, from.Y);
        var right = Screen(frame, to.CardLeft, to.Y);
        var bend = Math.Max(
            VariantGraphDrawing.CurveLeast,
            Math.Abs(right.X - left.X) / zoom * 0.38
        ) * zoom;

        return (
            left,
            right,
            new Point(left.X + bend, left.Y),
            new Point(right.X - bend, right.Y),
            kind is not GraphLinkKind.Same
        );
    }

    /// <summary>Draws the base versions the index does not hold, and the edges from them.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="drawing">The graph.</param>
    private void DrawGhosts(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        VariantGraphDrawing drawing
    )
    {
        foreach (var ghost in drawing.Ghosts)
        {
            var at = Screen(frame, ghost.X, ghost.Y);
            var target = drawing.Cards[ghost.Target];
            var radius = 18 * frame.Zoom;
            var pen = new Pen(new SolidColorBrush(ink.GhostEdge), Math.Max(0.9, 1.2 * frame.Zoom))
            {
                DashStyle = new DashStyle([3, 3], 0),
            };

            context.DrawEllipse(
                new SolidColorBrush(ink.GhostFill),
                pen,
                at,
                radius,
                radius
            );

            if (frame.Zoom >= DetailZoom)
            {
                var mark = Text(GraphFont.Name, "?", ink.Muted, frame.Zoom);
                context.DrawText(
                    mark,
                    new Point(at.X - (mark.Width / 2), at.Y - (mark.Baseline / 2))
                );

                var label = Text(GraphFont.Tag, _words.UnknownBase, ink.Faint, frame.Zoom);
                context.DrawText(
                    label,
                    new Point(at.X - (label.Width / 2), at.Y - radius - (14 * frame.Zoom))
                );
            }

            var end = Screen(frame, target.CardLeft, target.Y);
            var start = new Point(at.X + radius, at.Y);
            var path = Curve(
                start,
                new Point(start.X + (40 * frame.Zoom), start.Y),
                new Point(end.X - (40 * frame.Zoom), end.Y),
                end
            );

            context.DrawGeometry(
                null,
                new Pen(new SolidColorBrush(ink.GhostEdge), Math.Max(0.8, 1 * frame.Zoom))
                {
                    DashStyle = new DashStyle([3, 4], 0),
                },
                path
            );
        }
    }

    /// <summary>Draws the cards the viewport holds, and nothing else.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="drawing">The graph.</param>
    /// <param name="visible">The content the viewport holds.</param>
    /// <param name="active">The cards to keep in full, or nothing when nothing is emphasised.</param>
    /// <param name="focused">Whether a focused chain is being emphasised.</param>
    /// <param name="hoveredEdge">The edge the pointer is on, when it is on one.</param>
    private void DrawCards(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        VariantGraphDrawing drawing,
        Rect visible,
        IReadOnlySet<int>? active,
        bool focused,
        int? hoveredEdge
    )
    {
        HashSet<int>? onTop = null;

        if (hoveredEdge is int index && index >= 0 && index < drawing.Links.Count)
        {
            var link = drawing.Links[index];
            onTop = [link.From, link.To];
        }

        foreach (var card in drawing.Cards)
        {
            if (onTop?.Contains(card.Index) is true || !CardVisible(card, visible))
            {
                continue;
            }

            DrawOneCard(context, ink, frame, drawing, card, active, focused, endpoint: false);
        }

        // The two ends of the edge under the pointer are drawn after the rest of the graph, so the edge
        // they belong to is read against them rather than under the cards that happen to cover them.
        if (onTop is null)
        {
            return;
        }

        foreach (var card in drawing.Cards)
        {
            if (!onTop.Contains(card.Index) || !CardVisible(card, visible))
            {
                continue;
            }

            DrawOneCard(
                context,
                ink,
                frame,
                drawing,
                card,
                active: null,
                focused: false,
                endpoint: true
            );
        }
    }

    /// <summary>Whether a card intersects the part of the drawing the viewport holds.</summary>
    /// <param name="card">The card.</param>
    /// <param name="visible">The viewport, in content units.</param>
    /// <returns>Whether it is drawn.</returns>
    private static bool CardVisible(GraphCard card, Rect visible) =>
        new Rect(
            card.CardLeft,
            card.CardTop,
            VariantGraphDrawing.CardWidth,
            (card.ContentBottom - card.CardTop) + 4
        ).Intersects(visible);

    /// <summary>Draws one card with the emphasis the focus and pointer give it.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="drawing">The graph.</param>
    /// <param name="card">The card.</param>
    /// <param name="active">The cards to keep in full, or nothing when nothing is emphasised.</param>
    /// <param name="focused">Whether a focused chain is being emphasised.</param>
    /// <param name="endpoint">Whether it is one end of the edge under the pointer.</param>
    private void DrawOneCard(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        VariantGraphDrawing drawing,
        GraphCard card,
        IReadOnlySet<int>? active,
        bool focused,
        bool endpoint
    )
    {
        var faint = active is not null && !active.Contains(card.Index);
        var onChain = focused && drawing.FocusChain.Contains(card.Index);
        var beside = focused && !onChain && drawing.FocusRelated.Contains(card.Index);
        var hovered = frame.Hover == card.Index;
        IDisposable? undo = faint ? context.PushOpacity(0.26) : null;

        DrawCard(context, ink, frame, card, onChain, beside, hovered, endpoint);

        undo?.Dispose();
    }

    /// <summary>Draws one card: its shape, its content and its pills.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="card">The card.</param>
    /// <param name="onChain">Whether it is on a focused entry's own chain.</param>
    /// <param name="beside">Whether it stands beside such a chain.</param>
    /// <param name="hovered">Whether the pointer is on it.</param>
    /// <param name="endpoint">Whether it is one end of the edge under the pointer.</param>
    private void DrawCard(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        GraphCard card,
        bool onChain,
        bool beside,
        bool hovered,
        bool endpoint
    )
    {
        var zoom = frame.Zoom;
        var left = frame.Pan.X + (card.CardLeft * zoom);
        var top = frame.Pan.Y + (card.CardTop * zoom);
        var right = frame.Pan.X + (card.CardRight * zoom);
        var bottom = frame.Pan.Y + (card.Y * zoom) + (VariantGraphDrawing.CardHeight / 2 * zoom);
        var width = right - left;
        var height = bottom - top;
        var radius = 8 * zoom;
        var (fill, stroke, dash, thickness) = CardStyle(card, ink, onChain);

        if (endpoint)
        {
            stroke = ink.Primary;
            dash = null;
            thickness = Math.Max(1.8, 2.2 * zoom);
        }
        else if (hovered)
        {
            stroke = ink.Text;
            dash = null;
            thickness = Math.Max(1.2, 1.8 * zoom);
        }
        else if (beside && !onChain)
        {
            stroke = GraphTint.Mix(ink.Muted, ink.CardEdge, 0.4);
            dash = new DashStyle([3, 2.5], 0);
        }

        if (zoom >= DetailZoom)
        {
            context.DrawRectangle(
                new SolidColorBrush(ink.Shadow),
                null,
                new Rect(left, top + (4 * zoom), width, height),
                radius,
                radius
            );
        }

        var pen = new Pen(new SolidColorBrush(stroke), thickness);

        if (dash is not null)
        {
            pen.DashStyle = dash;
        }

        context.DrawRectangle(new SolidColorBrush(fill), pen, new Rect(left, top, width, height), radius, radius);

        // The file's colour runs down the card's left edge, so a lane is readable from any card of it.
        // It is a short line rather than the whole edge: a band of colour beside the content would
        // compete with the pills, and what says the lane is the colour, not its length.
        if (zoom >= DetailZoom)
        {
            var stripe = (VariantGraphDrawing.CardHeight - 18) * 0.7;

            context.DrawRectangle(
                new SolidColorBrush(card.LaneColour),
                null,
                new Rect(
                    left + (2 * zoom),
                    top + (((VariantGraphDrawing.CardHeight - stripe) / 2) * zoom),
                    Math.Max(1, 2.5 * zoom),
                    stripe * zoom
                ),
                Math.Max(0.5, 1.2 * zoom),
                Math.Max(0.5, 1.2 * zoom)
            );
        }

        DrawBigLabel(context, ink, frame, card, onChain, right, top, height);

        if (zoom >= TextZoom)
        {
            DrawContent(context, ink, frame, card, left, top);
        }

        if (zoom >= DetailZoom)
        {
            DrawPills(context, ink, frame, card, left, top);
        }
    }

    /// <summary>What a card's shape is: its fill, its border, whether the border is broken and how thick.</summary>
    /// <param name="card">The card.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="onChain">Whether it is on the focused chain.</param>
    /// <returns>The fill, the border, its dash style and its thickness.</returns>
    private static (Color Fill, Color Stroke, DashStyle? Dash, double Width) CardStyle(
        GraphCard card,
        GraphInk ink,
        bool onChain
    )
    {
        if (card.Kind == GraphCardKind.Draft)
        {
            var wrong = card.Draft is { Correct: false };

            return (
                GraphTint.Mix(ink.Card, wrong ? ink.Danger : ink.Local, wrong ? 0.11 : 0.07),
                GraphTint.Mix(wrong ? ink.Danger : ink.Local, ink.CardEdge, wrong ? 0.68 : 0.62),
                new DashStyle([3, 2.5], 0),
                1.1
            );
        }

        if (card.Pending)
        {
            return (
                GraphTint.Mix(ink.Card, ink.Pending, 0.14),
                ink.Pending,
                new DashStyle([3.5, 2.5], 0),
                1.2
            );
        }

        if (onChain)
        {
            return (ink.Card, ink.Local, null, 1.8);
        }

        if (card.Number is null && !card.MissingBase)
        {
            return (ink.Card, ink.CardEdge, new DashStyle([2.5, 2.5], 0), 1.1);
        }

        if (card.MissingBase)
        {
            return (ink.Card, ink.CardEdge, new DashStyle([2.5, 2.5], 0), 1.1);
        }

        return (ink.Card, ink.CardEdge, null, 1.1);
    }

    /// <summary>Draws the big, faint version number that stands behind a card's content.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="card">The card.</param>
    /// <param name="onChain">Whether it is on a focused entry's chain.</param>
    /// <param name="right">The card's right edge, in screen units.</param>
    /// <param name="top">The card's top edge, in screen units.</param>
    /// <param name="height">The card's height, in screen units.</param>
    private void DrawBigLabel(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        GraphCard card,
        bool onChain,
        double right,
        double top,
        double height
    )
    {
        var zoom = frame.Zoom;
        var said = card.Kind == GraphCardKind.Draft
            ? card.Label
            : VariantGraphDrawing.VersionLabel(card);
        var colour = card.Kind == GraphCardKind.Draft
            ? (card.Draft is { Correct: false } ? ink.Danger : ink.Local)
            : card.Pending
                ? ink.Pending
                : onChain
                    ? ink.Local
                    : card.Number is null
                        ? ink.Muted
                        : ink.Text;
        var alpha = card.Kind == GraphCardKind.Draft
            ? (byte)(card.Draft is { Correct: false } ? 46 : 42)
            : card.Pending
                ? (byte)40
                : onChain
                    ? (byte)32
                    : card.Number is null
                        ? (byte)30
                        : (byte)24;

        var text = Text(GraphFont.Big, said, GraphTint.Alpha(colour, alpha), zoom);
        context.DrawText(
            text,
            new Point(
                right - (12 * zoom) - text.Width,
                top + height - (10 * zoom) - text.Baseline
            )
        );
    }

    /// <summary>Draws the card's first row: the creator's name and the message.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="card">The card.</param>
    /// <param name="left">The card's left edge, in screen units.</param>
    /// <param name="top">The card's top edge, in screen units.</param>
    private void DrawContent(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        GraphCard card,
        double left,
        double top
    )
    {
        var zoom = frame.Zoom;
        var middle = top + (VariantGraphDrawing.CardHeight / 2 * zoom);

        // The creator's name is written as text rather than in a pill of its own: the card has one pill
        // row already, and a second chip in the first row would read as another fact about the version
        // instead of the name that signed it. It is metadata, so it takes the second ink and lets the
        // message carry the card's own words.
        var nameLeft = left + (13 * zoom);
        var name = Text(GraphFont.Creator, card.Creator, ink.Muted, zoom);
        context.DrawText(name, new Point(nameLeft, middle + (4 * zoom) - name.Baseline));

        if (card.Message.Length == 0)
        {
            return;
        }

        var textLeft = left + ((23 + card.CreatorWidth) * zoom);
        var room = (VariantGraphDrawing.CardWidth - 23 - card.CreatorWidth - 14) * zoom;

        if (room <= 4)
        {
            return;
        }

        var message = new FormattedText(
            card.Message,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            GraphMeasure.Face(GraphFont.Message).Typeface,
            13 * zoom,
            new SolidColorBrush(ink.Text)
        )
        {
            MaxTextWidth = room,
            MaxLineCount = 1,
            Trimming = TextTrimming.CharacterEllipsis,
        };

        context.DrawText(message, new Point(textLeft, middle + (4.5 * zoom) - message.Baseline));
    }

    /// <summary>Draws a card's pills, directly under its first row.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="card">The card.</param>
    /// <param name="left">The card's left edge, in screen units.</param>
    /// <param name="top">The card's top edge, in screen units.</param>
    private void DrawPills(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        GraphCard card,
        double left,
        double top
    )
    {
        var zoom = frame.Zoom;
        var y = top + ((VariantGraphDrawing.CardHeight + VariantGraphDrawing.PillGap) * zoom);
        var readable = zoom >= TextZoom;

        foreach (var row in card.Rows)
        {
            var x = left;

            foreach (var chip in row)
            {
                DrawChip(context, ink, frame, card, chip, x, y, readable);
                x += (chip.Width + VariantGraphDrawing.ChipGap) * zoom;
            }

            y += (VariantGraphDrawing.ChipHeight + VariantGraphDrawing.ChipGap) * zoom;
        }
    }

    /// <summary>Draws one pill.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="card">The card it hangs under.</param>
    /// <param name="chip">The pill.</param>
    /// <param name="x">Its left edge, in screen units.</param>
    /// <param name="y">Its top edge, in screen units.</param>
    /// <param name="readable">Whether the zoom is large enough for words.</param>
    private void DrawChip(
        DrawingContext context,
        GraphInk ink,
        VariantGraphFrame frame,
        GraphCard card,
        GraphChip chip,
        double x,
        double y,
        bool readable
    )
    {
        var zoom = frame.Zoom;
        var width = chip.Width * zoom;
        var height = VariantGraphDrawing.ChipHeight * zoom;
        var rect = new Rect(x, y, width, height);
        var radius = 5 * zoom;
        var side = chip.Entry is { } entry ? entry.Side : chip.Kind == GraphChipKind.File ? "local" : null;
        var sideColour = chip.Tracked
            ? ink.Primary
            : side switch
            {
                null => ink.Muted,
                "local" when chip.Chosen => ink.Text,
                _ => ink.Side(side),
            };
        var (fill, stroke, dash) = chip.Kind switch
        {
            GraphChipKind.Hash => (
                ink.ChipHashFill,
                ink.ChipHashEdge,
                (DashStyle?)null
            ),
            GraphChipKind.Badge => (
                Colors.Transparent,
                chip.Wrong ? ink.Danger : chip.Warn ? ink.Pending : ink.ChipEdge,
                new DashStyle([2, 2], 0)
            ),
            _ => (
                ink.ChipFill,
                chip.Tracked ? ink.Primary : GraphTint.Mix(sideColour, ink.ChipEdge, 0.45),
                (DashStyle?)null
            ),
        };
        var pen = new Pen(
            new SolidColorBrush(stroke),
            Math.Max(chip.Tracked ? 1 : 0.7, (chip.Tracked ? 1.2 : 0.9) * zoom)
        );

        if (dash is not null)
        {
            pen.DashStyle = dash;
        }

        context.DrawRectangle(new SolidColorBrush(fill), pen, rect, radius, radius);

        if (!readable)
        {
            return;
        }

        var baseline = y + (13.5 * zoom);

        switch (chip.Kind)
        {
            case GraphChipKind.Hash:
            {
                var text = Text(GraphFont.Hash, chip.Text, ink.HashText, zoom);
                context.DrawText(text, new Point(x + (9 * zoom), baseline - text.Baseline));

                break;
            }

            case GraphChipKind.Badge:
            {
                var colour = chip.Wrong ? ink.Danger : chip.Warn ? ink.Pending : ink.Muted;
                var text = Text(GraphFont.Name, chip.Text, colour, zoom);
                context.DrawText(text, new Point(x + (9 * zoom), baseline - text.Baseline));

                break;
            }

            default:
            {
                var tag = Text(GraphFont.Tag, chip.Tag ?? string.Empty, sideColour, zoom);
                context.DrawText(tag, new Point(x + (7 * zoom), baseline - tag.Baseline));

                var name = Text(
                    GraphFont.Name,
                    chip.Name,
                    chip.Tracked ? ink.Primary : chip.Strike ? ink.Muted : ink.Text,
                    zoom
                );
                var nameX = x + ((8 + chip.TagWidth + 7) * zoom);
                context.DrawText(name, new Point(nameX, baseline - name.Baseline));

                if (chip.Strike)
                {
                    context.DrawLine(
                        new Pen(new SolidColorBrush(ink.Muted), Math.Max(0.7, 0.9 * zoom)),
                        new Point(nameX, baseline - (3 * zoom)),
                        new Point(nameX + name.Width, baseline - (3 * zoom))
                    );
                }

                break;
            }
        }
    }

    /// <summary>The point a piece of the content stands at, in the control's coordinates.</summary>
    /// <param name="frame">Where the graph is drawn.</param>
    /// <param name="x">The content x.</param>
    /// <param name="y">The content y.</param>
    /// <returns>The screen point.</returns>
    private static Point Screen(VariantGraphFrame frame, double x, double y) =>
        new(frame.Pan.X + (x * frame.Zoom), frame.Pan.Y + (y * frame.Zoom));

    /// <summary>Shapes a piece of text at the zoom in force.</summary>
    /// <param name="face">The face it is set in.</param>
    /// <param name="text">The text.</param>
    /// <param name="colour">Its colour.</param>
    /// <param name="zoom">How large content units are drawn.</param>
    /// <returns>The shaped text.</returns>
    private FormattedText Text(GraphFont face, string text, Color colour, double zoom)
    {
        var size = Math.Max(1, (int)Math.Round(GraphMeasure.Face(face).Size * zoom * 10));
        var key = (face, text, size, GraphTint.Key(colour));

        if (_texts.TryGetValue(key, out var known))
        {
            return known;
        }

        var (typeface, _) = GraphMeasure.Face(face);
        var shaped = new FormattedText(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            typeface,
            size / 10.0,
            new SolidColorBrush(colour)
        );

        _texts[key] = shaped;

        return shaped;
    }

    /// <summary>A curve through two control points.</summary>
    /// <param name="start">Where it starts.</param>
    /// <param name="control1">Its first control point.</param>
    /// <param name="control2">Its second control point.</param>
    /// <param name="end">Where it ends.</param>
    /// <returns>The geometry.</returns>
    private static StreamGeometry Curve(Point start, Point control1, Point control2, Point end)
    {
        var geometry = new StreamGeometry();

        using (var path = geometry.Open())
        {
            path.BeginFigure(start, false);
            path.CubicBezierTo(control1, control2, end);
            path.EndFigure(false);
        }

        return geometry;
    }

    /// <summary>The head of an arrow, pointing along the end of a curve.</summary>
    /// <param name="tip">Where the curve ends.</param>
    /// <param name="before">The control point before it.</param>
    /// <param name="zoom">How large content units are drawn.</param>
    /// <returns>The geometry.</returns>
    private static StreamGeometry Arrow(Point tip, Point before, double zoom)
    {
        var length = Math.Max(5, 7 * zoom);
        var half = Math.Max(2.5, 3.5 * zoom);
        var dx = tip.X - before.X;
        var dy = tip.Y - before.Y;
        var span = Math.Sqrt((dx * dx) + (dy * dy));

        if (span < 0.01)
        {
            dx = 1;
            dy = 0;
            span = 1;
        }

        dx /= span;
        dy /= span;

        var back = new Point(tip.X - (dx * length), tip.Y - (dy * length));
        var normal = new Point(-dy, dx);
        var geometry = new StreamGeometry();

        using (var path = geometry.Open())
        {
            path.BeginFigure(tip, true);
            path.LineTo(new Point(back.X + (normal.X * half), back.Y + (normal.Y * half)));
            path.LineTo(new Point(back.X - (normal.X * half), back.Y - (normal.Y * half)));
            path.EndFigure(true);
        }

        return geometry;
    }
}

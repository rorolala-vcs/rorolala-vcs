using Avalonia;
using Avalonia.Media;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

internal sealed partial class VersionGraphView
{
    /// <inheritdoc />
    public override void Render(DrawingContext context)
    {
        var ink = CurrentInk();

        context.FillRectangle(
            new SolidColorBrush(ink.Ground),
            new Rect(Bounds.Size)
        );

        if (Bounds.Width <= 0 || Bounds.Height <= 0)
        {
            return;
        }

        if (_drawing is null)
        {
            DrawGrid(context, ink);
            DrawEmpty(context, ink);
            DrawZoom(context, ink);

            return;
        }

        var metrics = CurrentMetrics();

        ClampPan(metrics.MaxRight + Inset * _zoom, _drawing.Height * _zoom);
        DrawGrid(context, ink);
        DrawArcs(context, ink);
        DrawVersions(context, ink, metrics);
        DrawUpstream(context, ink, metrics);
        DrawVariants(context, ink, metrics);
        DrawBubbles(context, ink, metrics);

        if (_hover?.Variant is { } hovered)
        {
            DrawVariantTooltip(context, ink, hovered);
        }
        else if (_hover?.Upstream is { Unknown: true } upstream)
        {
            DrawUpstreamTooltip(context, ink, upstream);
        }

        DrawZoom(context, ink);
    }

    /// <summary>Draws the 20px/40px grid the drawing stands on.</summary>
    private void DrawGrid(DrawingContext context, Ink ink)
    {
        var step = 20 * _zoom;

        // Below this the lines are closer together than they can be read, so the overview is left
        // as a plain ground.
        if (step < 6)
        {
            return;
        }

        var showMinor = step >= 8;
        var minorPen = new Pen(new SolidColorBrush(ink.GridMinor), 1);
        var majorPen = new Pen(new SolidColorBrush(ink.GridMajor), 1);
        var width = Bounds.Width;
        var height = Bounds.Height;

        if (showMinor)
        {
            for (
                var index = (long)Math.Ceiling(-_panX / step);
                ;
                index++
            )
            {
                var x = _panX + index * step;

                if (x > width)
                {
                    break;
                }

                if (index % 2 != 0)
                {
                    context.DrawLine(minorPen, new Point(x, 0), new Point(x, height));
                }
            }

            for (
                var index = (long)Math.Ceiling(-_panY / step);
                ;
                index++
            )
            {
                var y = _panY + index * step;

                if (y > height)
                {
                    break;
                }

                if (index % 2 != 0)
                {
                    context.DrawLine(minorPen, new Point(0, y), new Point(width, y));
                }
            }
        }

        for (
            var index = (long)Math.Ceiling(-_panX / step);
            ;
            index++
        )
        {
            var x = _panX + index * step;

            if (x > width)
            {
                break;
            }

            if (index % 2 == 0)
            {
                context.DrawLine(majorPen, new Point(x, 0), new Point(x, height));
            }
        }

        for (
            var index = (long)Math.Ceiling(-_panY / step);
            ;
            index++
        )
        {
            var y = _panY + index * step;

            if (y > height)
            {
                break;
            }

            if (index % 2 == 0)
            {
                context.DrawLine(majorPen, new Point(0, y), new Point(width, y));
            }
        }
    }

    /// <summary>Draws the dashed line that runs from each version down to the next.</summary>
    /// <remarks>
    /// A broken line rather than a solid one: what the versions are joined by is the chain of variants,
    /// and this only says they stand in one column where no variant sits between them. It is drawn before
    /// the nodes, so a circle covers its ends rather than a dash poking out from under one.
    /// </remarks>
    private void DrawVersionAxis(DrawingContext context, Ink ink)
    {
        if (_drawing is null || _drawing.Versions.Count < 2)
        {
            return;
        }

        var pen = new Pen(new SolidColorBrush(ink.Muted), Math.Max(1, 1 * _zoom))
        {
            DashStyle = new DashStyle([5, 5], 0),
        };
        var x = _panX + _drawing.VersionX * _zoom;

        for (var at = 0; at + 1 < _drawing.Versions.Count; at++)
        {
            var above = _drawing.Versions[at];
            var below = _drawing.Versions[at + 1];

            context.DrawLine(
                pen,
                new Point(x, _panY + above.Y * _zoom),
                new Point(x, _panY + below.Y * _zoom)
            );
        }
    }

    /// <summary>
    /// Draws the edges: the dashed line down the version axis, the quarter arcs of the spine, and the
    /// joins between variants.
    /// </summary>
    private void DrawArcs(DrawingContext context, Ink ink)
    {
        DrawVersionAxis(context, ink);
        DrawUpstreamAxis(context, ink);

        var pen = new Pen(
            new SolidColorBrush(ink.Muted),
            Math.Max(1, 1.5 * _zoom)
        )
        {
            LineCap = PenLineCap.Round,
            LineJoin = PenLineJoin.Round,
        };

        for (var level = 0UL; level < _drawing!.Top; level++)
        {
            if (
                !_drawing.VersionByLevel.TryGetValue(level, out var below)
                || !_drawing.VersionByLevel.TryGetValue(level + 1, out var above)
                || !_drawing.SpineByLevel.TryGetValue(level, out var variant)
            )
            {
                continue;
            }

            // From the version above to the variant: the right point to the bottom point of an
            // ellipse centred on the variant's column.
            DrawArc(
                context,
                pen,
                new Point(
                    _panX + variant.X * _zoom,
                    _panY + above.Y * _zoom
                ),
                (above.X - variant.X) * _zoom,
                (variant.Y - above.Y) * _zoom,
                0,
                Math.PI / 2,
                SweepDirection.Clockwise
            );

            // From the variant on to the version below: the left point to the bottom point of an
            // ellipse centred on the version's column.
            DrawArc(
                context,
                pen,
                new Point(
                    _panX + below.X * _zoom,
                    _panY + variant.Y * _zoom
                ),
                (below.X - variant.X) * _zoom,
                (below.Y - variant.Y) * _zoom,
                Math.PI,
                Math.PI / 2,
                SweepDirection.CounterClockwise
            );
        }

        void BaseEdge(VersionGraphDrawing.VariantNode variant)
        {
            if (!_drawing!.VersionByLevel.TryGetValue(variant.Level, out var below))
            {
                return;
            }

            // The arc is the one the chain draws from a variant down to the version it is based on,
            // carried over to this variant's own column: the same quarter turn at every column, so
            // every base edge is read as the same curve.
            var endX =
                _panX
                + (variant.X + VersionGraphDrawing.AxisGap) * _zoom;

            DrawArc(
                context,
                pen,
                new Point(endX, _panY + variant.Y * _zoom),
                VersionGraphDrawing.AxisGap * _zoom,
                (below.Y - variant.Y) * _zoom,
                Math.PI,
                Math.PI / 2,
                SweepDirection.CounterClockwise
            );

            // What the arc does not reach — the run from its end to the version's own column — is a
            // straight line, so a variant further out is the same arc with a longer run attached.
            var versionX = _panX + below.X * _zoom;

            if (versionX > endX)
            {
                var row = _panY + below.Y * _zoom;

                context.DrawLine(pen, new Point(endX, row), new Point(versionX, row));
            }
        }

        void Elbow(
            IPen line,
            double sourceX,
            double sourceY,
            double targetX,
            double targetY,
            double half
        )
        {
            var fromX = _panX + sourceX * _zoom;
            var fromY = _panY + sourceY * _zoom;
            var toX = _panX + targetX * _zoom;
            var toY = _panY + targetY * _zoom;
            var dx = toX - fromX;
            var dy = toY - fromY;
            var directionX = Math.Sign(dx);
            var directionY = Math.Sign(dy);

            if (directionX == 0 || directionY == 0)
            {
                var edge = directionY == 0
                    ? new Point(fromX, fromY)
                    : new Point(fromX, fromY + directionY * half * _zoom);

                context.DrawLine(line, edge, new Point(toX, toY));

                return;
            }

            // The same quarter the chain draws from a variant to the version it is based on: the
            // horizontal span and the drop are the chain's own, so every arc is the same curve. What
            // the arc does not cover — a longer run down or across — is a straight line, before it or
            // after it.
            var rx = Math.Min(VersionGraphDrawing.AxisGap * _zoom, Math.Abs(dx));
            var ry = Math.Min(
                VersionGraphDrawing.VersionToVariant * _zoom,
                Math.Abs(dy)
            );
            var extra = Math.Abs(dy) - ry;
            var start = new Point(fromX, fromY + directionY * extra);
            var end = new Point(fromX + directionX * rx, toY);

            if (extra > 0.5)
            {
                context.DrawLine(
                    line,
                    new Point(fromX, fromY + directionY * half * _zoom),
                    start
                );
            }

            var geometry = new StreamGeometry();

            // A quintic that leaves the vertical and enters the horizontal with no curvature at either
            // end: the straight and the curve meet with the same tangent and the same curvature, which
            // is what keeps the join from reading as a kink. Its control points are on the two tangents,
            // so it stays within the same quarter the chain's arc covers.
            //
            // The control points sit on the two tangents at 20% and 56%, the distances that make the
            // quintic track the chain's quarter ellipse of AxisGap by VersionToVariant — within half a
            // unit along the curve and a third of a unit in length — while meeting the straight runs
            // with no step in curvature, which is the kink an ellipse leaves at both ends.
            var first = new Point(start.X, start.Y + directionY * ry * 0.2);
            var second = new Point(start.X, start.Y + directionY * ry * 0.56);
            var third = new Point(end.X - directionX * rx * 0.56, end.Y);
            var fourth = new Point(end.X - directionX * rx * 0.2, end.Y);
            var steps = Math.Clamp(
                (int)Math.Ceiling(Math.Max(rx, ry) / 2),
                16,
                256
            );

            using (var path = geometry.Open())
            {
                path.BeginFigure(start, false);

                for (var at = 1; at <= steps; at++)
                {
                    var t = at / (double)steps;
                    var u = 1 - t;
                    var b0 = u * u * u * u * u;
                    var b1 = 5 * u * u * u * u * t;
                    var b2 = 10 * u * u * u * t * t;
                    var b3 = 10 * u * u * t * t * t;
                    var b4 = 5 * u * t * t * t * t;
                    var b5 = t * t * t * t * t;

                    path.LineTo(
                        new Point(
                            (b0 * start.X)
                                + (b1 * first.X)
                                + (b2 * second.X)
                                + (b3 * third.X)
                                + (b4 * fourth.X)
                                + (b5 * end.X),
                            (b0 * start.Y)
                                + (b1 * first.Y)
                                + (b2 * second.Y)
                                + (b3 * third.Y)
                                + (b4 * fourth.Y)
                                + (b5 * end.Y)
                        )
                    );
                }
            }

            context.DrawGeometry(null, line, geometry);

            if (Math.Abs(toX - end.X) > 0.5)
            {
                context.DrawLine(line, end, new Point(toX, toY));
            }
        }

        void PendingEdge(
            VersionGraphDrawing.VariantNode edit,
            VersionGraphDrawing.VariantNode pending
        )
        {
            var line = new Pen(
                new SolidColorBrush(RorolalaColours.UnderWay),
                Math.Max(1, 1.5 * _zoom)
            )
            {
                DashStyle = new DashStyle([5, 4], 0),
                LineCap = PenLineCap.Round,
            };

            // The run the join line will make once the merge is recorded: it starts at the variant being
            // joined and ends at the one the work becomes, where the ? stands now. Drawing it in the same
            // direction and by the same curve is what lets the dash turn into the line without a change
            // of shape.
            Elbow(
                line,
                pending.X,
                pending.Y,
                edit.X,
                edit.Y,
                VersionGraphDrawing.CapsuleHeight / 2
            );
        }

        foreach (var variant in _drawing!.Variants)
        {
            if (variant.State is not null)
            {
                // The edit is based on the highest version and would make the one the index does not
                // have: only the second half of a spine edge exists, from the capsule down to it.
                BaseEdge(variant);

                // A merge waiting under a change is the other half of what the ? node is about: the
                // dashed edge says the work is on its way into the pending variant as well as being
                // based on the version above.
                if (variant.Merging)
                {
                    var pending = _drawing.Variants.FirstOrDefault(
                        one => one.Unjoined && one.Merging
                    );

                    if (pending is not null)
                    {
                        PendingEdge(variant, pending);
                    }
                }

                continue;
            }

            // Every variant that is not on the spine is based on a version of the chain and is not
            // reached by the two arcs a spine variant has: its base edge is the one that says where it
            // stands. A merged variant and an unjoined one are the same here — the join edge is the
            // other relation, not this one.
            if (
                variant.State is null
                && !variant.Spine
                && _drawing.VersionByLevel.TryGetValue(variant.Level, out var below)
            )
            {
                Elbow(
                    pen,
                    variant.X,
                    variant.Y,
                    below.X,
                    below.Y,
                    VersionGraphDrawing.CapsuleHeight / 2
                );
            }

            if (
                variant.Join is not { } join
                || join < 0
                || join >= _drawing.Variants.Count
            )
            {
                continue;
            }

            // The joined variant is the one the run leaves from: it is the lower one, and the line goes
            // up (or down) from its own column, turns, and runs into the merge variant's column.
            var target = _drawing.Variants[join];

            Elbow(
                pen,
                target.X,
                target.Y,
                variant.X,
                variant.Y,
                VersionGraphDrawing.CapsuleHeight / 2
            );
        }
    }

    /// <summary>Draws one elliptical arc.</summary>
    private static void DrawArc(
        DrawingContext context,
        IPen pen,
        Point center,
        double rx,
        double ry,
        double start,
        double end,
        SweepDirection sweep
    )
    {
        if (rx <= 0 || ry <= 0)
        {
            return;
        }

        var from = new Point(
            center.X + rx * Math.Cos(start),
            center.Y + ry * Math.Sin(start)
        );
        var to = new Point(
            center.X + rx * Math.Cos(end),
            center.Y + ry * Math.Sin(end)
        );
        var geometry = new StreamGeometry();

        using (var path = geometry.Open())
        {
            path.BeginFigure(from, false);
            path.ArcTo(to, new Size(rx, ry), 0, false, sweep);
        }

        context.DrawGeometry(null, pen, geometry);
    }

    /// <summary>Draws the dashed line down the Vault's chain, and — where it parted — the branch from the common ancestor.</summary>
    /// <remarks>
    /// A Vault that is only ahead continues the work's own chain, so its versions stand in the same column
    /// and the line is that axis continued. Once both sides have moved on they stand in a column of their
    /// own to the right, and the line from the common ancestor to the lowest version only the Vault has is
    /// the branch. Either way it is broken like the version axis, so a line between two versions is read as
    /// the chain they are part of rather than as a variant's edge.
    /// </remarks>
    private void DrawUpstreamAxis(DrawingContext context, Ink ink)
    {
        if (_drawing is null || _drawing.Upstream.Count == 0)
        {
            return;
        }

        var pen = new Pen(new SolidColorBrush(ink.Muted), Math.Max(1, 1 * _zoom))
        {
            DashStyle = new DashStyle([4, 5], 0),
        };
        var x = _panX + _drawing.UpstreamX * _zoom;

        for (var at = 0; at + 1 < _drawing.Upstream.Count; at++)
        {
            var above = _drawing.Upstream[at];
            var below = _drawing.Upstream[at + 1];

            context.DrawLine(
                pen,
                new Point(x, _panY + above.Y * _zoom),
                new Point(x, _panY + below.Y * _zoom)
            );
        }

        if (
            _drawing.BranchFrom is { } level
            && _drawing.VersionByLevel.TryGetValue(level, out var from)
        )
        {
            var to = _drawing.Upstream[^1];
            var start = new Point(_panX + from.X * _zoom, _panY + from.Y * _zoom);
            var end = new Point(x, _panY + to.Y * _zoom);

            // A chain that is only the work's own, continued upstream, is one line joining the two
            // columns. The arc is for a divergence, where both sides have moved on from the common
            // ancestor and the two rows are read as a fork rather than as one chain.
            if (!_drawing.Diverged)
            {
                context.DrawLine(pen, start, end);

                return;
            }

            var dx = end.X - start.X;
            var dy = end.Y - start.Y;
            var branch = new StreamGeometry();

            using (var path = branch.Open())
            {
                path.BeginFigure(start, false);
                path.CubicBezierTo(
                    new Point(start.X + dx * 0.55, start.Y),
                    new Point(end.X, end.Y - dy * 0.55),
                    end
                );
            }

            context.DrawGeometry(null, pen, branch);
        }
    }

    /// <summary>Draws the version nodes, their labels and the Vault's ring.</summary>
    private void DrawVersions(
        DrawingContext context,
        Ink ink,
        Metrics metrics
    )
    {
        var primary = new SolidColorBrush(ink.Primary);
        var ring = new Pen(primary, Math.Max(1.5, 2 * _zoom));

        foreach (var version in _drawing!.Versions)
        {
            var cx = _panX + version.X * _zoom;
            var cy = _panY + version.Y * _zoom;
            var radius = VersionGraphDrawing.Radius * _zoom;

            if (
                cx + radius < 0
                || cx - radius > Bounds.Width
                || cy + radius < 0
                || cy - radius > Bounds.Height
            )
            {
                continue;
            }

            context.DrawEllipse(primary, null, new Point(cx, cy), radius, radius);

            if (version.Origin)
            {
                context.DrawEllipse(
                    null,
                    ring,
                    new Point(cx, cy),
                    radius + 5 * _zoom,
                    radius + 5 * _zoom
                );
            }

            var label = metrics.Versions[version.Level].Label;
            context.DrawText(
                label,
                new Point(cx - label.Width / 2, cy - label.Height / 2)
            );
        }
    }

    /// <summary>Draws the Vault's versions that the work's own chain does not hold.</summary>
    /// <remarks>
    /// They are hollow rather than filled, which is what tells them from the work's own versions at a
    /// glance: the work is what the reader acts on, and the Vault's chain is what it stands beside. A
    /// version only the Layout names — one this index never fetched — is drawn broken, since no level
    /// or words are known for it.
    /// </remarks>
    private void DrawUpstream(DrawingContext context, Ink ink, Metrics metrics)
    {
        if (_drawing is null || _drawing.Upstream.Count == 0)
        {
            return;
        }

        var fill = new SolidColorBrush(ink.Elevated);
        var muted = new SolidColorBrush(ink.Muted);
        var pen = new Pen(muted, Math.Max(1, 1.25 * _zoom));
        var broken = new Pen(muted, Math.Max(1, 1 * _zoom))
        {
            DashStyle = new DashStyle([3, 3], 0),
        };

        for (var at = 0; at < _drawing.Upstream.Count; at++)
        {
            var version = _drawing.Upstream[at];
            var cx = _panX + version.X * _zoom;
            var cy = _panY + version.Y * _zoom;
            var radius = VersionGraphDrawing.Radius * _zoom;

            if (
                cx + radius < 0
                || cx - radius > Bounds.Width
                || cy + radius < 0
                || cy - radius > Bounds.Height
            )
            {
                continue;
            }

            context.DrawEllipse(
                fill,
                version.Unknown ? broken : pen,
                new Point(cx, cy),
                radius,
                radius
            );

            var label = metrics.Upstream[at].Label;
            context.DrawText(
                label,
                new Point(cx - label.Width / 2, cy - label.Height / 2)
            );
        }
    }

    /// <summary>Draws the variant capsules and their labels.</summary>
    private void DrawVariants(
        DrawingContext context,
        Ink ink,
        Metrics metrics
    )
    {
        var fill = new SolidColorBrush(ink.Elevated);

        foreach (var variant in _drawing!.Variants)
        {
            var metric = metrics.Variants[variant.Index];
            var width = metric.Width;
            var height = VersionGraphDrawing.CapsuleHeight * _zoom;
            var cx = _panX + variant.X * _zoom;
            var cy = _panY + variant.Y * _zoom;
            var stroke = variant.Merging
                ? RorolalaColours.UnderWay
                : variant.State is null
                    ? ink.Accent
                    : variant.Wrong
                        ? ink.Del
                        : ink.Add;
            var pen = new Pen(new SolidColorBrush(stroke), Math.Max(1, 1.5 * _zoom));

            if (
                cx + width / 2 < 0
                || cx - width / 2 > Bounds.Width
                || cy + height / 2 < 0
                || cy - height / 2 > Bounds.Height
            )
            {
                continue;
            }

            context.DrawRectangle(
                fill,
                pen,
                new Rect(cx - width / 2, cy - height / 2, width, height),
                height / 2,
                height / 2
            );
            context.DrawText(
                metric.Label,
                new Point(
                    cx - metric.Label.Width / 2,
                    cy - metric.Label.Height / 2
                )
            );
        }
    }

    /// <summary>Draws the version bubbles, and the hash under the one hovered.</summary>
    private void DrawBubbles(
        DrawingContext context,
        Ink ink,
        Metrics metrics
    )
    {
        var fill = new SolidColorBrush(ink.Elevated);

        // The Vault's bubbles are drawn first, so a version of the work's own that is hovered in the
        // same row can be read over the one that stands there instead of it.
        for (var at = 0; at < _drawing!.Upstream.Count; at++)
        {
            var version = _drawing.Upstream[at];

            if (version.Unknown || metrics.Upstream[at].Bubble is not { } bubble)
            {
                continue;
            }

            DrawBubble(
                context,
                ink,
                fill,
                version.X,
                version.Y,
                false,
                bubble,
                ReferenceEquals(_hover?.Upstream, version)
            );
        }

        foreach (var version in _drawing.Versions)
        {
            var bubble = metrics.Versions[version.Level].Bubble;

            if (bubble is null)
            {
                continue;
            }

            var hovered = ReferenceEquals(_hover?.Version, version);

            // A version of the work's and one of the Vault's are in the same row only where the two
            // chains have parted: there the Vault's bubble is the one drawn, and the work's is left
            // for the pointer. Rows the Vault has no version in keep their own bubble.
            if (
                !hovered
                && _drawing.Upstream.Any(
                    one => !one.Unknown && one.Level == version.Level
                )
            )
            {
                continue;
            }

            DrawBubble(
                context,
                ink,
                fill,
                version.X,
                version.Y,
                version.Origin,
                bubble,
                hovered
            );
        }
    }

    /// <summary>Draws one version's bubble, with its tail on the node.</summary>
    private void DrawBubble(
        DrawingContext context,
        Ink ink,
        IBrush fill,
        double nodeX,
        double nodeY,
        bool origin,
        BubbleMetrics bubble,
        bool hovered
    )
    {
        var x =
            _panX
            + (nodeX + VersionGraphDrawing.Radius + VersionGraphDrawing.BubbleGap)
                * _zoom;
        var cy = _panY + nodeY * _zoom;
        var width = hovered ? bubble.Width : bubble.RestWidth;
        var height = hovered
            ? bubble.RestHeight + bubble.HashGap + (bubble.Hash?.Height ?? 0)
            : bubble.RestHeight;
        var y = cy - height / 2;
        var border = origin ? ink.Primary : ink.Splitter;
        var pen = new Pen(
            new SolidColorBrush(border),
            Math.Max(1, (origin ? 1.5 : 1.25) * _zoom)
        );
        var tail = VersionGraphDrawing.BubbleTail * _zoom;
        var half = VersionGraphDrawing.BubbleTailHalf * _zoom;
        var tip = new Point(x - tail, cy);
        var radius = 10 * _zoom;

        // Body and tail are one outline, because two strokes would draw a line across the joint —
        // the bubble's left edge — and cut the tail off the bubble it belongs to. The edge stops
        // at the tail's base on both sides and the tail bridges the rest.
        var geometry = new StreamGeometry();

        using (var path = geometry.Open())
        {
            path.BeginFigure(new Point(x, cy + half), false);
            path.LineTo(new Point(x, y + height - radius));
            path.ArcTo(
                new Point(x + radius, y + height),
                new Size(radius, radius),
                0,
                false,
                SweepDirection.CounterClockwise
            );
            path.LineTo(new Point(x + width - radius, y + height));
            path.ArcTo(
                new Point(x + width, y + height - radius),
                new Size(radius, radius),
                0,
                false,
                SweepDirection.CounterClockwise
            );
            path.LineTo(new Point(x + width, y + radius));
            path.ArcTo(
                new Point(x + width - radius, y),
                new Size(radius, radius),
                0,
                false,
                SweepDirection.CounterClockwise
            );
            path.LineTo(new Point(x + radius, y));
            path.ArcTo(
                new Point(x, y + radius),
                new Size(radius, radius),
                0,
                false,
                SweepDirection.CounterClockwise
            );
            path.LineTo(new Point(x, cy - half));
            path.LineTo(tip);
            path.EndFigure(true);
        }

        context.DrawGeometry(fill, pen, geometry);

        var textY = y + bubble.PaddingY;

        if (bubble.Creator is { } creator)
        {
            // A neutral chip, not a colour of state: the bubble's border and the node's ring are
            // what say a version is the origin, so the name does not have to say it as well.
            var pill = new Rect(
                x + bubble.PaddingX,
                textY,
                bubble.PillWidth,
                bubble.PillHeight
            );

            context.DrawRectangle(
                new SolidColorBrush(ink.Splitter),
                null,
                pill,
                bubble.PillHeight / 2,
                bubble.PillHeight / 2
            );
            context.DrawText(
                creator,
                new Point(
                    pill.X + bubble.PillPaddingX,
                    pill.Y + bubble.PillPaddingY
                )
            );
            textY += bubble.PillHeight + bubble.Gap;
        }

        if (bubble.Message is { } message)
        {
            context.DrawText(
                message,
                new Point(x + bubble.PaddingX, textY)
            );
            textY += message.Height;
        }

        if (hovered && bubble.Hash is { } hashText)
        {
            textY += bubble.HashGap;
            context.DrawText(hashText, new Point(x + bubble.PaddingX, textY));
        }
    }

    /// <summary>Draws a variant's own words, above or below its capsule.</summary>
    private void DrawVariantTooltip(
        DrawingContext context,
        Ink ink,
        VersionGraphDrawing.VariantNode variant
    )
    {
        var padding = 9 * _zoom;
        var contentWidth = TooltipMaxWidth * _zoom - padding * 2;

        // An edit has no maker and no message of its own — the only thing to say about it is what the
        // working file is — so the state's own sentence is the tooltip.
        var creator = variant.State is null && variant.Creator.Length > 0
            ? Text(
                $"({variant.Creator})",
                FaceMedium,
                11.5 * _zoom,
                ink.Muted
            )
            : null;
        var said = variant.State ?? variant.Message;
        var message = said.Length > 0
            ? Text(said, FaceNormal, 12.5 * _zoom, ink.Front)
            : null;

        if (message is not null)
        {
            var lineHeight = 18 * _zoom;

            message.MaxTextWidth = contentWidth;
            message.MaxTextHeight = 4 * lineHeight;
            message.LineHeight = lineHeight;
            message.Trimming = TextTrimming.CharacterEllipsis;
        }

        if (creator is null && message is null)
        {
            return;
        }

        var gap = creator is not null && message is not null ? 2 * _zoom : 0;
        var width =
            Math.Max(creator?.Width ?? 0, message?.Width ?? 0) + padding * 2;
        var height =
            padding * 2
            + (creator?.Height ?? 0)
            + gap
            + (message?.Height ?? 0);
        var cx = _panX + variant.X * _zoom;
        var cy = _panY + variant.Y * _zoom;
        var half = VersionGraphDrawing.CapsuleHeight / 2 * _zoom;
        var maxX = Math.Max(8, Bounds.Width - width - 8);
        var x = Math.Clamp(cx - width / 2, 8, maxX);
        var above = true;
        var y = cy - half - 10 * _zoom - height;

        if (y < 8)
        {
            y = cy + half + 10 * _zoom;
            above = false;
        }

        var maxY = Math.Max(8, Bounds.Height - height - 8);
        y = Math.Clamp(y, 8, maxY);

        var tail = 8 * _zoom;
        var tailX = Math.Clamp(cx - x, 14 * _zoom, width - 14 * _zoom);
        var fill = new SolidColorBrush(ink.Elevated);
        var pen = new Pen(
            new SolidColorBrush(ink.Splitter),
            Math.Max(1, 1.25 * _zoom)
        );
        var baseY = above ? y + height : y;
        var tipY = above ? baseY + tail : baseY - tail;
        var tailGeometry = new StreamGeometry();

        using (var path = tailGeometry.Open())
        {
            path.BeginFigure(new Point(x + tailX, tipY), true);
            path.LineTo(new Point(x + tailX - tail, baseY));
            path.LineTo(new Point(x + tailX + tail, baseY));
            path.EndFigure(true);
        }

        context.DrawGeometry(fill, null, tailGeometry);
        context.DrawRectangle(
            fill,
            pen,
            new Rect(x, y, width, height),
            8 * _zoom,
            8 * _zoom
        );
        context.DrawLine(
            pen,
            new Point(x + tailX, tipY),
            new Point(x + tailX - tail, baseY)
        );
        context.DrawLine(
            pen,
            new Point(x + tailX, tipY),
            new Point(x + tailX + tail, baseY)
        );

        var textY = y + padding;

        if (creator is not null)
        {
            context.DrawText(creator, new Point(x + padding, textY));
            textY += creator.Height + gap;
        }

        if (message is not null)
        {
            context.DrawText(message, new Point(x + padding, textY));
        }
    }

    /// <summary>Draws the words for a version only the Vault holds.</summary>
    /// <param name="context">Where to draw.</param>
    /// <param name="ink">The colours in force.</param>
    /// <param name="version">The upstream version under the pointer.</param>
    private void DrawUpstreamTooltip(
        DrawingContext context,
        Ink ink,
        VersionGraphDrawing.UpstreamNode version
    )
    {
        var padding = 9 * _zoom;
        var contentWidth = TooltipMaxWidth * _zoom - padding * 2;
        var vault = _drawing!.UpstreamVault;
        var head = version.Unknown
            ? Text(
                RolaI18N.Get("rorolala_vcs.version_graph.upstream_unknown"),
                FaceNormal,
                11.5 * _zoom,
                ink.Muted
            )
            : Text(
                vault.Length > 0
                    ? $"{RolaI18N.Get("rorolala_vcs.column_upstream")}: {vault}"
                    : RolaI18N.Get("rorolala_vcs.column_upstream"),
                FaceMedium,
                11 * _zoom,
                ink.Muted
            );
        head.MaxTextWidth = contentWidth;

        var creator = version.Creator.Length > 0
            ? Text($"({version.Creator})", FaceMedium, 11.5 * _zoom, ink.Muted)
            : null;
        FormattedText? message = null;

        if (version.Message.Length > 0)
        {
            var lineHeight = 18 * _zoom;

            message = Text(version.Message, FaceNormal, 12.5 * _zoom, ink.Front);
            message.MaxTextWidth = contentWidth;
            message.MaxTextHeight = 4 * lineHeight;
            message.LineHeight = lineHeight;
            message.Trimming = TextTrimming.CharacterEllipsis;
        }

        var hash = version.Hash.Length > 0
            ? Text(version.Hash, MonoMedium, 10 * _zoom, ink.Muted)
            : null;

        if (hash is not null)
        {
            hash.MaxTextWidth = contentWidth;
        }

        var gap = 3 * _zoom;
        var width =
            Math.Max(
                head.Width,
                Math.Max(
                    creator?.Width ?? 0,
                    Math.Max(message?.Width ?? 0, hash?.Width ?? 0)
                )
            )
            + padding * 2;
        var height =
            padding * 2
            + head.Height
            + (creator is null ? 0 : gap + creator.Height)
            + (message is null ? 0 : gap + message.Height)
            + (hash is null ? 0 : gap + hash.Height);
        var cx = _panX + version.X * _zoom;
        var cy = _panY + version.Y * _zoom;
        var radius = VersionGraphDrawing.Radius * _zoom;
        var x = cx + radius + 10 * _zoom;

        if (x + width > Bounds.Width - 8)
        {
            x = cx - radius - 10 * _zoom - width;
        }

        var maxX = Math.Max(8, Bounds.Width - width - 8);
        var maxY = Math.Max(8, Bounds.Height - height - 8);

        x = Math.Clamp(x, 8, maxX);

        var y = Math.Clamp(cy - height / 2, 8, maxY);
        var fill = new SolidColorBrush(ink.Elevated);
        var pen = new Pen(
            new SolidColorBrush(ink.Splitter),
            Math.Max(1, 1.25 * _zoom)
        );

        context.DrawRectangle(
            fill,
            pen,
            new Rect(x, y, width, height),
            8 * _zoom,
            8 * _zoom
        );

        var textY = y + padding;

        context.DrawText(head, new Point(x + padding, textY));
        textY += head.Height;

        if (creator is not null)
        {
            textY += gap;
            context.DrawText(creator, new Point(x + padding, textY));
            textY += creator.Height;
        }

        if (message is not null)
        {
            textY += gap;
            context.DrawText(message, new Point(x + padding, textY));
            textY += message.Height;
        }

        if (hash is not null)
        {
            textY += gap;
            context.DrawText(hash, new Point(x + padding, textY));
        }
    }

    /// <summary>Draws the words for a file with no graph.</summary>
    private void DrawEmpty(DrawingContext context, Ink ink)
    {
        if (string.IsNullOrEmpty(_emptyText))
        {
            return;
        }

        var maxWidth = Math.Min(420, Bounds.Width - 64);

        if (maxWidth <= 0)
        {
            return;
        }

        var text = Text(_emptyText, FaceNormal, 13, ink.Muted);
        text.MaxTextWidth = maxWidth;
        text.TextAlignment = TextAlignment.Center;
        context.DrawText(
            text,
            new Point(
                (Bounds.Width - maxWidth) / 2,
                (Bounds.Height - text.Height) / 2
            )
        );
    }

    /// <summary>Draws the scale in the corner.</summary>
    private void DrawZoom(DrawingContext context, Ink ink)
    {
        var text = Text(_zoomText, MonoMedium, 11, ink.Muted);
        const double paddingX = 5;
        const double paddingY = 2;
        var width = text.Width + paddingX * 2;
        var height = text.Height + paddingY * 2;
        var x = Bounds.Width - 12 - width;
        var y = Bounds.Height - 9 - height;

        if (x < 0 || y < 0)
        {
            return;
        }

        context.DrawRectangle(
            new SolidColorBrush(
                Color.FromArgb(0xB8, ink.Ground.R, ink.Ground.G, ink.Ground.B)
            ),
            null,
            new Rect(x, y, width, height),
            4,
            4
        );
        context.DrawText(text, new Point(x + paddingX, y + paddingY));
    }
}

using Avalonia;
using Avalonia.Controls;

namespace FileSystemPlugin;

/// <summary>
/// The tiles of a grid, wrapped into lines that fill the width they are given.
/// </summary>
/// <remarks>
/// A line with room to spare puts that room between its tiles rather than at its ends, so the inset at the
/// grid's left and right edges is the same however wide the dock is and however many tiles a line happens
/// to hold — which a wrapping panel that leaves the room at the end does not do, its inset then depending on
/// where the last tile of a line fell.
/// <para>
/// Every line that wrapped is filled to the width, so the room between its tiles is the same on all of
/// them. The last line keeps that same room rather than its own spare, so that the tiles of a short last
/// line stand in the same columns as the tiles above instead of being pulled together; what is left over
/// stays at its end. A grid that never wrapped has no filled line to take the room from and keeps the
/// least room, reading from its start — sharing its room out would spread the few tiles of a short line
/// across the whole width.
/// </para>
/// </remarks>
internal sealed class JustifiedPanel : Panel
{
    /// <summary>
    /// The room left between two tiles, before any spare room is shared out.
    /// </summary>
    /// <remarks>
    /// The tiles carry no horizontal margin of their own here, so that the grid's outermost inset is the room
    /// around it and nothing else: the horizontal room between tiles is this panel's to place, and this is the
    /// least of it.
    /// </remarks>
    public double Gap { get; set; } = 12;

    /// <inheritdoc />
    protected override Size MeasureOverride(Size availableSize)
    {
        foreach (var child in Children)
        {
            child.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        }

        // Unbounded width is a width nothing wraps against, so every tile stays on the one line.
        var bound = double.IsPositiveInfinity(availableSize.Width)
            ? double.PositiveInfinity
            : availableSize.Width;

        var height = 0.0;
        var widest = 0.0;

        foreach (var line in Lines(bound))
        {
            height += line.Height;
            widest = Math.Max(widest, line.Width);
        }

        return new Size(double.IsPositiveInfinity(bound) ? widest : Math.Min(widest, bound), height);
    }

    /// <inheritdoc />
    protected override Size ArrangeOverride(Size finalSize)
    {
        var lines = Lines(finalSize.Width);
        var y = 0.0;
        var step = Gap;

        for (var index = 0; index < lines.Count; index++)
        {
            var line = lines[index];

            // A line that wrapped is filled to the width and shares its spare room between its tiles; the
            // last line keeps that same room, so its tiles stand in the columns above rather than being
            // pulled together, and leaves whatever is still over at its end. A grid of one line never
            // wrapped, so there is no filled line to take the room from and the least room is kept.
            if (index < lines.Count - 1)
            {
                step = line.Count > 1
                    ? Gap + ((finalSize.Width - line.Width) / (line.Count - 1))
                    : Gap;
            }

            var x = 0.0;

            for (var at = line.Start; at < line.Start + line.Count; at++)
            {
                var size = Children[at].DesiredSize;
                Children[at].Arrange(new Rect(x, y, size.Width, size.Height));
                x += size.Width + step;
            }

            y += line.Height;
        }

        return finalSize;
    }

    /// <summary>
    /// The tiles as they fall into lines when wrapped at a width.
    /// </summary>
    /// <remarks>
    /// The same walk measures and arranges, so that what a line is said to hold is what it is laid out as.
    /// </remarks>
    /// <param name="width">The width a line wraps against.</param>
    private List<Line> Lines(double width)
    {
        var lines = new List<Line>();
        var start = 0;
        var count = 0;
        var extent = 0.0;
        var height = 0.0;

        for (var at = 0; at < Children.Count; at++)
        {
            var size = Children[at].DesiredSize;

            if (count > 0 && extent + Gap + size.Width > width)
            {
                lines.Add(new Line(start, count, extent, height));
                start = at;
                count = 0;
                extent = 0;
                height = 0;
            }

            extent += count > 0 ? Gap + size.Width : size.Width;
            height = Math.Max(height, size.Height);
            count++;
        }

        if (count > 0)
        {
            lines.Add(new Line(start, count, extent, height));
        }

        return lines;
    }

    /// <summary>One line: which tiles are on it, how wide it is, and how tall.</summary>
    /// <param name="Start">The first tile's index.</param>
    /// <param name="Count">How many tiles it holds.</param>
    /// <param name="Width">How wide it is with its least room between tiles.</param>
    /// <param name="Height">How tall its tallest tile is.</param>
    private readonly record struct Line(int Start, int Count, double Width, double Height);
}

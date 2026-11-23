using Avalonia;

namespace FileSystemPlugin;

/// <summary>
/// How a justified grid falls into lines, which the panel that draws it and the model that sizes it
/// both read.
/// </summary>
/// <remarks>
/// The rule is the one a line has always been wrapped by: items are put on a line while the line still
/// has room for the next one and the gap before it, a line is as tall as its tallest item, and the last
/// line keeps the room the lines above it shared out rather than being pulled together.
/// </remarks>
internal static class JustifiedLayout
{
    /// <summary>One line: which items are on it, how wide it is, and how tall.</summary>
    /// <param name="Start">The first item's index.</param>
    /// <param name="Count">How many items it holds.</param>
    /// <param name="Width">How wide it is with its least room between items.</param>
    /// <param name="Height">How tall its tallest item is.</param>
    internal readonly record struct Line(int Start, int Count, double Width, double Height);

    /// <summary>The items as they fall into lines when wrapped at a width.</summary>
    /// <param name="count">How many items there are.</param>
    /// <param name="sizeOf">How large one item is.</param>
    /// <param name="width">The width a line wraps against.</param>
    /// <param name="gap">The room left between two items on one line.</param>
    /// <returns>The lines, in the order they stand.</returns>
    internal static List<Line> Lines(int count, Func<int, Size> sizeOf, double width, double gap)
    {
        ArgumentNullException.ThrowIfNull(sizeOf);

        var lines = new List<Line>();
        var start = 0;
        var held = 0;
        var extent = 0.0;
        var height = 0.0;

        for (var at = 0; at < count; at++)
        {
            var size = sizeOf(at);

            if (held > 0 && extent + gap + size.Width > width)
            {
                lines.Add(new Line(start, held, extent, height));
                start = at;
                held = 0;
                extent = 0;
                height = 0;
            }

            extent += held > 0 ? gap + size.Width : size.Width;
            height = Math.Max(height, size.Height);
            held++;
        }

        if (held > 0)
        {
            lines.Add(new Line(start, held, extent, height));
        }

        return lines;
    }
}

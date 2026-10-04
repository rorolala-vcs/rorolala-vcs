using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media.Imaging;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a command said, drawn as the shapes its lines mean.
/// </summary>
/// <remarks>
/// A line a command reported is not only words: which of the three it is decides the shape it is read in. What
/// went wrong is ordinary text with a mark in front of it — a mark in the ink of the words beside it, since what
/// is drawn is not an alarm but a label. What to be careful of stands in a box with the look's own amber edge and
/// a wash of it behind, and what can be done about it in the same box in green, which is the shape a reader
/// already knows from the alerts a markdown document is written with.
/// </remarks>
internal static class ReportView
{
    /// <summary>How wide and tall the mark beside a line is.</summary>
    private const double Mark = 16;

    /// <summary>How wide the edge of a box is.</summary>
    private const double Edge = 1;

    /// <summary>Draws what a command said.</summary>
    /// <param name="blocks">What it said, in the order it said it.</param>
    /// <param name="picture">Where a picture key is answered from.</param>
    /// <param name="ink">What a named colour inside a message is drawn in.</param>
    /// <returns>What to show in the window.</returns>
    public static Control Of(
        IReadOnlyList<ReportBlock> blocks,
        Func<string, Bitmap?> picture,
        Func<string, string?> ink
    )
    {
        ArgumentNullException.ThrowIfNull(blocks);

        var said = new StackPanel { Spacing = 12 };

        foreach (var block in blocks)
        {
            said.Children.Add(Block(block, picture, ink));
        }

        return said;
    }

    /// <summary>Draws one thing said.</summary>
    /// <param name="block">What was said.</param>
    /// <param name="picture">Where a picture key is answered from.</param>
    /// <param name="ink">What a named colour inside it is drawn in.</param>
    /// <returns>What to show for it.</returns>
    private static Control Block(
        ReportBlock block,
        Func<string, Bitmap?> picture,
        Func<string, string?> ink
    )
    {
        var words = new TextBlock { TextWrapping = TextWrapping.Wrap };

        ReportLines.Draw(words, string.Join('\n', block.Lines), ink);

        if (block.Kind == ReportKind.Plain)
        {
            return words;
        }

        // What the three are drawn with: a picture, the ink it is drawn in, and the ground it stands on where it
        // stands in a box of its own. What went wrong is in the ordinary ink, so that the window does not shout
        // about something the reader has already been told by the words; the other two are the look's own colours,
        // which is what a reader knows an alert by.
        var (key, inkKey, groundKey) = block.Kind switch
        {
            ReportKind.Error => (Icons.Error, "rorolala.fg", (string?)null),
            ReportKind.Warning => (Icons.Warning, "rorolala.warn", "rorolala.warn.bg"),
            _ => (Icons.LiveHelp, "rorolala.add", "rorolala.add.bg"),
        };

        var row = new StackPanel { Orientation = Orientation.Horizontal };

        if (picture(key) is { } bitmap)
        {
            var mark = new Border
            {
                Width = Mark,
                Height = Mark,
                Margin = new Thickness(0, 3, 8, 0),
                VerticalAlignment = VerticalAlignment.Top,
                OpacityMask = new ImageBrush(bitmap) { Stretch = Stretch.Uniform },
            };
            mark[!Border.BackgroundProperty] = new DynamicResourceExtension(inkKey);

            row.Children.Add(mark);
        }

        row.Children.Add(words);

        if (groundKey is null)
        {
            return row;
        }

        var box = new Border
        {
            CornerRadius = new CornerRadius(6),
            BorderThickness = new Thickness(Edge),
            Padding = new Thickness(10, 8),
            Child = row,
        };
        box[!Border.BorderBrushProperty] = new DynamicResourceExtension(inkKey);
        box[!Border.BackgroundProperty] = new DynamicResourceExtension(groundKey);

        return box;
    }
}

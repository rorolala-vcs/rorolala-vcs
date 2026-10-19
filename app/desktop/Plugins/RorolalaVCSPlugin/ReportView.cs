using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media.Imaging;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;

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

    /// <summary>
    /// What a colour named inside a message stands for.
    /// </summary>
    /// <remarks>
    /// The three a level is reported in are answered with the look's own resources rather than with the
    /// terminal's palette, so that a message says the same thing in a window as it says in a terminal while
    /// being drawn in the colours the window is drawn in. A colour written as six digits is the colour it says,
    /// and a name nothing answers to is left as it stands.
    /// </remarks>
    /// <param name="colour">The colour the message named.</param>
    /// <returns>The colour or theme resource to draw it in, or nothing.</returns>
    public static string? Ink(string colour) =>
        colour.StartsWith('#')
            ? colour
            : colour.ToLowerInvariant() switch
            {
                "red" => PaletteKeys.Del,
                "yellow" => PaletteKeys.Warn,
                "green" => PaletteKeys.Add,
                _ => null,
            };

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

        if (block.Kind == ReportKind.Plain)
        {
            ReportLines.Draw(words, string.Join('\n', block.Lines), ink);

            return words;
        }

        // What the three are drawn with: a picture, the ink it is drawn in, and the ground it stands on where it
        // stands in a box of its own. What went wrong is in the ordinary ink, so that the window does not shout
        // about something the reader has already been told by the words; the other two are the look's own colours,
        // which is what a reader knows an alert by.
        var (key, inkKey, groundKey) = block.Kind switch
        {
            ReportKind.Error => (Icons.Error, PaletteKeys.Front, (string?)null),
            ReportKind.Warning => (Icons.Warning, PaletteKeys.Warn, PaletteKeys.WarnBackground),
            _ => (Icons.LiveHelp, PaletteKeys.Add, PaletteKeys.AddBackground),
        };

        // A code span inside a help box belongs to the box rather than to the program's accent: the green is
        // what the reader knows the advice by, and the pale wash behind it is the same colour laid thinner.
        var codeInk = block.Kind == ReportKind.Help ? inkKey : PaletteKeys.Accent;
        var codeGround =
            block.Kind == ReportKind.Help ? groundKey ?? PaletteKeys.AddBackground : PaletteKeys.Mid;

        ReportLines.Draw(words, string.Join('\n', block.Lines), ink, codeInk, codeGround);

        if (picture(key) is not { } bitmap)
        {
            return words;
        }

        // A grid rather than a row of a horizontal stack: the words have to be measured against what is
        // left of the width rather than against no width at all, or a sentence longer than the card would
        // stay on one line and be cut off instead of wrapping.
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("Auto,*") };
        var mark = new Border
        {
            Width = Mark,
            Height = Mark,
            Margin = new Thickness(0, 3, 8, 0),
            VerticalAlignment = VerticalAlignment.Top,
            OpacityMask = new ImageBrush(bitmap) { Stretch = Stretch.Uniform },
        };

        // The picture is drawn much larger than the mark it is shown as, so it is reduced here: the
        // high filter is what keeps a thin stroke whole on the way down.
        Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
            mark,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );
        mark[!Border.BackgroundProperty] = new DynamicResourceExtension(inkKey);

        Grid.SetColumn(mark, 0);
        Grid.SetColumn(words, 1);
        row.Children.Add(mark);
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

using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Transformation;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using Path = Avalonia.Controls.Shapes.Path;

namespace RorolalaVCSPlugin;

/// <summary>
/// The fold a group of rows is shown and hidden by, drawn the way the Overview draws it.
/// </summary>
/// <remarks>
/// The caret is a path rather than a picture, so it is drawn at whatever the look's ink is and turns
/// without a second picture for the turned state. The rows' height and their opacity move together:
/// a fold that only clipped the height would show the rows sliding away rather than folding.
/// </remarks>
internal static class WorkspaceFold
{
    /// <summary>How long a fold takes.</summary>
    /// <remarks>
    /// The same span the Overview uses, so a reader who has folded its groups finds this one moving
    /// at the speed they know rather than at another program's.
    /// </remarks>
    private static readonly TimeSpan Span = TimeSpan.FromMilliseconds(160);

    /// <summary>The caret a fold is opened and closed by.</summary>
    /// <param name="open">Whether the rows it heads are shown.</param>
    /// <returns>The caret, turned to say so.</returns>
    public static Path Caret(bool open)
    {
        var arrow = new Path
        {
            Data = Geometry.Parse("M 1,0 L 5,4 L 1,8"),
            StrokeThickness = 1.5,
            StrokeLineCap = PenLineCap.Round,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransformOrigin = RelativePoint.Center,
            RenderTransform = TransformOperations.Parse(open ? "rotate(90deg)" : "rotate(0deg)"),
            Transitions = Turning(),
        };
        arrow[!Shape.StrokeProperty] = new DynamicResourceExtension(PaletteKeys.Faint);

        return arrow;
    }

    /// <summary>Turns a caret to the state its rows are in.</summary>
    /// <param name="caret">The caret to turn.</param>
    /// <param name="open">Whether the rows are shown.</param>
    public static void Turn(Path caret, bool open)
    {
        ArgumentNullException.ThrowIfNull(caret);

        caret.RenderTransform = TransformOperations.Parse(open ? "rotate(90deg)" : "rotate(0deg)");
    }

    /// <summary>The transition a caret turns with.</summary>
    /// <returns>A new set, since a control holds the one it was given.</returns>
    private static Transitions Turning() =>
        new()
        {
            new TransformOperationsTransition
            {
                Property = Visual.RenderTransformProperty,
                Duration = Span,
                Easing = new CubicEaseOut(),
            },
        };

    /// <summary>The transitions a block of rows folds with.</summary>
    /// <returns>A new set, since a control holds the one it was given.</returns>
    public static Transitions Motion() =>
        new()
        {
            new DoubleTransition
            {
                Property = Layoutable.MaxHeightProperty,
                Duration = Span,
                Easing = new CubicEaseOut(),
            },
            new DoubleTransition
            {
                Property = Visual.OpacityProperty,
                Duration = Span,
                Easing = new CubicEaseOut(),
            },
        };

    /// <summary>
    /// Folds a block of rows away, or unfolds it.
    /// </summary>
    /// <remarks>
    /// The height an unfolded block is going to take is not known while it is folded, so it is
    /// measured without the transition in the way and the limit is put back before the animation is
    /// told to run. Both halves are posted rather than run here, because a block that is being shown
    /// has to be measured before the height it is to grow to is known.
    /// </remarks>
    /// <param name="rows">The rows, clipped and with the motion on them.</param>
    /// <param name="motion">The transitions the rows hold.</param>
    /// <param name="folded">Whether they are to be hidden.</param>
    public static void Rows(StackPanel rows, Transitions motion, bool folded)
    {
        ArgumentNullException.ThrowIfNull(rows);
        ArgumentNullException.ThrowIfNull(motion);

        if (folded)
        {
            rows.MaxHeight = rows.Bounds.Height;
            Dispatcher.UIThread.Post(() =>
            {
                rows.MaxHeight = 0;
                rows.Opacity = 0;
            });

            return;
        }

        rows.Transitions = null;
        rows.MaxHeight = double.PositiveInfinity;
        rows.Measure(new Size(rows.Bounds.Width, double.PositiveInfinity));
        var height = rows.DesiredSize.Height;
        rows.MaxHeight = 0;
        rows.Transitions = motion;

        Dispatcher.UIThread.Post(() =>
        {
            rows.MaxHeight = height;
            rows.Opacity = 1;
        });
    }
}

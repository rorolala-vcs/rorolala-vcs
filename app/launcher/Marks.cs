using Avalonia;
using Avalonia.Media;
using ShapePath = Avalonia.Controls.Shapes.Path;

namespace RorolalaLauncher;

/// <summary>
/// The one picture this program still draws for itself: the cog that opens what it may be told.
/// </summary>
/// <remarks>
/// Everything else the program is drawn with is a picture from the pinned Material Design set — the three
/// the toolbar wears and the two a window's frame carries — because those are the product's artwork and are
/// drawn from one manifest by <c>cargo dev icons</c>. The cog is the exception: the set this project pins has
/// no settings picture, and a mark from another set would be a second artwork in a program drawn with one.
/// </remarks>
internal static class Marks
{
    /// <summary>The cog, drawn as a line rather than stamped from a file.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <param name="thickness">How wide the line is.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Cog(IBrush ink, double size, double thickness = 1.4) =>
        Drawn(
            "M8 5.2a2.8 2.8 0 1 0 0 5.6a2.8 2.8 0 0 0 0-5.6M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2"
                + "M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4",
            ink,
            size,
            size,
            thickness
        );

    /// <summary>
    /// The caret a button that opens a menu carries.
    /// </summary>
    /// <remarks>
    /// Its room is the shape's own: a caret is wider than it is tall, and one given a square box would be
    /// drawn in the top of it — which is a caret sitting above the line of the words it belongs to.
    /// </remarks>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="width">How wide it is drawn.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Caret(IBrush ink, double width) =>
        Drawn("M0 0l4 4l4-4", ink, width, width / 2, 1.2);

    /// <summary>One of the pictures drawn as a line.</summary>
    /// <param name="data">The path's drawing.</param>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="width">The room it is drawn in across.</param>
    /// <param name="height">The room it is drawn in down.</param>
    /// <param name="thickness">How wide the line is.</param>
    /// <returns>The drawing.</returns>
    private static ShapePath Drawn(string data, IBrush ink, double width, double height, double thickness) =>
        new()
        {
            Data = Geometry.Parse(data),
            Stroke = ink,
            StrokeThickness = thickness,
            StrokeLineCap = PenLineCap.Round,
            StrokeJoin = PenLineJoin.Round,
            Width = width,
            Height = height,
            Stretch = Stretch.Uniform,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Center,
            VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center,
            IsHitTestVisible = false,
        };
}

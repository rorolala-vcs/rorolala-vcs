using Avalonia;
using Avalonia.Media;
using ShapePath = Avalonia.Controls.Shapes.Path;

namespace RorolalaLauncher;

/// <summary>
/// The pictures this program draws for itself: the caret a menu button carries, and the small marks the
/// docks and their menus are made of.
/// </summary>
/// <remarks>
/// Everything else the program is drawn with is a picture from the pinned Material Design set, the
/// pictures the toolbar wears and the frame's own marks, because those are the product's artwork and are
/// drawn from one manifest by <c>cargo dev icons</c>. These are the exceptions: the set this project pins
/// has no rack, no refresh and no menu marks, and a mark from another set would be a second artwork in a
/// program drawn with one.
/// </remarks>
internal static class Marks
{
    /// <summary>The rack the Vaults dock is headed by.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Vault(IBrush ink, double size) =>
        Drawn(
            "M3.5 3h9a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z"
                + "M3.5 9h9a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-2a1 1 0 0 1 1-1Z"
                + "M5 5h.01M5 11h.01",
            ink,
            size,
            size,
            1.4
        );

    /// <summary>The window the Workspaces dock is headed by.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Workspace(IBrush ink, double size) =>
        Drawn(
            "M3.4 3.2h9.2a1 1 0 0 1 1 1v8.6a1 1 0 0 1-1 1H3.4a1 1 0 0 1-1-1V4.2a1 1 0 0 1 1-1Z"
                + "M2.4 6.4h11.2M6.5 6.4v7.4",
            ink,
            size,
            size,
            1.4
        );

    /// <summary>The plus a button that adds something carries.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Plus(IBrush ink, double size) =>
        Drawn("M8 3.4v9.2M3.4 8h9.2", ink, size, size, 1.4);

    /// <summary>The arrow that checks an address again.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Refresh(IBrush ink, double size) =>
        Drawn(
            "M12.6 8a4.6 4.6 0 1 1-1.5-3.4M12.8 2.9v2.4h-2.4",
            ink,
            size,
            size,
            1.4
        );

    /// <summary>The pencil a menu that edits an address carries.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Pencil(IBrush ink, double size) =>
        Drawn(
            "M10.4 3.4l2.2 2.2M3 13l.5-2.6L10 3.9l2.1 2.1-6.5 6.5L3 13Z",
            ink,
            size,
            size,
            1.3
        );

    /// <summary>The bin a menu that removes an address carries.</summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Trash(IBrush ink, double size) =>
        Drawn(
            "M3.4 4.5h9.2M6.4 4.5V3.1h3.2v1.4M4.5 4.5l.6 8a1 1 0 0 0 1 .9h3.8a1 1 0 0 0 1-.9l.6-8",
            ink,
            size,
            size,
            1.3
        );

    /// <summary>
    /// The tick a chosen language carries.
    /// </summary>
    /// <param name="ink">What it is drawn in.</param>
    /// <param name="size">The room it is drawn in, on both axes.</param>
    /// <returns>The drawing.</returns>
    public static ShapePath Check(IBrush ink, double size) =>
        Drawn("M3.2 8.4l3.1 3.1 6.5-6.6", ink, size, size, 1.4);

    /// <summary>
    /// The caret a button that opens a menu carries.
    /// </summary>
    /// <remarks>
    /// Its room is the shape's own: a caret is wider than it is tall, and one given a square box would be
    /// drawn in the top of it, which is a caret sitting above the line of the words it belongs to.
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

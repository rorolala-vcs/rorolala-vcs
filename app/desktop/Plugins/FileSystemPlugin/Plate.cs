using Avalonia;
using Avalonia.Controls;
using Avalonia.Markup.Xaml.MarkupExtensions;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The rounded rectangle a grid's picture stands on, whatever the listing is of.
/// </summary>
/// <remarks>
/// A file's icon and a place's icon stand on the same plate: the <c>Mid</c> ground with a <c>Splitter</c> line,
/// and the picture inset so that the plate reads as its frame. What the plate is made of is stated here rather
/// than where a tile is drawn, because a grid draws two kinds of entry and a plate drawn twice would be two
/// plates that disagree the first time either was changed.
/// <para>
/// The plate is the tile: its size is the tile's, and the room a badge stands in is taken around it by the grid
/// (see <see cref="BadgeArc.Reach"/>), not by the plate itself.
/// </para>
/// </remarks>
internal static class Plate
{
    /// <summary>How round the plate's corners are, as a share of the tile.</summary>
    private const double CornerShare = 0.14;

    /// <summary>How much of the tile is left around the picture, as a share of it.</summary>
    private const double InsetShare = 0.07;

    /// <summary>How round the plate's corners are at a tile of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <returns>The corner radius, at least two pixels.</returns>
    public static double Corner(double size) =>
        Math.Max(2, Math.Round(size * CornerShare, MidpointRounding.AwayFromZero));

    /// <summary>How round the corners of a thumbnail inside the plate are.</summary>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <returns>The inner corner radius, at least two pixels.</returns>
    public static double Inner(double size) =>
        Math.Max(2, Corner(size) - (size * InsetShare));

    /// <summary>How wide and tall the plate's content is at a tile of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <returns>The content's size, the tile less the margin on both sides.</returns>
    public static int Content(int size) =>
        Math.Max(1, (int)Math.Round(size * (1 - (2 * InsetShare)), MidpointRounding.AwayFromZero));

    /// <summary>
    /// The plate itself, holding what the tile draws.
    /// </summary>
    /// <remarks>
    /// It is a resource rather than a colour so that the ground and the line are the look's, and so that a colour
    /// changed while the program runs reaches a tile that is already drawn.
    /// </remarks>
    /// <param name="size">How many pixels wide and tall the tile is.</param>
    /// <param name="content">What stands on the plate, which the plate stretches to its own content area.</param>
    /// <returns>The plate.</returns>
    public static Border Frame(int size, Control content)
    {
        ArgumentNullException.ThrowIfNull(content);

        var plate = new Border
        {
            Width = size,
            Height = size,
            CornerRadius = new CornerRadius(Corner(size)),
            BorderThickness = new Thickness(1),
            ClipToBounds = true,
            Child = content,
        };

        plate[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Mid);
        plate[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        return plate;
    }
}

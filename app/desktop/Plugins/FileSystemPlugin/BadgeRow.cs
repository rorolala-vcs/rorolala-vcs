using Avalonia;
using Avalonia.Controls;

namespace FileSystemPlugin;

/// <summary>
/// The badges an entry's icon wears, in one row along its top right corner.
/// </summary>
/// <remarks>
/// A row rather than a ring about the corner: the marks read as one line, the first at the left and the last
/// nearest the corner, which is the order the contributions were registered in and the order a tree row wears
/// them in. The row hangs a little past the picture's top and right edges, the way a mark worn on a tile does,
/// and the rest of it lies over the picture.
/// <para>
/// The row is what limits the count. A badge that cannot stand clear of its neighbour says nothing that the
/// one beside it does not, so a row narrower than the picture takes what fits and drops the rest, the
/// contribution registered last first.
/// </para>
/// </remarks>
internal sealed class BadgeRow : Panel
{
    /// <summary>How many badges one icon wears at most.</summary>
    /// <remarks>
    /// Five, which is where a row stops being read as a set at the size an icon opens at: past that the badges
    /// are crowded together, and a badge that cannot be told from its neighbour is not a badge. What does not
    /// fit is dropped rather than crowded into the same row.
    /// </remarks>
    public const int Most = 5;

    /// <summary>
    /// How much of the icon's width one badge takes.
    /// </summary>
    /// <remarks>
    /// A share rather than a size, because an icon follows the zoom: a badge of a fixed size is a blob on the
    /// smallest icon and a speck on the largest. The share is the one the design was drawn at, a badge
    /// 34/160ths of the square it stands on.
    /// </remarks>
    private const double BadgeShare = 34.0 / 160.0;

    /// <summary>The least a badge is drawn at, in pixels.</summary>
    /// <remarks>
    /// What keeps the picture inside a badge drawable at the smallest an icon ever is: a badge a few pixels
    /// across is a badge whose glyph is not read at all, and what it is drawn for is said by nothing else.
    /// </remarks>
    private const double BadgeLeast = 10;

    /// <summary>How far apart two badges of the row stand, in pixels.</summary>
    /// <remarks>
    /// The same gap a tree row gives the badges at its right, because the two are one set of marks read at two
    /// sizes: a row of the corner read tighter than the row of a tree would make the same two badges look like
    /// two different things.
    /// </remarks>
    private const double Gap = 2;

    /// <summary>How far the row stands past the picture's edge, as a share of one badge.</summary>
    /// <remarks>
    /// A badge stands on the tile rather than inside the picture, so the row hangs a little past the top and
    /// right edges and the rest of it covers the corner of the picture. The share is what keeps that overhang
    /// the same at every zoom.
    /// </remarks>
    private const double OverhangShare = 0.3;

    /// <summary>How large one badge is at an icon of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels wide and tall the badge is.</returns>
    public static double Extent(double size) =>
        Math.Round(Math.Max(BadgeLeast, size * BadgeShare));

    /// <summary>How far the row stands past the picture's top and right edges, at an icon of `size`.</summary>
    /// <remarks>
    /// The room the tile has to leave around the picture, which is what the row reaches into: a badge drawn
    /// past the tile is a badge nothing points at.
    /// </remarks>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels past its top and right edges the row reaches.</returns>
    public static double Reach(double size) => Overhang(Extent(size));

    /// <summary>How many badges an icon wears of what its contributions answered.</summary>
    /// <remarks>
    /// Read out rather than tested where the badges are gathered, because the limit is the row's own: it is the
    /// row that runs out of room, and a second place that decided the count would be a second answer to how
    /// wide the corner is.
    /// </remarks>
    /// <param name="contributed">How many badges the contributions answered with.</param>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many are worn.</returns>
    public static int Worn(int contributed, double size) => Math.Min(contributed, Fit(size));

    /// <inheritdoc />
    protected override Size ArrangeOverride(Size finalSize)
    {
        // The badges are placed by their middles rather than by a side of the panel they are drawn in: the panel
        // is the picture, and a badge aligned within it would be a badge on an edge instead of on the corner.
        var size = Math.Min(finalSize.Width, finalSize.Height);

        if (Children.Count == 0)
        {
            return finalSize;
        }

        var overhang = Overhang(Extent(size));
        var row = (Children.Count - 1) * Gap;

        for (var at = 0; at < Children.Count; at++)
        {
            row += Children[at].DesiredSize.Width;
        }

        var x = size + overhang - row;
        var y = -overhang;

        for (var at = 0; at < Children.Count; at++)
        {
            var wanted = Children[at].DesiredSize;

            Children[at].Arrange(new Rect(x, y, wanted.Width, wanted.Height));
            x += wanted.Width + Gap;
        }

        return finalSize;
    }

    /// <summary>How many badges fit across the corner of an icon of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>The count, which is never fewer than one.</returns>
    private static int Fit(double size)
    {
        var extent = Extent(size);
        var room = size + (2 * Overhang(extent)) + Gap;
        var wide = extent + Gap;
        var fit = (int)Math.Floor(room / wide);

        return Math.Clamp(fit, 1, Most);
    }

    /// <summary>How far a badge of an extent hangs past the picture's edge.</summary>
    /// <param name="extent">How many pixels wide and tall the badge is.</param>
    /// <returns>How many pixels of it stand outside the picture.</returns>
    private static double Overhang(double extent) => Math.Max(1, Math.Round(extent * OverhangShare));
}

using Avalonia;
using Avalonia.Controls;

namespace FileSystemPlugin;

/// <summary>
/// The badges an entry's icon wears, laid around its top right corner.
/// </summary>
/// <remarks>
/// The badges stand on an arc whose centre is the middle of the icon's top right quarter and whose radius is
/// between that quarter's inscribed and circumscribed circles, a little inside the mean of the two — see
/// [`Radius`]. That is the ring a corner reads as holding: nearer than the inscribed circle the badges crowd
/// the middle of the picture, and further than the circumscribed one they have left the corner altogether. The
/// whole arc is at most a quarter turn and the middle of it points at the icon's own top right corner, so one
/// badge stands on the diagonal and a crowd spreads about it rather than marching along an edge.
/// <para>
/// A badge is a little wider than the room between the ring and the icon's edge, so the ends of a full arc
/// stand a few pixels outside the icon and nothing clips them: a mark worn on the tile reads as a mark worn on
/// the tile, which is what a badge on a picture is. What that needs is the room the tile already leaves around
/// its icon.
/// </para>
/// <para>
/// The badges are numbered from the counter-clockwise end of the arc and run clockwise, which is the order
/// the contributions were registered in: the first badge is the one nearest the top of the icon and the last
/// is the one nearest its right edge.
/// </para>
/// <para>
/// The arc is what limits the count. A badge is wider than the ring it stands on, so past a handful the
/// badges of one arc meet; the whole arc is held to a quarter turn rather than grown to fit them, which is
/// what keeps the badges on the corner and lets a full arc crowd rather than spread onto the picture.
/// </para>
/// </remarks>
internal sealed class BadgeArc : Panel
{
    /// <summary>How many badges one icon wears at most.</summary>
    /// <remarks>
    /// Five, which is where an arc of a quarter turn stops being read as a set: past that the badges stand on
    /// top of one another, and a badge that cannot be told from its neighbour is not a badge. What is not
    /// drawn is dropped rather than crowded into the same arc, since the contribution that is dropped is the
    /// one registered last and a listing that drew it twice over would be worse than one that did not.
    /// </remarks>
    public const int Most = 5;

    /// <summary>How much of the icon's width one badge takes.</summary>
    /// <remarks>
    /// A share rather than a size, because an icon follows the zoom: a badge of a fixed size is a blob on the
    /// smallest icon and a speck on the largest. The share is the one the design was drawn at — a badge
    /// 34/160ths of the square it stands on.
    /// </remarks>
    private const double BadgeShare = 34.0 / 160.0;

    /// <summary>The least a badge is drawn at, in pixels.</summary>
    /// <remarks>
    /// What keeps the picture inside a badge drawable at the smallest an icon ever is: a badge a few pixels
    /// across is a badge whose glyph is not read at all, and what it is drawn for is said by nothing else.
    /// </remarks>
    private const double BadgeLeast = 10;

    /// <summary>The greatest spread of the whole arc, which is a quarter turn.</summary>
    /// <remarks>
    /// The constraint the arrangement is built around: an arc wider than the corner it belongs to is an arc
    /// that has left the corner, and what stands at its far end reads as being about the picture rather than
    /// about where the entry stands in it.
    /// </remarks>
    private const double Widest = Math.PI / 2;

    /// <summary>
    /// How far the ring stands between the quarter's inscribed and circumscribed circles.
    /// </summary>
    /// <remarks>
    /// Half of the way is the mean of the two, which is where the design was first drawn: the ring is pulled in
    /// from it so that a badge reaches the icon's own corner by no more than the mark it is — see
    /// <see cref="Radius"/>.
    /// </remarks>
    private const double Between = 0.3;

    /// <summary>How large one badge is at an icon of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels wide and tall the badge is.</returns>
    public static double Extent(double size) => Math.Max(BadgeLeast, size * BadgeShare);

    /// <summary>
    /// How far the badges stand from the middle of the icon's top right quarter, at an icon of `size`.
    /// </summary>
    /// <remarks>
    /// The quarter's inscribed circle has half its edge as a radius and its circumscribed one has
    /// <c>halfSize·√2/2</c>; the ring is drawn between the two, at <see cref="Between"/> of the way from the
    /// first to the second rather than at the mean of them.
    /// <para>
    /// The mean is where the design was first drawn and it is a little too far out: a badge standing there
    /// reaches past the icon's own corner by more than a mark worn on the tile reads as, and the whole arc
    /// ends up looking laid against the edge rather than on the corner. Pulling the ring in towards the
    /// inscribed circle brings a full arc back onto the picture without crowding the middle of it — the arc
    /// still spreads about the corner, since it is the angle that says where a badge stands and not the ring.
    /// </para>
    /// </remarks>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels from the quarter's middle a badge stands.</returns>
    public static double Radius(double size) =>
        size * (0.25 + (Between * ((Math.Sqrt(2) / 4) - 0.25)));

    /// <summary>How many badges an icon wears of what its contributions answered.</summary>
    /// <remarks>
    /// Read out rather than tested where the badges are gathered, because the limit is the arc's own: it is
    /// the arc that cannot tell one badge from its neighbour, and a second place that decided the count would
    /// be a second answer to how wide the corner is.
    /// </remarks>
    /// <param name="contributed">How many badges the contributions answered with.</param>
    /// <returns>How many are worn.</returns>
    public static int Worn(int contributed) => Math.Min(contributed, Most);

    /// <summary>
    /// The middle of the badge a contribution wears, in the icon's own coordinates.
    /// </summary>
    /// <remarks>
    /// Read out rather than arranged in place so that the arrangement can be checked without a renderer: what
    /// is written here is the whole of the geometry the arc is, and a badge drawn from anything else would be
    /// a badge off the arc it was asked to stand on.
    /// </remarks>
    /// <param name="at">Which badge, from nothing, in the order the contributions were registered.</param>
    /// <param name="count">How many badges the icon wears.</param>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>Where the middle of the badge stands.</returns>
    public static Point Centre(int at, int count, double size)
    {
        // The middle of the icon's top right quarter: the square is divided in two each way and this is the
        // part that holds the corner the badges are read at.
        var quarter = new Point(size * 3 / 4, size / 4);
        var radius = Radius(size);

        // Towards the icon's top right corner, which is the middle of the arc and the direction a lone badge
        // stands in. The sign is the screen's: y grows downwards, so the corner is at a negative angle.
        var towards = -Math.PI / 4;

        if (count <= 1)
        {
            return On(quarter, radius, towards);
        }

        // The gap two badges want so that they meet without overlapping, from the triangle between the two
        // middles and the ring's own centre: half the chord they subtend is the badge's radius, and the
        // angle it subtends is twice the arcsine of that over the ring's radius.
        //
        // A badge wider than the ring is one whose whole arc cannot hold it — the smallest an icon is, at the
        // smallest zoom — and there is no angle to take an arcsine of past that. Held at a half turn each,
        // which is what the arc's own limit then answers with, the badges crowd as tightly as the limit
        // allows rather than being placed by a number that is not a direction.
        var wanted = 2 * Math.Asin(Math.Min(1, Extent(size) / 2 / radius));

        // The arc is as wide as the badges want until that exceeds a quarter turn, and no wider: a crowd is
        // answered with by crowding the badges rather than by spreading the arc past the corner.
        var span = Math.Min((count - 1) * wanted, Widest);

        // Numbered from the counter-clockwise end, so a badge's number is read in the direction the arc runs.
        return On(quarter, radius, towards - (span / 2) + (at * (span / (count - 1))));
    }

    /// <inheritdoc />
    protected override Size ArrangeOverride(Size finalSize)
    {
        // The badges stand on a ring about the corner, so each is placed by where its middle is rather than
        // by a side of the panel it is drawn in: the panel is the icon, and a badge aligned within it would
        // be a badge on an edge instead.
        var size = Math.Min(finalSize.Width, finalSize.Height);

        for (var at = 0; at < Children.Count; at++)
        {
            var badge = Children[at];
            var wanted = badge.DesiredSize;
            var middle = Centre(at, Children.Count, size);

            badge.Arrange(
                new Rect(
                    middle.X - (wanted.Width / 2),
                    middle.Y - (wanted.Height / 2),
                    wanted.Width,
                    wanted.Height
                )
            );
        }

        return finalSize;
    }

    /// <summary>Where a point on the ring stands, in the icon's own coordinates.</summary>
    /// <param name="quarter">The ring's centre.</param>
    /// <param name="radius">The ring's radius.</param>
    /// <param name="angle">Which way round the ring, in radians, measured on the screen.</param>
    /// <returns>The point.</returns>
    private static Point On(Point quarter, double radius, double angle) =>
        new(quarter.X + (radius * Math.Cos(angle)), quarter.Y + (radius * Math.Sin(angle)));
}

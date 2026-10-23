using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Styling;
using FileSystemPlugin;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The pictures a plugin draws for its own furniture: that what the manifest names reached the assembly.
/// </summary>
/// <remarks>
/// What is checked is the link between the three things a picture passes through — the manifest, the run that
/// draws it, and the assembly the plugin is built as — because a break in any of them is a window that opens
/// with a hole in its toolbar and nothing said. What is not checked is the drawing: that needs a renderer, and
/// it is <c>cargo dev icons</c> that runs one.
/// </remarks>
public sealed class IconTests
{
    /// <summary>Every picture the manifest names is embedded in this plugin's own assembly.</summary>
    [Fact]
    public void EveryPictureTheManifestNamesIsEmbedded()
    {
        Assert.NotEmpty(Icons.Pictures);

        foreach (var (key, name) in Icons.Pictures)
        {
            var resource = $"FileSystemPlugin.icons.{name}.png";

            using var stream = typeof(Icons).Assembly.GetManifestResourceStream(resource);

            Assert.NotNull(stream);
            Assert.StartsWith("rorolala_file_system.", key, StringComparison.Ordinal);
            Assert.EndsWith(name, key, StringComparison.Ordinal);
        }
    }

    /// <summary>Every picture a menu item is meant to wear is one this plugin draws.</summary>
    /// <remarks>
    /// What a menu item wears is looked up by the words it is named with, so a picture named there that the
    /// manifest does not hold is an item that draws its icon and finds nothing — which is a menu where one
    /// entry is a word where its neighbours are a mark and a word. The lookup itself cannot be exercised here,
    /// because what it makes is a picture and a picture needs a screen to be made on.
    /// </remarks>
    [Fact]
    public void EveryPictureAMenuItemWearsIsOneThisPluginDraws()
    {
        var drawn = Icons.Pictures.Select(picture => picture.Key).ToHashSet(StringComparer.Ordinal);

        Assert.NotEmpty(BrowserActions.Pictures);
        Assert.All(BrowserActions.Pictures.Values, picture => Assert.Contains(picture, drawn));
    }

    /// <summary>Badges stand on an arc about the icon's top right corner, and never past a quarter turn.</summary>
    /// <remarks>
    /// The arrangement is checked as geometry rather than as a drawing, because a drawing needs a renderer
    /// and this suite has none: what is here is the whole of what places a badge — the ring it stands on, the
    /// direction a lone one stands in, the order the numbers run, and the arc's own limit — and a badge drawn
    /// anywhere else is a badge off the arc it was asked to stand on.
    /// </remarks>
    [Fact]
    public void BadgesStandOnAnArcAboutTheTopRightCorner()
    {
        const int size = 64;

        // The ring is centred on the icon's top right quarter, and it stands between that quarter's inscribed
        // and circumscribed circles — inside the mean of the two, which is what keeps a badge from reaching
        // past the corner by more than the mark it is.
        var quarter = new Point(size * 3 / 4, size / 4);
        var radius = BadgeArc.Radius(size);
        var inscribed = size / 4.0;
        var circumscribed = size * Math.Sqrt(2) / 4;

        Assert.True(radius > inscribed, "the ring has crowded the middle of the quarter");
        Assert.True(radius < (inscribed + circumscribed) / 2, "the ring is not pulled in from the mean");
        Assert.True(radius < circumscribed, "the ring leaves the quarter it belongs to");

        // One badge stands up and to the right of the icon's middle: the direction of the corner itself.
        var alone = BadgeArc.Centre(0, 1, size);

        Assert.True(alone.X > size / 2.0, "a lone badge is not towards the right");
        Assert.True(alone.Y < size / 2.0, "a lone badge is not towards the top");
        Assert.Equal(radius, Distance(alone, quarter), 1e-9);

        // Every badge of an arc stands on the ring, and the whole arc is a quarter turn at the most.
        for (var count = 1; count <= BadgeArc.Most; count++)
        {
            var placed = Enumerable
                .Range(0, count)
                .Select(at => BadgeArc.Centre(at, count, size))
                .ToArray();

            Assert.All(placed, at => Assert.Equal(radius, Distance(at, quarter), 1e-9));

            var angles = placed.Select(at => Math.Atan2(at.Y - quarter.Y, at.X - quarter.X)).ToArray();

            Assert.True(
                angles.Max() - angles.Min() <= (Math.PI / 2) + 1e-9,
                $"{count} badges spread past a quarter turn"
            );
        }

        // The numbers run clockwise on the screen: the second badge of a pair is the further right and the
        // further down, which is what makes the first the one read at the top of the arc.
        var first = BadgeArc.Centre(0, 2, size);
        var second = BadgeArc.Centre(1, 2, size);

        Assert.True(second.X > first.X, "the second badge is not clockwise of the first");
        Assert.True(second.Y > first.Y, "the second badge is not clockwise of the first");
    }

    /// <summary>No icon wears more than five badges, however many are contributed.</summary>
    /// <remarks>
    /// An arc of a quarter turn stops being read as a set past a handful, so what is contributed beyond the
    /// last badge of it is dropped rather than crowded in: this is the count the drawing wears, and the one
    /// place it is decided.
    /// </remarks>
    [Fact]
    public void NoIconWearsMoreThanFiveBadges()
    {
        Assert.Equal(5, BadgeArc.Most);
        Assert.Equal(0, BadgeArc.Worn(0));
        Assert.Equal(3, BadgeArc.Worn(3));
        Assert.Equal(BadgeArc.Most, BadgeArc.Worn(BadgeArc.Most));
        Assert.Equal(BadgeArc.Most, BadgeArc.Worn(12));
    }

    /// <summary>A badge a contribution coloured itself is painted in that colour, ring and picture alike.</summary>
    /// <remarks>
    /// A contribution that has a palette of its own — the states a work's files are in, which the plugin owns
    /// and the user chooses — names a colour rather than one of the look's roles, and what it names is the whole
    /// of the badge: the picture is a mask over the colour and the ring is the ground's edge, so a picture drawn
    /// in the role and a ring drawn in the contribution's colour is one mark in two colours, which reads as two
    /// marks laid over each other rather than as a state.
    /// <para>
    /// It is drawn inside a window, with the look's own resources over it, and that is the point rather than
    /// ceremony: a resource a control carries is asked for when the control is attached to a tree and is
    /// answered last, so a badge that named a role beside the contribution's colour came out in the look's grey
    /// the moment it reached the screen — a fault that no windowless test can see.
    /// </para>
    /// </remarks>
    [Fact]
    public void ABadgeColouredByItsContributionIsPaintedInThatColour()
    {
        var look = new RorolalaTheme(
            ThemePalette.DefaultDark,
            Color.Parse("#223300"),
            WindowStyle.Windows
        );
        var resources = Assert.IsAssignableFrom<ResourceDictionary>(
            look.Palette.ThemeDictionaries[ThemeVariant.Dark]
        );
        var faint = Assert.IsType<SolidColorBrush>(resources[PaletteKeys.Faint]).Color;

        using var picture = Icons.Picture(Icons.Pictures[0].Key);
        var ours = new EntryMark(Colour: Colors.Lime, Tagged: true, Text: "it");
        var theirs = new EntryMark(Ink: MarkInk.Quiet, Tagged: true, Text: "it");

        var window = new Window
        {
            Resources = resources,
            Content = new StackPanel
            {
                Children =
                {
                    Icons.Marked(new Border(), [new EntryBadge(picture, ours)], 64),
                    Icons.Marked(new Border(), [new EntryBadge(picture, theirs)], 64),
                },
            },
        };

        window.Show();

        var both = Assert.IsType<StackPanel>(window.Content);

        // The ground the badge is worn on is the mark's own rather than the look's, and the picture and the ring
        // drawn on it are the contribution's colour — the look's resources being in reach and all.
        var (ball, glyph) = Worn(Assert.IsType<Grid>(both.Children[0]));

        Assert.Equal(Colors.Lime, Assert.IsType<SolidColorBrush>(ball.BorderBrush).Color);
        Assert.Equal(Colors.Lime, Assert.IsType<SolidColorBrush>(glyph.Background).Color);
        Assert.NotEqual(Colors.Lime, Assert.IsType<SolidColorBrush>(ball.Background).Color);

        // A mark that named no colour is the look's own answer, which is the other half of the same rule.
        var (quiet, quietGlyph) = Worn(Assert.IsType<Grid>(both.Children[1]));

        Assert.Equal(faint, Assert.IsType<SolidColorBrush>(quiet.BorderBrush).Color);
        Assert.Equal(faint, Assert.IsType<SolidColorBrush>(quietGlyph.Background).Color);
    }

    /// <summary>The ball and the picture one drawn badge is made of, wherever the arc put them.</summary>
    /// <param name="marked">What the icon was drawn as.</param>
    /// <returns>The ball, and the picture inside it.</returns>
    private static (Border Ball, Border Glyph) Worn(Grid marked)
    {
        var arc = Assert.IsType<BadgeArc>(marked.Children[1]);
        var ball = Assert.IsType<Border>(
            Assert.IsType<ThemeVariantScope>(Assert.Single(arc.Children)).Child
        );

        return (ball, Assert.IsType<Border>(ball.Child));
    }

    /// <summary>The arc puts a badge where the geometry says, rather than in the panel's own corner.</summary>
    /// <remarks>
    /// The arrangement is what a badge is actually drawn at, and it is the one part of the arc that the
    /// geometry above does not reach: a panel that arranged its children the way a panel ordinarily does would
    /// put every badge in the same place, which is one corner worn by as many badges as were contributed and
    /// no sign of the others.
    /// </remarks>
    [Fact]
    public void TheArcPutsItsBadgesWhereTheGeometrySays()
    {
        const int size = 64;
        var arc = new BadgeArc();

        for (var at = 0; at < 3; at++)
        {
            arc.Children.Add(
                new Border { Width = BadgeArc.Extent(size), Height = BadgeArc.Extent(size) }
            );
        }

        arc.Measure(new Size(size, size));
        arc.Arrange(new Rect(0, 0, size, size));

        // What is arranged is snapped to whole pixels, because that is what the toolkit does with everything
        // it lays out: a badge lands within a pixel of the ring rather than on it to the last digit. What is
        // checked is that it lands there at all — a panel that arranged its children the way a panel
        // ordinarily does would put all three in one place, which is many pixels from two of them.
        const double slack = 1.0;

        for (var at = 0; at < arc.Children.Count; at++)
        {
            var middle = BadgeArc.Centre(at, arc.Children.Count, size);
            var placed = arc.Children[at].Bounds;

            Assert.Equal(middle.X, placed.Center.X, slack);
            Assert.Equal(middle.Y, placed.Center.Y, slack);
            Assert.Equal(BadgeArc.Extent(size), placed.Width, slack);
        }
    }

    /// <summary>How far apart two points are.</summary>
    /// <param name="one">One point.</param>
    /// <param name="other">The other.</param>
    /// <returns>The distance between them.</returns>
    private static double Distance(Point one, Point other) =>
        Math.Sqrt(((one.X - other.X) * (one.X - other.X)) + ((one.Y - other.Y) * (one.Y - other.Y)));
}

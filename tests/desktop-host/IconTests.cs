using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The pictures a plugin draws for its own furniture: that what the manifest names reached the assembly.
/// </summary>
/// <remarks>
/// What is checked is the link between the three things a picture passes through, the manifest, the run that
/// draws it, and the assembly the plugin is built as, because a break in any of them is a window that opens
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
    /// manifest does not hold is an item that draws its icon and finds nothing, which is a menu where one
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

    /// <summary>Badges stand in a row along the icon's top right corner, in the order they were contributed.</summary>
    /// <remarks>
    /// The arrangement is checked as geometry rather than as a drawing, because a drawing needs a renderer
    /// and this suite has none: what is here is the whole of what places a badge, one line of them at the same
    /// height, the last reaching past the corner and the first nearest the middle of the picture, and a badge
    /// drawn anywhere else is a badge off the row it was asked to stand on.
    /// </remarks>
    [Fact]
    public void BadgesStandInARowAlongTheTopRightCorner()
    {
        const int size = 64;
        const int count = 3;

        var row = new BadgeRow();

        for (var at = 0; at < count; at++)
        {
            row.Children.Add(new Border { Width = BadgeRow.Extent(size), Height = BadgeRow.Extent(size) });
        }

        row.Measure(new Size(size, size));
        row.Arrange(new Rect(0, 0, size, size));

        var extent = BadgeRow.Extent(size);
        var reach = BadgeRow.Reach(size);
        var placed = row.Children.Select(child => child.Bounds).ToArray();

        // The row hangs the same share past the picture's top and right edges, and the badge nearest the corner
        // is the last one contributed.
        Assert.Equal(size + reach, placed[^1].Right, 1e-9);
        Assert.Equal(-reach, placed[0].Top, 1e-9);
        Assert.True(placed[0].Left >= -reach, "the row has left the room the tile reserves");

        for (var at = 0; at < placed.Length; at++)
        {
            Assert.Equal(placed[0].Top, placed[at].Top, 1e-9);
            Assert.Equal(extent, placed[at].Width, 1e-9);
            Assert.Equal(extent, placed[at].Height, 1e-9);

            if (at > 0)
            {
                Assert.True(
                    placed[at].Left >= placed[at - 1].Right,
                    "two badges of the row stand over one another"
                );
            }
        }
    }

    /// <summary>A full row stands no further past the picture than a tile's least room reserves.</summary>
    /// <remarks>
    /// The row hangs past the picture's edge, and the tile is the box a badge is worn in: what the tile leaves
    /// around the picture is at least what the row reaches into, so the badges of a tile stand on the tile
    /// rather than over the gap beside it, where the drawing is cut and the pointer does not reach. What is
    /// checked is the two halves of that one number, at every size a picture is drawn at.
    /// </remarks>
    [Fact]
    public void TheRowStandsNoFurtherThanATileReserves()
    {
        foreach (var size in new[] { Icons.Row, 64, 110, Icons.SizeAt(200), Icons.SizeAt(400) })
        {
            var reach = BadgeRow.Reach(size);
            var worn = BadgeRow.Worn(12, size);
            var row = new BadgeRow();

            for (var at = 0; at < worn; at++)
            {
                row.Children.Add(
                    new Border { Width = BadgeRow.Extent(size), Height = BadgeRow.Extent(size) }
                );
            }

            row.Measure(new Size(size, size));
            row.Arrange(new Rect(0, 0, size, size));

            var placed = row.Children.Select(child => child.Bounds).ToArray();

            Assert.Equal(size + reach, placed[^1].Right, 1e-9);
            Assert.Equal(-reach, placed[0].Top, 1e-9);

            Assert.True(
                placed[0].Left >= -reach - 1e-9,
                $"{worn} badges at {size} reach further left than the tile reserves"
            );
        }

        // At the size a grid opens at, the room a tile already leaves is the whole of what the row needs: a
        // listing at rest is not spread out for a mark it does not wear.
        Assert.True(BadgeRow.Reach(64) <= 12);
    }

    /// <summary>No icon wears more badges than its row holds, and never more than five.</summary>
    /// <remarks>
    /// A full row stops being read as a set past a handful, and a row narrower than the picture holds fewer
    /// than that: what is contributed beyond what the row can hold is dropped rather than crowded in, since a
    /// badge nothing can tell from the one beside it says nothing.
    /// </remarks>
    [Fact]
    public void NoIconWearsMoreBadgesThanItsRowHolds()
    {
        Assert.Equal(5, BadgeRow.Most);
        Assert.Equal(0, BadgeRow.Worn(0, 64));
        Assert.Equal(3, BadgeRow.Worn(3, 64));

        // Four badges fit across the icon a grid opens at, and five at the largest an icon is drawn at.
        Assert.Equal(4, BadgeRow.Worn(12, 64));
        Assert.Equal(BadgeRow.Most, BadgeRow.Worn(12, Icons.SizeAt(400)));
    }

    /// <summary>A bead is the look's face tinted with the contribution's colour, its picture that colour.</summary>
    /// <remarks>
    /// A contribution that has a palette of its own, the states a work's files are in, which the plugin owns
    /// and the user chooses, names a colour rather than one of the look's roles, and what it names is the whole
    /// of the badge: the ground is the look's face taken part of the way to that colour and the picture is drawn
    /// in it. A picture drawn in the role instead would be one mark in two colours, which reads as two marks
    /// laid over each other rather than as a state.
    /// <para>
    /// It is drawn inside a window, with the look's own resources over it, and that is the point rather than
    /// ceremony: a resource a control carries is asked for when the control is attached to a tree and is
    /// answered last, so a badge that named a role beside the contribution's colour came out in the look's grey
    /// the moment it reached the screen, a fault that no windowless test can see.
    /// </para>
    /// </remarks>
    [Fact]
    public void ABadgeColouredByItsContributionIsTintedWithThatColour()
    {
        var application = Application.Current!;
        var variant = application.RequestedThemeVariant;
        var styles = application.Styles.Count;
        var (theme, _) = ThemeService.Make(application, new ThemeConfiguration());

        theme.Apply();
        application.RequestedThemeVariant = ThemeVariant.Dark;
        Pump();

        using var picture = Icons.Picture(Icons.Pictures[0].Key);
        var ours = new EntryMark(Colour: Colors.Lime, Tagged: true, Text: "it");
        var theirs = new EntryMark(Ink: MarkInk.Quiet, Tagged: true, Text: "it");

        var window = new Window
        {
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
        Pump();

        try
        {
            var bright = Colour(PaletteKeys.Bright);
            var faint = Colour(PaletteKeys.Faint);
            var both = Assert.IsType<StackPanel>(window.Content);

            // The bead is the look's face tinted with the mark's colour, its picture that colour taken towards
            // white on the dark look, and a ring of the bead's own ground rather than a shadow.
            var (ball, glyph) = Worn(Assert.IsType<Grid>(both.Children[0]));
            var ground = Mix(bright, Colors.Lime, 0.35);

            Assert.Equal(ground, Solid(ball.Background));
            Assert.Equal(Mix(ground, Colors.White, 0.10), Solid(ball.BorderBrush));
            Assert.Equal(1, ball.BorderThickness.Left);
            Assert.Equal(default, ball.BoxShadow);
            Assert.Equal(Mix(Colors.Lime, Colors.White, 0.25), Solid(glyph.Background));

            // A mark that named no colour is the look's own answer, which is the other half of the same rule.
            var (quiet, quietGlyph) = Worn(Assert.IsType<Grid>(both.Children[1]));

            Assert.Equal(Mix(bright, faint, 0.35), Solid(quiet.Background));
            Assert.Equal(Mix(faint, Colors.White, 0.25), Solid(quietGlyph.Background));
        }
        finally
        {
            window.Close();

            while (application.Styles.Count > styles)
            {
                application.Styles.RemoveAt(application.Styles.Count - 1);
            }

            application.RequestedThemeVariant = variant;
            Pump();
        }
    }

    /// <summary>The ball and the picture one drawn badge is made of.</summary>
    /// <param name="marked">What the icon was drawn as.</param>
    /// <returns>The ball, and the picture inside it.</returns>
    private static (Border Ball, Border Glyph) Worn(Grid marked)
    {
        var row = Assert.IsType<BadgeRow>(marked.Children[1]);
        var ball = Assert.IsType<Border>(Assert.Single(row.Children));

        return (ball, Assert.IsType<Border>(ball.Child));
    }

    /// <summary>The colour a brush holds.</summary>
    /// <param name="brush">The brush.</param>
    /// <returns>Its colour.</returns>
    private static Color Solid(IBrush? brush) => Assert.IsType<SolidColorBrush>(brush).Color;

    /// <summary>A role the look publishes, as the dark look answers it.</summary>
    /// <param name="key">The resource key.</param>
    /// <returns>The colour.</returns>
    private static Color Colour(string key) =>
        Application.Current!.TryFindResource(key, ThemeVariant.Dark, out var found)
        && found is ISolidColorBrush brush
            ? brush.Color
            : throw new InvalidOperationException($"the look states no {key}");

    /// <summary>Lets the window run a few turns of its loop, which is what lays the badges out.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>A colour taken part of the way to another, which is what the badge's two are made of.</summary>
    /// <param name="from">Where it starts.</param>
    /// <param name="to">Where it ends.</param>
    /// <param name="share">How far along.</param>
    /// <returns>The mixed colour.</returns>
    private static Color Mix(Color from, Color to, double share)
    {
        byte At(byte a, byte b) => (byte)Math.Round(a + ((b - a) * share));

        return Color.FromArgb(from.A, At(from.R, to.R), At(from.G, to.G), At(from.B, to.B));
    }
}

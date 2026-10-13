using Avalonia;
using Avalonia.Controls;
using Avalonia.Styling;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.SysIcons;

namespace FileSystemPlugin;

/// <summary>
/// The icons this plugin draws: what entries are shown with, and the marks its own toolbar wears.
/// </summary>
/// <remarks>
/// What an entry is drawn as comes from the system: what a folder looks like is the desktop's business, and a
/// browser that draws its own folder is a browser that looks wrong on every desktop but the one it was drawn
/// for. The icon library is this, and what is left of it is a picture per kind of file,
/// which no system is asked about yet — a file is drawn as a file and not as a kind of one.
/// <para>
/// The toolbar's own pictures are the other way round, and are the program's: a switch that shows what the
/// rules hide, and whether a dock follows the whole, are this program's own ideas, which no system has a
/// picture for. They are drawn from the pinned Material Design set by <c>./run.sh icons</c> — the manifest is
/// <c>app/desktop/icons.toml</c> — and embedded in this assembly, so that what the host loads is the one file
/// it found the plugin in. The keys they are handed over under are generated from that manifest into the
/// other half of this class, so that no one writes them twice.
/// </para>
/// <para>
/// A system with no icon to give, or one this does not ask, falls back to a mark drawn here, which says
/// which of the two kinds a row is: a listing with no pictures beside it is still a listing.
/// </para>
/// </remarks>
internal static partial class Icons
{
    /// <summary>How wide and tall an icon is where no size is asked for.</summary>
    private const int Default = 16;

    /// <summary>The size a grid's icons are at the zoom it opens at.</summary>
    private const int Base = 64;

    /// <summary>How many pixels wide and tall the mark on a menu item is.</summary>
    /// <remarks>
    /// The look gives a menu item a square of sixteen pixels to put a picture in, and the picture is drawn to
    /// fill it: a mark smaller than the square it was given reads as a picture that lost its way in, and one
    /// larger is clipped by it.
    /// </remarks>
    public const int Menu = 16;

    /// <summary>
    /// The theme resource an icon of a tool that is not on is drawn in.
    /// </summary>
    /// <remarks>
    /// What is drawn in it is a mark that is nobody's button — a menu item's — and is therefore read
    /// from the look rather than from the control that holds it. Which name that is belongs to the
    /// look, which is why it is not written here (see <see cref="PaletteKeys"/>).
    /// </remarks>
    public const string PlainInk = PaletteKeys.PlainInk;

    /// <summary>How many pixels wide and tall a toolbar's icon is.</summary>
    /// <remarks>
    /// A tool is a square hit area of its own — 28 pixels, which the look gives it — and the picture sits
    /// inside it with room around it: a mark drawn to the edges of its target reads as a button with a picture
    /// for a background rather than as a picture with a button around it.
    /// </remarks>
    public const int Tool = 18;

    /// <summary>
    /// The step a zoom's icon size is rounded to.
    /// </summary>
    /// <remarks>
    /// A slider is dragged through every value between two, and an icon is read from the desktop once per
    /// size it is asked for: rounding to a step is what keeps a zoom from asking a theme for a picture per
    /// pixel of travel, and from keeping one per pixel afterwards.
    /// </remarks>
    private const int Step = 8;

    /// <summary>The colour a directory falls back to.</summary>
    private static readonly Color Directory = Color.Parse("#E8B339");

    /// <summary>The colour a file falls back to.</summary>
    private static readonly Color File = Color.Parse("#8A94A6");

    /// <summary>The icon for an entry, at the size a row draws it.
    /// </summary>
    /// <param name="entry">The entry to draw.</param>
    /// <returns>What to draw beside the entry's name.</returns>
    public static Control For(Entry entry) => For(entry, Default);

    /// <summary>The icon for an entry, at a size in pixels.</summary>
    /// <param name="entry">The entry to draw.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw beside the entry's name.</returns>
    public static Control For(Entry entry, int size)
    {
        var directory = entry.Kind == EntryKind.Directory;
        var icon = directory ? SysIcons.Directory(size) : SysIcons.File(size);

        return icon is null
            ? Marked(directory, size)
            : new Image
            {
                Source = icon,
                Width = size,
                Height = size,
                Stretch = Stretch.Uniform,
                VerticalAlignment = VerticalAlignment.Center,
            };
    }

    /// <summary>
    /// How large a grid's icons are at a zoom, in pixels.
    /// </summary>
    /// <param name="zoom">The zoom, as a percentage of the size they open at.</param>
    /// <returns>How many pixels wide and tall to ask the system for.</returns>
    public static int SizeAt(double zoom)
    {
        var wanted = (int)Math.Round(Base * zoom / 100.0, MidpointRounding.AwayFromZero);
        var rounded = (int)Math.Round(wanted / (double)Step, MidpointRounding.AwayFromZero) * Step;

        return Math.Max(Step, rounded);
    }

    /// <summary>
    /// How much of a badge is left around the picture inside it.
    /// </summary>
    /// <remarks>
    /// The room around the picture is a share of the badge rather than of the icon, since what it has to clear
    /// is the badge's own line: at this size a fixed few pixels is most of the badge at the smallest zoom and
    /// nothing at the largest. The least is what keeps a picture to be drawn at all on the smallest badge.
    /// </remarks>
    private const double BallInset = 0.2;

    /// <inheritdoc cref="BallInset" />
    private const double BallLeast = 2;

    /// <summary>How heavy the ring around a ball is.</summary>
    /// <remarks>
    /// Thicker than the hairline a control wears, because a ball is a few pixels across and a line thinner than
    /// this disappears into the antialiasing at the smallest zoom — and thinner than the border a letter tag
    /// wears, because at this size that would be a third of the ball.
    /// </remarks>
    private const double BallEdge = 1.5;

    /// <summary>How much of an icon's width a dot takes, and the least and most it is drawn at.</summary>
    /// <remarks>
    /// It follows the zoom for the same reason a mark does, and stops: a dot is read as a colour, and one
    /// grown past the size a colour is read at is a blob on the corner of a picture.
    /// </remarks>
    private const double DotShare = 0.2;
    private const double DotLeast = 8;
    private const double DotMost = 14;

    /// <summary>
    /// The ground a tagged mark is drawn on, which is the dark one on either look.
    /// </summary>
    /// <remarks>
    /// A colour of its own rather than a wash of the ink, because a badge on a picture is not a surface of the
    /// program: what it is drawn on decides how the ink on it reads, and the ink does not change with the look.
    /// A wash would be the theme's answer, which is the thing a mark worn on a picture must not have.
    /// </remarks>
    private static readonly Color TagGround = Color.Parse("#1A1A1A");

    /// <summary>The theme resource a contributed mark is drawn in.</summary>
    /// <remarks>
    /// The names are the look's own and are not written here: a plugin may not reference the host, so a
    /// literal in this file would be a second statement of a name the look publishes, and the two would
    /// become two the day one of them was corrected (see <see cref="PaletteKeys"/>).
    /// </remarks>
    /// <param name="ink">The role the mark is drawn in.</param>
    /// <returns>The name of the resource it is read from.</returns>
    public static string Ink(MarkInk ink) =>
        ink switch
        {
            MarkInk.Plain => PaletteKeys.Front,
            MarkInk.Primary => PaletteKeys.Primary,
            MarkInk.Accent => PaletteKeys.Accent,
            MarkInk.Error => PaletteKeys.Del,
            MarkInk.Add => PaletteKeys.Add,
            MarkInk.Warn => PaletteKeys.Warn,
            _ => PaletteKeys.Faint,
        };

    /// <summary>
    /// A contributed mark drawn as a dot.
    /// </summary>
    /// <remarks>
    /// A dot rather than a picture: what a column of marks says is a state, and a state is a colour before it
    /// is anything else. It is drawn against whatever it is put beside — a name in a tile, a name in a list —
    /// since what it means is the mark's own and where it stands is the view's. What it means is said when the
    /// pointer rests on it, which is what a colour cannot say by itself.
    /// </remarks>
    /// <param name="contributed">What the provider answered about the entry.</param>
    /// <param name="size">How many pixels wide and tall the entry's icon is, which the dot follows.</param>
    /// <returns>The dot to draw.</returns>
    public static Control Dot(EntryColumnMark contributed, int size)
    {
        var extent = DotExtent(size);
        var dot = new Border
        {
            Width = extent,
            Height = extent,
            CornerRadius = new CornerRadius(extent / 2),
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The colour is the contribution's where it named one, and the role it named otherwise: a mark drawn in
        // the role and then painted over would be drawn twice, and the role's own brush would win.
        if (contributed.Colour is { } colour)
        {
            dot.Background = new SolidColorBrush(colour);
        }
        else
        {
            dot[!Border.BackgroundProperty] = new DynamicResourceExtension(Ink(contributed.Ink));
        }

        if (contributed.Edge is { } edge)
        {
            dot.BorderThickness = new Thickness(1);
            dot.BorderBrush = new SolidColorBrush(edge);
        }

        if (contributed.Hint is { Length: > 0 } hint)
        {
            ToolTip.SetTip(dot, hint);

            // At once rather than after the wait a tooltip usually takes: the mark is a colour, and a reader
            // who has to stop and wait for what a colour means is a reader who has to stop.
            ToolTip.SetShowDelay(dot, 0);
        }

        return dot;
    }

    /// <summary>
    /// An entry's icon with the badges its contributions asked for around the top right corner.
    /// </summary>
    /// <remarks>
    /// Every contribution that speaks for the entry is worn at once, numbered the way it was registered: two
    /// badges say two different things — who holds the entry, and how far its version has come — and neither
    /// is the other's to replace.
    /// <para>
    /// The picture is drawn as a mask over a coloured ground rather than as a picture, so that it takes the
    /// ink the theme gives it: a picture carries whatever colours it was drawn with, and a mark that had to be
    /// drawn once per colour would be a mark per theme.
    /// </para>
    /// <para>
    /// A mark the contribution asked to have tagged is drawn on the dark ground a badge wears, which is how
    /// the shell draws a state, so that a badge saying an entry is somebody else's reads as an answer rather
    /// than as another icon beside the file's own.
    /// </para>
    /// </remarks>
    /// <param name="icon">What the entry is drawn as.</param>
    /// <param name="badges">What the contributions said, in the order the badges are numbered.</param>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>The icon and its badges, as one thing to draw.</returns>
    public static Control Marked(Control icon, IReadOnlyList<EntryBadge> badges, int size)
    {
        ArgumentNullException.ThrowIfNull(icon);
        ArgumentNullException.ThrowIfNull(badges);

        // Nothing contributed is nothing drawn: an icon that wore an empty panel would be one the layout has
        // to place for an answer that is not there.
        if (badges.Count == 0)
        {
            return icon;
        }

        var arc = new BadgeArc();

        // Only the first of them are worn. Past that the arc cannot tell one badge from its neighbour, and
        // what it cannot tell is not worn rather than worn on top of another (see BadgeArc.Most).
        for (var at = 0; at < BadgeArc.Worn(badges.Count); at++)
        {
            arc.Children.Add(Badge(badges[at], size));
        }

        var grid = new Grid { Width = size, Height = size };
        grid.Children.Add(icon);
        grid.Children.Add(arc);

        return grid;
    }

    /// <summary>
    /// One badge drawn: the picture on the ground it was asked to wear, or bare in the ink.
    /// </summary>
    /// <param name="badge">The picture and what the contribution said about the entry.</param>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>What to stand on the arc.</returns>
    private static Control Badge(EntryBadge badge, int size)
    {
        var extent = BadgeArc.Extent(size);
        var colour = Ink(badge.Mark.Ink);

        Control drawn;

        if (badge.Mark.Tagged)
        {
            // The picture is taken off the badge's own edge, and the badge's line is the ground's edge rather
            // than the picture's: a mark drawn to the very edge of what holds it reads as a picture with a
            // border drawn over it rather than as a picture inside something.
            var room = Math.Max(BallLeast, extent * BallInset);
            var glyph = Glyph(badge.Picture, colour, extent - (2 * room));
            glyph.CornerRadius = new CornerRadius(glyph.Width / 2);

            var worn = new Border
            {
                Width = extent,
                Height = extent,
                // A hairline of the ring, since the badge is a few pixels across: a line as heavy as the
                // letter tags wear would be a third of the badge and would crowd the picture inside it.
                BorderThickness = new Thickness(BallEdge),
                CornerRadius = new CornerRadius(extent / 2),
                Child = glyph,
            };
            // The ground is the mark's own and always the dark one, whatever the program is drawn on: a badge
            // worn on a picture is part of the picture, and one that turned pale on the light look would be a
            // mark that changed what it is when the user changed their colours. What says which mark it is is
            // the ring and the picture, both in the role's ink.
            worn.Background = new SolidColorBrush(TagGround);
            worn[!Border.BorderBrushProperty] = new DynamicResourceExtension(colour);

            // The ink the badge is drawn in is the dark look's, whatever the program is drawn on: the ground
            // is the dark one, so the colour that reads on it is the one the dark look holds — a mark that took
            // the light look's darker ink would be a dark lock on a dark badge. Asked for as a scope rather
            // than as a second set of colours, so that what the ink is stays the look's to say.
            drawn = new ThemeVariantScope
            {
                RequestedThemeVariant = ThemeVariant.Dark,
                Child = worn,
            };
        }
        else
        {
            drawn = Glyph(badge.Picture, colour, extent);
        }

        // What the badge stands for is said in words when the pointer rests on it: the badge is a picture, and
        // who holds an entry is a name rather than a picture. Set on whatever was finally drawn — the ground
        // when there is one — so that the whole of it answers rather than the glyph inside it.
        if (badge.Mark.Hover() is { Length: > 0 } hover)
        {
            ToolTip.SetTip(drawn, hover);
        }

        return drawn;
    }

    /// <summary>
    /// The picture itself: a mask over the ink, which is what makes it take the theme's colour.
    /// </summary>
    /// <remarks>
    /// The ink is named rather than written into the picture, so that a mark takes the colour the user is
    /// looking at: a picture carries whatever it was drawn with, and one drawn once per colour would be one per
    /// theme. What is named here is a theme resource, which is what a mark that is not a button's — an entry's
    /// lock, say — is drawn in; a mark that <em>is</em> a button's is [`Inked`], which takes the button's own.
    /// </remarks>
    /// <param name="picture">The picture to stamp.</param>
    /// <param name="ink">What it is drawn in, as the name of a theme resource.</param>
    /// <param name="extent">How many pixels wide and tall it is.</param>
    /// <returns>What to draw, whose background is the ink it was given.</returns>
    public static Border Glyph(Bitmap picture, string ink, double extent)
    {
        var glyph = new Border
        {
            Width = extent,
            Height = extent,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };
        glyph[!Border.BackgroundProperty] = new DynamicResourceExtension(ink);

        // Every picture this plugin draws is read at many times the size it is shown at, so what the mask
        // does is a reduction: the high filter is what keeps a thin stroke whole through it, where a cheaper
        // one samples a few of the picture's pixels and leaves the stroke broken.
        Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
            glyph,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );

        return glyph;
    }

    /// <summary>
    /// A picture drawn as a mask over the ink of the button it is in.
    /// </summary>
    /// <remarks>
    /// The ink is the button's own <c>Foreground</c> rather than a colour picked here, because what a button is
    /// filled with is not this plugin's to know: a tool that is on is filled with the primary, one under the
    /// pointer with the sunken ground, and what is written on either is the look's answer — the ink the user
    /// chose, or black or white decided by how bright the fill turned out to be. A picture that picked its own
    /// ink would be a mark that vanished into the fill the day the primary was a colour it could not be read
    /// on.
    /// </remarks>
    /// <param name="picture">The picture to stamp.</param>
    /// <param name="extent">How many pixels wide and tall it is.</param>
    /// <returns>What to draw, which is drawn in the ink of the button it was put in.</returns>
    public static Border Inked(Bitmap picture, double extent) => Inked(picture, extent, typeof(Button));

    /// <summary>
    /// A picture drawn as a mask over the ink of the control it is in, which is the one named.
    /// </summary>
    /// <remarks>
    /// The holder is named rather than guessed at because the ink is stated by whatever holds the picture — a
    /// button states one, and so does a menu item — and what is between the two is a template that states
    /// nothing of its own. A mark that read the ink of whatever it happened to sit in would take the ink of a
    /// grid or a presenter, which have none to give.
    /// </remarks>
    /// <param name="picture">The picture to stamp.</param>
    /// <param name="extent">How many pixels wide and tall it is.</param>
    /// <param name="holder">What the ink is read from.</param>
    /// <returns>What to draw, which is drawn in the ink of the holder.</returns>
    public static Border Inked(Bitmap picture, double extent, Type holder)
    {
        ArgumentNullException.ThrowIfNull(holder);

        var glyph = new Border
        {
            Width = extent,
            Height = extent,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        glyph.Bind(
            Border.BackgroundProperty,
            new Binding("Foreground")
            {
                RelativeSource = new RelativeSource
                {
                    Mode = RelativeSourceMode.FindAncestor,
                    AncestorType = holder,
                },
            }
        );

        Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
            glyph,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );

        return glyph;
    }

    /// <summary>
    /// How large a dot is at an icon of `size`.
    /// </summary>
    /// <remarks>
    /// Smaller than a badge and never larger than a mark of its own: a dot is a mark rather than a mark's
    /// ground, so it grows with the icon only until it is big enough to read, and a tile at the largest zoom
    /// wears a dot the size of the one at the smallest.
    /// <para>
    /// Read by the view as well as by the dot, because what the dot takes off what stands beside it is room
    /// the view has to work out before the layout does: a thing measured wider than it is arranged is a thing
    /// cut off by the edge of what holds it rather than trimmed by its own rule.
    /// </para>
    /// </remarks>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels wide and tall the dot is.</returns>
    public static double DotExtent(int size) => Math.Clamp(size * DotShare, DotLeast, DotMost);

    /// <summary>
    /// The resource a tag's ground is drawn from: the wash of the ink it is edged in.
    /// </summary>
    /// <remarks>
    /// Read beside [`Ink`] because the two are one answer: a tag is the ink and its wash, and a look that
    /// renamed one of them would leave a mark painted with nothing, which is a mark that is not there.
    /// </remarks>
    /// <param name="ink">The role the mark is drawn in.</param>
    /// <returns>The name of the resource the ground is read from.</returns>
    public static string Wash(MarkInk ink) =>
        ink switch
        {
            MarkInk.Accent => PaletteKeys.AccentWash,
            MarkInk.Error => PaletteKeys.DelBackground,
            MarkInk.Add => PaletteKeys.AddBackground,
            MarkInk.Warn => PaletteKeys.WarnBackground,
            _ => PaletteKeys.Dark,
        };

    /// <summary>What an entry is drawn as where the system had nothing to give.</summary>
    /// <param name="directory">Whether the entry is a directory.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    private static Control Marked(bool directory, int size) =>
        new Border
        {
            Width = size,
            Height = size,
            CornerRadius = new CornerRadius(3),
            Background = new SolidColorBrush(directory ? Directory : File),
            VerticalAlignment = VerticalAlignment.Center,
        };

    /// <summary>Every picture read so far, so that one asked for twice is read once.</summary>
    private static readonly Dictionary<string, Bitmap> Read = new(StringComparer.Ordinal);

    /// <summary>
    /// A picture this plugin draws, by the key the manifest gives it.
    /// </summary>
    /// <remarks>
    /// Read from this plugin's own assembly rather than contributed to the host's library, because it is this
    /// plugin's own furniture: what the library is for is a picture offered to somebody else — the lock a
    /// provider stamps on a listing, say — and a toolbar's own marks are nobody else's to ask for. The keys are
    /// this plugin's too, so a second plugin reading one would be reading something it has no business with.
    /// <para>
    /// Read when it is first wanted rather than at start-up, which is what keeps a program with no window in it
    /// from needing a drawing surface only a window has. What is read is kept, since a toolbar asks for the same
    /// picture whenever a dock is opened.
    /// </para>
    /// </remarks>
    /// <param name="key">The key, which is one of the generated ones.</param>
    /// <returns>The picture.</returns>
    public static Bitmap Picture(string key)
    {
        ArgumentNullException.ThrowIfNull(key);

        if (Read.TryGetValue(key, out var known))
        {
            return known;
        }

        var name = Pictures
            .Where(picture => string.Equals(picture.Key, key, StringComparison.Ordinal))
            .Select(picture => picture.Name)
            .SingleOrDefault()
            ?? throw new InvalidOperationException($"the icon `{key}` is not one this plugin draws");
        var resource = $"FileSystemPlugin.icons.{name}.png";

        using var stream =
            typeof(Icons).Assembly.GetManifestResourceStream(resource)
            ?? throw new InvalidOperationException($"the icon `{resource}` is not embedded");

        var picture = new Bitmap(stream);
        Read[key] = picture;

        return picture;
    }
}

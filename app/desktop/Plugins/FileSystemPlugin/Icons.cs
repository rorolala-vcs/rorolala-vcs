using Avalonia;
using Avalonia.Controls;
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
/// for. What Section 9 calls the icon library is this, and what is left of it is a picture per kind of file,
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

    /// <summary>The theme resource an icon of a tool that is not on is drawn in.</summary>
    /// <remarks>
    /// Written as the literal the look publishes, because a plugin has nowhere else to read it from: the shell's
    /// own constants live in the host, which a plugin may not reference. What is drawn in it is a mark that is
    /// nobody's button — a menu item's — and is therefore read from the look rather than from the control that
    /// holds it.
    /// </remarks>
    public const string PlainInk = "rorolala.fg";

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

    /// <summary>How much of an icon's width a mark takes, and the least it is drawn at.</summary>
    /// <remarks>
    /// A share of the icon rather than a size, because a tile's icon follows the zoom and a mark of
    /// a fixed size would be lost on a large icon and cover a small one. The least is what keeps it
    /// drawable at the smallest a list row ever is.
    /// </remarks>
    private const double MarkShare = 0.42;
    private const double MarkLeast = 12;

    /// <summary>How much of a tag's width is left around the mark drawn inside it, and the least of it.</summary>
    /// <remarks>
    /// The mark is a glyph with nothing around it, so a tag holding it at its own size would have the
    /// glyph running into the tag's edge — which reads as a picture with a border drawn over it rather
    /// than as a state.
    /// </remarks>
    private const double TagInset = 0.16;
    private const double TagLeast = 3;

    /// <summary>The rounding a tag wears, which is the design's radius for a small control.</summary>
    private static readonly CornerRadius TagCorners = new(5);

    /// <summary>The theme resource a lock mark is drawn in.</summary>
    /// <remarks>
    /// Written as the literals the look publishes, because a plugin has nowhere else to read them
    /// from: the shell's own constants live in the host, which a plugin may not reference.
    /// </remarks>
    /// <param name="ink">The role the mark is drawn in.</param>
    /// <returns>The name of the resource it is read from.</returns>
    public static string Ink(LockInk ink) =>
        ink switch
        {
            LockInk.Plain => "rorolala.fg",
            LockInk.Accent => "rorolala.accent",
            LockInk.Error => "rorolala.del",
            _ => "rorolala.fg.faint",
        };

    /// <summary>
    /// An entry's icon with the lock its provider contributed at the top-right corner.
    /// </summary>
    /// <remarks>
    /// The picture is drawn as a mask over a coloured ground rather than as a picture, so that it
    /// takes the ink the theme gives it: a picture carries whatever colours it was drawn with, and a
    /// mark that had to be drawn once per colour would be a mark per theme.
    /// <para>
    /// A mark the contribution asked to have tagged is drawn the way the shell draws a state — a wash
    /// of the ink with the ink's own edge, which is how the plugin manager draws a plugin that failed —
    /// so that a corner saying an entry is somebody else's reads as an answer rather than as another
    /// icon beside the file's own.
    /// </para>
    /// </remarks>
    /// <param name="icon">What the entry is drawn as.</param>
    /// <param name="mark">The picture to stamp on it.</param>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <param name="contributed">What the provider said about the entry.</param>
    /// <returns>The icon and its mark, as one thing to draw.</returns>
    public static Control Marked(Control icon, Bitmap mark, int size, EntryLockMark contributed)
    {
        var extent = Mark(size);
        var inset = Inset(extent);
        var glyph = Glyph(
            mark,
            Ink(contributed.Ink),
            contributed.Tagged ? extent - (2 * inset) - 2 : extent
        );

        Control corner = glyph;

        if (contributed.Tagged)
        {
            var tag = new Border
            {
                Width = extent,
                Height = extent,
                HorizontalAlignment = HorizontalAlignment.Right,
                VerticalAlignment = VerticalAlignment.Top,
                BorderThickness = new Thickness(1),
                CornerRadius = TagCorners,
                Padding = new Thickness(inset),
                Child = glyph,
            };
            tag[!Border.BackgroundProperty] = new DynamicResourceExtension(Wash(contributed.Ink));
            tag[!Border.BorderBrushProperty] = new DynamicResourceExtension(Ink(contributed.Ink));

            corner = tag;
        }
        else
        {
            glyph.HorizontalAlignment = HorizontalAlignment.Right;
            glyph.VerticalAlignment = VerticalAlignment.Top;
        }

        var grid = new Grid { Width = size, Height = size };
        grid.Children.Add(icon);
        grid.Children.Add(corner);

        return grid;
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

        return glyph;
    }

    /// <summary>How many pixels a lock mark is at an icon of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels wide and tall the mark is.</returns>
    private static double Mark(int size) => Math.Max(MarkLeast, size * MarkShare);

    /// <summary>How many pixels a tag leaves around the mark drawn inside it.</summary>
    /// <param name="extent">How many pixels wide and tall the tag is.</param>
    /// <returns>How many pixels of inset it wears.</returns>
    private static double Inset(double extent) => Math.Max(TagLeast, extent * TagInset);

    /// <summary>The resource a tag's ground is drawn from: the wash of the ink it is edged in.</summary>
    /// <param name="ink">The role the mark is drawn in.</param>
    /// <returns>The name of the resource the ground is read from.</returns>
    private static string Wash(LockInk ink) =>
        ink switch
        {
            LockInk.Accent => "rorolala.accent.wash",
            LockInk.Error => "rorolala.del.bg",
            _ => "rorolala.bg.sunken",
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

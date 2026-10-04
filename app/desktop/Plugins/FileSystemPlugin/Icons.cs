using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.SysIcons;

namespace FileSystemPlugin;

/// <summary>
/// The icons entries are shown with.
/// </summary>
/// <remarks>
/// Both come from the system: what a folder looks like is the desktop's business, and a browser that
/// draws its own folder is a browser that looks wrong on every desktop but the one it was drawn for.
/// What Section 9 calls the icon library is this, and what is left of it is a picture per kind of file,
/// which no system is asked about yet — a file is drawn as a file and not as a kind of one.
/// <para>
/// A system with no icon to give, or one this does not ask, falls back to a mark drawn here, which says
/// which of the two kinds a row is: a listing with no pictures beside it is still a listing.
/// </para>
/// </remarks>
internal static class Icons
{
    /// <summary>How wide and tall an icon is where no size is asked for.</summary>
    private const int Default = 16;

    /// <summary>The size a grid's icons are at the zoom it opens at.</summary>
    private const int Base = 64;

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

    /// <summary>How much of an icon's width a lock mark takes, and the least it is drawn at.</summary>
    /// <remarks>
    /// A share of the icon rather than a size, because a tile's icon follows the zoom and a mark of
    /// a fixed size would be lost on a large icon and cover a small one. The least is what keeps it
    /// drawable at the smallest a list row ever is.
    /// </remarks>
    private const double MarkShare = 0.42;
    private const double MarkLeast = 12;

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
    /// An entry's icon with a lock mark at its top-right corner.
    /// </summary>
    /// <remarks>
    /// The picture is drawn as a mask over a coloured ground rather than as a picture, so that it
    /// takes the ink the theme gives it: a picture carries whatever colours it was drawn with, and
    /// a mark that had to be drawn once per colour would be a mark per theme.
    /// </remarks>
    /// <param name="icon">What the entry is drawn as.</param>
    /// <param name="mark">The picture to stamp on it.</param>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <param name="ink">What the mark is drawn in.</param>
    /// <returns>The icon and its mark, as one thing to draw.</returns>
    public static Control Badged(Control icon, Bitmap mark, int size, LockInk ink)
    {
        var spot = new Border
        {
            Width = Mark(size),
            Height = Mark(size),
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            OpacityMask = new ImageBrush(mark) { Stretch = Stretch.Uniform },
        };
        spot[!Border.BackgroundProperty] = new DynamicResourceExtension(Ink(ink));

        var grid = new Grid { Width = size, Height = size };
        grid.Children.Add(icon);
        grid.Children.Add(spot);

        return grid;
    }

    /// <summary>How many pixels a lock mark is at an icon of `size`.</summary>
    /// <param name="size">How many pixels wide and tall the icon is.</param>
    /// <returns>How many pixels wide and tall the mark is.</returns>
    private static double Mark(int size) => Math.Max(MarkLeast, size * MarkShare);

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
}

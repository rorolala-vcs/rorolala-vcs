using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
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
    /// <summary>How wide and tall an icon is.</summary>
    private const int Size = 16;

    /// <summary>The colour a directory falls back to.</summary>
    private static readonly Color Directory = Color.Parse("#E8B339");

    /// <summary>The colour a file falls back to.</summary>
    private static readonly Color File = Color.Parse("#8A94A6");

    /// <summary>The icon for an entry.</summary>
    /// <param name="entry">The entry to draw.</param>
    /// <returns>What to draw beside the entry's name.</returns>
    public static Control For(Entry entry)
    {
        var directory = entry.Kind == EntryKind.Directory;
        var icon = directory ? SysIcons.Directory(Size) : SysIcons.File(Size);

        return icon is null ? Marked(directory) : new Image
        {
            Source = icon,
            Width = Size,
            Height = Size,
            Stretch = Stretch.Uniform,
            VerticalAlignment = VerticalAlignment.Center,
        };
    }

    /// <summary>What an entry is drawn as where the system had nothing to give.</summary>
    /// <param name="directory">Whether the entry is a directory.</param>
    private static Control Marked(bool directory) =>
        new Border
        {
            Width = Size,
            Height = Size,
            CornerRadius = new CornerRadius(2),
            Background = new SolidColorBrush(directory ? Directory : File),
            VerticalAlignment = VerticalAlignment.Center,
        };
}

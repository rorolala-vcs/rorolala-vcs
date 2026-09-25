using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The icons entries are shown with.
/// </summary>
/// <remarks>
/// A placeholder for the icon library Section 9 says this plugin provides. An entry is told apart
/// by its kind and the name beside it rather than by a drawn set, which is enough for the browser to
/// be usable and honest about what is not there yet: the real library replaces this and nothing
/// else, and the badge composition will read its keys from where these are drawn.
/// </remarks>
internal static class Icons
{
    /// <summary>How wide and tall an icon is.</summary>
    private const double Size = 14;

    /// <summary>The colour a directory is drawn in.</summary>
    private static readonly Color Directory = Color.Parse("#E8B339");

    /// <summary>The colour a file is drawn in.</summary>
    private static readonly Color File = Color.Parse("#8A94A6");

    /// <summary>The icon for an entry.</summary>
    /// <param name="entry">The entry to draw.</param>
    /// <returns>A control standing in for the entry's icon.</returns>
    public static Control For(Entry entry) =>
        new Border
        {
            Width = Size,
            Height = Size,
            CornerRadius = new CornerRadius(2),
            Background = new SolidColorBrush(
                entry.Kind == EntryKind.Directory ? Directory : File
            ),
            VerticalAlignment = VerticalAlignment.Center,
        };
}

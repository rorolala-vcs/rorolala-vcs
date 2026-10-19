using Avalonia.Controls;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// What an entry looks like, for a view of one that is not a listing of a directory.
/// </summary>
/// <remarks>
/// A place a plugin answers for holds things that are not files, but what they look like is still the folder a
/// picture comes from: the icon set in force, and the system's own pictures where the set says nothing. So the one
/// answer this plugin gives a listing is given to a place as well, rather than a second look being drawn beside it.
/// <para>
/// The path is matched, not opened: an icon is chosen by what the entry is called — its extension, and whether it
/// is a folder — so a path nothing is at draws the same picture a file of that name would.
/// </para>
/// </remarks>
public static class EntryIcons
{
    /// <summary>The picture a folder wears, at a size in pixels.</summary>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw where a folder's picture goes.</returns>
    public static Control Folder(int size) => For(string.Empty, EntryKind.Directory, size);

    /// <summary>The picture an entry wears, at a size in pixels.</summary>
    /// <param name="path">The path the entry is named by, which is what its picture is chosen from.</param>
    /// <param name="kind">Whether it is a file or a folder.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw where the entry's picture goes.</returns>
    public static Control For(string path, EntryKind kind, int size)
    {
        ArgumentNullException.ThrowIfNull(path);

        return Icons.For(new Entry(path, kind), size);
    }
}

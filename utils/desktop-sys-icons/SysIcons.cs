using Avalonia.Media.Imaging;

namespace RorolalaDesktop.SysIcons;

/// <summary>What an icon is wanted for.</summary>
internal enum Kind
{
    /// <summary>A plain file.</summary>
    File,

    /// <summary>A directory.</summary>
    Directory,
}

/// <summary>
/// The icons a desktop draws its own files and places with.
/// </summary>
/// <remarks>
/// Read from the system rather than shipped with the program: what a folder looks like is the
/// desktop's business, and an application that draws its own folder is an application that looks
/// wrong on every desktop but the one it was drawn for.
/// <para>
/// Each system is asked in its own way — a freedesktop desktop through its icon theme, Windows
/// through its shell — and a system with no way to be asked answers with nothing, which a caller
/// draws as it likes. macOS is one of those for now.
/// </para>
/// <para>
/// What comes back is kept, so asking for one icon a thousand times reads the disk once. An answer
/// of nothing is kept too: a system with no icon to give is not worth asking again for every row.
/// </para>
/// </remarks>
public static class SysIcons
{
    /// <summary>The directory icons read so far, by the size asked for.</summary>
    private static readonly Dictionary<int, Bitmap?> Directories = [];

    /// <summary>The file icons read so far, by the size asked for.</summary>
    private static readonly Dictionary<int, Bitmap?> Files = [];

    /// <summary>What the two caches are guarded by, so that asking twice at once reads twice at most.</summary>
    private static readonly object Gate = new();

    /// <summary>
    /// The icon the system gives a directory, at a size in pixels.
    /// </summary>
    /// <param name="size">How many pixels wide the icon is wanted.</param>
    /// <returns>What to draw, or nothing where the system has nothing to give.</returns>
    public static Bitmap? Directory(int size) => Looking(Kind.Directory, size);

    /// <summary>
    /// The icon the system gives a plain file, at a size in pixels.
    /// </summary>
    /// <remarks>
    /// A file is asked for as a file and not as a kind of file: what an icon per file type would take
    /// is the type, which this does not ask a system about yet.
    /// </remarks>
    /// <param name="size">How many pixels wide the icon is wanted.</param>
    /// <returns>What to draw, or nothing where the system has nothing to give.</returns>
    public static Bitmap? File(int size) => Looking(Kind.File, size);

    /// <summary>Reads an icon, or hands back the one read the last time it was wanted.</summary>
    private static Bitmap? Looking(Kind kind, int size)
    {
        var cache = kind == Kind.Directory ? Directories : Files;

        lock (Gate)
        {
            if (cache.TryGetValue(size, out var kept))
            {
                return kept;
            }

            var read = Asked(kind, size);
            cache[size] = read;

            return read;
        }
    }

    /// <summary>
    /// Asks the system, which is one system per platform.
    /// </summary>
    /// <remarks>
    /// A system that answers with an error is a system with no icon to give, and a listing without a
    /// picture beside it is still a listing: every way of asking is allowed to fail into nothing here
    /// rather than into an exception a caller would have to catch around drawing a row.
    /// </remarks>
    private static Bitmap? Asked(Kind kind, int size)
    {
        try
        {
            if (OperatingSystem.IsWindows())
            {
                return ShellIcon.Drawn(kind);
            }

            if (OperatingSystem.IsLinux())
            {
                return IconTheme.Drawn(kind, size);
            }

            return null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return null;
        }
    }
}

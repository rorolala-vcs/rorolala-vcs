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
/// Each system is asked in its own way — a freedesktop desktop through its icon theme and its MIME
/// database, Windows through its shell — and a system with no way to be asked answers with nothing,
/// which a caller draws as it likes. macOS is one of those for now.
/// </para>
/// <para>
/// A file is asked about by name, not merely as a file, because the icon of a file is the icon of its
/// kind: the shell knows what a PSD looks like and a theme knows what a PDF looks like, and a caller
/// that passed only "a file" would be answered with the icon for a file that is nothing in
/// particular. What the name says is the extension, and that is what both systems are asked with.
/// </para>
/// <para>
/// What comes back is kept, one answer per kind of file and per size, so asking for one icon a thousand
/// times reads the disk once. An answer of nothing is kept too: a system with no icon to give is not
/// worth asking again for every row.
/// </para>
/// </remarks>
public static class SysIcons
{
    /// <summary>The directory icons read so far, by the size asked for.</summary>
    private static readonly Dictionary<int, Bitmap?> Directories = [];

    /// <summary>
    /// The file icons read so far, by what the name says the file is, and then by the size asked for.
    /// </summary>
    /// <remarks>
    /// The first key is the extension — or the whole name where there is none, since a Makefile and a
    /// Dockerfile are names rather than extensions — because that is all either system is asked with. Two
    /// files with one extension therefore share one answer, which is what an icon for a kind of thing is.
    /// </remarks>
    private static readonly Dictionary<string, Dictionary<int, Bitmap?>> Files = new(
        StringComparer.Ordinal
    );

    /// <summary>What the caches are guarded by, so that asking twice at once reads twice at most.</summary>
    private static readonly object Gate = new();

    /// <summary>
    /// The icon the system gives a directory, at a size in pixels.
    /// </summary>
    /// <param name="size">How many pixels wide the icon is wanted.</param>
    /// <returns>What to draw, or nothing where the system has nothing to give.</returns>
    public static Bitmap? Directory(int size) => Looking(Kind.Directory, string.Empty, size);

    /// <summary>
    /// The icon the system gives a plain file whose name says nothing, at a size in pixels.
    /// </summary>
    /// <remarks>
    /// This is the file that is nothing in particular. A caller that has a name — which is every caller
    /// drawing a listing — should pass it to <see cref="File(string, int)"/>: the icon of a file is the
    /// icon of its kind, and a name is how the kind is known.
    /// </remarks>
    /// <param name="size">How many pixels wide the icon is wanted.</param>
    /// <returns>What to draw, or nothing where the system has nothing to give.</returns>
    public static Bitmap? File(int size) => File(string.Empty, size);

    /// <summary>
    /// The icon the system gives a file with the name it has, at a size in pixels.
    /// </summary>
    /// <remarks>
    /// What is asked about is the name rather than the file: nothing has to exist on disk, and the answer
    /// is the icon for the kind of thing the name says — a PSD as a PSD, a PDF as a PDF. A name with no
    /// extension asks for the icon that is nothing in particular, which is what the name says.
    /// </remarks>
    /// <param name="name">The file's name or path.</param>
    /// <param name="size">How many pixels wide the icon is wanted.</param>
    /// <returns>What to draw, or nothing where the system has nothing to give.</returns>
    public static Bitmap? File(string name, int size) => Looking(Kind.File, name, size);

    /// <summary>Reads an icon, or hands back the one read the last time it was wanted.</summary>
    private static Bitmap? Looking(Kind kind, string name, int size)
    {
        lock (Gate)
        {
            if (kind == Kind.Directory)
            {
                if (Directories.TryGetValue(size, out var kept))
                {
                    return kept;
                }

                var read = Asked(kind, string.Empty, size);
                Directories[size] = read;

                return read;
            }

            var key = Key(name);

            if (!Files.TryGetValue(key, out var answers))
            {
                answers = [];
                Files[key] = answers;
            }

            if (answers.TryGetValue(size, out var known))
            {
                return known;
            }

            var picture = Asked(kind, name, size);
            answers[size] = picture;

            return picture;
        }
    }

    /// <summary>What a name says a file is, as the key its answer is kept under.</summary>
    /// <param name="name">The file's name or path.</param>
    /// <returns>The extension, or the whole name where there is no extension.</returns>
    private static string Key(string name)
    {
        var extension = Path.GetExtension(name);

        return extension.Length > 0
            ? extension.ToLowerInvariant()
            : Path.GetFileName(name).ToLowerInvariant();
    }

    /// <summary>
    /// Asks the system, which is one system per platform.
    /// </summary>
    /// <remarks>
    /// A system that answers with an error is a system with no icon to give, and a listing without a
    /// picture beside it is still a listing: every way of asking is allowed to fail into nothing here
    /// rather than into an exception a caller would have to catch around drawing a row.
    /// </remarks>
    private static Bitmap? Asked(Kind kind, string name, int size)
    {
        try
        {
            if (OperatingSystem.IsWindows())
            {
                return ShellIcon.Drawn(kind, name, size);
            }

            if (OperatingSystem.IsLinux())
            {
                return IconTheme.Drawn(kind, name, size);
            }

            return null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return null;
        }
    }
}

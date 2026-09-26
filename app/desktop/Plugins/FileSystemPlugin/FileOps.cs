namespace FileSystemPlugin;

/// <summary>
/// Moving and copying entries on disk, which is what a paste and a drop do.
/// </summary>
/// <remarks>
/// Everything here is best-effort and reports rather than throws: one entry that cannot be moved must
/// not abandon the others, and a file that has gone since the listing was read is a thing to say rather
/// than a thing to stop on.
/// <para>
/// A name already taken is never overwritten. A paste or a drop onto a directory that already holds the
/// name makes a free one beside it — <c>name (2)</c>, and so on — which is what a user expects of a
/// second copy and is the one behaviour that cannot lose data.
/// </para>
/// </remarks>
internal static class FileOps
{
    /// <summary>
    /// A path without the separator a directory's may carry at its end.
    /// </summary>
    /// <remarks>
    /// This exists because of one bug that is worth not having again: a path that came from a drag is read
    /// from a URI, and a directory's URI ends in a separator — so the name taken from it is the empty string,
    /// a "directory" of no name is the directory it sits in, that directory always exists, and the entry was
    /// therefore renamed instead of arriving under its own name. Bare the path before asking anything of it.
    /// </remarks>
    /// <param name="path">The path to bare.</param>
    /// <returns>The path without trailing separators, or the path itself when it is one all the way.</returns>
    public static string Bare(string path)
    {
        var trimmed = path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        return trimmed.Length == 0 ? path : trimmed;
    }

    /// <summary>Moves an entry into a directory, under a free name.</summary>
    /// <param name="source">What to move.</param>
    /// <param name="into">The directory to move it into.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public static void Move(string source, string into, Action<string> failed)
    {
        source = Bare(source);

        try
        {
            var target = Free(into, Path.GetFileName(source));

            if (Directory.Exists(source))
            {
                try
                {
                    System.IO.Directory.Move(source, target);
                }
                catch (IOException)
                {
                    // A directory move that the filesystem refuses — across two devices, most often —
                    // is still a move to the user, so it is done the long way: copied, then removed.
                    CopyTree(source, target);
                    System.IO.Directory.Delete(source, true);
                }
            }
            else
            {
                File.Move(source, target);
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }
    }

    /// <summary>Copies an entry into a directory, under a free name.</summary>
    /// <param name="source">What to copy.</param>
    /// <param name="into">The directory to copy it into.</param>
    /// <param name="failed">Where a failure is reported.</param>
    public static void Copy(string source, string into, Action<string> failed)
    {
        source = Bare(source);

        try
        {
            var target = Free(into, Path.GetFileName(source));

            if (Directory.Exists(source))
            {
                CopyTree(source, target);
            }
            else
            {
                File.Copy(source, target);
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);
        }
    }

    /// <summary>
    /// A name in a directory that nothing is using, made free by numbering it.
    /// </summary>
    /// <remarks>
    /// The whole name is kept, extension and all, and the number goes before the extension so that the
    /// system still knows what the file is: <c>sheet (2).psd</c> rather than <c>sheet.psd (2)</c>.
    /// </remarks>
    /// <param name="into">The directory the name is to be free in.</param>
    /// <param name="name">The name wanted.</param>
    /// <returns>A path in the directory that nothing holds.</returns>
    private static string Free(string into, string name)
    {
        name = Bare(name);
        var wanted = Path.Combine(into, name);

        if (!Held(wanted))
        {
            return wanted;
        }

        var stem = Path.GetFileNameWithoutExtension(name);
        var extension = Path.GetExtension(name);

        for (var number = 2; ; number++)
        {
            var beside = Path.Combine(into, $"{stem} ({number}){extension}");

            if (!Held(beside))
            {
                return beside;
            }
        }
    }

    /// <summary>Whether a path is taken, by a file or by a directory.</summary>
    /// <param name="path">The path to ask about.</param>
    private static bool Held(string path) => File.Exists(path) || System.IO.Directory.Exists(path);

    /// <summary>Copies a directory and everything under it.</summary>
    /// <param name="source">The directory to copy.</param>
    /// <param name="target">Where to copy it to.</param>
    private static void CopyTree(string source, string target)
    {
        System.IO.Directory.CreateDirectory(target);

        foreach (var path in System.IO.Directory.EnumerateFileSystemEntries(source))
        {
            var name = Path.GetFileName(path);
            var under = Path.Combine(target, name);

            if (System.IO.Directory.Exists(path))
            {
                CopyTree(path, under);
            }
            else
            {
                File.Copy(path, under);
            }
        }
    }
}

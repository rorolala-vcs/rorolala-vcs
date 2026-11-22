namespace RorolalaVCSPlugin;

/// <summary>
/// Which readings a change to one directory makes stale.
/// </summary>
/// <remarks>
/// A reading is taken of a directory and holds the work under it, so a change anywhere below that directory
/// sours it, and so does a change in the directory itself. A reading of a directory that holds the changed one
/// is soured as well, since what that directory lists is an entry whose own state has just moved. A reading
/// taken of another branch of the work is not made wrong by the change and is kept.
/// </remarks>
internal static class Subtree
{
    /// <summary>Whether a change in one directory sours a reading taken of another.</summary>
    /// <param name="reading">The directory the reading was taken of.</param>
    /// <param name="directory">The directory whose files may have changed.</param>
    /// <returns>Whether the reading has to be read again.</returns>
    public static bool Holds(string reading, string directory) =>
        string.Equals(Full(reading), Full(directory), Comparison)
        || Full(directory).StartsWith(Full(reading) + Path.DirectorySeparatorChar, Comparison);

    /// <summary>
    /// The comparison the platform names paths with, so two spellings of one path compare equal
    /// where the file system says they are.
    /// </summary>
    private static StringComparison Comparison =>
        OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;

    /// <summary>A path made absolute and stripped of a trailing separator, for comparison.</summary>
    /// <param name="path">The path to make comparable.</param>
    /// <returns>The path, fully spelled and without a trailing separator.</returns>
    private static string Full(string path) => Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
}

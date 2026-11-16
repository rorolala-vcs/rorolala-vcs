using Avalonia.Media;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace GitVCSPlugin;

/// <summary>
/// The colour a signed-in material is marked with in the file tree: the program's disabled grey.
/// </summary>
/// <remarks>
/// A material is not the tree's to change by hand: `git rola` places it from the lock, and an edit
/// made to it is what `check` and `refresh` report. The grey says that as a state rather than as a
/// rule, and the semantic colour is the look's own, so a light and a dark theme say it in the grey
/// that reads on them.
/// <para>
/// The lock is the answer to "which paths are signed in", read once per write of it and kept until
/// the next: a listing asks about many rows, and reading the file per row would be reading one file
/// once per entry. A tree's own row is what is marked, and the entries under it inherit the colour
/// the way they inherit a marked directory's.
/// </para>
/// </remarks>
internal sealed class GitRolaColour : IEntryColourProvider
{
    /// <summary>The palette in force, which is where the grey and the red are read from.</summary>
    private readonly IPalette _palette;

    /// <summary>What the project's record says about the work, which tells the grey from the red.</summary>
    private readonly GitRolaCheck _check;

    /// <summary>What each lock last said, by the file it was read from.</summary>
    private readonly Dictionary<string, Reading> _materials = new(StringComparer.Ordinal);

    /// <summary>The provider, offered to the File System plugin's registry.</summary>
    /// <param name="palette">The palette in force.</param>
    /// <param name="check">What the project's record says about the work.</param>
    public GitRolaColour(IPalette palette, GitRolaCheck check)
    {
        ArgumentNullException.ThrowIfNull(palette);
        ArgumentNullException.ThrowIfNull(check);

        _palette = palette;
        _check = check;
    }

    /// <inheritdoc />
    public string Id => "rorolala.git.rola";

    /// <inheritdoc />
    public Color? Colour(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        if (BaseDirectory.Current is not { Length: > 0 } root || entry.Path is not { Length: > 0 } path)
        {
            return null;
        }

        var lockPath = System.IO.Path.Combine(root, GitRolaLock.FileName);

        if (!File.Exists(lockPath))
        {
            return null;
        }

        var relative = System.IO.Path.GetRelativePath(root, path).Replace('\\', '/');

        if (!Held(lockPath).Paths.Contains(relative))
        {
            return null;
        }

        // A material the lock names is this side's to place, so it is grey; one that does not hold
        // what the lock records is wrong, so it is the program's error red.
        return _check.Wrong(path) ? _palette.Semantic.Error : _palette.Semantic.Disabled;
    }

    /// <inheritdoc />
    public void Forget()
    {
        _materials.Clear();
        _check.Forget();
    }

    /// <summary>The paths the lock names, read again when the file was written.</summary>
    /// <param name="lockPath">The lock.</param>
    /// <returns>What it names.</returns>
    private Reading Held(string lockPath)
    {
        var stamp = File.GetLastWriteTimeUtc(lockPath);
        var length = new FileInfo(lockPath).Length;

        if (
            _materials.TryGetValue(lockPath, out var known)
            && known.Stamp == stamp
            && known.Length == length
        )
        {
            return known;
        }

        IReadOnlySet<string> paths;

        try
        {
            paths = GitRolaLock.Materials(File.ReadAllText(lockPath));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            paths = new HashSet<string>(StringComparer.Ordinal);
        }

        var materials = new Reading(stamp, length, paths);

        _materials[lockPath] = materials;

        return materials;
    }

    /// <summary>One reading of a lock, and what it was read from.</summary>
    /// <param name="Stamp">When the file was last written.</param>
    /// <param name="Length">How long the file is.</param>
    /// <param name="Paths">The paths it names.</param>
    private sealed record Reading(DateTime Stamp, long Length, IReadOnlySet<string> Paths);
}

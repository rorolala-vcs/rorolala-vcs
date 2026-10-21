using System.Text.Json;
using Avalonia.Threading;

namespace FileSystemPlugin;

/// <summary>
/// The steps of a tree that were open, kept between runs.
/// </summary>
/// <remarks>
/// What a user opens is what they are working in, so a tree that comes back with every step closed is a
/// user walking the same way again on every start. The set is kept under the program's own cache rather
/// than among the settings: a step that is gone, or a file that is damaged, is something to read again
/// rather than something to ask about, and emptying the cache costs nothing but the walking.
/// <para>
/// A path is remembered by what it is rather than by the tree that showed it, so the same directory is
/// open in every tree rooted above it. What is written waits for a moment of quiet: opening a step that
/// holds steps opens several, and a file written once per row would be a file written while the tree is
/// still being opened.
/// </para>
/// </remarks>
internal sealed class TreeMemory
{
    /// <summary>The file the open steps are kept in.</summary>
    private const string FileName = "tree-opened.json";

    /// <summary>How many steps are worth remembering, so that a cache cannot grow without end.</summary>
    private const int Most = 4096;

    /// <summary>How long a change waits before it is written.</summary>
    private static readonly TimeSpan Quiet = TimeSpan.FromMilliseconds(400);

    /// <summary>The way the set is written, which is one path a line of a row rather than all on one.</summary>
    private static readonly JsonSerializerOptions Written = new() { WriteIndented = true };

    /// <summary>Where the file is.</summary>
    private readonly string _path;

    /// <summary>Every step that was open, by its path.</summary>
    private readonly HashSet<string> _opened = new(StringComparer.Ordinal);

    /// <summary>The clock a change is written on, once it has been quiet for a moment.</summary>
    private readonly DispatcherTimer _write = new() { Interval = Quiet };

    /// <summary>Whether there is something to write.</summary>
    private bool _dirty;

    /// <summary>Reads the steps that were open, from the cache directory this plugin was given.</summary>
    /// <param name="directory">The directory this plugin keeps its cache in.</param>
    public TreeMemory(string directory)
    {
        ArgumentException.ThrowIfNullOrEmpty(directory);
        _path = Path.Combine(directory, FileName);
        _write.Tick += (_, _) => Flush();

        Load();
    }

    /// <summary>The steps remembered under a root, outermost first.</summary>
    /// <remarks>
    /// Outermost first is what opening them needs: a step whose parent is closed has no row to be found by,
    /// and opening a step is what reads the steps under it.
    /// </remarks>
    /// <param name="root">The directory the tree is rooted at.</param>
    /// <returns>The paths of the steps to open.</returns>
    public IReadOnlyList<string> Opened(string root)
    {
        ArgumentException.ThrowIfNullOrEmpty(root);

        return
        [
            .. _opened
                .Where(path => Under(root, path))
                .OrderBy(path => path.Count(character => character == Path.DirectorySeparatorChar))
                .ThenBy(path => path, StringComparer.Ordinal),
        ];
    }

    /// <summary>Remembers a step as open or closed, and has the set written once it goes quiet.</summary>
    /// <param name="path">The directory the step stands for.</param>
    /// <param name="open">Whether it is open.</param>
    public void Remember(string path, bool open)
    {
        ArgumentException.ThrowIfNullOrEmpty(path);

        if (open ? !_opened.Add(path) : !_opened.Remove(path))
        {
            return;
        }

        _dirty = true;

        // Restarted rather than started: what is wanted is the set as it stands once the opening has
        // stopped, not the first of the steps it opened.
        _write.Stop();
        _write.Start();
    }

    /// <summary>Says whether a path is the root or stands under it.</summary>
    /// <param name="root">The directory the tree is rooted at.</param>
    /// <param name="path">The path to ask about.</param>
    /// <returns>Whether the path is that directory or one under it.</returns>
    private static bool Under(string root, string path) =>
        string.Equals(path, root, StringComparison.Ordinal)
        || path.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.Ordinal);

    /// <summary>Reads what was kept, dropping what is no longer a directory.</summary>
    private void Load()
    {
        string[] held;

        try
        {
            if (!File.Exists(_path))
            {
                return;
            }

            held = JsonSerializer.Deserialize<string[]>(File.ReadAllText(_path)) ?? [];
        }
        catch (Exception error) when (error is IOException or JsonException or UnauthorizedAccessException)
        {
            // A cache that cannot be read is a cache that is not there: what a user loses is the walking
            // back to where they were, which is what they would have done with no cache at all.
            return;
        }

        foreach (var path in held)
        {
            if (_opened.Count < Most && path is { Length: > 0 } && Directory.Exists(path))
            {
                _ = _opened.Add(path);
            }
        }
    }

    /// <summary>Writes the set, if there is one to write.</summary>
    private void Flush()
    {
        _write.Stop();

        if (!_dirty)
        {
            return;
        }

        _dirty = false;

        try
        {
            _ = Directory.CreateDirectory(Path.GetDirectoryName(_path) ?? ".");
            File.WriteAllText(
                _path,
                JsonSerializer.Serialize(_opened.OrderBy(path => path, StringComparer.Ordinal).ToArray(), Written)
            );
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            // The set is worth keeping, not worth telling somebody about: a run that cannot write its cache
            // still opens the steps it is told to, and the next run walks the way it did before.
            _dirty = true;
            _write.Start();
            _ = error;
        }
    }
}

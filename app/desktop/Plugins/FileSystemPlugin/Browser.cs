using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>How the browser lays its entries out.</summary>
internal enum BrowserView
{
    /// <summary>One entry to a line.</summary>
    List,

    /// <summary>Entries as tiles, wrapping across the width.</summary>
    Grid,

    /// <summary>Directories as a tree, opened downward.</summary>
    Tree,
}

/// <summary>
/// Where the browser is, where it has been, and what is there.
/// </summary>
/// <remarks>
/// One per plugin, not one per dock: the File System has one location, and every dock it opens is a
/// view onto it — how the entries are laid out differs from dock to dock, the directory does not. A
/// second dock is therefore a second look at the same place rather than a place of its own, which is
/// what lets the navigation dock have one address to show and one history to walk.
/// <para>
/// The state is the browser's own, kept by the plugin rather than the host: what a directory holds,
/// and where the user has been, are the browser's to know.
/// </para>
/// </remarks>
internal sealed class Browser
{
    /// <summary>Where it has been, most recent last.</summary>
    private readonly List<string> _back = [];

    /// <summary>Where going back would come to, most recent last.</summary>
    private readonly List<string> _forward = [];

    /// <summary>The directory being shown.</summary>
    private string _current;

    /// <summary>What the directory held when it was last read.</summary>
    private IReadOnlyList<Entry> _entries;

    /// <summary>Makes a browser onto a directory.</summary>
    /// <param name="directory">The directory to show first.</param>
    public Browser(string directory)
    {
        _current = directory;
        _entries = Read(directory);
    }

    /// <summary>Raised whenever what is shown has changed.</summary>
    public event Action? Changed;

    /// <summary>The directory being shown.</summary>
    public string Current => _current;

    /// <summary>Whether going back has anywhere to go.</summary>
    public bool CanGoBack => _back.Count > 0;

    /// <summary>Whether going forward has anywhere to go.</summary>
    public bool CanGoForward => _forward.Count > 0;

    /// <summary>Whether going up has anywhere to go.</summary>
    public bool CanGoUp => System.IO.Directory.GetParent(_current) is not null;

    /// <summary>What the directory holds, directories first and then by name.</summary>
    public IReadOnlyList<Entry> Entries => _entries;

    /// <summary>
    /// Switches which directory is being looked at.
    /// </summary>
    /// <remarks>
    /// The one way the location changes, whatever asked for it: a path typed into the address, a
    /// directory clicked in the tree, or one opened in a list or a grid. A path that is not a
    /// directory is refused rather than shown, so that a caller need not check first and cannot
    /// leave the browser somewhere that cannot be read.
    /// <para>
    /// Going to where it already is reads the directory again, which is what the address typed
    /// unchanged asks for.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory to look at.</param>
    /// <returns>Whether it went there.</returns>
    public bool Go(string directory)
    {
        if (!System.IO.Directory.Exists(directory))
        {
            return false;
        }

        if (Same(directory, _current))
        {
            Refresh();

            return true;
        }

        _back.Add(_current);
        _forward.Clear();
        Move(directory);

        return true;
    }

    /// <summary>Shows what was shown before, if anything was.</summary>
    public void Back()
    {
        if (_back.Count == 0)
        {
            return;
        }

        var to = _back[^1];
        _back.RemoveAt(_back.Count - 1);
        _forward.Add(_current);
        Move(to);
    }

    /// <summary>Shows what going back came from, if anything did.</summary>
    public void Forward()
    {
        if (_forward.Count == 0)
        {
            return;
        }

        var to = _forward[^1];
        _forward.RemoveAt(_forward.Count - 1);
        _back.Add(_current);
        Move(to);
    }

    /// <summary>Shows the directory holding this one, if there is one.</summary>
    public void Up()
    {
        if (System.IO.Directory.GetParent(_current) is { } parent)
        {
            Go(parent.FullName);
        }
    }

    /// <summary>Reads the directory again, which is what shows a change made elsewhere.</summary>
    public void Refresh()
    {
        _entries = Read(_current);
        Changed?.Invoke();
    }

    /// <summary>Moves without touching the history, for back, forward and up.</summary>
    /// <param name="directory">The directory to look at.</param>
    private void Move(string directory)
    {
        // Held in full rather than as it was given: what is typed may be relative or have a step in
        // it, and the address is what says where the browser actually is.
        _current = Path.GetFullPath(directory);
        _entries = Read(_current);
        Changed?.Invoke();
    }

    /// <summary>Whether two paths name the same directory.</summary>
    /// <remarks>
    /// Compared in full, so that <c>..</c> and a trailing separator are the same place rather than
    /// a second step in the history.
    /// </remarks>
    private static bool Same(string left, string right) =>
        string.Equals(
            Path.GetFullPath(left),
            Path.GetFullPath(right),
            StringComparison.Ordinal
        );

    /// <summary>
    /// What a directory holds, or nothing when it could not be read.
    /// </summary>
    /// <remarks>
    /// A directory that cannot be read is shown as empty rather than as a failure: the user can see
    /// where they are and move on, and a browser that stopped on every unreadable directory would be
    /// unusable as root.
    /// </remarks>
    private static IReadOnlyList<Entry> Read(string directory)
    {
        try
        {
            var entries = new List<Entry>();

            foreach (var path in System.IO.Directory.EnumerateFileSystemEntries(directory))
            {
                entries.Add(
                    new Entry(
                        path,
                        System.IO.Directory.Exists(path) ? EntryKind.Directory : EntryKind.File
                    )
                );
            }

            entries.Sort(Compare);

            return entries;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }

    /// <summary>Directories before files, then by name the way a listing reads.</summary>
    private static int Compare(Entry left, Entry right)
    {
        if (left.Kind != right.Kind)
        {
            return left.Kind == EntryKind.Directory ? -1 : 1;
        }

        return string.Compare(
            Path.GetFileName(left.Path),
            Path.GetFileName(right.Path),
            StringComparison.OrdinalIgnoreCase
        );
    }
}

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
/// Where the browser is, where it has been, and what it is showing.
/// </summary>
/// <remarks>
/// This is the browser's own state, kept by the plugin rather than the host: what a directory holds,
/// and where the user has been, are the browser's to know. One instance belongs to one dock
/// instance, so two File System docks browse apart from each other — which is what open mode
/// <c>New</c> means.
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

    /// <summary>The layout the entries are shown in.</summary>
    public BrowserView View { get; private set; } = BrowserView.List;

    /// <summary>What the directory holds, directories first and then by name.</summary>
    public IReadOnlyList<Entry> Entries => _entries;

    /// <summary>Shows a directory, remembering where it came from.</summary>
    /// <param name="directory">The directory to show.</param>
    public void Go(string directory)
    {
        if (Same(directory, _current))
        {
            Refresh();
            return;
        }

        _back.Add(_current);
        _forward.Clear();
        Move(directory);
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

    /// <summary>Shows the entries in another layout.</summary>
    /// <param name="view">The layout to show.</param>
    public void Show(BrowserView view)
    {
        View = view;
        Changed?.Invoke();
    }

    /// <summary>Moves without touching the history, for back, forward and up.</summary>
    /// <param name="directory">The directory to show.</param>
    private void Move(string directory)
    {
        _current = directory;
        _entries = Read(directory);
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

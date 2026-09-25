using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>How a directory's entries are laid out.</summary>
internal enum BrowserView
{
    /// <summary>One entry to a line.</summary>
    List,

    /// <summary>Entries as tiles, wrapping across the width.</summary>
    Grid,
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
    /// <summary>
    /// The path that names the computer rather than a directory: where a Windows user picks a drive.
    /// </summary>
    /// <remarks>
    /// Windows has no path that names every drive at once, so the one place that is not a directory is
    /// held as the empty path, which no directory can be. Unix has no such place — its root is a
    /// directory — so this is reached there by nothing at all.
    /// </remarks>
    public const string Computer = "";

    /// <summary>Where it has been, most recent last.</summary>
    private readonly List<string> _back = [];

    /// <summary>Where going back would come to, most recent last.</summary>
    private readonly List<string> _forward = [];

    /// <summary>The directory being looked at.</summary>
    private string _current;

    /// <summary>What the directory held when it was last read.</summary>
    private IReadOnlyList<Entry> _entries;

    /// <summary>Makes a browser onto a directory.</summary>
    /// <param name="directory">The directory to look at, and to root the tree at.</param>
    public Browser(string directory)
    {
        _current = directory;
        BaseDir = directory;
        _entries = Read(directory);
    }

    /// <summary>
    /// The top a platform has.
    /// </summary>
    /// <remarks>
    /// Windows is topped by the computer, where the drives are chosen from; Unix by its root, which is
    /// a directory like any other.
    /// </remarks>
    public static string Root() => OperatingSystem.IsWindows() ? Computer : "/";

    /// <summary>Whether a path names the computer rather than a directory.</summary>
    /// <param name="path">The path to ask about.</param>
    public static bool IsComputer(string path) => path.Length == 0;

    /// <summary>
    /// The entries one location holds, without going there.
    /// </summary>
    /// <remarks>
    /// The one place a directory is read, so that the tree and the listings agree about what is in one
    /// — including the computer, whose entries are the drives and are read by nothing else.
    /// </remarks>
    /// <param name="directory">The directory to read, or the computer.</param>
    public static IReadOnlyList<Entry> Read(string directory)
    {
        return IsComputer(directory) ? Drives() : Listing(directory);
    }

    /// <summary>Raised whenever where the browser is looking, or what the tree is rooted at, changes.</summary>
    public event Action? Changed;

    /// <summary>The directory being looked at.</summary>
    public string Current => _current;

    /// <summary>
    /// The directory the tree is rooted at, which a directory's own menu sets.
    /// </summary>
    /// <remarks>
    /// The tree is rooted here rather than at the location so that stepping through directories does
    /// not move the tree: what a user opens stays open, and the tree is the one view that is not
    /// rearranged by where the browser happens to be.
    /// </remarks>
    public string BaseDir { get; private set; }

    /// <summary>The directory holding the one being looked at, or nothing when there is none.</summary>
    public string? Parent => ParentOf(_current);

    /// <summary>Whether going back has anywhere to go.</summary>
    public bool CanGoBack => _back.Count > 0;

    /// <summary>Whether going forward has anywhere to go.</summary>
    public bool CanGoForward => _forward.Count > 0;

    /// <summary>Whether going up has anywhere to go.</summary>
    public bool CanGoUp => ParentOf(_current) is not null;

    /// <summary>What the directory holds, directories first and then by name.</summary>
    public IReadOnlyList<Entry> Entries => _entries;

    /// <summary>
    /// Switches which directory is being looked at.
    /// </summary>
    /// <remarks>
    /// The one way the location changes, whatever asked for it: a path typed into the address, a
    /// directory clicked in the tree, an `..` opened in a list or a grid. A path that is not a
    /// directory is refused rather than shown, so that a caller need not check first and cannot leave
    /// the browser somewhere that cannot be read; the computer is a place rather than a directory, and
    /// is taken as one.
    /// <para>
    /// Going to where it already is reads the directory again, which is what the address typed
    /// unchanged asks for.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory to look at.</param>
    /// <returns>Whether it went there.</returns>
    public bool Go(string directory)
    {
        if (!IsComputer(directory) && !System.IO.Directory.Exists(directory))
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

    /// <summary>
    /// Roots the tree at a directory, and looks there.
    /// </summary>
    /// <remarks>
    /// Setting the base moves the browser as well, because the two are one act to whoever asked: the
    /// tree is a view of the place being worked in, and a base the browser is not in would leave the
    /// tree showing somewhere else entirely.
    /// </remarks>
    /// <param name="directory">The directory to root the tree at.</param>
    /// <returns>Whether it went there and rooted it there.</returns>
    public bool SetBase(string directory)
    {
        if (!Go(directory))
        {
            return false;
        }

        if (BaseDir == _current)
        {
            return true;
        }

        BaseDir = _current;

        // Where the browser is looking did not change, but what the tree is rooted at did, so what
        // draws the tree has to be told.
        Changed?.Invoke();

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

    /// <summary>Looks at the directory holding this one, if there is one.</summary>
    public void Up()
    {
        if (ParentOf(_current) is { } parent)
        {
            Go(parent);
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
        // it, and the address is what says where the browser actually is. The computer is not a path
        // and is held as it is.
        _current = IsComputer(directory) ? directory : Path.GetFullPath(directory);
        _entries = Read(_current);
        Changed?.Invoke();
    }

    /// <summary>
    /// The directory holding one, or nothing when there is nowhere further up.
    /// </summary>
    /// <remarks>
    /// A drive root is held by the computer, which is where the other drives are chosen from; the
    /// computer is held by nothing. A Unix root is a directory whose parent is nothing.
    /// </remarks>
    private static string? ParentOf(string directory)
    {
        if (IsComputer(directory))
        {
            return null;
        }

        if (System.IO.Directory.GetParent(directory) is { } parent)
        {
            return parent.FullName;
        }

        return OperatingSystem.IsWindows() && directory == Path.GetPathRoot(directory)
            ? Computer
            : null;
    }

    /// <summary>Whether two paths name the same directory.</summary>
    /// <remarks>
    /// Compared in full, so that <c>..</c> and a trailing separator are the same place rather than
    /// a second step in the history. A place that is not a path is compared as itself.
    /// </remarks>
    private static bool Same(string left, string right) =>
        IsComputer(left) || IsComputer(right)
            ? left == right
            : string.Equals(
                Path.GetFullPath(left),
                Path.GetFullPath(right),
                StringComparison.Ordinal
            );

    /// <summary>
    /// The drives a computer has, which is all the computer holds.
    /// </summary>
    /// <remarks>
    /// Every drive is listed, ready or not: one that cannot be read shows as empty, which is what a
    /// reader is looking for anyway, and hiding an empty drive would hide the one about to be used.
    /// </remarks>
    private static IReadOnlyList<Entry> Drives()
    {
        try
        {
            return System.IO.DriveInfo
                .GetDrives()
                .Select(drive => new Entry(drive.Name, EntryKind.Directory))
                .OrderBy(drive => drive.Path, StringComparer.OrdinalIgnoreCase)
                .ToArray();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }

    /// <summary>
    /// What a directory holds, or nothing when it could not be read.
    /// </summary>
    /// <remarks>
    /// A directory that cannot be read is shown as empty rather than as a failure: the user can see
    /// where they are and move on, and a browser that stopped on every unreadable directory would be
    /// unusable as root.
    /// </remarks>
    private static IReadOnlyList<Entry> Listing(string directory)
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

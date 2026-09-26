using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// Where one is, where it has been, and what is there.
/// </summary>
/// <remarks>
/// The plugin has one of these and every dock is a view onto it — unless the dock has been taken out of
/// step, in which case it is given one of its own and looks at a directory of its own (Section 7.5). How the
/// entries are laid out differs from dock to dock, and so may the directory; what every one of them agrees
/// on — where the tree is rooted, and whether hidden entries are shown — is not here but in
/// <see cref="Shared"/>.
/// <para>
/// The rest of the state is the location's own, kept by the plugin rather than the host: what a directory
/// holds, and where this one has been, are its to know.
/// </para>
/// </remarks>
internal sealed class Browser : IDisposable
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

    /// <summary>
    /// The name of the way up rather than a directory: what a listing leads one step out of itself by.
    /// </summary>
    /// <remarks>
    /// A name no directory can have, which is why it can stand for the step out of the listing: the entries
    /// around it are named by their own last part, and one of them being called <c>..</c> would be a
    /// directory the filesystem does not allow. It carries no path of its own — where it leads is the
    /// listing's parent rather than anything written down on it — so what handles it must ask rather than
    /// resolve it.
    /// </remarks>
    public const string UpName = "..";

    /// <summary>The answers every location shares, which this one reads and restages itself for.</summary>
    private readonly Shared _shared;

    /// <summary>Where it has been, most recent last.</summary>
    private readonly List<string> _back = [];

    /// <summary>Where going back would come to, most recent last.</summary>
    private readonly List<string> _forward = [];

    /// <summary>The directory being looked at.</summary>
    private string _current;

    /// <summary>What the directory held when it was last read.</summary>
    private IReadOnlyList<Entry> _entries;

    /// <summary>What a listing shows of it, which is those entries with the way up before them.</summary>
    private IReadOnlyList<Entry> _shown;

    /// <summary>Makes a location onto a directory.</summary>
    /// <param name="shared">The answers every location shares.</param>
    /// <param name="directory">The directory to look at.</param>
    public Browser(Shared shared, string directory)
    {
        _shared = shared;
        _current = directory;
        _entries = Read(directory);
        _shown = Stage(directory, _entries);

        // A listing follows the shared answers wherever it is, so this location has to be told when they
        // change — and that is why a location given to a dock out of step is let go of rather than dropped.
        _shared.Changed += Restage;
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

    /// <summary>Whether a path names the way up rather than a directory.</summary>
    /// <param name="path">The path to ask about.</param>
    public static bool IsUp(string path) => string.Equals(path, UpName, StringComparison.Ordinal);

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
    /// The answer is the whole plugin's rather than this location's (see <see cref="Shared"/>), and it is
    /// reached through a location because a location is what a view holds.
    /// </remarks>
    public string BaseDir => _shared.BaseDir;

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
    /// Whether the entries the platform hides are shown.
    /// </summary>
    /// <remarks>
    /// One answer for the whole plugin rather than for this location (see <see cref="Shared"/>), reached
    /// through a location because a location is what a view holds — and because the completions an address
    /// offers keep the same entries out, a path that cannot be reached by typing it not being an address.
    /// </remarks>
    public bool ShowHidden
    {
        get => _shared.ShowHidden;
        set => _shared.ShowHidden = value;
    }

    /// <summary>
    /// The entries as a listing shows them: the way up, when there is somewhere to go, and then the entries.
    /// </summary>
    /// <remarks>
    /// The way up is one of the listing's entries rather than a row of the view's own, so that it is laid
    /// out, selected and scrolled with the rest of them — and so that a grid, which is not rows at all,
    /// does not have to say how a row is drawn. It is staged here rather than in each view because both
    /// views show the same listing, and a second one staging its own would eventually stage something else.
    /// </remarks>
    public IReadOnlyList<Entry> Shown => _shown;

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

        // Setting the base moves this location as well, because the two are one act to whoever asked: a user
        // who roots the tree at a directory is working in it. A dock that has been taken out of step roots the
        // tree for everybody and moves only itself (Section 7.5), so the base and where the whole is looking
        // may then come apart — which is what being out of step means.
        _shared.BaseDir = _current;

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
        _shown = Stage(_current, _entries);
        Changed?.Invoke();
    }

    /// <summary>
    /// Reads the listing again for what the shared answers now say.
    /// </summary>
    /// <remarks>
    /// The directory has not changed; what a listing makes of it has. Both shared answers are read while a
    /// listing is staged, so one of them changing is a listing to make again — and the tree, which is rooted
    /// at one of them, is told by the same raising rather than by a second one of its own.
    /// </remarks>
    private void Restage()
    {
        _shown = Stage(_current, _entries);
        Changed?.Invoke();
    }

    /// <summary>
    /// Lets go of the shared answers.
    /// </summary>
    /// <remarks>
    /// A location made for a dock that was taken out of step is thrown away when that dock goes back in, and
    /// what it holds of the plugin's has to be let go of with it: a subscription left behind would keep an
    /// abandoned location reading directories nobody looks at, once per toggle.
    /// </remarks>
    public void Dispose() => _shared.Changed -= Restage;

    /// <summary>Moves without touching the history, for back, forward and up.</summary>
    /// <param name="directory">The directory to look at.</param>
    private void Move(string directory)
    {
        // Held in full rather than as it was given: what is typed may be relative or have a step in
        // it, and the address is what says where the browser actually is. The computer is not a path
        // and is held as it is.
        _current = IsComputer(directory) ? directory : Path.GetFullPath(directory);
        _entries = Read(_current);
        _shown = Stage(_current, _entries);
        Changed?.Invoke();
    }

    /// <summary>
    /// Puts the way up before a directory's entries, when there is somewhere further up.
    /// </summary>
    /// <remarks>
    /// There is nowhere further up at the base as well as at the top of the platform: the base is the place
    /// a user works in, and a listing that offered a step out of it would offer a step out of the work — which
    /// is what the base is for preventing. Going above it is still possible by the other ways the location
    /// changes; what the listing does not do is make one of them.
    /// </remarks>
    /// <param name="directory">The directory the entries were read from.</param>
    /// <param name="entries">What it holds.</param>
    private IReadOnlyList<Entry> Stage(string directory, IReadOnlyList<Entry> entries)
    {
        // The hidden ones come out here rather than never being read, because a listing that was read
        // without them would have to be read again the moment they were asked for.
        var shown = _shared.ShowHidden ? entries : entries.Where(entry => !entry.Hidden).ToArray();

        return ParentOf(directory) is null || Same(directory, _shared.BaseDir)
            ? shown
            : [new Entry(UpName, EntryKind.Directory), .. shown];
    }

    /// <summary>
    /// Whether the platform hides an item.
    /// </summary>
    /// <remarks>
    /// A name beginning with a dot is the Unix convention; Windows marks an attribute instead, but a
    /// dot-name is read the same way there by everything that is not Explorer, so both count there. Asking
    /// for the attribute can fail on an item this process may not touch, and an item that cannot be asked
    /// about is shown rather than hidden: hiding something for not being inspectable is the worse mistake.
    /// <para>
    /// It is the browser's rule rather than a listing's, because the completions an address offers keep the
    /// same entries out — a path that cannot be reached by typing it is not an address.
    /// </para>
    /// </remarks>
    /// <param name="path">The item.</param>
    public static bool IsHidden(string path)
    {
        var name = Path.GetFileName(path);

        if (name.Length > 1 && name[0] == '.' && name != UpName)
        {
            return true;
        }

        if (!OperatingSystem.IsWindows())
        {
            return false;
        }

        try
        {
            return (File.GetAttributes(path) & FileAttributes.Hidden) != 0;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return false;
        }
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
                        System.IO.Directory.Exists(path) ? EntryKind.Directory : EntryKind.File,
                        IsHidden(path)
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

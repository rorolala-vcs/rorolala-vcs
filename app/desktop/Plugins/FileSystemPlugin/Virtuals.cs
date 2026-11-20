using System.ComponentModel;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// What a virtual entry is saying about the thing that would be at its path.
/// </summary>
/// <remarks>
/// The three are exclusive: an entry that is being made is not also one that is lost, and an entry with a
/// proportion has nothing left to announce. What each is drawn as is the listing's answer, see
/// <see cref="VirtualEntries"/>.
/// </remarks>
public enum VirtualEntryState
{
    /// <summary>Something is on its way here, and how far along it is is not known.</summary>
    Pending,

    /// <summary>Something is on its way here, and <see cref="VirtualEntry.Progress"/> says how far.</summary>
    Progress,

    /// <summary>A path something used to be at, which holds nothing now.</summary>
    Lost,
}

/// <summary>
/// One entry a listing shows that is not on disk: a thing on its way, or a thing that has gone.
/// </summary>
/// <remarks>
/// It is a class rather than a record because a row outlives the value it was made from: the ring drawn over
/// the icon listens to this very object, so a proportion that moves is redrawn in place rather than the whole
/// listing being built again every few dozen milliseconds. What may be written is written only through
/// <see cref="VirtualEntries"/>, which is the one place the catalogue and the object are kept in step.
/// <para>
/// <see cref="Path"/> is the identity: two entries with one path are one entry, and replacing the kind is the
/// only way one stops being the same thing.
/// </para>
/// </remarks>
public sealed class VirtualEntry : INotifyPropertyChanged
{
    /// <summary>How close two proportions have to be to be the same one, as a drawing can tell.</summary>
    private const float Same = 0.001f;

    /// <summary>What it says about the path.</summary>
    private VirtualEntryState _state;

    /// <summary>How far along what is on its way is, from nothing to all of it.</summary>
    private float _progress;

    /// <summary>What the entry was given to wear, if anything.</summary>
    private IReadOnlyList<EntryMark> _badges;

    /// <summary>Makes an entry, which only the catalogue does.</summary>
    /// <param name="path">The absolute path the entry is at.</param>
    /// <param name="kind">Whether the path is a file or a directory.</param>
    /// <param name="state">What the entry says about the path.</param>
    /// <param name="progress">How far along it is, clamped to the range a proportion has.</param>
    /// <param name="badges">What the entry was given to wear.</param>
    internal VirtualEntry(
        string path,
        EntryKind kind,
        VirtualEntryState state,
        float progress,
        IReadOnlyList<EntryMark> badges
    )
    {
        Path = path;
        Kind = kind;
        _state = state;
        _progress = Clamp(progress);
        _badges = badges;
    }

    /// <summary>Raised when what the entry says about its path changes.</summary>
    /// <remarks>
    /// Raised on the thread that changed it, which is not always the window's: a bring is watched by a poll of
    /// its own. What listens is a control, and a control is the one that knows how to reach its own thread.
    /// </remarks>
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>The absolute path the entry stands at.</summary>
    public string Path { get; }

    /// <summary>Whether the entry stands for a file or a directory.</summary>
    public EntryKind Kind { get; }

    /// <summary>The entry as a listing holds it, which is the shape a row and a column are read through.</summary>
    public Entry Entry => new(Path, Kind);

    /// <summary>What the entry says about its path.</summary>
    public VirtualEntryState State => _state;

    /// <summary>How far along what is on its way is, from nothing to all of it.</summary>
    public float Progress => _progress;

    /// <summary>
    /// What the entry wears, in the order the badges are numbered, or nothing when it wears none.
    /// </summary>
    public IReadOnlyList<EntryMark> Badges => _badges;

    /// <summary>Writes what the entry says, telling whoever listens about each thing that changed.</summary>
    /// <remarks>
    /// Every property is told apart rather than one change being announced for all three: a ring that is
    /// turned by the state does not need to be told about a proportion it does not draw, and the other way
    /// round.
    /// </remarks>
    /// <param name="state">What the entry is to say.</param>
    /// <param name="progress">How far along it is.</param>
    /// <param name="badges">What it is to wear.</param>
    internal void Show(
        VirtualEntryState state,
        float progress,
        IReadOnlyList<EntryMark> badges
    )
    {
        if (_state != state)
        {
            _state = state;
            Told(nameof(State));
        }

        var clamped = Clamp(progress);

        if (Math.Abs(_progress - clamped) > Same)
        {
            _progress = clamped;
            Told(nameof(Progress));
        }

        if (!ReferenceEquals(_badges, badges))
        {
            _badges = badges;
            Told(nameof(Badges));
        }
    }

    /// <summary>A proportion held to the range a proportion has.</summary>
    /// <param name="progress">What was asked for.</param>
    /// <returns>The proportion to keep, which is nothing for a number that is not a number.</returns>
    internal static float Clamp(float progress) =>
        float.IsNaN(progress) ? 0f : Math.Clamp(progress, 0f, 1f);

    /// <summary>Says one property changed.</summary>
    /// <param name="property">The property's name.</param>
    private void Told(string property) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(property));
}

/// <summary>
/// The entries a listing shows that are not on disk, and the one way to make, move and take them away.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on the File System plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c> may hand
/// entries to <see cref="Place"/>. The catalogue is the run's rather than one listing's, because an entry
/// belongs to where it would be and not to a dock: the same thing on its way is shown by every dock looking
/// at its directory, and taken away once.
/// <para>
/// A path is the identity, and a path that exists on disk wins: <see cref="Find"/> and <see cref="In"/> treat
/// an entry whose path is now a real file as spent, so a thing that has arrived is shown as itself rather
/// than under the picture that was waiting for it. Nothing is read from the disk here except that question,
/// and only for a path the catalogue is holding.
/// </para>
/// <para>
/// What a change to the catalogue comes to is one answer for every view: adding, taking away, or turning an
/// entry into or out of being lost raises <see cref="Changed"/>, which the plugin hands on as a redraw. A
/// proportion is not such a change: it is drawn by the ring the entry is bound to, and redrawing a listing of
/// a thousand rows for every step of a transfer would lose the choice and flicker.
/// </para>
/// </remarks>
public static class VirtualEntries
{
    /// <summary>What the catalogue holds, by the path each entry stands at.</summary>
    private static readonly Dictionary<string, VirtualEntry> Placed = new(Comparer);

    /// <summary>What guards <see cref="Placed"/>, <see cref="_shape"/> and the entries being written.</summary>
    private static readonly object Gate = new();

    /// <summary>How many times the set of entries has changed, which is what a view compares.</summary>
    private static int _shape;

    /// <summary>The comparison the platform names paths with.</summary>
    private static StringComparison Comparison =>
        OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;

    /// <summary>The comparer the catalogue's paths are kept by.</summary>
    private static StringComparer Comparer =>
        OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal;

    /// <summary>Raised when an entry is added, taken away or turned into or out of being lost.</summary>
    /// <remarks>
    /// Raised outside the catalogue's lock, and on whatever thread made the change: a listener that draws
    /// reaches its own thread itself. A proportion is not announced here.
    /// </remarks>
    internal static event Action? Changed;

    /// <summary>How many times the set of entries has changed, which a view compares to know to read again.</summary>
    public static int Shape
    {
        get
        {
            lock (Gate)
            {
                return _shape;
            }
        }
    }

    /// <summary>
    /// Puts an entry at a path, or moves the entry already there to what is asked.
    /// </summary>
    /// <remarks>
    /// The entry already at the path is reused rather than replaced, because the row drawn for it holds a ring
    /// bound to that object: a replacement would leave the ring listening to something nothing draws. The one
    /// exception is a change of kind, which is another thing at the same path and is drawn as one.
    /// <para>
    /// A loss does not overwrite something on its way. A path a bring is fetching is a path the work does not
    /// hold yet, so a reading of the work taken during the bring says it is lost; the bring is the newer and
    /// more particular answer, and the next reading after the bring has finished will say so if it still is.
    /// </para>
    /// </remarks>
    /// <param name="path">The absolute path the entry stands at.</param>
    /// <param name="kind">Whether it stands for a file or a directory.</param>
    /// <param name="state">What it says about the path.</param>
    /// <param name="progress">How far along it is, clamped to the range a proportion has.</param>
    /// <param name="badges">What it wears, or nothing to keep what it already wears.</param>
    /// <returns>The entry now at the path, which is the one already there when it was reused.</returns>
    /// <exception cref="ArgumentException">The path is not an absolute one.</exception>
    public static VirtualEntry? Place(
        string path,
        EntryKind kind,
        VirtualEntryState state = VirtualEntryState.Pending,
        float progress = 0f,
        IReadOnlyList<EntryMark>? badges = null
    )
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);

        if (!TryNormalize(path, out var at))
        {
            throw new ArgumentException(
                $"`{path}` is not an absolute path, and a virtual entry is at one",
                nameof(path)
            );
        }

        var worn = badges is null ? null : (IReadOnlyList<EntryMark>)[.. badges];
        VirtualEntry? entry = null;
        var wrote = false;
        var rose = false;

        lock (Gate)
        {
            if (Placed.TryGetValue(at, out var known))
            {
                if (state == VirtualEntryState.Lost
                    && known.State is VirtualEntryState.Pending or VirtualEntryState.Progress)
                {
                    return known;
                }

                if (known.Kind != kind)
                {
                    entry = new VirtualEntry(at, kind, state, progress, worn ?? []);
                    Placed[at] = entry;
                    rose = true;
                }
                else
                {
                    entry = known;
                    wrote = true;
                    rose = (known.State == VirtualEntryState.Lost) != (state == VirtualEntryState.Lost)
                        || worn is not null;
                }
            }
            else
            {
                entry = new VirtualEntry(at, kind, state, progress, worn ?? []);
                Placed[at] = entry;
                rose = true;
            }

            if (rose)
            {
                _shape++;
            }
        }

        if (wrote)
        {
            entry.Show(state, progress, worn ?? entry.Badges);
        }

        if (rose)
        {
            Changed?.Invoke();
        }

        return entry;
    }

    /// <summary>
    /// Says how far along the entry at a path is, and that it is no longer merely pending.
    /// </summary>
    /// <remarks>
    /// A proportion is not a change of the set, so nothing is raised for a view to read the landing again: the
    /// ring bound to the entry draws the new proportion itself. An entry that is lost has no proportion to
    /// move, and one that is not here has nothing to say.
    /// </remarks>
    /// <param name="path">The absolute path the entry stands at.</param>
    /// <param name="progress">How far along it is, clamped to the range a proportion has.</param>
    /// <returns>Whether an entry was there to be told.</returns>
    public static bool Advance(string path, float progress)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (!TryNormalize(path, out var at))
        {
            return false;
        }

        VirtualEntry? entry;

        lock (Gate)
        {
            if (!Placed.TryGetValue(at, out entry))
            {
                return false;
            }
        }

        if (entry.State == VirtualEntryState.Lost)
        {
            return false;
        }

        entry.Show(VirtualEntryState.Progress, progress, entry.Badges);

        return true;
    }

    /// <summary>Takes the entry at a path away, which is what a thing that has been given up on needs.</summary>
    /// <param name="path">The absolute path the entry stands at.</param>
    /// <returns>Whether an entry was there to take away.</returns>
    public static bool Withdraw(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (!TryNormalize(path, out var at))
        {
            return false;
        }

        lock (Gate)
        {
            if (!Placed.Remove(at))
            {
                return false;
            }

            _shape++;
        }

        Changed?.Invoke();

        return true;
    }

    /// <summary>Takes the entries at several paths away, which is what a reading that has moved on needs.</summary>
    /// <remarks>
    /// One change is raised for the set rather than one per path: a tree whose deletions have all been restored
    /// is one drawing, not one per file.
    /// </remarks>
    /// <param name="paths">The absolute paths the entries stand at.</param>
    public static void Withdraw(IEnumerable<string> paths)
    {
        ArgumentNullException.ThrowIfNull(paths);

        var took = false;

        lock (Gate)
        {
            foreach (var path in paths)
            {
                if (TryNormalize(path, out var at) && Placed.Remove(at))
                {
                    took = true;
                }
            }

            if (took)
            {
                _shape++;
            }
        }

        if (took)
        {
            Changed?.Invoke();
        }
    }

    /// <summary>
    /// The entry at a path, or nothing when nothing is there or the path is now a real entry.
    /// </summary>
    /// <remarks>
    /// The question of the disk is asked only after the catalogue is known to hold the path, so an ordinary
    /// entry of a listing costs no more than a dictionary miss. A virtual entry that has been covered is left
    /// in the catalogue rather than taken out here: this is asked while a row is drawn, and changing the set
    /// from a drawing would raise the very redraw that asked.
    /// </remarks>
    /// <param name="path">The path to ask about.</param>
    /// <returns>The entry, or nothing.</returns>
    public static VirtualEntry? Find(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (!TryNormalize(path, out var at))
        {
            return null;
        }

        VirtualEntry? entry;

        lock (Gate)
        {
            if (!Placed.TryGetValue(at, out entry))
            {
                return null;
            }
        }

        return Pathing.Exists(entry.Path) ? null : entry;
    }

    /// <summary>
    /// The entries directly in one directory, in the order a listing shows them.
    /// </summary>
    /// <remarks>
    /// What has arrived is left out, which is what makes a real file take the place of the entry that was
    /// waiting for it. Directories come before files and each kind is read by name, the same order a read of
    /// the directory asks of the filesystem.
    /// </remarks>
    /// <param name="directory">The absolute path of the directory.</param>
    /// <returns>The entries, or nothing when none is directly in it.</returns>
    public static IReadOnlyList<VirtualEntry> In(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        if (!TryNormalize(directory, out var parent))
        {
            return [];
        }

        VirtualEntry[] holding;

        lock (Gate)
        {
            holding =
            [
                .. Placed.Values.Where(entry =>
                    Path.GetDirectoryName(entry.Path) is { Length: > 0 } holder
                    && string.Equals(holder, parent, Comparison)
                ),
            ];
        }

        return
        [
            .. holding
                .Where(entry => !Pathing.Exists(entry.Path))
                .OrderBy(entry => entry.Kind == EntryKind.Directory ? 0 : 1)
                .ThenBy(entry => Path.GetFileName(entry.Path), StringComparer.OrdinalIgnoreCase)
                .ThenBy(entry => Path.GetFileName(entry.Path), StringComparer.Ordinal),
        ];
    }

    /// <summary>
    /// Forgets every entry and every listener, which only a suite that starts the plugin again needs.
    /// </summary>
    /// <remarks>
    /// The catalogue is the run's, and a run starts a program once: a suite that starts the plugin over and
    /// over in one process has to clear it, or one test's entries would answer for the next test's listing.
    /// The listeners go with the entries, since they belong to a host that has stopped.
    /// </remarks>
    internal static void Clear()
    {
        lock (Gate)
        {
            Placed.Clear();
            _shape++;
        }

        Changed = null;
    }

    /// <summary>The path a lookup is made by, or nothing when the path is not one an entry can be at.</summary>
    /// <remarks>
    /// The way up and the computer are places rather than paths, and a path relative to the process's own
    /// directory is not a place a listing can name: a virtual entry is always at an absolute one.
    /// </remarks>
    /// <param name="path">The path to read.</param>
    /// <param name="at">The path the catalogue keeps it by.</param>
    /// <returns>Whether it is a path the catalogue can name.</returns>
    private static bool TryNormalize(string path, out string at)
    {
        if (path.Length == 0 || !Path.IsPathFullyQualified(path))
        {
            at = string.Empty;

            return false;
        }

        at = Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));

        return true;
    }
}

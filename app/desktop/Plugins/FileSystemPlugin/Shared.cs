using System.Diagnostics.CodeAnalysis;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The few things about looking at files that are the whole plugin's rather than one location's.
/// </summary>
/// <remarks>
/// Where the tree is rooted, which providers hide what, and the news that the files themselves may have
/// changed. None of them is where anything is looking, and each is one answer for the whole File System: a
/// base per dock would be several answers to where the tree is rooted, a hiding per dock would let two docks
/// list one directory differently, and a change made through one dock is one every other dock may be looking
/// at.
/// <para>
/// They live here rather than on a <see cref="Browser"/> because a dock may be taken out of step and given a
/// location of its own. The location is then that dock's alone, while these stay everybody's,
/// which is what a location reads when it stages a listing, and this is what tells every one of them that
/// something they read has moved on.
/// </para>
/// </remarks>
[SuppressMessage(
    "Design",
    "CA1001:Types that own disposable fields should be disposable",
    Justification = "the watch lives as long as the program does: there is one of these per plugin, and nothing closes a plugin"
)]
internal sealed class Shared
{
    /// <summary>How long a burst of filesystem changes is gathered before a view is told about it.</summary>
    private static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(250);

    /// <summary>The setting the choice about showing what is hidden is kept under.</summary>
    public const string ShowSetting = "Hides/show";

    /// <summary>Whether a read has been asked for and is waiting for the turn to end.</summary>
    private bool _asked;

    /// <summary>The directories a change has been said of in this turn, so that one turn says it once.</summary>
    private readonly HashSet<string> _asking = new(StringComparer.Ordinal);

    /// <summary>The entries a change has been said of in this turn, so that one turn says it once.</summary>
    private readonly HashSet<string> _askingEntry = new(StringComparer.Ordinal);

    /// <summary>The directory the tree is rooted at.</summary>
    private string _base;

    /// <summary>Whether the entries the providers hide are shown, faded.</summary>
    private bool _shown;

    /// <summary>Which providers hide what, asked afresh wherever an entry is staged or faded.</summary>
    private readonly HideRegistry _hides;

    /// <summary>The plugin's own settings, where the choice about showing what is hidden is kept.</summary>
    private readonly IPluginConfig _config;

    /// <summary>Where the plugin says that what it did changed the files.</summary>
    private readonly IFileChanges _files;

    /// <summary>Whether a touch is being handed to the host, so that the host's answer does not come back as one.</summary>
    private bool _telling;

    /// <summary>The directories on screen, watched for what another program does to them.</summary>
    private readonly WatchService _watch;

    /// <summary>What is watched, for the caller that wires what a change is said through.</summary>
    internal WatchService Watch => _watch;

    /// <summary>Makes the shared answers, with the tree rooted at a directory.</summary>
    /// <param name="baseDir">The directory to root the tree at.</param>
    /// <param name="hides">The hide providers, and the choice of which are in force.</param>
    /// <param name="config">The plugin's settings, where the show choice is read and kept.</param>
    /// <param name="files">Where the plugin says that the files may have changed.</param>
    public Shared(string baseDir, HideRegistry hides, IPluginConfig config, IFileChanges files)
    {
        _base = baseDir;
        _hides = hides;
        _config = config;
        _files = files;
        _shown = config.ReadKeyAs(ShowSetting, false);
        _watch = new WatchService(Settle);

        // A read that some other plugin worked out is drawn again rather than read again: the two are the halves of one
        // channel, and taking the other one would empty every reader's answers for a reading that changed nothing.
        files.Redrawn += Drawn;

        // Said here as well as where the base moves, because a dependent plugin reads it before
        // anything moves: the directory a run starts on is as worth publishing as one it is rooted
        // at later, and a view of the work that was blank until the first move would be wrong about
        // the run it was opened in.
        BaseDirectory.Publish(_base);
    }

    /// <summary>
    /// Raised when either answer changes, so that every location stages its listing again.
    /// </summary>
    /// <remarks>
    /// A location is what holds a listing, so a change here has to reach every location there is, including
    /// one a dock was given for being out of step, since a listing follows the base and the hiding wherever it
    /// is.
    /// </remarks>
    public event Action? Changed;

    /// <summary>
    /// Raised when the files may have changed, so that every location reads its directory again.
    /// </summary>
    /// <remarks>
    /// Every location and not the one that acted, because a change made through one dock is one another may
    /// be looking at: a move is answered by the dock it was dropped on, and the dock the entries came out of
    /// is showing the directory they left, which, a dock being able to be out of step, may be a directory
    /// only that dock is looking at.
    /// </remarks>
    public event Action? Touched;

    /// <summary>
    /// Raised when the files of one directory may have changed, so that what draws it reads it again.
    /// </summary>
    /// <remarks>
    /// The narrow half of <see cref="Touched"/>: a change another program makes is a fact about one directory,
    /// and a location over another directory has nothing to read for it. What is read is the location's own
    /// directory, and only a location showing that one reads at all.
    /// </remarks>
    public event Action<string>? TouchedIn;

    /// <summary>
    /// Raised when one entry may have changed content or time, so that what draws it asks for it again.
    /// </summary>
    /// <remarks>
    /// The entry has not moved in its directory, so nothing is read: what draws it draws that entry again, which
    /// is where its picture, its marks and its columns are asked for afresh.
    /// </remarks>
    public event Action<string>? EntryIn;

    /// <summary>
    /// Raised when an answer a plugin worked out is at hand, so that every location draws again without reading.
    /// </summary>
    /// <remarks>
    /// The other half of <see cref="Touched"/>: a reading that landed changed nothing on disk, so what is drawn from it
    /// is drawn again and nobody's answers are given back. Saying a touch instead would have every reader of the work
    /// read again for a reading it is already holding, which is one program run and one walk of the tree per drawing,
    /// see <see cref="FilesChanged"/>.
    /// </remarks>
    public event Action? Redrawn;

    /// <summary>
    /// Says that the files may have changed, which every location reads its directory again for.
    /// </summary>
    /// <remarks>
    /// Deferred to the end of the turn and said at most once in it. Deferred, because whoever asks has just
    /// finished an event and reading a directory again rebuilds the views showing it, one of which may be the
    /// view answering that event. Once a turn, because several things that finish together are one change to go
    /// and look for.
    /// <para>
    /// The host is told as well, once: what this plugin changed by a command of its own is a change the next plugin's
    /// readers have to know about, and nothing else says so. What comes back from the host is a touch of its own, and
    /// is dropped rather than posted again (<see cref="_telling"/>), the ring that would otherwise be one touch per
    /// turn for as long as the window is open.
    /// </para>
    /// </remarks>
    public void Touch()
    {
        if (_telling || _asked)
        {
            return;
        }

        _asked = true;

        Frame.Post(() =>
        {
            _asked = false;
            FilesChanged();
            Tell();
        });
    }

    /// <summary>A directory that is now on screen, which is watched until it is hidden as many times.</summary>
    /// <param name="directory">The directory.</param>
    public void Showing(string directory) => _watch.Show(directory);

    /// <summary>A directory that is no longer on screen.</summary>
    /// <param name="directory">The directory.</param>
    public void Hiding(string directory) => _watch.Hide(directory);

    /// <summary>
    /// Says that what one directory holds may have changed, which what draws it reads again for.
    /// </summary>
    /// <remarks>
    /// Said per directory rather than across the board, because that is what is known: a file appeared, went or
    /// was renamed in one place, and a location drawing another place has nothing to read for it. Deferred to the
    /// end of the turn and said at most once per directory in it, for the reason <see cref="Touch"/> is.
    /// </remarks>
    /// <param name="directory">The directory whose entries may differ.</param>
    public void DirectoryChanged(string directory)
    {
        ArgumentException.ThrowIfNullOrEmpty(directory);

        Forget();

        // The next plugin's readers are told the same thing, and told what it is about: what they keep is kept per
        // directory, and a reading for another directory is not made wrong by a change in this one.
        _files.TouchIn(directory);

        if (!_asking.Add(directory))
        {
            return;
        }

        Frame.Post(() =>
        {
            _asking.Remove(directory);
            TouchedIn?.Invoke(directory);
        });
    }

    /// <summary>
    /// Says that one entry may have changed content or time, which what draws it asks for again.
    /// </summary>
    /// <remarks>
    /// Nothing is read: where the entry is and what it is called have not moved, and what a view draws of it is
    /// the one thing that has to be worked out again. Deferred and said once per entry in a turn, as a change is.
    /// </remarks>
    /// <param name="path">The entry.</param>
    public void EntryChanged(string path)
    {
        ArgumentException.ThrowIfNullOrEmpty(path);

        Forget();

        if (Path.GetDirectoryName(path) is { Length: > 0 } directory)
        {
            _files.TouchIn(directory);
        }

        if (!_askingEntry.Add(path))
        {
            return;
        }

        Frame.Post(() =>
        {
            _askingEntry.Remove(path);
            EntryIn?.Invoke(path);
        });
    }

    /// <summary>
    /// What a touch comes to: what the providers answered is let go of, and every location is told to read
    /// its directory again.
    /// </summary>
    /// <remarks>
    /// The letting go is why this is a step of its own rather than part of the raising. What a provider
    /// answered is read from the files this says may have changed, a lock is read from the Layout a file
    /// operation renames a path in, and a hide rule is read from files a change may have rewritten, so a
    /// listing read from an answer that was already stale would be drawn wrong until the next touch. What is
    /// read again and what is drawn from it are both later than this, which is what makes the order the whole
    /// of the point.
    /// </remarks>
    public void FilesChanged()
    {
        Forget();

        Touched?.Invoke();
    }

    /// <summary>Lets go of what the readers answered, since what they read may have moved on.</summary>
    /// <remarks>
    /// Said to the catalogue rather than to this reading of the settings: what a provider answered is worth
    /// letting go of whether or not it is in force, and what is in force is not what decides it.
    /// </remarks>
    private static void Forget()
    {
        global::FileSystemPlugin.HideRegistry.Forget();
        EntryLockProviders.Forget();
        EntryMarkProviders.Forget();
        EntryColourProviders.Forget();
        EntryColumns.Forget();
    }

    /// <summary>
    /// Says that what is shown of the files has changed, which every location stages again.
    /// </summary>
    /// <remarks>
    /// Raised when a hide provider is switched on or off, which changes what a listing shows without
    /// changing what the directory holds: every location stages its listing again and none is read again.
    /// </remarks>
    public void Hidden() => Changed?.Invoke();

    /// <summary>Hands a touch to the host, once, and drops the touch the host says back.</summary>
    /// <remarks>
    /// Guarded rather than compared: the host's own answer arrives while this is still telling it, so the flag is what
    /// says the touch was this plugin's own rather than one from another plugin.
    /// </remarks>
    private void Tell()
    {
        _telling = true;

        try
        {
            _files.Touch();
        }
        finally
        {
            _telling = false;
        }
    }

    /// <summary>Says that an answer this plugin read is at hand, so every location draws again without reading.</summary>
    private void Drawn() => Redrawn?.Invoke();

    /// <summary>
    /// The directory the tree is rooted at, which a directory's own menu sets.
    /// </summary>
    /// <remarks>
    /// The tree is rooted here rather than at a location so that stepping through directories does not move
    /// the tree: what a user opens stays open, and the tree is the one view that is not rearranged by where a
    /// browser happens to be.
    /// </remarks>
    public string BaseDir
    {
        get => _base;
        set
        {
            if (string.Equals(_base, value, StringComparison.Ordinal))
            {
                return;
            }

            _base = value;
            BaseDirectory.Publish(_base);
            Changed?.Invoke();
        }
    }

    /// <summary>
    /// Whether the entries a hiding rule takes out are shown, faded.
    /// </summary>
    /// <remarks>
    /// It is the plugin's rather than a dock's, because the listing is what it changes: two docks looking at
    /// one directory must not list different things. It is kept in the plugin's own settings rather than in a
    /// dock's own state, so that one switch serves every dock and the choice outlives the dock it was made in.
    /// <para>
    /// Kept here and written through the plugin's settings in both directions, so that the preference panel
    /// drawing it and a listing reading it are one answer. A rule in the faded state is drawn faded whether
    /// this is on or off: the switch is about the rules that hide.
    /// </para>
    /// </remarks>
    public bool ShowHidden
    {
        get => _shown;
        set
        {
            if (_shown == value)
            {
                return;
            }

            _shown = value;

            // Written rather than restaged directly, and the restage left to the one listener of a setting
            // change: a second path here would stage every listing twice for one click.
            _config.Keep(ShowSetting, value ? "true" : "false");
        }
    }

    /// <summary>
    /// Whether an entry is hidden by a provider in force.
    /// </summary>
    /// <remarks>
    /// It is the plugin's rather than a view's, because the listing is what it changes: two docks looking at
    /// one directory, and a dock out of step still looks at directories, must not list different things.
    /// What a dock shows is the one setting every dock reads, so a switch turned in one of them is turned
    /// in all.
    /// <para>
    /// Asked of the registry rather than kept here, so that changing a rule takes effect without a second
    /// read of any directory: what the directory holds is unchanged, and only what is shown of it is. Only a
    /// rule in the hiding state answers yes here; one in the faded state is handled by <see cref="Dims(Entry, string)"/>.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether it is hidden.</returns>
    public bool Hides(Entry entry) => HideRegistry.Hides(entry);

    /// <summary>
    /// Whether an entry is hidden by a provider in force, read against a view's root.
    /// </summary>
    /// <remarks>
    /// The root is what a rule about a repository is read from, which is the place a view is rooted at
    /// rather than the entry's own directory: a tree reads directories under one base, and a rule read
    /// from each of them in turn would let a repository inside the base answer for itself.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>Whether it is hidden.</returns>
    public bool Hides(Entry entry, string root) => HideRegistry.Hides(entry, root);

    /// <summary>
    /// Whether an entry is drawn faded, read against a view's root.
    /// </summary>
    /// <remarks>
    /// A rule in the faded state draws its entries faded whether or not the eye is on. A rule in the hiding
    /// state draws them faded only while the eye is on, because that is what the eye now says: the entries
    /// the rules would leave out are being looked at, in gray rather than as ordinary entries.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>Whether it is drawn faded.</returns>
    public bool Dims(Entry entry, string root) =>
        HideRegistry.Dims(entry, root) || (ShowHidden && HideRegistry.Hides(entry, root));

    /// <summary>
    /// Whether an entry is drawn faded, read against the directory holding it.
    /// </summary>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether it is drawn faded.</returns>
    public bool Dims(Entry entry) => HideRegistry.Dims(entry) || (ShowHidden && HideRegistry.Hides(entry));

    /// <summary>The registry itself, for the choice of which providers are in force.</summary>
    /// <remarks>
    /// The catalogue it reads is the run's rather than this instance's, see <see cref="HideRegistry.All"/>,
    /// so a dock draws the rules from the class and the choice from the registry.
    /// </remarks>
    public HideRegistry HideRegistry => _hides;
}

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
/// location of its own (Section 7.5). The location is then that dock's alone, while these stay everybody's —
/// which is what a location reads when it stages a listing, and this is what tells every one of them that
/// something they read has moved on.
/// </para>
/// </remarks>
internal sealed class Shared
{
    /// <summary>The setting the choice about showing what is hidden is kept under.</summary>
    public const string ShowSetting = "Hides/show";

    /// <summary>Whether a read has been asked for and is waiting for the turn to end.</summary>
    private bool _asked;

    /// <summary>The directory the tree is rooted at.</summary>
    private string _base;

    /// <summary>Whether the entries the providers hide are shown, faded.</summary>
    private bool _shown;

    /// <summary>Which providers hide what, asked afresh wherever an entry is staged or faded.</summary>
    private readonly HideRegistry _hides;

    /// <summary>The plugin's own settings, where the choice about showing what is hidden is kept.</summary>
    private readonly IPluginConfig _config;

    /// <summary>Makes the shared answers, with the tree rooted at a directory.</summary>
    /// <param name="baseDir">The directory to root the tree at.</param>
    /// <param name="hides">The hide providers, and the choice of which are in force.</param>
    /// <param name="config">The plugin's settings, where the show choice is read and kept.</param>
    public Shared(string baseDir, HideRegistry hides, IPluginConfig config)
    {
        _base = baseDir;
        _hides = hides;
        _config = config;
        _shown = config.ReadKeyAs(ShowSetting, false);
    }

    /// <summary>
    /// Raised when either answer changes, so that every location stages its listing again.
    /// </summary>
    /// <remarks>
    /// A location is what holds a listing, so a change here has to reach every location there is — including
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
    /// is showing the directory they left — which, a dock being able to be out of step, may be a directory
    /// only that dock is looking at.
    /// </remarks>
    public event Action? Touched;

    /// <summary>
    /// Says that the files may have changed, which every location reads its directory again for.
    /// </summary>
    /// <remarks>
    /// Deferred to the end of the turn and said at most once in it. Deferred, because whoever asks has just
    /// finished an event and reading a directory again rebuilds the views showing it — one of which may be the
    /// view answering that event. Once a turn, because several things that finish together are one change to go
    /// and look for.
    /// </remarks>
    public void Touch()
    {
        if (_asked)
        {
            return;
        }

        _asked = true;

        Dispatcher.UIThread.Post(() =>
        {
            _asked = false;
            FilesChanged();
        });
    }

    /// <summary>
    /// What a touch comes to: what the lock providers answered is let go of, and every location is told to
    /// read its directory again.
    /// </summary>
    /// <remarks>
    /// The letting go is why this is a step of its own rather than part of the raising. An answer about an
    /// entry is read from the tree this says has changed — a Layout names each path by a `Uuid`, and a move
    /// renames one — so a listing read from an answer that was already stale would be drawn wrong until the
    /// next touch. What is read again and what is drawn from it are both later than this, which is what makes
    /// the order the whole of the point.
    /// </remarks>
    public void FilesChanged()
    {
        EntryLockProviders.Forget();
        Touched?.Invoke();
    }

    /// <summary>
    /// Says that what is shown of the files has changed, which every location stages again.
    /// </summary>
    /// <remarks>
    /// Raised when a hide provider is switched on or off, which changes what a listing shows without
    /// changing what the directory holds: every location stages its listing again and none is read again.
    /// </remarks>
    public void Hidden() => Changed?.Invoke();

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
            Changed?.Invoke();
        }
    }

    /// <summary>
    /// Whether the entries a provider hides are shown, faded.
    /// </summary>
    /// <remarks>
    /// It is the plugin's rather than a dock's, because the listing is what it changes: two docks looking at
    /// one directory must not list different things. It is kept in the plugin's own settings rather than in a
    /// dock's own state, so that one switch serves every dock and the choice outlives the dock it was made in.
    /// <para>
    /// Kept here and written through the plugin's settings in both directions, so that the preference panel
    /// drawing it and a listing reading it are one answer.
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
    /// one directory — and a dock out of step still looks at directories — must not list different things.
    /// What a dock shows is the one setting every dock reads, so a switch turned in one of them is turned
    /// in all (Section 7.5).
    /// <para>
    /// Asked of the registry rather than kept here, so that turning a provider off takes effect without a
    /// second read of any directory: what the directory holds is unchanged, and only what is shown of it is.
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

    /// <summary>The registry itself, for the choice of which providers are in force.</summary>
    /// <remarks>
    /// The catalogue it reads is the run's rather than this instance's — see <see cref="HideRegistry.All"/> —
    /// so a dock draws the rules from the class and the choice from the registry.
    /// </remarks>
    public HideRegistry HideRegistry => _hides;
}

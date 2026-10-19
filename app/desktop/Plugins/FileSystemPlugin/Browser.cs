using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// Where one is, where it has been, and what is there.
/// </summary>
/// <remarks>
/// The plugin has one of these and every dock is a view onto it — unless the dock has been taken out of
/// step, in which case it is given one of its own and looks at a directory of its own. How the
/// entries are laid out differs from dock to dock, and so may the directory; what every one of them agrees
/// on — where the tree is rooted, and whether hidden entries are shown — is not here but in
/// <see cref="Shared"/>.
/// <para>
/// The rest of the state is the location's own, kept by the plugin rather than the host: what a directory
/// holds, and where this one has been, are its to know. What it reads is said in the log, because a view
/// that did not draw a change it was told about and a view that was never told read the same from the
/// outside.
/// </para>
/// <para>
/// Where one is may also be a place a plugin answers rather than a directory, written as an address
/// (<see cref="LogicalPlace"/>): a plugin's own kind of place, with its own entries and its own title, reached
/// through the very same history and the very same address bar. Keeping the two in one location is what makes
/// stepping between a directory and a place an ordinary move rather than a second kind of navigation.
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

    /// <summary>Where a read is said, so that a view that was told can be told from one that was not.</summary>
    private readonly ILog _log;

    /// <summary>What answers the prefixes of the addresses this location may be given.</summary>
    private readonly ILogicalViewRegistry _views;

    /// <summary>Where it has been, most recent last.</summary>
    private readonly List<string> _back = [];

    /// <summary>Where going back would come to, most recent last.</summary>
    private readonly List<string> _forward = [];

    /// <summary>The directory being looked at.</summary>
    private string _current;

    /// <summary>
    /// The place being looked at, or nothing while this location is looking at a directory.
    /// </summary>
    /// <remarks>
    /// One location or the other, never both: where a browser is is either a path the filesystem answers or an
    /// address a plugin answers, and a listing is drawn from whichever it is. Keeping the two on one location
    /// rather than giving logical views a browser of their own is what keeps one history and one way to move —
    /// back, forward and the address switch between a directory and a place as freely as between two directories.
    /// </remarks>
    private LogicalPlace? _logical;

    /// <summary>What the directory held when it was last read.</summary>
    private IReadOnlyList<Entry> _entries;

    /// <summary>What a listing shows of it, which is those entries with the way up before them.</summary>
    private IReadOnlyList<Entry> _shown;

    /// <summary>
    /// The directories whose hide rules are being read, so that one read is asked for once.
    /// </summary>
    /// <remarks>
    /// A rule about a repository is a program run, which is why it is read off the window's thread. Every listing of
    /// the directory asks while it is going, and one run per asking would be a run per frame.
    /// </remarks>
    private readonly HashSet<string> _asking = new(StringComparer.Ordinal);

    /// <summary>
    /// The directories whose hide rules have been asked for, so a provider that cannot answer is not asked per drawing.
    /// </summary>
    /// <remarks>
    /// A provider that throws, or one whose program is not installed, leaves <see cref="HideRegistry.Ready"/> saying no
    /// for as long as it is asked — and asking again on every staging would be a drawing that starts a read which
    /// cannot finish. What was asked is therefore asked again only when the files are said to have changed, which is
    /// when a rule or a repository may have appeared.
    /// </remarks>
    private readonly HashSet<string> _asked = new(StringComparer.Ordinal);

    /// <summary>Makes a location onto a directory, or onto a place.</summary>
    /// <param name="log">Where a read of the directory is said.</param>
    /// <param name="shared">The answers every location shares.</param>
    /// <param name="directory">
    /// The directory to look at, or the address of a place to open instead — which is what a dock taken out of
    /// step is given when it was out of step at a logical address.
    /// </param>
    /// <param name="views">What answers the prefixes of the addresses this location may be given.</param>
    public Browser(ILog log, Shared shared, string directory, ILogicalViewRegistry views)
    {
        _log = log;
        _shared = shared;
        _views = views;
        _current = directory;

        if (LogicalAddress.TryRead(directory, out _))
        {
            _logical = Logical.Find(directory, views);
            _entries = [];
            _shown = [];
        }
        else
        {
            _entries = Read(directory);
            _shown = Stage(directory, _entries);
        }

        // A listing follows the shared answers wherever it is, so this location has to be told when they
        // change — and that is why a location given to a dock out of step is let go of rather than dropped.
        // The two things told apart: an answer changing is a listing to make again out of what was read, where
        // the files changing is the directory to read again. A read that landed is neither: it is this very listing
        // drawn again, out of answers the readers already hold.
        _shared.Changed += Restage;
        _shared.Touched += Refresh;
        _shared.Redrawn += Redraw;
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
    /// Expands a leading <c>~</c> into the user's home directory.
    /// </summary>
    /// <remarks>
    /// The address is the one place a shell-like path is typed, and <c>~</c> is what a hand reaches for to
    /// name home. Only a <c>~</c> that names a step of its own is expanded — <c>~</c> alone, or <c>~/</c> and
    /// what follows it; anywhere else it is an ordinary name that begins with a tilde, and naming one is left
    /// to <c>./~</c>, which is how a shell reads it too.
    /// <para>
    /// <c>~user</c> is not expanded: this process knows the directory of the user it runs as and no other, so
    /// a home guessed from a name would be a path that answers for somebody else.
    /// </para>
    /// </remarks>
    /// <param name="path">The typed path.</param>
    /// <returns>The path with a leading <c>~</c> replaced by the home directory.</returns>
    public static string Expand(string path)
    {
        if (path.Length == 0 || path[0] != '~')
        {
            return path;
        }

        if (
            path.Length > 1
            && path[1] != Path.DirectorySeparatorChar
            && path[1] != Path.AltDirectorySeparatorChar
        )
        {
            return path;
        }

        var rest = path.Length > 1
            ? path[2..].TrimStart(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar)
            : string.Empty;

        return rest.Length == 0
            ? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile)
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), rest);
    }

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

    /// <summary>
    /// Raised when the directory has been read again, which is what the files having changed looks like here.
    /// </summary>
    /// <remarks>
    /// Apart from <see cref="Changed"/>, which also says where the browser went: a tree is rooted at the base and
    /// is not rearranged by a step, but what lies under the steps it has read may have moved — which is a thing
    /// only a view that draws more than the one directory has to read again for.
    /// </remarks>
    public event Action? Reread;

    /// <summary>
    /// The directory being looked at, or the address of the place being looked at.
    /// </summary>
    /// <remarks>
    /// Spelled the one way wherever it is asked for, so that the address the path bar shows and the way back
    /// through the history are the same string in both modes.
    /// </remarks>
    public string Current => _logical?.Address.ToString() ?? _current;

    /// <summary>
    /// The place being looked at, or nothing while this location is looking at a directory.
    /// </summary>
    /// <remarks>
    /// Named for the place rather than for the view, so that a property and the reader that answers it are not
    /// read as one word.
    /// </remarks>
    public LogicalPlace? Place => _logical;

    /// <summary>Whether this location is looking at a place a plugin answers rather than at a directory.</summary>
    public bool IsLogical => _logical is not null;

    /// <summary>
    /// The directory the tree is rooted at, which a directory's own menu sets.
    /// </summary>
    /// <remarks>
    /// The answer is the whole plugin's rather than this location's (see <see cref="Shared"/>), and it is
    /// reached through a location because a location is what a view holds.
    /// </remarks>
    public string BaseDir => _shared.BaseDir;

    /// <summary>
    /// What is over where this location is looking: the directory holding a directory, or the address over a place.
    /// </summary>
    /// <remarks>
    /// A place has no directory over it — an address is not a path — so what is over one is the plugin's answer
    /// rather than this one's (<see cref="ILogicalViewProvider.Parent"/>), and nothing when the plugin says there
    /// is no step up. Either way it is one string, which is what makes the way up one button that goes somewhere.
    /// </remarks>
    public string? Parent => _logical is not null ? _logical.Parent?.ToString() : ParentOf(_current);

    /// <summary>Whether going back has anywhere to go.</summary>
    public bool CanGoBack => _back.Count > 0;

    /// <summary>Whether going forward has anywhere to go.</summary>
    public bool CanGoForward => _forward.Count > 0;

    /// <summary>Whether going up has anywhere to go.</summary>
    public bool CanGoUp => Parent is not null;

    /// <summary>What the directory holds, directories first and then by name.</summary>
    public IReadOnlyList<Entry> Entries => _entries;

    /// <summary>
    /// Whether a provider in force hides an entry.
    /// </summary>
    /// <remarks>
    /// One answer for the whole plugin rather than for this location (see <see cref="Shared"/>), reached
    /// through a location because a location is what a view holds — and because the completions an address
    /// offers keep the same entries out, a path that cannot be reached by typing it not being an address.
    /// <para>
    /// Asked rather than kept on the entry, because the answer changes with a switch the user turns and must
    /// not need the directory read again: a listing read without the hidden entries would be a listing that
    /// had to be read again the moment they were asked for.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether it is hidden.</returns>
    public bool Hides(Entry entry) => _shared.Hides(entry);

    /// <summary>
    /// Whether a provider in force hides an entry, read against a view's root.
    /// </summary>
    /// <remarks>
    /// A view that reads more than the one directory — the tree — is rooted somewhere, and a rule about
    /// a repository is read from there rather than from each directory it walks into (see
    /// <see cref="IEntryHideProvider.Hides(Entry, string)"/>).
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>Whether it is hidden.</returns>
    public bool Hides(Entry entry, string root) => _shared.Hides(entry, root);

    /// <summary>
    /// The providers in force, which a view that reads a whole tree watches to know when to read again.
    /// </summary>
    /// <remarks>
    /// A tree's rows are read a step at a time and kept, so a change to what is hidden is not something
    /// a repaint reaches: which rows there are changes with it. What a view compares is the selection
    /// itself, since a location's <see cref="Changed"/> says that something about where it looks moved
    /// and not what it was.
    /// </remarks>
    public HideRegistry HideRegistry => _shared.HideRegistry;

    /// <summary>
    /// Whether the entries a provider hides are shown, which the address completions read too.
    /// </summary>
    /// <remarks>
    /// The completions offer what the listing offers, so a path that is not in the listing is not offered by
    /// typing it either — and a path the listing is showing is one that can be reached.
    /// </remarks>
    public bool ShowHidden => _shared.ShowHidden;

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
    /// <para>
    /// A leading <c>~</c> names the user's home directory, which is expanded before anything is asked
    /// about the path (see <see cref="Expand"/>), so that what the address offers and what it goes to
    /// agree wherever home is.
    /// </para>
    /// <para>
    /// What is of the shape of an address goes to the plugin that claimed its prefix instead, whether or not one
    /// did: an address nobody answers is a place with nobody to ask, and saying so in the dock is what a browser
    /// that refused it could not do.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory or the address to look at.</param>
    /// <returns>Whether it went there.</returns>
    public bool Go(string directory)
    {
        // An address is asked of the registry before anything is asked of the filesystem: the two name different
        // kinds of place, and a directory that happens to hold an at-sign is not of the shape an address is.
        if (LogicalAddress.TryRead(directory, out _))
        {
            if (Same(directory, Current))
            {
                Refresh();

                return true;
            }

            _back.Add(Current);
            _forward.Clear();
            Enter(directory);

            return true;
        }

        // A leading `~` names home, which is what the address reaches for; anything else is the path it is.
        var target = Expand(directory);

        if (!IsComputer(target) && !System.IO.Directory.Exists(target))
        {
            return false;
        }

        if (Same(target, Current))
        {
            Refresh();

            return true;
        }

        _back.Add(Current);
        _forward.Clear();
        Move(target);

        return true;
    }

    /// <summary>
    /// Roots the tree at a directory, and looks there.
    /// </summary>
    /// <remarks>
    /// Setting the base moves the browser as well, because the two are one act to whoever asked: the
    /// tree is a view of the place being worked in, and a base the browser is not in would leave the
    /// tree showing somewhere else entirely.
    /// <para>
    /// A place is not a directory to root a tree at, so an address is refused here rather than moved to: what
    /// the tree would be rooted at is a question a plugin answers, and this is not the plugin being asked.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory to root the tree at.</param>
    /// <returns>Whether it went there and rooted it there.</returns>
    public bool SetBase(string directory)
    {
        if (LogicalAddress.TryRead(directory, out _))
        {
            return false;
        }

        if (!Go(directory))
        {
            return false;
        }

        // Setting the base moves this location as well, because the two are one act to whoever asked: a user
        // who roots the tree at a directory is working in it. A dock that has been taken out of step roots the
        // tree for everybody and moves only itself, so the base and where the whole is looking
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
        _forward.Add(Current);
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
        _back.Add(Current);
        Move(to);
    }

    /// <summary>
    /// Looks at the directory holding this one, if there is one.
    /// </summary>
    /// <remarks>
    /// A place has nowhere to go up to, which is what <see cref="Parent"/> being nothing there means: what is
    /// over a place is the plugin's answer and not a directory, so the way up is left to the toolbar's own.
    /// </remarks>
    public void Up()
    {
        if (Parent is { } parent)
        {
            Go(parent);
        }
    }

    /// <summary>Reads the directory again, which is what shows a change made elsewhere.</summary>
    public void Refresh()
    {
        if (_logical is not null)
        {
            // The place is asked again rather than the address kept, because a read that is asked for is what
            // the plugin's answer is for: what it holds may have moved without where it is having moved.
            var written = _logical.Address.ToString();

            _logical = Logical.Find(written, _views);

            // Said here rather than left to the views: what a view was told and what it drew are two different
            // answers to "did that change reach it", and a report of one that did not cannot tell them apart.
            _log.Info($"read `{written}` again: {_logical?.Entries.Count ?? 0} entries");

            Changed?.Invoke();
            Reread?.Invoke();

            return;
        }

        _entries = Read(_current);
        _shown = Stage(_current, _entries);

        // The files may have changed, which is when a rule or a repository may have appeared: what could not be read
        // before is asked for again.
        _asked.Clear();

        // Said here rather than left to the views: what a view was told and what it drew are two different
        // answers to "did that change reach it", and a report of one that did not cannot tell them apart.
        _log.Info($"read `{_current}` again: {_shown.Count} entries");

        Changed?.Invoke();
        Reread?.Invoke();
    }

    /// <summary>
    /// Draws what is here again, out of the answers that are already held.
    /// </summary>
    /// <remarks>
    /// A reading that a plugin worked out is drawn when it lands, and drawing it is all that is owed: nothing on disk
    /// changed, so nothing is read again — not the directory, and not what a hide rule makes of it.
    /// <para>
    /// A place is the exception, and it is the reason this is not one line: what a location holds of a place is the
    /// answer read when the address was entered, so a reading that landed is not in it yet. The address is asked again
    /// — which answers from what the provider has just read and starts nothing — and the listing that follows is the
    /// one the landing was waiting for. Without it a place whose first read is still going would stay empty for as
    /// long as it was open.
    /// </para>
    /// <para>
    /// For a directory the listing is staged as a new list rather than repainted: a column's value is read as its row
    /// is built, so a repaint would leave the value that was there before the reading landed.
    /// </para>
    /// </remarks>
    public void Redraw()
    {
        if (_logical is not null)
        {
            Refresh();

            return;
        }

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
    /// <para>
    /// A place is not staged from a directory, so there is nothing here for a shared answer to make again: what
    /// a plugin's listing leaves out is the plugin's own answer, and neither the base nor a hiding has a word in it.
    /// </para>
    /// </remarks>
    private void Restage()
    {
        if (_logical is not null)
        {
            return;
        }

        _shown = Stage(_current, _entries);
        Changed?.Invoke();
    }

    /// <summary>
    /// Says that the files may have changed, which every location reads its directory again for.
    /// </summary>
    /// <remarks>
    /// Said through a location because a location is what a view holds, and reaching every location rather than
    /// this one because the directory a change was made in is not always the directory the change was made
    /// <em>from</em>: a move is answered by the dock it was dropped on, and the dock the entries were dragged
    /// out of is showing where they were. Whoever calls this defers it.
    /// </remarks>
    public void Touch() => _shared.Touch();

    /// <summary>
    /// Lets go of the shared answers.
    /// </summary>
    /// <remarks>
    /// A location made for a dock that was taken out of step is thrown away when that dock goes back in, and
    /// what it holds of the plugin's has to be let go of with it: a subscription left behind would keep an
    /// abandoned location reading directories nobody looks at, once per toggle.
    /// </remarks>
    public void Dispose()
    {
        _shared.Changed -= Restage;
        _shared.Touched -= Refresh;
        _shared.Redrawn -= Redraw;
    }

    /// <summary>Moves without touching the history, for back, forward and up.</summary>
    /// <param name="directory">The directory or the address to look at.</param>
    private void Move(string directory)
    {
        // A history holds what was written rather than what a place was read as, so a step back into an address
        // is the address asked for again — which is the same read the address itself would have made.
        if (LogicalAddress.TryRead(directory, out _))
        {
            Enter(directory);

            return;
        }

        _logical = null;

        // Held in full rather than as it was given: what is typed may be relative or have a step in
        // it, and the address is what says where the browser actually is. The computer is not a path
        // and is held as it is.
        _current = IsComputer(directory) ? directory : Path.GetFullPath(directory);
        _entries = Read(_current);
        _shown = Stage(_current, _entries);
        Changed?.Invoke();
    }

    /// <summary>
    /// Goes to a place a plugin answers for, reading everything about it in one go.
    /// </summary>
    /// <remarks>
    /// The address is kept as it was written and the place is read afresh, so that the title, the entries and
    /// whatever answers a drag are one answer about one address — and an address nobody answers is kept the same
    /// way, since saying so is the dock's business rather than this one's.
    /// </remarks>
    /// <param name="written">The address, as it was written.</param>
    private void Enter(string written)
    {
        _logical = Logical.Find(written, _views);
        _entries = [];
        _shown = [];
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
    /// <para>
    /// The hidden ones come out here rather than never being read, because a listing that was read without
    /// them would have to be read again the moment the switch that shows them was turned. What is hidden is
    /// what a provider in force says, asked each time the listing is staged rather than carried on the entry.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory the entries were read from.</param>
    /// <param name="entries">What it holds.</param>
    private IReadOnlyList<Entry> Stage(string directory, IReadOnlyList<Entry> entries)
    {
        // A rule that would run a program is not run here: the window's thread is where a listing is staged, and a
        // program behind it is the window that stops. Until every rule can answer, everything is shown — a listing
        // missing a column for a moment is better than one that stops the window — and the rules are read on a thread
        // of their own, which draws the listing again when they are in.
        var rules = !_shared.ShowHidden && HideRegistry.Ready(directory);

        if (!_shared.ShowHidden && !rules)
        {
            Ask(directory);
        }

        var shown = rules
            ? entries.Where(entry => !Hides(entry)).ToArray()
            : [.. entries];

        // Copied rather than handed on as it came, so that a staging with nothing switched is still a list
        // another one is not: what a dock holds to tell a staged listing from one it has not staged again is
        // the list itself, and the switch that shows what is hidden leaves the entries exactly as they are.
        var staged = shown.ToArray();

        return ParentOf(directory) is null || Same(directory, _shared.BaseDir)
            ? staged
            : [new Entry(UpName, EntryKind.Directory), .. staged];
    }

    /// <summary>
    /// Has the hide rules of a directory read off the window's thread, and the listing drawn again when they are in.
    /// </summary>
    /// <remarks>
    /// Asked once per directory while the read is going, and the answer is what <see cref="HideRegistry.Ready"/> then
    /// reports: the listing that follows filters by it without asking again, so a turn is not a program run.
    /// </remarks>
    /// <param name="directory">The directory whose rules are read.</param>
    private void Ask(string directory)
    {
        if (!_asking.Add(directory) || !_asked.Add(directory))
        {
            return;
        }

        _ = Task.Run(() =>
        {
            HideRegistry.Prepare(directory);

            // Asked after the reading rather than assumed: a provider whose program is not there answers no, and the
            // listing shows everything rather than being staged again for a reading that will never come.
            var read = HideRegistry.Ready(directory);

            Dispatcher.UIThread.Post(() =>
            {
                _asking.Remove(directory);

                if (read)
                {
                    // A staging and not a read: nothing on disk changed, and what a rule makes of it is all that was
                    // missing.
                    _shared.Hidden();

                    return;
                }

                _log.Warn($"what hides the entries of `{directory}` could not be read, so they are all shown");
            });
        });
    }

    /// <summary>
    /// Whether an item is a file or a directory.
    /// </summary>
    /// <remarks>
    /// Asked of the platform rather than taken from what a listing read, because a path typed into the
    /// address is not an entry of any listing yet, and a hidden provider that rules on files apart from
    /// directories has to be told which one it is looking at.
    /// </remarks>
    /// <param name="path">The item.</param>
    /// <returns>What it is.</returns>
    public static EntryKind KindOf(string path) =>
        System.IO.Directory.Exists(path) ? EntryKind.Directory : EntryKind.File;

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

    /// <summary>Whether two locations name the same place.</summary>
    /// <remarks>
    /// Compared in full, so that <c>..</c> and a trailing separator are the same place rather than
    /// a second step in the history. A place that is not a path is compared as itself.
    /// <para>
    /// An address is compared as the words it was written in and never as a path: <c>Path.GetFullPath</c> would
    /// read one against the working directory and hand back something that is neither, and an address and a path
    /// are two kinds of place that cannot be the same one.
    /// </para>
    /// </remarks>
    private static bool Same(string left, string right)
    {
        var leftAddress = LogicalAddress.TryRead(left, out _);
        var rightAddress = LogicalAddress.TryRead(right, out _);

        if (leftAddress || rightAddress)
        {
            return leftAddress
                && rightAddress
                && string.Equals(left, right, StringComparison.Ordinal);
        }

        return IsComputer(left) || IsComputer(right)
            ? string.Equals(left, right, StringComparison.Ordinal)
            : string.Equals(
                Path.GetFullPath(left),
                Path.GetFullPath(right),
                StringComparison.Ordinal
            );
    }

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
                entries.Add(new Entry(path, KindOf(path)));
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

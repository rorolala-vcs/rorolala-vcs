using System.Security.Cryptography;
using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>What the last fetch came to, which the Overview's status bar draws.</summary>
internal enum FetchState
{
    /// <summary>No fetch has been made since the program started.</summary>
    Never,

    /// <summary>A fetch is going.</summary>
    Fetching,

    /// <summary>The last fetch brought the Vault's Layout here.</summary>
    Ready,

    /// <summary>The last fetch could not bring it.</summary>
    Failed,
}

/// <summary>What a fetch said about the Vault being reachable.</summary>
/// <remarks>
/// It is told apart from <see cref="FetchState"/> because a fetch can fail for a reason that is not the leg to
/// the Vault: an account that is not here, or a Workspace that names no Vault, says nothing about whether one
/// could be reached. Only the transport's own status is taken as evidence.
/// </remarks>
internal enum Reachability
{
    /// <summary>What failed was not the leg to the Vault, so this fetch says nothing about it.</summary>
    Unknown,

    /// <summary>The Vault answered.</summary>
    Reached,

    /// <summary>The Vault could not be reached.</summary>
    Unreachable,
}

/// <summary>
/// One fetch of the Vault's Layout at a time, shared by the views that read the work.
/// </summary>
/// <remarks>
/// The copy every reading is made from is one file, so two views asking at once would be two fetches for
/// a thing one of them is about to write. A request made while a fetch is going is run when it ends, it
/// asked for what that fetch is bringing, rather than started beside it.
/// <para>
/// What the fetch wrote is what every reading of the work is made from, so the files are said to have
/// changed when it ends: the columns of a listing, the Overview and the Version Graph all read the copy
/// this run brought over. A fetch is never begun by a view that merely reads the work. The one automatic
/// fetch is the loop this class starts: once a minute, without a command card and without a failure in
/// the window, it brings the copy over to keep the reading current. A foreground change that used to fetch
/// marks the copy dirty instead, and the loop's next success is what reads the views again.
/// </para>
/// </remarks>
internal sealed class VaultFetch
{
    /// <summary>How long the loop waits between fetches.</summary>
    private static readonly TimeSpan Interval = TimeSpan.FromMinutes(1);

    /// <summary>The host, for the loop's directory, for the files having changed, and for a failure's report.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the run is drawn by, beside every other command this plugin runs.</summary>
    private readonly IProgressHub _hub;

    /// <summary>Where the work the loop fetches is rooted, which is the base the views are read against.</summary>
    private readonly Func<string?> _directory;

    /// <summary>What asked to be read once the fetch is over, in the order they asked.</summary>
    private readonly List<Action> _after = [];

    /// <summary>What a foreground change asked to run once the next fetch has brought the copy over.</summary>
    private readonly List<Action> _dirtyDone = [];

    /// <summary>Whether a fetch is going, which is what a second request waits for.</summary>
    private bool _fetching;

    /// <summary>Whether a foreground change is waiting for the loop's next fetch.</summary>
    private bool _dirty;

    /// <summary>The loop, made when the plugin starts and run for as long as it does.</summary>
    private DispatcherTimer? _loop;

    /// <summary>What the last fetch came to.</summary>
    private FetchState _state = FetchState.Never;

    /// <summary>Whether the last fetch that said anything about the leg to the Vault found it unreachable.</summary>
    /// <remarks>
    /// Reads come from the window's thread and from the workers that run a command, so it is written and read
    /// whole: a stale answer costs one command that could have gone on from what is here, or one that waits on a
    /// Vault that is not there, and both are the answer the state had a moment ago.
    /// </remarks>
    private volatile bool _offline;

    /// <summary>The last fetch's verdict on whether the Vault can be reached, or nothing when it gave none.</summary>
    /// <param name="done">Whether the fetch ended well.</param>
    /// <param name="code">The status it ended with.</param>
    /// <returns>What the fetch said about the Vault.</returns>
    internal static Reachability Reach(bool done, int code) =>
        done ? Reachability.Reached
        : code == global::RorolalaVCSPlugin.Fetch.Transport ? Reachability.Unreachable
        : Reachability.Unknown;

    /// <summary>Makes the fetcher.</summary>
    /// <param name="host">The host, for the run's card and for what a failure is said with.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="directory">
    /// Where the loop fetches from, or nothing to read the base the browser is rooted at. The parameter is what the
    /// checks that make the fetcher without a browser use.
    /// </param>
    public VaultFetch(IPluginHost host, IProgressHub hub, Func<string?>? directory = null)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);

        _host = host;
        _hub = hub;
        _directory = directory ?? new Func<string?>(() => BaseDirectory.Current);
    }

    /// <summary>What the last fetch came to, which the status bar draws.</summary>
    public FetchState State => _state;

    /// <summary>Whether the Vault could not be reached at the last check.</summary>
    /// <remarks>
    /// It is what the writes that can go on from what is here are opened offline with, and what the ones that
    /// need the Vault are refused with: a run that waits on a leg already known to fail spends the reader's
    /// time to learn what the Overview's lamp already says. A fetch that reaches the Vault again clears it.
    /// </remarks>
    public bool Offline => _offline;

    /// <summary>Whether a foreground change is waiting for the loop's next fetch.</summary>
    public bool Dirty => _dirty;

    /// <summary>Raised on the window's thread when <see cref="State"/> or <see cref="Dirty"/> changes.</summary>
    public event Action? Changed;

    /// <summary>Raised on the window's thread when <see cref="Offline"/> changes.</summary>
    /// <remarks>
    /// It is kept apart from <see cref="Changed"/>, which fires for a fetch merely going too: a reader of this
    /// one asks again only where the answer a write is made with has moved.
    /// </remarks>
    public event Action? ReachabilityChanged;

    /// <summary>
    /// Starts the loop that keeps the Vault's copy current.
    /// </summary>
    /// <remarks>
    /// Started once by the plugin: a window may open and close, while the work the loop reads is the one the
    /// plugin was started for. The first fetch is a minute away, which is the interval the status bar says is not
    /// fetching yet.
    /// </remarks>
    public void Start()
    {
        // A host under test has no Vault to keep current, and a loop that reached for one would be a
        // network exchange between tests. The suite states this rather than the plugin guessing from
        // the platform.
        if (Environment.GetEnvironmentVariable("ROLA_DESKTOP_TEST") is { Length: > 0 })
        {
            return;
        }

        if (!Dispatcher.UIThread.CheckAccess())
        {
            Dispatcher.UIThread.Post(Start);

            return;
        }

        if (_loop is not null)
        {
            return;
        }

        _loop = MakeLoop();
        _loop.Start();
    }

    /// <summary>Makes the timer that asks the Vault once a minute.</summary>
    /// <returns>The timer, not started.</returns>
    private DispatcherTimer MakeLoop()
    {
        var loop = new DispatcherTimer { Interval = Interval };
        loop.Tick += (_, _) => Tick();

        return loop;
    }

    /// <summary>
    /// Says that a foreground change left the copy behind, and asks the next fetch to read the views again.
    /// </summary>
    /// <remarks>
    /// Nothing is fetched here. The change happened on the window's thread, and a fetch made from that moment would
    /// be a fetch per menu entry or per sync rather than the one the loop makes. A callback is kept with the mark
    /// because what a change reads, such as a plan or a graph, is ready only after the copy has been brought over.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, or nothing when it is not known.</param>
    /// <param name="done">What to run once the next fetch has brought the copy over.</param>
    public void MarkDirty(string? directory, Action? done = null)
    {
        _dirty = true;

        if (done is not null)
        {
            _dirtyDone.Add(done);
        }

        Changed?.Invoke();
    }

    /// <summary>
    /// Fetches the Vault's Layout under <paramref name="directory"/>, and reads what was asked for once
    /// it is over.
    /// </summary>
    /// <remarks>
    /// A window opened on its own has no program to fetch with, so what there is to read is read: a
    /// reader who asked to see the work is not answered with nothing because of how the window started.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="done">What to run on the window's thread once the copy is here.</param>
    /// <param name="silent">Whether a failure is left to the status bar rather than shown in the window.</param>
    /// <param name="quiet">Whether the run is left out of the command stack, which the loop's fetch is.</param>
    public void Fetch(string directory, Action done, bool silent = false, bool quiet = false)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(done);

        if (_fetching)
        {
            _after.Add(done);

            return;
        }

        // The Vault was unreachable at the last check, so a second leg to it now would only wait on the
        // failure already seen. What the copy holds is read instead: the caller's continuation runs, and the
        // window says why nothing was brought over. The loop's own fetch is not held back, since it is what
        // finds the Vault again.
        if (_offline && !quiet)
        {
            // Both the continuation and the notice belong to the window's thread, as they do when a fetch
            // completes, since a caller is free to have asked from a worker.
            Dispatcher.UIThread.Post(() =>
            {
                done();

                if (!silent)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.offline_title"),
                        RolaI18N.Get("rorolala_vcs.offline")
                    );
                }
            });

            return;
        }

        if (Environment.GetEnvironmentVariable(Sync.Program) is not { Length: > 0 } program)
        {
            done();

            return;
        }

        _fetching = true;
        SetState(FetchState.Fetching);

        var run = quiet ? (IProgressRun?)null : _hub.Begin("rorolala_vcs.progress.fetch");

        _ = Task.Run(() =>
        {
            // What the copy holds before the fetch, so that a run which brings the copy already here is
            // told from one that changed it. The loop's fetch is the quiet one, and its no-change is what
            // keeps a minute from redrawing the same rows.
            var before = quiet ? Fingerprint(directory) : null;

            // The static runner is named in full: this class's own Fetch would be the one looked for.
            var (done_well, said, code) = global::RorolalaVCSPlugin.Fetch.Run(program, directory);
            var changed =
                !quiet || !string.Equals(before, Fingerprint(directory), StringComparison.Ordinal);

            Dispatcher.UIThread.Post(() =>
            {
                _fetching = false;

                // The card is let go of here rather than when the run is over, because the panel keeps it for a
                // few seconds after: what ended is drawn as ended before it is taken away.
                if (run is not null)
                {
                    _hub.End(run);
                }

                if (done_well)
                {
                    Reached(Reachability.Reached);

                    // What was fetched is what every reading is made from, so what the columns answered about the
                    // entries is let go of with the files, and the listing and the reading are read again. A quiet
                    // fetch that brought the copy already here has nothing a view would read, and a minute that
                    // redrew the same rows would be the user's time spent on no change. What a foreground change
                    // marked still reads, because the change is this side's.
                    var dirty = _dirty;

                    if (!quiet || changed || dirty)
                    {
                        _host.Files.Touch();
                    }

                    _dirty = false;
                    SetState(FetchState.Ready);
                    done();

                    var waiting = _dirtyDone.ToArray();

                    _dirtyDone.Clear();

                    foreach (var read in waiting)
                    {
                        read();
                    }
                }
                else
                {
                    SetState(FetchState.Failed);
                    Reached(Reach(done_well, code));

                    if (!silent)
                    {
                        // Drawn as every other command's failure is, and not as one line of raw text: what the
                        // command wrote says which of its lines went wrong and which can be acted on, and the
                        // notification draws those marks rather than the words they are written with.
                        FailureNotice.Show(
                            _host.Dialogs,
                            _host.Icons,
                            RolaI18N.Get("rorolala_vcs.fetch.failed"),
                            said
                        );
                    }
                }

                // A request made while this fetch was going asked for the copy it brought, so each one reads
                // now rather than fetching the same layout again. The list is taken before it is run, since a
                // callback is free to ask for a fetch of its own.
                var after = _after.ToArray();

                _after.Clear();

                foreach (var read in after)
                {
                    read();
                }
            });
        });
    }

    /// <summary>Fetches once when the loop's minute has passed.</summary>
    private void Tick()
    {
        if (_fetching)
        {
            return;
        }

        if (_directory() is not { Length: > 0 } at || !WorkspaceNotice.Holds(at, _host.Log))
        {
            return;
        }

        Fetch(at, () => { }, silent: true, quiet: true);
    }

    /// <summary>
    /// What a fetch would have to change to leave a view with something new to read.
    /// </summary>
    /// <remarks>
    /// A Layout's copy is kept in generations and named by a pointer, and a fetch that finds the Vault's
    /// Layout already here writes nothing at all: the pointer is the whole of what says the copy became
    /// another one. The rules are swapped whole on every fetch even when they say the same, so their bytes
    /// rather than their times are what says whether they changed. A fingerprint that cannot be read is
    /// answered as nothing, which the caller treats as a change: a view read once more costs less than one
    /// left stale.
    /// </remarks>
    /// <param name="directory">Where the work is rooted.</param>
    /// <returns>One line per file, or nothing when they cannot be read.</returns>
    internal static string? Fingerprint(string directory)
    {
        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            var root = workspace.Root.FullName;

            return Fingerprint(
                RolaPaths.WorkspaceReadonlyLayouts(root),
                RolaPaths.WorkspaceIgnoreRules(root)
            );
        }
        catch (Exception error)
            when (error is RolaReadError or IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>The fingerprint of the copies a Workspace keeps and the rules it fetched.</summary>
    /// <param name="layouts">Where the fetched Layout copies are kept.</param>
    /// <param name="rules">Where the fetched ignore rules are kept.</param>
    /// <returns>One line per file, in the order their paths sort.</returns>
    internal static string Fingerprint(string layouts, string rules)
    {
        var lines = new List<string>();

        Adding(lines, layouts, "CURRENT", hash: false);
        Adding(lines, rules, "*", hash: true);

        lines.Sort(StringComparer.Ordinal);

        return string.Join('\n', lines);

        static void Adding(List<string> lines, string folder, string pattern, bool hash)
        {
            if (!Directory.Exists(folder))
            {
                return;
            }

            foreach (var file in Directory.EnumerateFiles(folder, pattern, SearchOption.AllDirectories))
            {
                var path = Path.GetRelativePath(folder, file);
                var value = hash
                    ? Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(file)))
                    : File.ReadAllText(file).Trim();

                lines.Add($"{path}={value}");
            }
        }
    }

    /// <summary>Says what the last fetch came to, when that is something else than it was.</summary>
    /// <param name="state">The state.</param>
    private void SetState(FetchState state)
    {
        if (_state == state)
        {
            return;
        }

        _state = state;
        Changed?.Invoke();
    }

    /// <summary>Takes the leg to the Vault as the last fetch found it, when it said anything about it.</summary>
    /// <param name="reachability">What the fetch said.</param>
    private void Reached(Reachability reachability)
    {
        var offline = reachability switch
        {
            Reachability.Reached => false,
            Reachability.Unreachable => true,
            _ => _offline,
        };

        if (offline == _offline)
        {
            return;
        }

        _offline = offline;
        ReachabilityChanged?.Invoke();
    }
}

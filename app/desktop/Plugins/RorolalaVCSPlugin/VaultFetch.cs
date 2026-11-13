using Avalonia.Threading;
using FileSystemPlugin;
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

    /// <summary>The loop, made when the plugin starts rather than when a view asks for a fetch.</summary>
    private DispatcherTimer? _loop;

    /// <summary>What the last fetch came to.</summary>
    private FetchState _state = FetchState.Never;

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

    /// <summary>Whether a foreground change is waiting for the loop's next fetch.</summary>
    public bool Dirty => _dirty;

    /// <summary>Raised on the window's thread when <see cref="State"/> or <see cref="Dirty"/> changes.</summary>
    public event Action? Changed;

    /// <summary>
    /// Starts the loop that keeps the Vault's copy current.
    /// </summary>
    /// <remarks>
    /// Started once by the plugin rather than by a view: a window may open and close, while the work the loop reads
    /// is the one the plugin was started for. The first fetch is a minute away, which is the interval the status bar
    /// says is not fetching yet.
    /// </remarks>
    public void Start()
    {
        if (!Dispatcher.UIThread.CheckAccess())
        {
            Dispatcher.UIThread.Post(Start);

            return;
        }

        if (_loop is not null)
        {
            return;
        }

        _loop = new DispatcherTimer { Interval = Interval };
        _loop.Tick += (_, _) => Tick();
        _loop.Start();
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
            // The static runner is named in full: this class's own Fetch would be the one looked for.
            var (done_well, said) = global::RorolalaVCSPlugin.Fetch.Run(program, directory);

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
                    // What was fetched is what every reading is made from, so what the columns answered about the
                    // entries is let go of with the files, and the listing and the reading are read again.
                    _host.Files.Touch();
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
}

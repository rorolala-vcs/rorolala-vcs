using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The sync's own state: what the buttons are drawn from, and what a run of theirs is begun as.
/// </summary>
/// <remarks>
/// One of these belongs to a dock's view, and the strip in the dock's header reads it: whether there is work to
/// carry, whether a run is going, and why not when there is not. What the run says while it goes is not kept here
/// but written to the <see cref="ProgressHub"/>, which is what the panel down the Overview's right draws from:
/// the run's card stands beside the cards of every other command this plugin may be running.
/// <para>
/// The plan is asked for off the window's thread, because asking costs a fetch of the Vault's Layout: a dock
/// that asked on its own thread would freeze while the network answered. Every change is said on the window's
/// thread, since what reads it draws with it.
/// </para>
/// </remarks>
internal sealed class SyncWatcher
{
    /// <summary>The i18n key of what a sync run is called in the panel.</summary>
    private const string TitleKey = "rorolala_vcs.progress.sync";

    /// <summary>The host, for the question a run is asked before it starts and for the files having changed.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a plan that could not be read is said, rather than in the window.</summary>
    private readonly ILog _log;

    /// <summary>What every run this plugin makes is gathered in, which the sync's own run joins.</summary>
    private readonly ProgressHub _hub;

    /// <summary>What brings the Vault's Layout over once a run has carried work.</summary>
    private readonly VaultFetch _fetch;

    /// <summary>Makes the watcher.</summary>
    /// <param name="host">The host.</param>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="hub">What every run this plugin makes is gathered in.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout, which a carried run is followed by.</param>
    public SyncWatcher(IPluginHost host, ILog log, ProgressHub hub, VaultFetch fetch)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(fetch);

        _host = host;
        _log = log;
        _hub = hub;
        _fetch = fetch;
    }

    /// <summary>Whether there is work to carry, which is what the three buttons are pressable for.</summary>
    /// <remarks>
    /// It is the last plan's answer and nothing more: false says the plan found nothing to move, and says
    /// nothing about whether a plan could be made at all — that is <see cref="Why"/>, which is stated where
    /// the command or its answer could not be read.
    /// </remarks>
    public bool Ready { get; private set; }

    /// <summary>Why the plan could not be made or read, or nothing when it could.</summary>
    public string? Why { get; private set; }

    /// <summary>Whether a run is going, which the buttons are drawn by.</summary>
    public bool Running { get; private set; }

    /// <summary>Whether a reading of the work itself says there is something to send or bring down.</summary>
    /// <remarks>
    /// It is the other hand of <see cref="Ready"/>: the plan is the command line's answer about everything a
    /// run may move — including a local entry the Vault does not name — while a reading of the work names only
    /// where the Layout's versions stand. A reading that says one side is ahead of, behind, or apart from the
    /// other is work whatever a plan could or could not say about it, so the buttons are not left unpressable
    /// while the table beside them says there is something to carry.
    /// </remarks>
    public bool Observed { get; private set; }

    /// <summary>Whether a sync may be begun at all.</summary>
    /// <remarks>
    /// A plan that could not be made is not a plan that found nothing: the command may still have work to
    /// carry, and a button that refused to ask it would hide the only thing the reader can be told — why the
    /// plan was not made. So an answer that is not known is drawn pressable and says its reason on hover;
    /// only a run's going, or both a plan and a reading that found nothing, leaves the buttons unpressable.
    /// </remarks>
    public bool Pressable => !Running && (Ready || Observed || Why is { Length: > 0 });

    /// <summary>Says what a reading of the work found, which is whether there is something to move.</summary>
    /// <param name="work">Whether the reading names a path that is ahead of, behind, or apart from the Vault.</param>
    public void Observe(bool work)
    {
        if (Observed == work)
        {
            return;
        }

        Observed = work;
        Changed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>What is told to draw again when any of this changes.</summary>
    public event EventHandler? Changed;

    /// <summary>Whether a plan is being asked for, so a burst of asks is one run of the command.</summary>
    private bool _planning;

    /// <summary>Whether a plan was asked for while one was being asked, so the newest answer is asked after it.</summary>
    private bool _planAgain;

    /// <summary>
    /// Where the newest ask is rooted, which is the plan asked after the one in flight and the one a run's
    /// end makes.
    /// </summary>
    private string? _planFor;

    /// <summary>
    /// Whether any ask has been made, so that a run ending before the first one falls back to its own
    /// directory while an ask for no directory at all is left as that.
    /// </summary>
    private bool _asked;

    /// <summary>
    /// Asks what a sync would carry, which decides whether the buttons may be pressed.
    /// </summary>
    /// <remarks>
    /// Asked rather than remembered, and asked again after a run and after what is carried is changed: what
    /// there is to carry is the Vault's answer rather than this side's, and a button left pressable after the
    /// work was carried would be a button that offered a run with nothing to do.
    /// <para>
    /// One plan is in flight at a time: asking costs a fetch of the Vault's Layout, and the answer that matters
    /// is the one about the work as it is now. An ask made while one is going is remembered and made once after
    /// it, rather than run beside it and racing it to say what the answer is.
    /// </para>
    /// <para>
    /// A run's going leaves what the last plan said standing: the buttons are already unpressable while one is
    /// going, and the run's own end asks again. Clearing it here would make a run that has just ended leave the
    /// buttons grey until another plan has been made over the network, which is the moment a reader is most
    /// likely to press one.
    /// </para>
    /// </remarks>
    /// <param name="directory">Where the work is rooted, or nothing where the tree is not rooted anywhere.</param>
    public void Plan(string? directory)
    {
        // A plugin may say that the files changed from the thread the command ran on, and this is
        // reached from that saying: what is kept here is what the window's thread draws from, so it
        // is touched there alone.
        if (!Dispatcher.UIThread.CheckAccess())
        {
            Dispatcher.UIThread.Post(() => Plan(directory));

            return;
        }

        // The newest ask, whatever state this is in: a change made while a run is going is asked about
        // when the run ends rather than dropped, and a plan already in flight is answered by asking
        // this one after it.
        _planFor = directory;
        _asked = true;

        if (directory is not { Length: > 0 } at)
        {
            Settled(false, null);
            return;
        }

        if (Running)
        {
            return;
        }

        if (_planning)
        {
            _planAgain = true;

            return;
        }

        _planning = true;

        _ = Task.Run(() =>
        {
            SyncPlan plan;

            try
            {
                plan = Sync.Plan(at);
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Warn($"what a sync would carry could not be read: {error.Message}");
                plan = new SyncPlan(false, null);
            }

            // A plan that could not be made is said in the log as well as on the buttons: the strip answers
            // with the reason on hover, and this is where a reader who has gone looking for it reads the
            // whole of what the command said.
            if (plan.Said is { Length: > 0 } said)
            {
                _log.Warn($"what a sync would carry could not be read: {said}");
            }

            On(() =>
            {
                _planning = false;

                // An ask that arrived while this was going is about work newer than this answer: drawing
                // this one would say what the work held before that change, and leave the buttons as it
                // said until the newer plan answers.
                if (_planAgain)
                {
                    _planAgain = false;
                    Plan(_planFor);

                    return;
                }

                Settled(plan.Ready, plan.Said);
            });
        });
    }

    /// <summary>
    /// Begins a sync, asking first about what it leaves out.
    /// </summary>
    /// <remarks>
    /// The question is put here rather than by the button, because what makes it necessary is the scope and not
    /// the button: which of the three parts is left out is what the warning is about, and a run that carries
    /// everything is begun with nothing said. Whether history may be changed is settled by the key being held
    /// when the button was pressed rather than when the question is answered, since the answer is given with the
    /// pointer rather than the keyboard.
    /// </remarks>
    /// <param name="directory">Where the work is rooted.</param>
    /// <param name="way">Which way work is to be carried.</param>
    /// <param name="force">Whether a chain that has moved apart is to be changed rather than refused.</param>
    public void Start(string directory, SyncWay way, bool force)
    {
        ArgumentNullException.ThrowIfNull(directory);

        if (Running)
        {
            return;
        }

        var scope = SyncSettings.Chosen();

        if (SyncWarnings.Key(scope, way) is { } warning)
        {
            _host.Dialogs.Show(
                new Dialog(
                    RolaI18N.Get("rorolala_vcs.sync.confirm_title"),
                    RolaI18N.Get(warning),
                    () => Begin(directory, way, scope, force)
                )
            );

            return;
        }

        Begin(directory, way, scope, force);
    }

    /// <summary>Runs it, saying on the window's thread whatever the run says of itself.</summary>
    /// <param name="directory">Where the work is rooted.</param>
    /// <param name="way">Which way work is carried.</param>
    /// <param name="scope">What is carried.</param>
    /// <param name="force">Whether history may be changed.</param>
    private void Begin(string directory, SyncWay way, SyncScope scope, bool force)
    {
        Running = true;
        Changed?.Invoke(this, EventArgs.Empty);

        // The run's card is opened before the work is handed off, so that the panel has a card to draw for it the
        // moment the run is said to be going — see the hub for why the runs are gathered there rather than here.
        var run = _hub.Begin(TitleKey);

        _ = Task.Run(() =>
        {
            var (done, said) = Sync.Run(
                directory,
                way,
                scope,
                force,
                // What the run says is read into the run's own records, and every record says so on the window's
                // thread: the run is on another one, and a control touched from there is a control touched by two
                // threads at once.
                run.Records,
                _hub.Touch
            );

            On(() =>
            {
                Running = false;

                // The card is let go of here rather than when the run is over, because the panel keeps it for a
                // few seconds after: what ended is drawn as ended before it is taken away.
                _hub.End(run);

                if (!done)
                {
                    _host.Dialogs.Report(
                        new Report(RolaI18N.Get("rorolala_vcs.sync.failed"), said)
                    );
                }

                // What the run carried is what the tree now holds, and what is left to carry is the Vault's
                // answer again: both are read rather than guessed at. A run that ended well is followed by
                // one explicit fetch, so what the readings are made from is the Layout as the Vault holds it
                // after the run, rather than the copy the run had when it planned. A run that failed fetched
                // nothing new, and is answered by the plan it would have made anyway.
                _host.Files.Touch();

                // The newest ask is the one about where the work is now: a base moved while the run was
                // going is one whose plan was not made at the time. The run's own directory answers when
                // nothing has been asked yet.
                var next = _asked ? _planFor : directory;

                if (done)
                {
                    _fetch.Fetch(directory, () => Plan(next));
                }
                else
                {
                    Plan(next);
                }
            });
        });
    }

    /// <summary>Says what the plan came to, on the window's thread.</summary>
    /// <param name="ready">Whether there is work to carry.</param>
    /// <param name="why">Why there is none.</param>
    private void Settled(bool ready, string? why)
    {
        Ready = ready;
        Why = why;
        Changed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Runs something on the window's thread, wherever it was called from.</summary>
    /// <param name="said">What to do.</param>
    private static void On(Action said) => Dispatcher.UIThread.Post(said);
}

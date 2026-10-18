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

    /// <summary>Makes the watcher.</summary>
    /// <param name="host">The host.</param>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="hub">What every run this plugin makes is gathered in.</param>
    public SyncWatcher(IPluginHost host, ILog log, ProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(hub);

        _host = host;
        _log = log;
        _hub = hub;
    }

    /// <summary>Whether there is work to carry, which is what the three buttons are pressable for.</summary>
    public bool Ready { get; private set; }

    /// <summary>Why there is none, which is what the strip says when the pointer rests on it.</summary>
    public string? Why { get; private set; }

    /// <summary>Whether a run is going, which the buttons are drawn by.</summary>
    public bool Running { get; private set; }

    /// <summary>Whether a sync may be begun at all.</summary>
    public bool Pressable => Ready && !Running;

    /// <summary>What is told to draw again when any of this changes.</summary>
    public event EventHandler? Changed;

    /// <summary>Whether a plan is being asked for, so a burst of asks is one run of the command.</summary>
    private bool _planning;

    /// <summary>Whether a plan was asked for while one was being asked, so the newest answer is asked after it.</summary>
    private bool _planAgain;

    /// <summary>Where the plan that will be asked for after the one in flight is rooted.</summary>
    private string? _planFor;

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
    /// </remarks>
    /// <param name="directory">Where the work is rooted, or nothing where the tree is not rooted anywhere.</param>
    public void Plan(string? directory)
    {
        if (directory is not { Length: > 0 } at || Running)
        {
            Settled(false, null);
            return;
        }

        if (_planning)
        {
            _planAgain = true;
            _planFor = at;

            return;
        }

        _planning = true;
        _planFor = at;

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

            On(() =>
            {
                _planning = false;
                Settled(plan.Ready, plan.Said);

                if (_planAgain)
                {
                    _planAgain = false;
                    Plan(_planFor);
                }
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
                // answer again: both are read rather than guessed at.
                _host.Files.Touch();
                Plan(directory);
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

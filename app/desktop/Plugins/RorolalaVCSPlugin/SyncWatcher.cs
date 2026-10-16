using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What the sync buttons and the bar under them are drawn from.
/// </summary>
/// <remarks>
/// One of these belongs to a dock's view, and both the strip in the dock's header and the panel down the right
/// of the view read it: whether there is work to carry, whether a run is going, and what that run has said of
/// itself. The two are two drawings of one answer, so neither keeps any of it.
/// <para>
/// The plan is asked for off the window's thread, because asking costs a fetch of the Vault's Layout: a dock
/// that asked on its own thread would freeze while the network answered. Every change is said on the window's
/// thread, since what reads it draws with it.
/// </para>
/// </remarks>
internal sealed class SyncWatcher
{
    /// <summary>The host, for the question a run is asked before it starts and for the files having changed.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a plan that could not be read is said, rather than in the window.</summary>
    private readonly ILog _log;

    /// <summary>Makes the watcher.</summary>
    /// <param name="host">The host.</param>
    /// <param name="log">Where a reading that could not be made is said.</param>
    public SyncWatcher(IPluginHost host, ILog log)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);

        _host = host;
        _log = log;
    }

    /// <summary>Whether there is work to carry, which is what the three buttons are pressable for.</summary>
    public bool Ready { get; private set; }

    /// <summary>Why there is none, which is what the strip says when the pointer rests on it.</summary>
    public string? Why { get; private set; }

    /// <summary>Whether a run is going, which the buttons and the bar are drawn by.</summary>
    public bool Running { get; private set; }

    /// <summary>What the run going now has said about itself.</summary>
    public SyncProgress Progress { get; } = new();

    /// <summary>Whether a sync may be begun at all.</summary>
    public bool Pressable => Ready && !Running;

    /// <summary>What is told to draw again when any of this changes.</summary>
    public event EventHandler? Changed;

    /// <summary>
    /// Asks what a sync would carry, which decides whether the buttons may be pressed.
    /// </summary>
    /// <remarks>
    /// Asked rather than remembered, and asked again after a run and after what is carried is changed: what
    /// there is to carry is the Vault's answer rather than this side's, and a button left pressable after the
    /// work was carried would be a button that offered a run with nothing to do.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, or nothing where the tree is not rooted anywhere.</param>
    public void Plan(string? directory)
    {
        if (directory is not { Length: > 0 } at || Running)
        {
            Settled(false, null);
            return;
        }

        _ = Task.Run(() =>
        {
            try
            {
                var plan = Sync.Plan(at);

                On(() => Settled(plan.Ready, plan.Said));
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Warn($"what a sync would carry could not be read: {error.Message}");

                On(() => Settled(false, null));
            }
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
        Progress.Reset();
        Changed?.Invoke(this, EventArgs.Empty);

        _ = Task.Run(() =>
        {
            var (done, said) = Sync.Run(
                directory,
                way,
                scope,
                force,
                // What the run says is read into the one progress the panel draws from, and every record says so
                // on the window's thread: the run is on another one, and a control touched from there is a control
                // touched by two threads at once.
                Progress,
                () => On(() => Changed?.Invoke(this, EventArgs.Empty))
            );

            On(() =>
            {
                Running = false;

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

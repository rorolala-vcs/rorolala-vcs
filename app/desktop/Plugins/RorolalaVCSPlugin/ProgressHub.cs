using Avalonia.Threading;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// One process: a command this plugin ran, as the Overview's panel reads it.
/// </summary>
/// <remarks>
/// The command line's own words for a task are what the tasks under this carry; what this adds is which command
/// the tasks belong to, since several commands may be going at once and a reader has to know which bar is whose.
/// The title is a key rather than the words, so that it is said in the reader's own language and stays in step
/// with the catalogue like every other line this plugin draws.
/// </remarks>
internal sealed class ProgressRun
{
    /// <summary>Makes a run named by a key, which the panel draws it under.</summary>
    /// <param name="titleKey">The i18n key of what the run is called.</param>
    public ProgressRun(string titleKey) => TitleKey = titleKey;

    /// <summary>The i18n key of what it is called, which is the command's own name for itself.</summary>
    public string TitleKey { get; }

    /// <summary>What it is called, as the reader's language says it.</summary>
    public string Title => RolaI18N.Get(TitleKey);

    /// <summary>What it has said about itself, which the card's bars are drawn from.</summary>
    public ProgressRecords Records { get; } = new();
}

/// <summary>
/// Every progress run this plugin has started, in the order they began.
/// </summary>
/// <remarks>
/// A command is run by whoever reached for it — the Overview's own strip, an entry's menu — and every one of them
/// is shown in the one panel down the Overview's right, so the runs are gathered here rather than kept by the
/// view that draws them: a run begun from a menu has to reach a panel that may not exist yet, and a panel opened
/// after the run began has to see it.
/// <para>
/// A run is taken off a few seconds after its process ends rather than the moment it does: a command that is over
/// in a moment would otherwise flash a card that was gone before it was read, and a reader wants to see that the
/// work was done rather than only that it is no longer being done.
/// </para>
/// <para>
/// The runs are written from whatever thread a command runs on and read from the window's, so the list is written
/// under a lock and whoever draws is told on the window's thread — a control belongs to the thread that made it.
/// </para>
/// </remarks>
internal sealed class ProgressHub
{
    /// <summary>How long a card stays after its process ends, before it is taken away.</summary>
    private static readonly TimeSpan Linger = TimeSpan.FromSeconds(5);

    /// <summary>What the runs are written under, since a process writes while the panel reads.</summary>
    private readonly object _gate = new();

    /// <summary>The runs to draw, in the order they began.</summary>
    private readonly List<ProgressRun> _runs = [];

    /// <summary>What is told to draw again whenever the runs change.</summary>
    public event EventHandler? Changed;

    /// <summary>The runs to draw, a copy taken under the lock.</summary>
    public IReadOnlyList<ProgressRun> Runs
    {
        get
        {
            lock (_gate)
            {
                return _runs.ToArray();
            }
        }
    }

    /// <summary>Whether there is a run to draw at all, which decides whether the panel has anywhere to stand.</summary>
    public bool Any
    {
        get
        {
            lock (_gate)
            {
                return _runs.Count > 0;
            }
        }
    }

    /// <summary>Begins a run, which is a command about to be run.</summary>
    /// <param name="titleKey">The i18n key of what the command is called.</param>
    /// <returns>The run, which is what its records are written to and what ends it.</returns>
    public ProgressRun Begin(string titleKey)
    {
        ArgumentNullException.ThrowIfNull(titleKey);

        var run = new ProgressRun(titleKey);

        lock (_gate)
        {
            _runs.Add(run);
        }

        On(() => Changed?.Invoke(this, EventArgs.Empty));

        return run;
    }

    /// <summary>
    /// Says that a run has said something of itself, so that whoever draws does so again.
    /// </summary>
    /// <remarks>
    /// The records are written straight to the run rather than through here — they carry their own lock — and
    /// this only says that they have changed, which is all a drawing needs to know.
    /// </remarks>
    public void Touch() => On(() => Changed?.Invoke(this, EventArgs.Empty));

    /// <summary>Ends a run, which is a command that is over, and leaves its card for the linger.</summary>
    /// <param name="run">The run that has ended.</param>
    public void End(ProgressRun run)
    {
        ArgumentNullException.ThrowIfNull(run);

        On(() =>
        {
            Changed?.Invoke(this, EventArgs.Empty);

            // The card is taken away on the window's own clock, since the timer belongs to the thread that made
            // it: a run ended on another thread would otherwise make a timer on that thread, which no drawing
            // would ever hear.
            var linger = new DispatcherTimer { Interval = Linger };

            linger.Tick += (_, _) =>
            {
                linger.Stop();

                lock (_gate)
                {
                    _runs.Remove(run);
                }

                Changed?.Invoke(this, EventArgs.Empty);
            };

            linger.Start();
        });
    }

    /// <summary>Runs something on the window's thread, wherever it was called from.</summary>
    /// <param name="said">What to do.</param>
    private static void On(Action said) => Dispatcher.UIThread.Post(said);
}

using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Progress;

/// <summary>
/// Every progress run the program is showing, in the order they began.
/// </summary>
/// <remarks>
/// A run is begun by whatever starts the work, a plugin action or a host batch, and every window of the host
/// draws the same list: a run begun from a menu has to reach a stack that may not exist yet, and a stack opened
/// after the run began has to see it.
/// <para>
/// The runs are written from whatever thread a command runs on and read from the window's, so the list is
/// written under a lock and whoever draws is told on the window's thread, a control belongs to the thread that
/// made it.
/// </para>
/// </remarks>
public sealed class ProgressHub : IProgressHub
{
    /// <summary>What the runs are written under, since a command writes while a window reads.</summary>
    private readonly object _gate = new();

    /// <summary>The runs to draw, in the order they began.</summary>
    private readonly List<ProgressRun> _runs = [];

    /// <inheritdoc />
    public event Action? Changed;

    /// <inheritdoc />
    public IReadOnlyList<IProgressRun> Runs
    {
        get
        {
            lock (_gate)
            {
                return [.. _runs];
            }
        }
    }

    /// <inheritdoc />
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

    /// <inheritdoc />
    public IProgressRun Begin(string titleKey, string? iconKey = null)
    {
        ArgumentNullException.ThrowIfNull(titleKey);

        var run = new ProgressRun(this, titleKey, iconKey);

        lock (_gate)
        {
            _runs.Add(run);
        }

        On(() => Changed?.Invoke());

        return run;
    }

    /// <summary>Says that a run has said something of itself, so that whoever draws does so again.</summary>
    public void Touch() => On(() => Changed?.Invoke());

    /// <summary>Ends a run, which takes its capsule away.</summary>
    /// <param name="run">The run that has ended.</param>
    public void End(IProgressRun run)
    {
        ArgumentNullException.ThrowIfNull(run);

        lock (_gate)
        {
            _runs.RemoveAll(each => ReferenceEquals(each, run));
        }

        On(() => Changed?.Invoke());
    }

    /// <summary>Runs something on the window's thread, wherever it was called from.</summary>
    /// <param name="said">What to do.</param>
    private static void On(Action said) => Dispatcher.UIThread.Post(said);
}

/// <summary>One run the host is showing, as the stack reads it.</summary>
internal sealed class ProgressRun : IProgressRun
{
    /// <summary>The hub that takes this run off its list when it ends.</summary>
    private readonly ProgressHub _hub;

    /// <summary>Makes a run named by a key.</summary>
    /// <param name="hub">The hub that holds it.</param>
    /// <param name="titleKey">The i18n key of what it is called.</param>
    /// <param name="iconKey">The picture to head its capsule with, or nothing.</param>
    public ProgressRun(ProgressHub hub, string titleKey, string? iconKey)
    {
        _hub = hub;
        TitleKey = titleKey;
        IconKey = iconKey;
    }

    /// <inheritdoc />
    public string TitleKey { get; }

    /// <inheritdoc />
    public string? IconKey { get; }

    /// <inheritdoc />
    public ProgressRecords Records { get; } = new();

    /// <inheritdoc />
    public void Touch() => _hub.Touch();

    /// <inheritdoc />
    public void End() => _hub.End(this);
}

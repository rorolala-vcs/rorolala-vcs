using Avalonia.Threading;

namespace FileSystemPlugin;

/// <summary>
/// The work this plugin owes the next frame, gathered so that one turn draws one answer.
/// </summary>
/// <remarks>
/// A change reaches the plugin from several places at once: a provider's answer landing, the files being
/// touched, a bring advancing, a virtual entry appearing. Each has its own subscription, and posting each
/// of them on its own is several rounds of measure, arrange and render for one thing the user did.
/// <para>
/// What is queued here is run once, at the priority the layout of the turn is finished at and before the
/// frame is drawn. The same action asked for twice in one turn is run once, which is what a change that
/// raises two events of the same kind comes to.
/// </para>
/// </remarks>
internal static class Frame
{
    /// <summary>What is waiting for the turn to finish being laid out.</summary>
    private static readonly HashSet<Action> Waiting = [];

    /// <summary>Whether the turn has already been asked to run what is waiting.</summary>
    private static bool _asked;

    /// <summary>Asks for some work in the frame being prepared, or in the one after it when there is none.</summary>
    /// <param name="work">What is to be done.</param>
    public static void Post(Action work)
    {
        ArgumentNullException.ThrowIfNull(work);

        if (!Waiting.Add(work) || _asked)
        {
            return;
        }

        _asked = true;

        Dispatcher.UIThread.Post(Run, DispatcherPriority.Loaded);
    }

    /// <summary>Runs what the turn owes, once.</summary>
    private static void Run()
    {
        _asked = false;

        var work = Waiting.ToArray();

        Waiting.Clear();

        foreach (var action in work)
        {
            action();
        }

        if (Waiting.Count > 0)
        {
            _asked = true;

            Dispatcher.UIThread.Post(Run, DispatcherPriority.Loaded);
        }
    }
}

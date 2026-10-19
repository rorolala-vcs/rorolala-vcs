using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Notifications;

/// <summary>
/// The notifications waiting to be shown in a window's stack.
/// </summary>
/// <remarks>
/// Nothing is drawn before there is a window, so what is posted while the host is still assembling the run
/// is held here and taken once a window is up. A subscription is told that something was posted, but the
/// one that draws takes the queue rather than the notification it was told about: the window's thread is
/// the only one that may build what a notification carries, and what is posted may come from any thread.
/// <para>
/// The queue is not deduplicated. What the kernel raises about a broken plugin is already deduplicated
/// before it reaches this, and a report a command produced is worth showing even when its words repeat:
/// the command was run again, and that is the fact the reader is being told about.
/// </para>
/// </remarks>
internal sealed class NotificationCenter
{
    /// <summary>What has been posted and not yet shown.</summary>
    private readonly List<Notification> _pending = [];

    /// <summary>Guards the queue, since a plugin may post from any thread.</summary>
    private readonly object _gate = new();

    /// <summary>Raised when something has been posted and is waiting to be shown.</summary>
    /// <remarks>
    /// Raised on the thread that posted it. What is listening moves the showing onto the window's own
    /// thread, and takes what it needs from the queue there.
    /// </remarks>
    public event EventHandler? Posted;

    /// <summary>Posts a notification, to be shown when there is a window to show it in.</summary>
    /// <param name="notification">What to show.</param>
    public void Post(Notification notification)
    {
        ArgumentNullException.ThrowIfNull(notification);

        lock (_gate)
        {
            _pending.Add(notification);
        }

        // Announced outside the lock: a listener runs on whatever thread posted this, and holding the
        // queue while it does would make every other poster wait for one window to be reached.
        Posted?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Takes everything posted since the last call, leaving the queue empty.</summary>
    /// <returns>The notifications, in the order they were posted.</returns>
    public IReadOnlyList<Notification> Take()
    {
        lock (_gate)
        {
            if (_pending.Count == 0)
            {
                return [];
            }

            var taken = _pending.ToArray();
            _pending.Clear();

            return taken;
        }
    }
}

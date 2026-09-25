namespace RorolalaDesktop.Logging;

/// <summary>
/// Notifications the user must act on, held until there is a window to show them in.
/// </summary>
/// <remarks>
/// Nothing is shown before the window exists: a failure while plugins are loaded happens before
/// there is anywhere to put a dialog, so what is raised is held and shown once the window is up.
/// Deduplication is the same service the log uses, so raising the same thing again adds nothing.
/// </remarks>
internal sealed class PopupService
{
    /// <summary>Where distinctness is decided, shared with the log.</summary>
    private readonly NotificationService _notifications;

    /// <summary>What has been raised and not yet shown.</summary>
    private readonly List<string> _pending = [];

    /// <summary>Guards the queue, since a plugin may raise from any thread.</summary>
    private readonly object _gate = new();

    /// <summary>Makes a popup queue over the shared deduplication service.</summary>
    /// <param name="notifications">Where distinctness is decided.</param>
    public PopupService(NotificationService notifications) => _notifications = notifications;

    /// <summary>
    /// Raises a notification, which is shown once however often its content is raised.
    /// </summary>
    /// <param name="level">The level it is raised at.</param>
    /// <param name="source">The plugin or kernel part raising it.</param>
    /// <param name="message">What to show.</param>
    public void Raise(LogLevel level, string source, string message)
    {
        var key = NotificationService.Key(level, source, message);

        lock (_gate)
        {
            if (_notifications.Note(key) != 1)
            {
                return;
            }

            _pending.Add($"[{source}] {message}");
        }
    }

    /// <summary>
    /// Takes everything raised since the last call, leaving the queue empty.
    /// </summary>
    /// <returns>The lines to show, in the order they were raised.</returns>
    public IReadOnlyList<string> Take()
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

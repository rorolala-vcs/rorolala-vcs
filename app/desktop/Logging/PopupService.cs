namespace RorolalaDesktop.Logging;

/// <summary>
/// Notifications the user must act on, held until there is a window to show them in.
/// </summary>
/// <remarks>
/// Nothing is shown before the window exists: a failure while plugins are loaded happens before
/// there is anywhere to put a dialog, so what is raised is held and shown once the window is up.
/// <para>
/// The deduplication service is shared with the log, and so is the count: an identical line and an
/// identical popup are one notification seen twice. What each holds is its own, though — the log
/// has one line for it and this shows it once — so a line the log has already recorded is still
/// shown if the user must act on it, and one that has been shown is not shown again.
/// </para>
/// </remarks>
internal sealed class PopupService
{
    /// <summary>Where distinctness and the count are decided, shared with the log.</summary>
    private readonly NotificationService _notifications;

    /// <summary>What has been shown, so a repeat is not shown again.</summary>
    private readonly HashSet<string> _shown = new(StringComparer.Ordinal);

    /// <summary>What has been raised and not yet shown.</summary>
    private readonly List<string> _pending = [];

    /// <summary>Guards the queue and what has been shown, since a plugin may raise from any thread.</summary>
    private readonly object _gate = new();

    /// <summary>Makes a popup queue over the shared deduplication service.</summary>
    /// <param name="notifications">Where distinctness and the count are decided.</param>
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

        _notifications.Note(key);

        lock (_gate)
        {
            if (!_shown.Add(key))
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

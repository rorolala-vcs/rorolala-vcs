using System.Collections.ObjectModel;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Logging;

/// <summary>
/// The host's one log, which the Log dock is a view over.
/// </summary>
/// <remarks>
/// Recording an identical line again increments its count rather than adding another, sharing the
/// one deduplication service with popups. Entries are kept in the order they were first said, so the
/// log reads as a history rather than as a stream of repeats.
/// </remarks>
internal sealed class LogService
{
    /// <summary>Where distinctness is decided, shared with popups.</summary>
    private readonly NotificationService _notifications;

    /// <summary>Each distinct entry by its identity, so a repeat finds its line.</summary>
    private readonly Dictionary<string, LogEntry> _byKey = new(StringComparer.Ordinal);

    /// <summary>Makes a log over the shared deduplication service.</summary>
    /// <param name="notifications">Where distinctness is decided.</param>
    public LogService(NotificationService notifications) => _notifications = notifications;

    /// <summary>Every distinct entry, in the order it was first said.</summary>
    public ObservableCollection<LogEntry> Entries { get; } = [];

    /// <summary>Records a message, counting a repeat rather than adding a line.</summary>
    /// <param name="level">The level it was said at.</param>
    /// <param name="source">The plugin or kernel part that said it.</param>
    /// <param name="message">What was said.</param>
    public void Record(LogLevel level, string source, string message)
    {
        var key = NotificationService.Key(level, source, message);
        var count = _notifications.Note(key);

        if (_byKey.TryGetValue(key, out var seen))
        {
            seen.Count = count;
            return;
        }

        var entry = new LogEntry(level, source, message, count);
        _byKey[key] = entry;
        Entries.Add(entry);
    }

    /// <summary>The log as one plugin sees it, naming the plugin on every line.</summary>
    /// <param name="source">The plugin's identity or display name.</param>
    /// <returns>A log that writes as that source.</returns>
    public ILog For(string source) => new SourceLog(this, source);
}

/// <summary>The log as one plugin sees it, naming the plugin on every line it writes.</summary>
internal sealed class SourceLog : ILog
{
    /// <summary>The log everything is written to.</summary>
    private readonly LogService _log;

    /// <summary>The name every line written here carries.</summary>
    private readonly string _source;

    /// <summary>Makes a view of a log under one name.</summary>
    /// <param name="log">The log everything is written to.</param>
    /// <param name="source">The name every line written here carries.</param>
    public SourceLog(LogService log, string source)
    {
        _log = log;
        _source = source;
    }

    /// <inheritdoc />
    public void Trace(string message) => _log.Record(LogLevel.Trace, _source, message);

    /// <inheritdoc />
    public void Debug(string message) => _log.Record(LogLevel.Debug, _source, message);

    /// <inheritdoc />
    public void Info(string message) => _log.Record(LogLevel.Info, _source, message);

    /// <inheritdoc />
    public void Warn(string message) => _log.Record(LogLevel.Warn, _source, message);

    /// <inheritdoc />
    public void Error(string message) => _log.Record(LogLevel.Error, _source, message);
}

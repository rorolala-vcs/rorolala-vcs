using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace RorolalaDesktop.Logging;

/// <summary>The five levels a log entry may be at, from finest to gravest.</summary>
internal enum LogLevel
{
    /// <summary>The finest steps.</summary>
    Trace,

    /// <summary>Useful while developing.</summary>
    Debug,

    /// <summary>What happened.</summary>
    Info,

    /// <summary>Something that may need attention.</summary>
    Warn,

    /// <summary>Something that went wrong.</summary>
    Error,
}

/// <summary>
/// One line of the log: what was said, by whom, at what level, and how often.
/// </summary>
/// <remarks>
/// The count is the number of times an identical line has been recorded. It changes rather than the
/// line being added again, so a message in a loop does not bury the rest of the log.
/// </remarks>
internal sealed class LogEntry : INotifyPropertyChanged
{
    private int _count;

    /// <summary>Makes a line.</summary>
    /// <param name="level">The level it was said at.</param>
    /// <param name="source">The plugin or kernel part that said it.</param>
    /// <param name="message">What was said.</param>
    /// <param name="count">How many times it has been said so far.</param>
    public LogEntry(LogLevel level, string source, string message, int count)
    {
        Level = level;
        Source = source;
        Message = message;
        _count = count;
    }

    /// <summary>The level it was said at.</summary>
    public LogLevel Level { get; }

    /// <summary>The plugin or kernel part that said it.</summary>
    public string Source { get; }

    /// <summary>What was said.</summary>
    public string Message { get; }

    /// <summary>How many times an identical line has been said.</summary>
    public int Count
    {
        get => _count;
        set
        {
            if (_count == value)
            {
                return;
            }

            _count = value;
            Raise();
        }
    }

    /// <inheritdoc />
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Tells the view the count changed.</summary>
    private void Raise([CallerMemberName] string? name = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}

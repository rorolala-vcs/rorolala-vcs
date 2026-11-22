using Avalonia.Threading;

namespace RorolalaDesktop.Configuration;

/// <summary>
/// Follows <c>theme.json</c> while the program runs, so a choice made in the other program is the choice here.
/// </summary>
/// <remarks>
/// The two programs are drawn from one file, and each reads it when it starts: a colour changed in one while the
/// other is open would otherwise be seen only by the next run of it. The file is a file, so the way to hear about
/// a write is to watch it. A burst of what one write looks like on the platform, a truncate and a write, is
/// gathered for a moment and read once.
/// <para>
/// A file that is being written and reads as nothing is not a choice, so a read that fails is left until the next
/// write rather than reported: the writer has not finished, and the watch is told again when it has.
/// </para>
/// </remarks>
internal sealed class ThemeFileWatch : IDisposable
{
    /// <summary>How long a burst of writes to the file is gathered before it is read.</summary>
    private static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(200);

    /// <summary>The platform's watch over the directory holding the file, or nothing when there is no such directory.</summary>
    private readonly FileSystemWatcher? _watcher;

    /// <summary>What is done once the file has settled.</summary>
    private readonly Action _changed;

    /// <summary>Whether the watch has been let go of, so a post already on its way does nothing.</summary>
    private bool _gone;

    /// <summary>What gathers a burst, made when the first event of one arrives.</summary>
    private DispatcherTimer? _timer;

    /// <summary>
    /// Watches one file in one directory, saying when it may have been written.
    /// </summary>
    /// <param name="directory">The directory the file is in.</param>
    /// <param name="file">The file's name.</param>
    /// <param name="changed">What is done once a write has settled.</param>
    public ThemeFileWatch(string directory, string file, Action changed)
    {
        ArgumentException.ThrowIfNullOrEmpty(directory);
        ArgumentException.ThrowIfNullOrEmpty(file);
        ArgumentNullException.ThrowIfNull(changed);

        _changed = changed;

        // A directory that is not there holds no file to follow: the writer that made one has not run yet,
        // and the next write is the first thing there is to hear.
        if (!Directory.Exists(directory))
        {
            return;
        }

        try
        {
            _watcher = new FileSystemWatcher(directory)
            {
                Filter = file,
                IncludeSubdirectories = false,
                NotifyFilter =
                    NotifyFilters.FileName
                    | NotifyFilters.LastWrite
                    | NotifyFilters.Size
                    | NotifyFilters.CreationTime,
            };
        }
        catch (Exception error)
            when (error is ArgumentException or IOException or UnauthorizedAccessException)
        {
            return;
        }

        _watcher.Changed += (_, _) => Soon();
        _watcher.Created += (_, _) => Soon();
        _watcher.Renamed += (_, _) => Soon();
        _watcher.EnableRaisingEvents = true;
    }

    /// <inheritdoc />
    public void Dispose()
    {
        _gone = true;

        if (_watcher is not null)
        {
            _watcher.EnableRaisingEvents = false;
        }

        _watcher?.Dispose();

        _timer?.Stop();
        _timer = null;
    }

    /// <summary>
    /// Reads now rather than when the burst settles, which the suite drives the watch with.
    /// </summary>
    internal void Flush()
    {
        if (_timer is not { } timer)
        {
            return;
        }

        timer.Stop();
        _timer = null;

        _changed();
    }

    /// <summary>Says that the file may have been written, once the burst of it has settled.</summary>
    /// <remarks>
    /// The platform says a write on a thread of its own, and the timer belongs to the window's: the post is what
    /// moves the answer from one to the other.
    /// </remarks>
    private void Soon() =>
        Dispatcher.UIThread.Post(() =>
        {
            if (_gone)
            {
                return;
            }

            _timer ??= Timer();

            _timer.Stop();
            _timer.Start();
        });

    /// <summary>The timer that ends a burst, which reads the file once.</summary>
    /// <returns>The timer.</returns>
    private DispatcherTimer Timer()
    {
        var timer = new DispatcherTimer { Interval = Settle };

        timer.Tick += (_, _) =>
        {
            timer.Stop();

            if (ReferenceEquals(_timer, timer))
            {
                _timer = null;
            }

            _changed();
        };

        return timer;
    }
}

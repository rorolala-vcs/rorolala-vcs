using System.Diagnostics.CodeAnalysis;
using Avalonia.Threading;

namespace FileSystemPlugin;

/// <summary>
/// The directories that are on screen, each watched for the changes another program makes.
/// </summary>
/// <remarks>
/// One watcher per directory rather than one over the whole base: what a view can be wrong about is the
/// directory it draws, and a directory nobody draws costs nothing. A directory a dock and a tree step both
/// show is watched once, which is what the count is for, and the watch survives the first of the two leaving.
/// <para>
/// The platform raises an event per change, and a program saving a file raises several of them, a temporary
/// file, a rename and a truncate. What is raised here is what a view can answer in one go: the directories
/// whose set of entries may differ, and the entries whose content may differ, each once per settle window,
/// which ends a settle time after the last event of the burst.
/// </para>
/// <para>
/// A watch that cannot be kept, a directory that was removed or is not permitted, is dropped and reported,
/// because the dock over it can do nothing about it and the program should say what it could not watch. An
/// overflowing buffer is reported too: the one thing a watcher cannot promise is that it saw everything, and
/// the caller answers that with a change across the board.
/// </para>
/// </remarks>
internal sealed class WatchService : IDisposable
{
    /// <summary>How long a burst of changes is gathered before it is raised.</summary>
    private readonly TimeSpan _settle;

    /// <summary>What is watched, by the directory made absolute and stripped of a trailing separator.</summary>
    private readonly Dictionary<string, Watching> _watching = new(StringComparer.Ordinal);

    /// <summary>The directories whose set of entries may have changed in this burst.</summary>
    private readonly HashSet<string> _sets = new(StringComparer.Ordinal);

    /// <summary>The entries whose content or time may have changed in this burst.</summary>
    private readonly HashSet<string> _entries = new(StringComparer.Ordinal);

    /// <summary>Whether a watch overflowed in this burst, which is a change no watcher can name.</summary>
    private bool _overflowed;

    /// <summary>What gathers a burst, made when the first change of one arrives.</summary>
    private DispatcherTimer? _burst;

    /// <summary>Makes the set of watches.</summary>
    /// <param name="settle">How long a burst of changes is gathered before it is raised.</param>
    public WatchService(TimeSpan settle) => _settle = settle;

    /// <summary>The directory whose set of entries may have changed, once per settle window.</summary>
    public event Action<string>? Set;

    /// <summary>The entry whose content or time may have changed, once per settle window.</summary>
    public event Action<string>? Entry;

    /// <summary>A burst the watcher could not name, which the caller answers with a change across the board.</summary>
    public event Action? Overflowed;

    /// <summary>A directory that could not be watched, which the caller says in the log.</summary>
    public event Action<string>? Unwatchable;

    /// <summary>
    /// Watches a directory from now until it is hidden as many times as it was shown.
    /// </summary>
    /// <remarks>
    /// Shown rather than watched, because a directory is on screen or it is not and the two of them can be
    /// showing the same one: what a caller states is that it is drawing the directory, and the last of them to
    /// stop is the one that stops the watch.
    /// </remarks>
    /// <param name="directory">The directory that is now on screen.</param>
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the watcher is kept in _watching and disposed by Hide or by Dispose"
    )]
    public void Show(string directory)
    {
        if (string.IsNullOrEmpty(directory))
        {
            return;
        }

        if (Watched(directory) is { } known)
        {
            known.Showing++;

            return;
        }

        var at = Path.TrimEndingDirectorySeparator(Path.GetFullPath(directory));

        FileSystemWatcher watcher;

        try
        {
            watcher = new FileSystemWatcher(at)
            {
                IncludeSubdirectories = false,
                NotifyFilter =
                    NotifyFilters.FileName
                    | NotifyFilters.DirectoryName
                    | NotifyFilters.LastWrite
                    | NotifyFilters.Size
                    | NotifyFilters.CreationTime,
            };
        }
        catch (Exception error)
            when (error
                is ArgumentException
                    or IOException
                    or UnauthorizedAccessException
                    or NotSupportedException
            )
        {
            Unwatchable?.Invoke(at);

            return;
        }

        try
        {
            // What a change of a directory's set is comes from the directory itself, its own name included: a
            // rename is one entry gone and another arrived, and both are the set of this directory changing.
            watcher.Created += (_, _) => Seen(at, entry: null);
            watcher.Deleted += (_, _) => Seen(at, entry: null);
            watcher.Renamed += (_, _) => Seen(at, entry: null);
            watcher.Changed += (_, changed) => Seen(at, changed.FullPath);
            watcher.Error += (_, _) => Spilled();
            watcher.EnableRaisingEvents = true;

            _watching[at] = new Watching(watcher);
        }
        catch (Exception error)
            when (error
                is ArgumentException
                    or IOException
                    or UnauthorizedAccessException
                    or NotSupportedException
            )
        {
            // A watcher that could not be put in place is let go of here rather than kept for a directory nothing
            // will ever hide.
            watcher.Dispose();
            Unwatchable?.Invoke(at);
        }
    }

    /// <summary>Stops watching a directory once everything that showed it has hidden it.</summary>
    /// <param name="directory">The directory that is no longer on screen.</param>
    public void Hide(string directory)
    {
        if (string.IsNullOrEmpty(directory) || Watched(directory) is not { } watching)
        {
            return;
        }

        if (--watching.Showing > 0)
        {
            return;
        }

        watching.Watcher.EnableRaisingEvents = false;
        watching.Watcher.Dispose();

        _watching.Remove(watching.At);
    }

    /// <inheritdoc />
    public void Dispose()
    {
        foreach (var watching in _watching.Values)
        {
            watching.Watcher.EnableRaisingEvents = false;
            watching.Watcher.Dispose();
        }

        _watching.Clear();

        _burst?.Stop();
        _burst = null;
    }

    /// <summary>The directory that is watched under a spelling, or nothing when it is not.</summary>
    /// <param name="directory">The directory to look for.</param>
    /// <returns>What is watched, or nothing.</returns>
    private Watching? Watched(string directory)
    {
        foreach (var (at, watching) in _watching)
        {
            if (Pathing.Same(at, directory))
            {
                return watching;
            }
        }

        return null;
    }

    /// <summary>Says that an entry of a directory changed, in the burst being gathered.</summary>
    /// <remarks>
    /// The platform raises its events on a thread of its own, and what is gathered here belongs to the window's
    /// thread: the sets are written there and read there, which is what one post per event is for.
    /// </remarks>
    /// <param name="directory">The watched directory the event came from.</param>
    /// <param name="entry">The entry that changed, or nothing when the set of the directory may have.</param>
    private void Seen(string directory, string? entry) =>
        Dispatcher.UIThread.Post(() =>
        {
            if (entry is null)
            {
                _sets.Add(directory);
            }
            else
            {
                _entries.Add(entry);
            }

            Gathered();
        });

    /// <summary>Says that a watch overflowed, in the burst being gathered.</summary>
    private void Spilled() =>
        Dispatcher.UIThread.Post(() =>
        {
            _overflowed = true;
            Gathered();
        });

    /// <summary>Starts the settle window again, which the last event of a burst is what ends.</summary>
    private void Gathered()
    {
        if (_burst is null)
        {
            _burst = new DispatcherTimer { Interval = _settle };
            _burst.Tick += (_, _) => Raised();
        }

        _burst.Stop();
        _burst.Start();
    }

    /// <summary>Raises the burst that has just ended, and starts the next one empty.</summary>
    private void Raised()
    {
        _burst?.Stop();

        var sets = _sets.ToArray();
        var entries = _entries.ToArray();
        var overflowed = _overflowed;

        _sets.Clear();
        _entries.Clear();
        _overflowed = false;

        if (overflowed)
        {
            Overflowed?.Invoke();
        }

        foreach (var directory in sets)
        {
            Set?.Invoke(directory);
        }

        foreach (var entry in entries)
        {
            Entry?.Invoke(entry);
        }
    }

    /// <summary>
    /// Raises the burst being gathered now, rather than when the settle window ends.
    /// </summary>
    /// <remarks>
    /// For the suite, which drives the platform's events itself and has no window loop to wait in: what is checked
    /// there is what a burst comes to, not how long one is gathered for.
    /// </remarks>
    internal void Flush() => Raised();

    /// <summary>One directory being watched, and how many things are showing it.</summary>
    /// <param name="watcher">The platform's watcher over it.</param>
    private sealed class Watching(FileSystemWatcher watcher)
    {
        /// <summary>The directory, as it was watched, which is how it is forgotten.</summary>
        public string At { get; init; } = watcher.Path;

        /// <summary>The platform's watcher over the directory.</summary>
        public FileSystemWatcher Watcher { get; } = watcher;

        /// <summary>How many things are showing the directory, the last of which ends the watch.</summary>
        public int Showing { get; set; } = 1;
    }
}

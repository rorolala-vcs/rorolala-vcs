using Avalonia.Threading;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a reading of the work says about the directories being listed, read once each and kept.
/// </summary>
/// <remarks>
/// It is shared by the views read from the work — the two columns, the badges, and the Version Graph's
/// edit — because a reading is not a cheap question: it walks the tree beside the Layout and, for every
/// path that records a version, walks the version chain as well. One reading per directory between them
/// is the difference between a listing that costs what it costs and one that costs it several times.
/// <para>
/// A directory both providers were asked about is read the first time and the same reading is handed over
/// after that, which is what makes the sharing worth having. Nothing is reached for: a reading is a snapshot
/// of what the tree held at one moment, and a snapshot is what a file operation makes stale — which is what
/// <see cref="Forget"/> is for.
/// </para>
/// <para>
/// It is not read on the window's thread. A reading is a walk of the whole tree, and a listing asks about a
/// directory while it draws: walking it there would stop the window for as long as the walk takes, on a
/// directory nobody is waiting for. So the read is started on a thread of its own and nothing is answered until
/// it lands — a listing drawn in that moment has no state marks, which is what a listing of a work nothing has
/// been read for looks like anyway. What lands is drawn again, and by a drawing rather than by a change: see
/// <see cref="IFileChanges.Redrawn"/>.
/// </para>
/// </remarks>
internal sealed class StatusReadings
{
    /// <summary>What was read for each directory, so a listing costs one read rather than one per row.</summary>
    /// <remarks>
    /// Guarded because the reading is shared by views that ask from different threads: the listing asks on
    /// the window's, and the Version Graph asks from the thread it reads a chain on. What is read is held
    /// outside the lock, since reading the work is the expensive part and two asks racing for one directory
    /// may both read rather than one wait for the other.
    /// </remarks>
    private readonly Dictionary<string, RolaStatus?> _readings = new(StringComparer.Ordinal);

    /// <summary>The directories a read is going for, so that one directory is read once at a time.</summary>
    private readonly HashSet<string> _asking = new(StringComparer.Ordinal);

    /// <summary>What guards <see cref="_readings"/>, <see cref="_asking"/>, <see cref="_version"/> and <see cref="_warned"/>.</summary>
    private readonly object _gate = new();

    /// <summary>Which reading of the work the kept answers are of, which is what a read that landed is judged by.</summary>
    private int _version;

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>How the views are told that a reading is at hand.</summary>
    private readonly IFileChanges _files;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the set of readings.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    /// <param name="files">How the views are told that a reading is at hand.</param>
    public StatusReadings(ILog log, IFileChanges files)
    {
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(files);

        _log = log;
        _files = files;
    }

    /// <summary>The reading of the work a directory sits in, read once and kept.</summary>
    /// <remarks>
    /// Nothing is answered for a directory that has not been read yet: the read is started here and lands later, and
    /// what asks is drawn again when it does.
    /// </remarks>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The reading, or nothing when the directory is not inside a Workspace that works in a Layout.</returns>
    public RolaStatus? Reading(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        int version;

        lock (_gate)
        {
            if (_readings.TryGetValue(directory, out var known))
            {
                return known;
            }

            if (!_asking.Add(directory))
            {
                return null;
            }

            version = _version;
        }

        _ = Task.Run(() => Land(directory, version, Locate(directory)));

        return null;
    }

    /// <summary>
    /// Whether a path a reading named is the file the program names at `path`.
    /// </summary>
    /// <remarks>
    /// What a reading names is relative to the Workspace it was made of — a state, a standing and a merge are
    /// all lists of such paths — and what a listing has is a path the system names, so the two are joined before
    /// they are compared. The reading itself does this for a state and a standing, which it is asked about one
    /// path at a time; a list it hands over whole is what a reader of those lists has to do it for.
    /// </remarks>
    /// <param name="root">Where the Workspace the reading was made of is rooted.</param>
    /// <param name="named">The path the reading named, relative to the root.</param>
    /// <param name="path">The path the program names.</param>
    /// <returns>Whether they are the same file.</returns>
    public static bool Names(string root, string named, string path)
    {
        ArgumentNullException.ThrowIfNull(root);
        ArgumentNullException.ThrowIfNull(named);
        ArgumentNullException.ThrowIfNull(path);

        // Both are made absolute before they are compared, since one is relative to the root and the other to
        // whatever the program was run in, and a path that is the same file spelled two ways is one file. What
        // the comparison is case-sensitive about is the platform's answer rather than this one's: Windows
        // holds one file under either case, and a filesystem that does not would have two.
        return string.Equals(
            Path.GetFullPath(Path.Combine(root, named)),
            Path.GetFullPath(path),
            OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal
        );
    }

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared by both columns, so this may be the second letting go of the same set: what
    /// is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget()
    {
        RolaStatus?[] statuses;

        lock (_gate)
        {
            _version++;
            statuses = [.. _readings.Values];
            _readings.Clear();
        }

        foreach (var status in statuses)
        {
            status?.Dispose();
        }
    }

    /// <summary>Reads how the work under a directory stands, saying so if it could not be read at all.</summary>
    /// <param name="directory">The directory the read is of.</param>
    /// <returns>The reading, or nothing when there is nothing to read.</returns>
    private RolaStatus? Locate(string directory)
    {
        try
        {
            return RolaStatus.Locate(directory);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"how the work under `{directory}` stands", error);

            return null;
        }
    }

    /// <summary>Keeps what a read that landed came to, and has the views drawn again for it.</summary>
    /// <param name="directory">The directory it was read for.</param>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="status">What it came to.</param>
    private void Land(string directory, int version, RolaStatus? status)
    {
        Dispatcher.UIThread.Post(() =>
        {
            bool current;

            lock (_gate)
            {
                _asking.Remove(directory);

                // The files were said to have changed while this was being read: what it came to is a snapshot of a
                // tree that is no longer the one being drawn, so it is given back rather than kept.
                current = version == _version;

                if (current)
                {
                    if (_readings.TryGetValue(directory, out var older))
                    {
                        older?.Dispose();
                    }

                    _readings[directory] = status;
                }
            }

            if (!current)
            {
                status?.Dispose();
            }

            // Drawn either way: what landed is drawn when it stands, and when it does not, the drawing is what asks
            // for it again — the answer being one from before the change the drawing is following. It is a drawing and
            // not a change, so nothing anyone else read is given back for it.
            _files.Redraw();
        });
    }

    /// <summary>
    /// Says once that something could not be read.
    /// </summary>
    /// <remarks>
    /// A listing that will not appear because the work could not be read is a worse answer than one that
    /// appears without the columns, and it is said once because a directory of a thousand entries would
    /// otherwise say the same thing a thousand times.
    /// </remarks>
    /// <param name="what">What could not be read.</param>
    /// <param name="error">Why it could not be read.</param>
    private void Warn(string what, Exception error)
    {
        lock (_gate)
        {
            if (_warned)
            {
                return;
            }

            _warned = true;
        }

        _log.Warn($"{what} could not be read: {error.Message}");
    }
}

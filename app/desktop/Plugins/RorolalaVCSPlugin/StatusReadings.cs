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

    /// <summary>What guards <see cref="_readings"/> and <see cref="_warned"/>.</summary>
    private readonly object _gate = new();

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the set of readings.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    public StatusReadings(ILog log) => _log = log;

    /// <summary>The reading of the work a directory sits in, read once and kept.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The reading, or nothing when the directory is not inside a Workspace that works in a Layout.</returns>
    public RolaStatus? Reading(string directory)
    {
        lock (_gate)
        {
            if (_readings.TryGetValue(directory, out var known))
            {
                return known;
            }
        }

        RolaStatus? status = null;

        try
        {
            status = RolaStatus.Locate(directory);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"how the work under `{directory}` stands", error);
        }

        lock (_gate)
        {
            // Another ask read the same directory in the meantime: its reading is the one that stands,
            // and this one is given back rather than kept beside it.
            if (_readings.TryGetValue(directory, out var raced))
            {
                status?.Dispose();

                return raced;
            }

            _readings[directory] = status;

            return status;
        }
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
            statuses = [.. _readings.Values];
            _readings.Clear();
        }

        foreach (var status in statuses)
        {
            status?.Dispose();
        }
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

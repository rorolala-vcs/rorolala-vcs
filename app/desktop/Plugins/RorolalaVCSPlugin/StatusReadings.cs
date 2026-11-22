using System.Diagnostics.CodeAnalysis;
using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a reading of the work says about the directories being listed, read once each and kept.
/// </summary>
/// <remarks>
/// It is shared by the views read from the work, the two columns, the badges, and the Version Graph's
/// edit, because a reading is not a cheap question: it walks the tree beside the Layout and, for every
/// path that records a version, walks the version chain as well. One reading per directory between them
/// is the difference between a listing that costs what it costs and one that costs it several times.
/// <para>
/// A directory both providers were asked about is read the first time and the same reading is handed over
/// after that, which is what makes the sharing worth having. Nothing is reached for: a reading is a snapshot
/// of what the tree held at one moment, and a snapshot is what a file operation makes stale, which is what
/// <see cref="Forget()"/> is for.
/// </para>
/// <para>
/// It is not read on the window's thread. A reading is a walk of the whole tree, and a listing asks about a
/// directory while it draws: walking it there would stop the window for as long as the walk takes, on a
/// directory nobody is waiting for. So the read is started on a thread of its own and nothing is answered until
/// it lands, a listing drawn in that moment has no state marks, which is what a listing of a work nothing has
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
    private readonly Dictionary<string, StatusReading?> _readings = new(StringComparer.Ordinal);

    /// <summary>
    /// The paths each Workspace was last read as having lost, which is what the catalogue is kept in step
    /// with.
    /// </summary>
    /// <remarks>
    /// Keyed by the Workspace's root, because a reading is taken of one Workspace and its paths are relative
    /// to it. What is kept is the set rather than the entries: an entry is the catalogue's, and what this
    /// needs to know is which ones it put there and may have to take away.
    /// </remarks>
    private readonly Dictionary<string, HashSet<string>> _lost = new(StringComparer.Ordinal);

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
    public StatusReading? Reading(string directory)
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
    /// What a reading names is relative to the Workspace it was made of, a state, a standing and a merge are
    /// all lists of such paths, and what a listing has is a path the system names, so the two are joined before
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
        Forget(_ => true);
    }

    /// <summary>
    /// Lets go of what was read for one directory and for every directory that holds it.
    /// </summary>
    /// <remarks>
    /// What a reading of a directory holds is the work under it, so a change anywhere under it makes it stale,
    /// and so does a change under any directory holding it. A reading of a directory beside it is not made wrong
    /// by the change and is kept, which is what keeps a change in one place from reading the whole work again.
    /// </remarks>
    /// <param name="directory">The directory whose files may have changed.</param>
    public void Forget(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        Forget(one => Subtree.Holds(one, directory));
    }

    /// <summary>Lets go of the readings a change can sour.</summary>
    /// <param name="stale">Which reading the change makes stale.</param>
    private void Forget(Func<string, bool> stale)
    {
        StatusReading?[] statuses;
        HashSet<string>[] lost;

        lock (_gate)
        {
            var at = _readings.Keys.Union(_lost.Keys).Where(stale).ToArray();

            // Nothing was read that the change can sour, which is the ordinary case for a directory no reader
            // has asked about yet, and nothing is given back for it.
            if (at.Length == 0)
            {
                return;
            }

            _version++;
            statuses = [.. at.Where(_readings.ContainsKey).Select(one => _readings[one])];
            lost = [.. at.Where(_lost.ContainsKey).Select(one => _lost[one])];

            foreach (var one in at)
            {
                _readings.Remove(one);
                _lost.Remove(one);
            }
        }

        // What was shown as lost is taken away before what it was read from is let go of: the next reading
        // will put back what is still gone, and a path that has come back is one nothing should be showing.
        foreach (var paths in lost)
        {
            VirtualEntries.Withdraw(paths);
        }

        foreach (var status in statuses)
        {
            status?.Dispose();
        }
    }

    /// <summary>Reads how the work under a directory stands, saying so if it could not be read at all.</summary>
    /// <param name="directory">The directory the read is of.</param>
    /// <returns>The reading, or nothing when there is nothing to read.</returns>
    // The scope holds only the reading, and the reading is what is kept: disposing the scope here would give
    // back the very reading the cache goes on to answer with, so the two are one owner and not two.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the scope holds only the reading, which the cache keeps and gives back on Forget"
    )]
    private StatusReading? Locate(string directory)
    {
        try
        {
            // The reading is kept rather than the scope: a disposed scope gives its reading back, and the
            // one kept here is let go of by Forget or by the replacement that lands after it.
            var scope = new StatusScope(directory);
            scope.Read();

            return scope.Status;
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.Unavailable or RolaReadErrorKind.NoWorkspace)
        {
            // No Workspace or no Layout is the absence the V1 call answered with nothing rather than a
            // failure, and nothing is said about it here for the same reason.
            return null;
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
    private void Land(string directory, int version, StatusReading? status)
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

            // What the work has lost is put in the catalogue before the drawing that shows it: the redraw is
            // handed on below, and an entry put after it would wait for the next one.
            if (current && status is not null)
            {
                Publish(status);
            }

            // Drawn either way: what landed is drawn when it stands, and when it does not, the drawing is what asks
            // for it again, the answer being one from before the change the drawing is following. It is a drawing and
            // not a change, so nothing anyone else read is given back for it.
            _files.Redraw();
        });
    }

    /// <summary>
    /// Keeps the catalogue in step with what a reading says the work has lost.
    /// </summary>
    /// <remarks>
    /// Every path is put rather than only the ones that were not there before: a path may already have been
    /// put as something on its way, which the catalogue keeps rather than replacing, and when the bring that
    /// put it is over this same reading is what puts the loss back if the file never arrived. A path the
    /// reading no longer names is taken away, which is what a file that came back looks like.
    /// </remarks>
    /// <param name="reading">What was read.</param>
    private void Publish(StatusReading reading)
    {
        var paths = LostPaths.Of(reading.Root, reading.Lost);
        var now = paths.ToHashSet(StringComparer.Ordinal);
        HashSet<string> old;

        lock (_gate)
        {
            if (!_lost.TryGetValue(reading.Root, out var known))
            {
                known = new(StringComparer.Ordinal);
            }

            old = known;
        }

        foreach (var gone in old.Except(now, StringComparer.Ordinal))
        {
            VirtualEntries.Withdraw(gone);
        }

        foreach (var path in paths)
        {
            VirtualEntries.Place(path, EntryKind.File, VirtualEntryState.Lost);
        }

        lock (_gate)
        {
            _lost[reading.Root] = now;
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

/// <summary>Where the paths a reading calls lost are, as the program names them.</summary>
/// <remarks>
/// A reading names a path relative to the Workspace it was taken of, and a listing, a tree step and the
/// catalogue all name one by the path the system gives: the two are joined before anything is compared or
/// put, which is the same joining the columns make for the standings.
/// </remarks>
internal static class LostPaths
{
    /// <summary>The absolute path for each path a reading named.</summary>
    /// <param name="root">Where the Workspace the reading was taken of is rooted.</param>
    /// <param name="lost">The paths the reading named, relative to the root.</param>
    /// <returns>The paths, each made absolute, without repeats.</returns>
    public static IReadOnlyList<string> Of(string root, IReadOnlyList<string> lost)
    {
        ArgumentNullException.ThrowIfNull(root);
        ArgumentNullException.ThrowIfNull(lost);

        var at = Path.GetFullPath(root);

        return
        [
            .. lost
                .Where(named => named.Length > 0)
                .Select(named =>
                    Path.GetFullPath(
                        Path.Combine(at, named.Replace('/', Path.DirectorySeparatorChar))
                    )
                )
                .Distinct(StringComparer.Ordinal),
        ];
    }
}

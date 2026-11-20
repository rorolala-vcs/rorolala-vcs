using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// What each rola-tree stands as, read once and kept, for the columns and badges that read it.
/// </summary>
/// <remarks>
/// A tree's status walks the tree beside its staging area and baseline, so it is not a question to ask
/// once per row. One reading per tree is what makes a listing of a tree cost one walk rather than one for
/// every directory in it.
/// <para>
/// Like the Work's own status, it is not read on the window's thread. The read is started on a thread of
/// its own, what asks in the meantime has no answer, and what lands is drawn again through
/// <see cref="IFileChanges.Redraw"/>. A file change makes every kept answer stale, which is what
/// <see cref="Forget"/> is for.
/// </para>
/// </remarks>
internal sealed class TreeReadings
{
    /// <summary>What was read for each tree, so a listing costs one read rather than one per row.</summary>
    private readonly Dictionary<string, TreeStatus?> _readings = new(StringComparer.Ordinal);

    /// <summary>
    /// The paths each tree was last read as having had deleted, which is what the catalogue is kept in step
    /// with.
    /// </summary>
    /// <remarks>
    /// Keyed by the tree's root, because a tree's changes name paths inside it. What is kept is the set of
    /// paths rather than the entries: an entry is the catalogue's, and what this needs to know is which ones
    /// it put there and may have to take away.
    /// </remarks>
    private readonly Dictionary<string, HashSet<string>> _lost = new(StringComparer.Ordinal);

    /// <summary>The trees a read is going for, so one tree is read once at a time.</summary>
    private readonly HashSet<string> _asking = new(StringComparer.Ordinal);

    /// <summary>What guards the readings, the asking set and the generation.</summary>
    private readonly object _gate = new();

    /// <summary>Which reading of the work the kept answers are of.</summary>
    private int _version;

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>How the views are told that a reading is at hand.</summary>
    private readonly IFileChanges _files;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the set of tree readings.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    /// <param name="files">How the views are told that a reading is at hand.</param>
    public TreeReadings(ILog log, IFileChanges files)
    {
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(files);

        _log = log;
        _files = files;
    }

    /// <summary>The status of a tree, read once and kept.</summary>
    /// <param name="tree">The tree's directory.</param>
    /// <returns>The reading, or nothing while it is being read or when it cannot be.</returns>
    public TreeStatus? Reading(string tree)
    {
        ArgumentNullException.ThrowIfNull(tree);

        int version;

        lock (_gate)
        {
            if (_readings.TryGetValue(tree, out var known))
            {
                return known;
            }

            if (!_asking.Add(tree))
            {
                return null;
            }

            version = _version;
        }

        _ = Task.Run(() => Land(tree, version, Locate(tree)));

        return null;
    }

    /// <summary>Lets go of what was read, so the next answer is read from the tree as it is now.</summary>
    public void Forget()
    {
        HashSet<string>[] lost;

        lock (_gate)
        {
            _version++;
            lost = [.. _lost.Values];
            _readings.Clear();
            _lost.Clear();
        }

        // What was shown as deleted is taken away before what it was read from is let go of: the next reading
        // puts back what is still gone, and a path that has been restored is one nothing should be showing.
        foreach (var paths in lost)
        {
            VirtualEntries.Withdraw(paths);
        }
    }

    /// <summary>Reads a tree's status, saying so if it could not be read at all.</summary>
    /// <param name="tree">The tree's directory.</param>
    /// <returns>The reading, or nothing when there is nothing to read.</returns>
    private TreeStatus? Locate(string tree)
    {
        try
        {
            // The scope is made over the tree rather than kept: it owns no handle, and the reading it
            // answers with is a value the cache keeps.
            var scope = new TreeScope(tree);
            scope.Read();

            return scope.Status;
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.InvalidArgument or RolaReadErrorKind.Unavailable)
        {
            // A directory that is not there, or is in no tree, is the absence the V1 call answered with
            // nothing rather than a failure, and nothing is said about it here for the same reason.
            return null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"how the tree `{tree}` stands", error);

            return null;
        }
    }

    /// <summary>Keeps what a read that landed came to, and has the views drawn again for it.</summary>
    /// <param name="tree">The tree it was read for.</param>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="status">What it came to.</param>
    private void Land(string tree, int version, TreeStatus? status)
    {
        Dispatcher.UIThread.Post(() =>
        {
            bool current;

            lock (_gate)
            {
                _asking.Remove(tree);

                current = version == _version;

                if (current)
                {
                    _readings[tree] = status;
                }
            }

            // What the work has deleted inside the tree is put in the catalogue before the drawing that shows
            // it: the redraw is handed on below, and an entry put after it would wait for the next one.
            if (current && status is not null)
            {
                Publish(tree, status);
            }

            _files.Redraw();
        });
    }

    /// <summary>
    /// Keeps the catalogue in step with what a reading says was deleted inside a tree.
    /// </summary>
    /// <remarks>
    /// Only the unstaged deletions: one the staging area has recorded is a deletion somebody has already
    /// said yes to, which the tree panel draws as any other recorded change. Every path is put rather than
    /// only the ones that were not there before, so a path that was put as something on its way is put back
    /// as lost if the thing never arrived.
    /// </remarks>
    /// <param name="tree">The tree the reading was taken of.</param>
    /// <param name="status">What it was read as.</param>
    private void Publish(string tree, TreeStatus status)
    {
        var placed = TreeLosses.Of(tree, status.Unstaged);
        var now = placed.Select(loss => loss.Path).ToHashSet(StringComparer.Ordinal);
        HashSet<string> old;

        lock (_gate)
        {
            if (!_lost.TryGetValue(tree, out var known))
            {
                known = new(StringComparer.Ordinal);
            }

            old = known;
        }

        foreach (var gone in old.Except(now, StringComparer.Ordinal))
        {
            VirtualEntries.Withdraw(gone);
        }

        foreach (var loss in placed)
        {
            VirtualEntries.Place(loss.Path, loss.Kind, VirtualEntryState.Lost);
        }

        lock (_gate)
        {
            _lost[tree] = now;
        }
    }

    /// <summary>
    /// Says once that something could not be read.
    /// </summary>
    /// <remarks>
    /// A listing that will not appear because a tree could not be read is a worse answer than one that
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

/// <summary>One path a tree reading says has been deleted, and what kind of thing stood there.</summary>
/// <param name="Path">The absolute path the thing stood at.</param>
/// <param name="Kind">Whether it was a file or a directory.</param>
internal sealed record TreeLoss(string Path, EntryKind Kind);

/// <summary>
/// Where the deletions one tree reading names are, as the program names them.
/// </summary>
/// <remarks>
/// A tree names a path relative to its own root, and a listing, a tree step and the catalogue name one by the
/// path the system gives: the two are joined before anything is compared or put, which is the same joining
/// the tree's columns make for a change. The kind the reading gives is kept, because a deleted directory and
/// a deleted file are drawn as what they were.
/// </remarks>
internal static class TreeLosses
{
    /// <summary>The word a reading uses for a path the work holds no longer.</summary>
    private const string Deleted = "deleted";

    /// <summary>The word a reading uses for a directory.</summary>
    private const string Directory = "Directory";

    /// <summary>The deletion each change of a reading stands for.</summary>
    /// <param name="tree">Where the tree the reading was taken of is rooted.</param>
    /// <param name="changes">What the work changed, as the reading names it.</param>
    /// <returns>The deletions, one per path, made absolute.</returns>
    public static IReadOnlyList<TreeLoss> Of(string tree, IReadOnlyList<TreeChange> changes)
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(changes);

        var at = Path.GetFullPath(tree);

        return
        [
            .. changes
                .Where(change =>
                    change.Path.Length > 0
                    && string.Equals(change.Change, Deleted, StringComparison.Ordinal)
                )
                .Select(change => new TreeLoss(
                    Path.GetFullPath(
                        Path.Combine(
                            at,
                            change.Path.Replace('/', Path.DirectorySeparatorChar)
                        )
                    ),
                    string.Equals(change.Kind, Directory, StringComparison.OrdinalIgnoreCase)
                        ? EntryKind.Directory
                        : EntryKind.File
                ))
                .DistinctBy(loss => loss.Path, StringComparer.Ordinal),
        ];
    }
}

using Avalonia.Threading;
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
    private readonly Dictionary<string, RolaTreeStatus?> _readings = new(StringComparer.Ordinal);

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
    public RolaTreeStatus? Reading(string tree)
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
        lock (_gate)
        {
            _version++;
            _readings.Clear();
        }
    }

    /// <summary>Reads a tree's status, saying so if it could not be read at all.</summary>
    /// <param name="tree">The tree's directory.</param>
    /// <returns>The reading, or nothing when there is nothing to read.</returns>
    private RolaTreeStatus? Locate(string tree)
    {
        try
        {
            return RolaTreeStatus.Locate(tree);
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
    private void Land(string tree, int version, RolaTreeStatus? status)
    {
        Dispatcher.UIThread.Post(() =>
        {
            lock (_gate)
            {
                _asking.Remove(tree);

                if (version == _version)
                {
                    _readings[tree] = status;
                }
            }

            _files.Redraw();
        });
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

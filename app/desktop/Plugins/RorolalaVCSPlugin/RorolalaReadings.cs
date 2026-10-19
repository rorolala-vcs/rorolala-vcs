using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// What the Vault copy says about the directories being listed, read once each.
/// </summary>
/// <remarks>
/// What is read is the copy the Workspace has fetched, through <see cref="RolaOwnership"/> — nothing is
/// reached for, so a listing says what the Vault held when it was last fetched. One reading answers for
/// everything this plugin shows about an entry: who holds it, and where upstream names it. Two
/// contributions asking separately would open the same two Layouts twice per directory.
/// <para>
/// The answers are read once per directory rather than once per row, and forgotten when the files may have
/// changed — which is when another run is most likely to have fetched a newer copy.
/// </para>
/// <para>
/// Opening the two Layouts is not done on the window's thread: a Layout is a snapshot of the whole tree, and a
/// listing asks about a directory while it draws. The read is started on a thread of its own and nothing is answered
/// until it lands; a listing drawn in that moment reads an ownership column that is not there yet, and is drawn again
/// when it is (see <see cref="IFileChanges.Redrawn"/>).
/// </para>
/// </remarks>
internal sealed class OwnershipReadings
{
    /// <summary>What was read for each directory, so a listing costs one read rather than one per row.</summary>
    /// <remarks>
    /// Guarded because a read lands from the thread it was made on, and the answers are asked for on the window's.
    /// </remarks>
    private readonly Dictionary<string, RolaOwnership?> _known = new(StringComparer.Ordinal);

    /// <summary>The directories a read is going for, so that one directory is read once at a time.</summary>
    private readonly HashSet<string> _asking = new(StringComparer.Ordinal);

    /// <summary>What guards <see cref="_known"/>, <see cref="_asking"/>, <see cref="_version"/> and <see cref="_warned"/>.</summary>
    private readonly object _gate = new();

    /// <summary>Which reading of the work the kept answers are of, which is what a read that landed is judged by.</summary>
    private int _version;

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>How the views are told that a reading is at hand.</summary>
    private readonly IFileChanges _files;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the readings.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    /// <param name="files">How the views are told that a reading is at hand.</param>
    public OwnershipReadings(ILog log, IFileChanges files)
    {
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(files);

        _log = log;
        _files = files;
    }

    /// <summary>
    /// Whether the Vault copy says anything about the entries of a directory.
    /// </summary>
    /// <remarks>
    /// Asked once for a listing rather than once per row, since it is what decides whether a column is
    /// there at all. A directory nothing has been read of yet answers nothing, and is read for the asking.
    /// </remarks>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether there is anything to read answers from.</returns>
    public bool Knows(string directory) => Of(directory) is not null;

    /// <summary>What holds the entry, or that nothing is known.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>What holds it; nothing is known when there is nothing to read it from.</returns>
    public EntryLock Lock(Entry entry) =>
        Answer(entry, (ownership, path) => ownership.LockOf(path), new EntryLock(EntryLockKind.Unnamed));

    /// <summary>Where the Vault's copy names the entry, or an empty string.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The path upstream names it by, or nothing when it names none.</returns>
    public string Upstream(Entry entry) =>
        Answer(entry, (ownership, path) => ownership.UpstreamOf(path), string.Empty);

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// What was read is a `RolaOwnership` — two Layouts opened once — and a Layout is a snapshot: what
    /// another run writes after it was opened is not in it. A move or a removal renames a path in the
    /// Layout the Workspace works in, so the answers kept here are about a tree that no longer exists, and
    /// the handles are given back rather than only forgotten.
    /// <para>
    /// Only a change is one: a reading that landed is drawn without this being said of it, so nothing read for it is
    /// given back (see <see cref="IFileChanges.Redrawn"/>).
    /// </para>
    /// </remarks>
    public void Forget()
    {
        RolaOwnership?[] ownerships;

        lock (_gate)
        {
            _version++;
            ownerships = [.. _known.Values];
            _known.Clear();
        }

        foreach (var ownership in ownerships)
        {
            ownership?.Dispose();
        }
    }

    /// <summary>
    /// One answer about one entry, read from the copy the directory's Workspace has fetched.
    /// </summary>
    /// <remarks>
    /// What an answer is about is a file of the work, so an entry that names no directory of its own — the
    /// way up, which is a step rather than a place — is nothing either answer speaks about. A read that
    /// fails is answered as nothing rather than raised: a listing that will not appear because a lock could
    /// not be read is a worse answer than one that appears without it.
    /// </remarks>
    /// <typeparam name="T">What is answered.</typeparam>
    /// <param name="entry">The entry to ask about.</param>
    /// <param name="ask">How the answer is read from the copy.</param>
    /// <param name="nothing">What is answered when there is nothing to read it from.</param>
    /// <returns>The answer.</returns>
    private T Answer<T>(Entry entry, Func<RolaOwnership, string, T> ask, T nothing)
    {
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || Of(directory) is not { } ownership
        )
        {
            return nothing;
        }

        try
        {
            return ask(ownership, entry.Path);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"what the Vault says about `{entry.Path}`", error);

            return nothing;
        }
    }

    /// <summary>The answers for a directory, read once and kept, asking for them in the background when they are not here.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The answers, or nothing when the directory is not inside a Workspace that tracks one.</returns>
    private RolaOwnership? Of(string directory)
    {
        int version;

        lock (_gate)
        {
            if (_known.TryGetValue(directory, out var known))
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

    /// <summary>Opens the two Layouts a directory's answers are read from, saying so if they could not be opened.</summary>
    /// <param name="directory">The directory the answers are for.</param>
    /// <returns>The answers, or nothing when there are none to read.</returns>
    private RolaOwnership? Locate(string directory)
    {
        try
        {
            return RolaOwnership.Locate(directory);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"the ownership of `{directory}`", error);

            return null;
        }
    }

    /// <summary>Keeps what a read that landed came to, and has the views drawn again for it.</summary>
    /// <param name="directory">The directory it was read for.</param>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="ownership">What it came to.</param>
    private void Land(string directory, int version, RolaOwnership? ownership)
    {
        Dispatcher.UIThread.Post(() =>
        {
            bool current;

            lock (_gate)
            {
                _asking.Remove(directory);
                current = version == _version;

                if (current)
                {
                    if (_known.TryGetValue(directory, out var older))
                    {
                        older?.Dispose();
                    }

                    _known[directory] = ownership;
                }
            }

            if (!current)
            {
                ownership?.Dispose();
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
    /// It is said once because a directory of a thousand entries would otherwise say the same thing a
    /// thousand times.
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

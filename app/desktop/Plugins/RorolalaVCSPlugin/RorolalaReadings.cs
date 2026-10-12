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
/// </remarks>
internal sealed class OwnershipReadings
{
    /// <summary>What was read for each directory, so a listing costs one read rather than one per row.</summary>
    private readonly Dictionary<string, RolaOwnership?> _known = new(StringComparer.Ordinal);

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the readings.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    public OwnershipReadings(ILog log) => _log = log;

    /// <summary>
    /// Whether the Vault copy says anything about the entries of a directory.
    /// </summary>
    /// <remarks>
    /// Asked once for a listing rather than once per row, since it is what decides whether a column is
    /// there at all.
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
    /// </remarks>
    public void Forget()
    {
        foreach (var ownership in _known.Values)
        {
            ownership?.Dispose();
        }

        _known.Clear();
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

    /// <summary>The answers for a directory, read once and kept.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The answers, or nothing when the directory is not inside a Workspace that tracks one.</returns>
    private RolaOwnership? Of(string directory)
    {
        if (_known.TryGetValue(directory, out var known))
        {
            return known;
        }

        RolaOwnership? ownership = null;

        try
        {
            ownership = RolaOwnership.Locate(directory);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"the ownership of `{directory}`", error);
        }

        _known[directory] = ownership;

        return ownership;
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
        if (_warned)
        {
            return;
        }

        _warned = true;
        _log.Warn($"{what} could not be read: {error.Message}");
    }
}

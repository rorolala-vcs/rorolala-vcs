using System.Diagnostics.CodeAnalysis;
using Avalonia.Media.Imaging;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to who holds an entry: the Ownership column, and the corner a locked tile wears.
/// </summary>
/// <remarks>
/// What it reads is the Vault copy the Workspace has fetched, through <see cref="RolaOwnership"/> —
/// nothing is reached for, so a listing says what the Vault held when it was last fetched. The two
/// pictures are contributed to the icon library rather than drawn here: the File System plugin draws
/// an entry's lock in the ink the theme gives it, and a picture contributed under a key is what it
/// draws.
/// <para>
/// The answers are read once per directory rather than once per row, and forgotten when the window
/// is come back to — which is when another run is most likely to have fetched a newer copy.
/// </para>
/// </remarks>
internal sealed class RorolalaLocks : IEntryLockProvider
{
    /// <summary>The key the picture of a lock is contributed under.</summary>
    private const string Locked = "rorolala_vcs.lock";

    /// <summary>The key the picture of a pencil is contributed under.</summary>
    private const string Create = "rorolala_vcs.create";

    /// <summary>What a cell says when nothing holds the entry.</summary>
    private const string Nothing = "-";

    /// <summary>The i18n key a held-by-you cell reads as.</summary>
    private const string Mine = "rorolala_vcs.ownership.mine";

    /// <summary>What was read for each directory, so a listing costs one read rather than one per row.</summary>
    private readonly Dictionary<string, RolaOwnership?> _owned = new(StringComparer.Ordinal);

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the provider.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    private RorolalaLocks(ILog log) => _log = log;

    /// <inheritdoc />
    public string Id => "rorolala.locks";

    /// <inheritdoc />
    public string ColumnKey => "rorolala_vcs.column_ownership";

    /// <inheritdoc />
    public bool Applies(string directory) => Ownership(directory) is not null;

    /// <inheritdoc />
    public EntryLockMark Mark(Entry entry)
    {
        // What a lock is about is a file of the work, so an entry that names no directory of its own
        // — the way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || Ownership(directory) is not { } ownership
        )
        {
            return new EntryLockMark();
        }

        return LockOf(ownership, entry.Path) switch
        {
            // Held by the reader, which is the pencil: holding it is what makes it theirs to change, and
            // the corner says so in the ordinary foreground rather than in a colour of its own — an entry
            // that is yours is not something to be told about. The words say the same in the column, since
            // "you" is a name like any other and a column that shouted it would shout every row it holds.
            { Kind: EntryLockKind.Mine } => new EntryLockMark(
                TextKey: Mine,
                IconKey: Create,
                Ink: LockInk.Plain,
                TextInk: LockInk.Plain
            ),

            // Held by somebody else, which is the lock: the one answer on the card that is not the
            // reader's, so it is drawn in the error red and worn as a tag, the way the shell draws a
            // state. The words name whoever holds it, in the same red.
            { Kind: EntryLockKind.Held, Holder: { Length: > 0 } holder } => new EntryLockMark(
                Text: holder,
                IconKey: Locked,
                Ink: LockInk.Error,
                TextInk: LockInk.Error,
                Tagged: true
            ),

            // Nobody holds it, or nothing says: the same thing to a reader, and the same cell.
            _ => new EntryLockMark(Text: Nothing),
        };
    }

    /// <summary>
    /// Contributes this plugin's two pictures and its provider to the host.
    /// </summary>
    /// <param name="host">The host, for the icon library and the refocus event.</param>
    // The pictures are handed to the library, which holds them for as long as the program runs: they
    // are not lost at the end of this scope, so there is nothing here to dispose.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the icon library holds each picture for the life of the program, so handing one over is not losing it"
    )]
    public static void Declare(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.Icons.Add(Locked, Picture("lock"));
        host.Icons.Add(Create, Picture("create"));

        var locks = new RorolalaLocks(host.Log);
        EntryLockProviders.Register(locks);

        // What a lock says is read from the Vault's fetched copy, and a fetch is what another run
        // does while this one is in front: coming back to the window is when to read it again.
        host.Refocus.Regained += locks.Forget;
    }

    /// <summary>The ownership answers for a directory, read once and kept.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The answers, or nothing when the directory is not inside a Workspace that tracks one.</returns>
    private RolaOwnership? Ownership(string directory)
    {
        if (_owned.TryGetValue(directory, out var known))
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

        _owned[directory] = ownership;

        return ownership;
    }

    /// <summary>Forgets what was read, so the next listing reads the copy again.</summary>
    private void Forget()
    {
        foreach (var ownership in _owned.Values)
        {
            ownership?.Dispose();
        }

        _owned.Clear();
    }

    /// <summary>
    /// Says once that something could not be read.
    /// </summary>
    /// <remarks>
    /// A listing that will not appear because a lock could not be read is a worse answer than one
    /// that appears without the column, so a read that fails is answered as nothing rather than
    /// raised. It is said once because a directory of a thousand entries would otherwise say the same
    /// thing a thousand times.
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

    /// <summary>What holds the entry at a path, or that nothing could be read.</summary>
    /// <param name="ownership">What was read for the directory.</param>
    /// <param name="path">The path of the entry.</param>
    /// <returns>What holds it; nothing is known when the read itself failed.</returns>
    private EntryLock LockOf(RolaOwnership ownership, string path)
    {
        try
        {
            return ownership.LockOf(path);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"the lock on `{path}`", error);

            return new EntryLock(EntryLockKind.Unnamed);
        }
    }

    /// <summary>The picture embedded under `name`.</summary>
    /// <param name="name">The icon's name, without its suffix.</param>
    /// <returns>What to contribute to the icon library.</returns>
    private static Bitmap Picture(string name)
    {
        var resource = $"RorolalaVCSPlugin.icons.{name}.png";

        using var stream =
            typeof(RorolalaLocks).Assembly.GetManifestResourceStream(resource)
            ?? throw new InvalidOperationException($"the icon `{resource}` is not embedded");

        return new Bitmap(stream);
    }
}

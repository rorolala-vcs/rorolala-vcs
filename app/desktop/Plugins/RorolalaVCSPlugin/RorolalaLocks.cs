using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to who holds an entry: the Ownership column, and the badge a locked tile wears.
/// </summary>
/// <remarks>
/// What it reads is the Vault copy the Workspace has fetched, through <see cref="OwnershipReadings"/> —
/// nothing is reached for, so a listing says what the Vault held when it was last fetched. The two
/// pictures are contributed to the icon library rather than drawn here: the File System plugin draws an
/// entry's lock in the ink the theme gives it, and a picture contributed under a key is what it draws.
/// </remarks>
internal sealed class RorolalaLocks : IEntryLockProvider
{
    /// <summary>What a cell says when nothing holds the entry.</summary>
    private const string Nothing = "-";

    /// <summary>The i18n key a held-by-you cell reads as.</summary>
    private const string Mine = "rorolala_vcs.ownership.mine";

    /// <summary>What the badge of the reader's own lock says when the pointer rests on it.</summary>
    private const string MineHover = "rorolala_vcs.ownership.mine_hover";

    /// <summary>The phrase the badge of another account's lock is hovered for, with the holder's name in it.</summary>
    private const string HeldBy = "rorolala_vcs.ownership.held_by";

    /// <summary>What the Vault copy says about the entries being listed.</summary>
    private readonly OwnershipReadings _readings;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the Vault copy says about the entries being listed.</param>
    private RorolalaLocks(OwnershipReadings readings) => _readings = readings;

    /// <inheritdoc />
    public string Id => "rorolala.locks";

    /// <inheritdoc />
    public string ColumnKey => "rorolala_vcs.column_ownership";

    /// <inheritdoc />
    public bool Applies(string directory) => _readings.Knows(directory);

    /// <inheritdoc />
    public EntryMark Mark(Entry entry) =>
        _readings.Lock(entry) switch
        {
            // Held by the reader, which is the pencil: holding it is what makes it theirs to change, and the
            // badge says so in the green the look uses for what has come in, worn on the dark ground the way
            // another account's lock is — the two are the two answers to the same question, so they are drawn
            // the same way round. The words stay in the ordinary ink, since "you" is a name like any other and
            // a column that shouted it would shout every row it holds.
            //
            // What the badge says on hover is what holding the file means to the person reading it, which the
            // column's own word for it does not: the column says a name, and a name does not say whether the
            // file may be edited. The phrase names nobody, because "you" is not somebody else's name.
            { Kind: EntryLockKind.Mine } => new EntryMark(
                TextKey: Mine,
                IconKey: Icons.Create,
                Ink: MarkInk.Add,
                TextInk: MarkInk.Plain,
                Tagged: true,
                HoverKey: MineHover
            ),

            // Held by somebody else, which is the lock: the one answer on the card that is not the
            // reader's, so it is drawn in the error red and worn on the dark ground, the way the shell draws a
            // state. The words name whoever holds it, in the same red — and the badge, which is only a
            // picture of a lock, is hovered for the phrase that name goes in.
            { Kind: EntryLockKind.Held, Holder: { Length: > 0 } holder } => new EntryMark(
                Text: holder,
                IconKey: Icons.Lock,
                Ink: MarkInk.Error,
                TextInk: MarkInk.Error,
                Tagged: true,
                HoverKey: HeldBy
            ),

            // Nobody holds it, or nothing says: the same thing to a reader, and the same cell.
            _ => new EntryMark(Text: Nothing),
        };

    /// <summary>
    /// Contributes this plugin's pictures, its column and its provider to the host.
    /// </summary>
    /// <param name="host">The host, for the icon library and the log.</param>
    public static void Declare(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        Icons.Declare(host.Icons);

        // One set of readings for both of this plugin's contributions: who holds an entry and where
        // upstream names it are read from the same two Layouts, and asking twice would open them twice.
        var readings = new OwnershipReadings(host.Log);
        var locks = new RorolalaLocks(readings);

        // The same answer is a tile's badge as well as the list's column, so it is contributed as both: a
        // badge is one of the marks a tile wears, and the ownership column is drawn from the provider rather
        // than from the badge.
        EntryLockProviders.Register(locks);
        EntryMarkProviders.Register(locks);

        // The State mark and the column saying how far a path is from upstream are both read from the work
        // itself rather than from the Vault's copy the two above read, so they share a set of readings of
        // their own: one reading of a directory answers both columns, and the badges too. The Upstream
        // column reads it as well, because a variant file is the one file that column can name and the Vault
        // cannot.
        var statuses = new StatusReadings(host.Log);

        EntryColumns.Register(new RorolalaUpstream(readings, statuses));
        EntryColumns.Register(new RorolalaStates(statuses));
        EntryColumns.Register(new RorolalaStanding(statuses));

        // A state is worn on a tile as well as drawn in the list's own column, and it is worn before the rest:
        // what a file of the work is, is the first thing about it — who holds it and how far it has come are
        // read after that.
        EntryMarkProviders.Register(new RorolalaStates(statuses));

        // How far a version has come is a different question about the same file as who holds it, so it is a
        // badge of its own, worn after the lock: a file may be somebody else's and be behind, and neither
        // answer may hide the other.
        EntryMarkProviders.Register(new RorolalaStandingBadge(statuses));

        // And what a variant file is — content waiting to be joined into the file beside it — is the third,
        // worn after both: it is the one answer that is about the file's own place in a merge rather than
        // about the work around it.
        EntryMarkProviders.Register(new RorolalaMergeBadge(statuses));

        // Nothing is subscribed to here for letting go of what was read: the File System plugin says
        // when the files may have changed — after an operation of its own, and when the window is come
        // back to — and that is the one thing an answer about a lock can go stale from.
    }

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the column this plugin contributes, so this may be the second letting
    /// go of the same set: what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

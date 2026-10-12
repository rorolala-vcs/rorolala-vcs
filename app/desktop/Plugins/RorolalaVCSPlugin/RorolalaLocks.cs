using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to who holds an entry: the Ownership column, and the corner a locked tile wears.
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

    /// <summary>The phrase the corner of a lock is hovered for, with the holder's name in it.</summary>
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
    public EntryLockMark Mark(Entry entry) =>
        _readings.Lock(entry) switch
        {
            // Held by the reader, which is the pencil: holding it is what makes it theirs to change, and the
            // corner says so in the green the look uses for what has come in, worn as a tag the way another
            // account's lock is — the two are the two answers to the same question, so they are drawn the same
            // way round. The words stay in the ordinary ink, since "you" is a name like any other and a column
            // that shouted it would shout every row it holds.
            { Kind: EntryLockKind.Mine } => new EntryLockMark(
                TextKey: Mine,
                IconKey: Icons.Create,
                Ink: MarkInk.Add,
                TextInk: MarkInk.Plain,
                Tagged: true
            ),

            // Held by somebody else, which is the lock: the one answer on the card that is not the
            // reader's, so it is drawn in the error red and worn as a tag, the way the shell draws a
            // state. The words name whoever holds it, in the same red — and the corner, which is only a
            // picture of a lock, is hovered for the phrase that name goes in.
            { Kind: EntryLockKind.Held, Holder: { Length: > 0 } holder } => new EntryLockMark(
                Text: holder,
                IconKey: Icons.Lock,
                Ink: MarkInk.Error,
                TextInk: MarkInk.Error,
                Tagged: true,
                HoverKey: HeldBy
            ),

            // Nobody holds it, or nothing says: the same thing to a reader, and the same cell.
            _ => new EntryLockMark(Text: Nothing),
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

        EntryLockProviders.Register(new RorolalaLocks(readings));
        EntryColumns.Register(new RorolalaUpstream(readings));

        // The State column reads the work itself rather than the Vault's copy the two above read, so it keeps
        // a reading of its own.
        EntryColumns.Register(new RorolalaStates(host.Log));

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

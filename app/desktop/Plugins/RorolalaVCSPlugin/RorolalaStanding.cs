using Avalonia.Media;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to how far a file is from upstream: the Standing column of a directory dock's list.
/// </summary>
/// <remarks>
/// A file's version and the one the Vault records are the same while the work is in step, and differ as soon
/// as either side moves. This side having recorded something the Vault has not been told of is <em>ahead</em>,
/// which is ordinary work; the Vault having moved on without this side is <em>behind</em>, which is work to be
/// fetched. The two are told apart because what is to be done about them is opposite, and the column says
/// which by the sign of a number rather than by a word: `+2` is two versions this side has that the Vault does
/// not, `-1` is one the Vault has that this side does not.
/// <para>
/// A number is not always there to give. A fetch brings the Vault's Layout and never its versions, so the
/// version the Vault is at is ordinarily one this side has never had: being behind is then a direction with no
/// count, and two sides that have each moved on are not a distance from one another at all. The three answers
/// are told apart by what the cell holds — a signed number, a bare sign, and a question mark — because what a
/// reader does about them differs too.
/// </para>
/// <para>
/// It is read from the work the same way the State mark beside it is, and through the same reading — see
/// <see cref="StatusReadings"/> — so a directory is read once for both columns rather than once each.
/// </para>
/// <para>
/// What it says is a value rather than a mark, because a distance is a number and not one of a handful: a
/// cell says the dash the listing says everything it does not know with, which here is a file that is in step
/// as well as one upstream does not name. There is nothing to act on in either, and the two are told apart by
/// the State mark and the Upstream column rather than by this one.
/// </para>
/// </remarks>
internal sealed class RorolalaStanding : IEntryColumnProvider
{
    /// <summary>How wide the column starts, which is room for a sign and a few digits.</summary>
    private const double ColumnWidth = 56;

    /// <summary>What the work says about the directories being listed, shared with the State column.</summary>
    private readonly StatusReadings _readings;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the work says about the directories being listed.</param>
    public RorolalaStanding(StatusReadings readings) => _readings = readings;

    /// <inheritdoc />
    public string Id => "rorolala.standing";

    /// <inheritdoc />
    public string ColumnKey => "rorolala_vcs.column_standing";

    /// <inheritdoc />
    public double Width => ColumnWidth;

    /// <summary>
    /// The column stands with the listing's own columns, in front of them.
    /// </summary>
    /// <remarks>
    /// How far a file is from upstream is read once what the entry is has been, alongside the listing's own
    /// columns — the time it was written, its size — and it stands before them because it is Rorolala's answer
    /// about the file rather than a fact of the filesystem. What is read with the name is where the entry is
    /// named elsewhere, which is the Upstream column's business and not this one's.
    /// </remarks>
    public ColumnPlacement Placement => ColumnPlacement.BeforeValues;

    /// <summary>
    /// The column is one of values: what it says about a file is a number.
    /// </summary>
    /// <remarks>
    /// A dash is what a file in step has, which is why this is not a column of marks: a column of marks says
    /// nothing by having no mark, and the one thing a reader wants here — that there is nothing between the
    /// two sides — would be the one thing it could not say.
    /// </remarks>
    public bool Marks => false;

    /// <inheritdoc />
    public bool Applies(string directory) => _readings.Reading(directory) is not null;

    /// <inheritdoc />
    public EntryColumnMark Value(Entry entry)
    {
        // What a standing is about is a file of the work, so an entry that names no directory of its own —
        // the way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || _readings.Reading(directory) is not { } status
        )
        {
            return new EntryColumnMark();
        }

        try
        {
            return Say(status.StandingOf(entry.Path));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new EntryColumnMark();
        }
    }

    /// <summary>
    /// What a standing is written as: a signed count of versions, a dash for nothing to say, and the whole of
    /// what it means said on the cell.
    /// </summary>
    /// <remarks>
    /// The colours are this plugin's own — see <see cref="RorolalaColours"/> — and they are the green and the
    /// red the plugin spends on the two sides of the same question everywhere else: green for work that is
    /// here and not recorded yet, which is what being ahead is, and red for what must not be recorded, which is
    /// what another side having moved on is.
    /// <para>
    /// The count is what the cell is for, and it is not always known: a fetch brings the Vault's Layout and
    /// never its versions, so the version the Vault is at is ordinarily one this side has never had, and the
    /// chain that would count the gap is the Vault's own. A standing whose count is not known says so by
    /// holding no number at all, and one whose chains have parted says so with a question mark — which is the
    /// one thing a number cannot say, since there is no number of versions between two sides that are not on
    /// one chain.
    /// </para>
    /// </remarks>
    /// <param name="standing">What the reading says about the entry.</param>
    /// <returns>What the cell says, which is the dash for a file with nothing between it and upstream.</returns>
    internal static EntryColumnMark Say(Standing standing)
    {
        ArgumentNullException.ThrowIfNull(standing);

        return standing.Relation switch
        {
            // A version ahead of what the Vault was told of: work recorded here that is not upstream's yet.
            StandingRelation.Ahead when standing.Distance is { } ahead => new EntryColumnMark(
                Text: $"+{ahead}",
                Colour: RorolalaColours.Untagged,
                Hint: Said(standing)
            ),

            // A version behind the Vault's: work upstream has that this side has not fetched, and the chain
            // that counts the versions between is the Vault's own.
            StandingRelation.Behind when standing.Distance is { } behind => new EntryColumnMark(
                Text: $"-{behind}",
                Colour: RorolalaColours.Wrong,
                Hint: Said(standing)
            ),

            // Behind it, with the count the Vault's chain would give not here: the same direction as above,
            // said without a number, and the words say so rather than leaving a zero to read as being in step.
            StandingRelation.Behind => new EntryColumnMark(
                Text: Diverged,
                Colour: RorolalaColours.Wrong,
                Hint: Said(standing)
            ),

            // Both sides moved on, so neither is based on the other and neither is a distance from it: there
            // is no number of versions between two chains, which is what the mark says.
            StandingRelation.Apart => new EntryColumnMark(
                Text: Apart,
                Colour: RorolalaColours.Wrong,
                Hint: Said(standing)
            ),

            // In step, or nothing says how the two stand, or the path is not one this reading is about: the
            // listing's own dash, which is what it says everything it does not know with.
            _ => new EntryColumnMark(Text: Diverged),
        };
    }

    /// <summary>
    /// What a standing says, which is the whole of what the count means and what to do about it.
    /// </summary>
    /// <remarks>
    /// It is what the cell is hovered for and what the badge on a tile says, read from here by both: a count
    /// beside a name is a fact a reader has to know how to act on, and a badge has no room for the number — so
    /// the sentence is where the two are one answer about one file rather than two halves of one.
    /// <para>
    /// Where the count is not known the sentence says the direction and stops: being behind is being behind, and
    /// how far is added when it is known rather than an apology for not knowing it. A number that is not known is
    /// not a number a reader may read as "in step", which is why the two are separate sentences at all.
    /// </para>
    /// </remarks>
    /// <param name="standing">What the reading says about the entry.</param>
    /// <returns>What to say, or nothing for a file with nothing between it and upstream.</returns>
    internal static string Said(Standing standing)
    {
        ArgumentNullException.ThrowIfNull(standing);

        return standing.Relation switch
        {
            StandingRelation.Ahead => RolaI18N.Get(
                "rorolala_vcs.standing.ahead",
                standing.Distance ?? 0UL
            ),
            StandingRelation.Behind when standing.Distance is { } behind => RolaI18N.Get(
                "rorolala_vcs.standing.behind",
                behind
            ),
            StandingRelation.Behind => RolaI18N.Get("rorolala_vcs.standing.behind_untold"),
            StandingRelation.Apart => RolaI18N.Get("rorolala_vcs.standing.apart"),
            _ => string.Empty,
        };
    }

    /// <summary>
    /// What a cell says when there is no distance to write.
    /// </summary>
    /// <remarks>
    /// Written out here rather than left to the listing, because the listing's own dash is what it says a
    /// value it knows nothing about is, and this is a value the reading has an answer for. It is the same
    /// character on purpose: a reader should not have to tell one dash from another.
    /// </remarks>
    private const string Diverged = "-";

    /// <summary>
    /// What a cell says when the two sides are not on one chain at all.
    /// </summary>
    /// <remarks>
    /// A question mark rather than a dash or a number, because what is being said is that the cell has no
    /// number to give: a dash reads as nothing between the two, and any count would be a lie about versions
    /// that were never on one chain. What it stands for is on the cell, where the pointer reaching it says it.
    /// </remarks>
    private const string Apart = "?";

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the State column, so this may be the second letting go of the same set:
    /// what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

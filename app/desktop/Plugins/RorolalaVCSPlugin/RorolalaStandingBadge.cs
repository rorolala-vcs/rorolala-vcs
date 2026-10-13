using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to how far a file is from upstream, worn as a badge of a tile's icon.
/// </summary>
/// <remarks>
/// A tile has no columns, so what the Standing column of a list says is worn on the icon instead: an arrow
/// pointing the way the work would have to move for the two sides to be in step, in the same two colours the
/// column writes its numbers in. It is the second badge of the arc, read after the lock's: who holds a file
/// and how far its version has come are different questions about the same file — a file may be somebody
/// else's and be behind — and neither answer may hide the other.
/// <para>
/// It is read through the same <see cref="StatusReadings"/> the two list columns read through, so a
/// directory that is listed twice is read once: a reading walks the version chain for every path that
/// records one, which is too much to do twice for the same answer.
/// </para>
/// </remarks>
internal sealed class RorolalaStandingBadge : IEntryMarkProvider
{
    /// <summary>The arrow the set draws for work this side has that the Vault has not been told of.</summary>
    private const string Upload = Icons.Upload;

    /// <summary>The arrow the set draws for work the Vault has that this side has not fetched.</summary>
    private const string Download = Icons.Download;

    /// <summary>The mark the set draws for two chains that have each moved on.</summary>
    /// <remarks>
    /// A fork rather than an arrow, because there is no direction the work could move in to settle it: the
    /// two arrows say which side is further on, and this says that neither is.
    /// </remarks>
    private const string Split = Icons.CallSplit;

    /// <summary>What the work says about the directories being listed, shared with the list's own columns.</summary>
    private readonly StatusReadings _readings;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the work says about the directories being listed.</param>
    public RorolalaStandingBadge(StatusReadings readings) => _readings = readings;

    /// <inheritdoc />
    public string Id => "rorolala.standing.badge";

    /// <inheritdoc />
    public bool Applies(string directory) => _readings.Reading(directory) is not null;

    /// <inheritdoc />
    public EntryMark Mark(Entry entry)
    {
        // What a standing is about is a file of the work, so an entry that names no directory of its own —
        // the way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || _readings.Reading(directory) is not { } status
        )
        {
            return new EntryMark();
        }

        try
        {
            return Say(status.StandingOf(entry.Path));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new EntryMark();
        }
    }

    /// <summary>
    /// What a standing is worn as: an arrow for the way the work would have to move, a fork where there is no
    /// way, drawn the way the lock beside it is — and nothing at all where there is nothing to move.
    /// </summary>
    /// <remarks>
    /// Worn on the dark ground the lock's badge wears: the two mark the same tile at once and are read one
    /// after the other, so a picture laid on the artwork and a badge the artwork carries would be two marks of
    /// two different kinds about one file. The ground is the badge's own dark whatever the look is, so the role
    /// the picture is drawn in is read against that and not against the tile — which is why the picture is the
    /// ink and the ground the badge's.
    /// <para>
    /// Green up for work waiting to be sent and red down for work waiting to be fetched: the two are opposite
    /// things to do about the same file, and the direction is what tells them apart where a glance at the colour
    /// only says that something is there to do. A fork is the third answer and is drawn as one: the two sides
    /// have each moved on, so the work cannot be settled by moving it either way.
    /// </para>
    /// <para>
    /// Behind is one answer whether or not the count is known. A fetch brings the Vault's Layout and never its
    /// versions, so what the Vault is at is ordinarily a version this side has never had: the direction is
    /// certain — it has work this side has not fetched — and how far is the Vault's own chain's to say.
    /// </para>
    /// </remarks>
    /// <param name="standing">What the reading says about the entry.</param>
    /// <returns>The mark to draw, which says nothing for a file with nothing between it and upstream.</returns>
    internal static EntryMark Say(Standing standing)
    {
        ArgumentNullException.ThrowIfNull(standing);

        return standing.Relation switch
        {
            // Ahead of what the Vault was told of: this side holds work the other has not been given.
            StandingRelation.Ahead => new EntryMark(
                IconKey: Upload,
                Ink: MarkInk.Add,
                Tagged: true,
                HoverKey: "rorolala_vcs.standing.ahead_hover"
            ),

            // Behind the Vault's: work upstream has that this side has not fetched, whether or not the chain
            // that would count the versions between is here.
            StandingRelation.Behind => new EntryMark(
                IconKey: Download,
                Ink: MarkInk.Error,
                Tagged: true,
                HoverKey: "rorolala_vcs.standing.behind_hover"
            ),

            // Both moved on: neither is based on the other, so there is no side the work could move to.
            StandingRelation.Apart => new EntryMark(
                IconKey: Split,
                Ink: MarkInk.Error,
                Tagged: true,
                HoverKey: "rorolala_vcs.standing.apart_hover"
            ),

            // In step, or nothing says how the two stand: no badge at all, since a mark saying "nothing is
            // between these" would be drawn on nearly every tile of a listing at rest.
            _ => new EntryMark(),
        };
    }

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the list's own columns, so this may be the third letting go of the same
    /// set: what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

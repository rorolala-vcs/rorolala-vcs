using Avalonia.Media;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to what a file of the work is: the State mark at the left of a directory's list, and the
/// badge a tile's icon wears.
/// </summary>
/// <remarks>
/// What it reads is a reading of the work, through <see cref="RolaStatus"/> — nothing is reached for, and a
/// reading is what the tree held when it was made. One reading answers for the whole directory being
/// listed, and one path is asked of it rather than answered from a copy of its lists: which state a path is
/// in is a judgement about the reading, and it is the library's to make.
/// <para>
/// What it contributes to a list is a dot rather than a column of words: a file is in one of a handful of
/// states, and which one is read from the colour before any word of it is read. A tile has no column to put
/// one in, so there the state is a badge on the icon — a picture in the same colour, since what a state is
/// coloured is the state (see <see cref="RorolalaColours"/>) — and the words are the dot's and the badge's
/// own, said when the pointer rests on either.
/// </para>
/// </remarks>
internal sealed class RorolalaStates : IEntryColumnProvider, IEntryMarkProvider
{
    /// <summary>How wide the column starts, which is room for a dot and the room around it.</summary>
    private const double ColumnWidth = 18;

    /// <summary>What the work says about the directories being listed, shared with the standing column.</summary>
    private readonly StatusReadings _readings;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the work says about the directories being listed.</param>
    public RorolalaStates(StatusReadings readings) => _readings = readings;

    /// <inheritdoc />
    public string Id => "rorolala.states";

    /// <summary>
    /// No key at all, since there is no word that would fit over a column of dots.
    /// </summary>
    /// <remarks>
    /// The listing draws a heading from a key, and a column that names none is headed by nothing — the same
    /// way it heads the picture column. What the mark means is said on the mark itself, which is where a
    /// reader looking at one is looking.
    /// </remarks>
    public string ColumnKey => string.Empty;

    /// <inheritdoc />
    public double Width => ColumnWidth;

    /// <inheritdoc />
    public ColumnPlacement Placement => ColumnPlacement.Leftmost;

    /// <summary>
    /// The column is one of marks: what it says about a file is a colour, and the word for it is on the mark.
    /// </summary>
    /// <remarks>
    /// A file that stands as it was recorded has no mark at all, which is what a file that is nothing to do
    /// with the work looks like too: the column is read for what is coloured and not for what is not.
    /// </remarks>
    public bool Marks => true;

    /// <inheritdoc />
    public bool Applies(string directory) => _readings.Reading(directory) is not null;

    /// <inheritdoc />
    public EntryColumnMark Value(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Answer(entry) is { } stated
            ? Say(stated.State, stated.Words)
            : new EntryColumnMark();
    }

    /// <inheritdoc />
    public EntryMark Mark(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Answer(entry) is { } stated
            ? Badge(stated.State, stated.Words)
            : new EntryMark();
    }

    /// <summary>What a reading says about one entry: the state it is in, and what that state is said as.</summary>
    /// <param name="State">What the reading says about the entry.</param>
    /// <param name="Words">What that is called, which is the same words wherever the state is shown.</param>
    private sealed record Stated(PathState State, string Words);

    /// <summary>What the reading says about an entry and what it is said as, or nothing.</summary>
    /// <remarks>
    /// The two places a state is drawn ask the same question of the same reading — a mark in a list and a
    /// badge on a tile are one answer about one file — so what they read is read once here: a directory the
    /// entry names none of, and a reading that could not answer, are both nothing to say rather than a
    /// failure, since a listing that will not appear is a worse answer than one without a mark on it.
    /// <para>
    /// The words are read here rather than in either view because they are the reading's to say: which of the
    /// two ways a change is wrong is not in the state, and a mark that had only the state to go by would say
    /// less than the panel does about the same file.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>What the reading says, or nothing.</returns>
    private Stated? Answer(Entry entry)
    {
        // What a state is about is a file of the work, so an entry that names no directory of its own — the
        // way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || _readings.Reading(directory) is not { } status
        )
        {
            return null;
        }

        try
        {
            var state = status.StateOf(entry.Path);

            return new Stated(
                state,
                state switch
                {
                    PathState.Wrong => Wrong(status, entry.Path),
                    PathState.Merging => Merging(status, entry.Path),
                    _ => Words(state),
                }
            );
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return null;
        }
    }

    /// <summary>
    /// What a state is said as, which is the Overview's own words for it.
    /// </summary>
    /// <remarks>
    /// The two are one answer about one file, so the words are the panel's rather than the listing's: a reader
    /// who has learnt what a badge means has learnt what the panel's pill means, and a sentence corrected in one
    /// place is corrected in the other. They live here rather than in either view because they are what the
    /// reading says about a file, and what a view does with them — a dot, a badge, a pill — is the view's own.
    /// <para>
    /// The one state whose words need more than the state is a change that is wrong: the two ways it can be are
    /// told apart by the lists the reading holds, which is <see cref="Wrong"/>, and what is said here is the
    /// answer where neither list names it.
    /// </para>
    /// </remarks>
    /// <param name="state">What the reading says about the entry.</param>
    /// <returns>What to say, which is nothing for a state a listing has no entry for.</returns>
    internal static string Words(PathState state) =>
        state switch
        {
            PathState.Untagged => RolaI18N.Get("rorolala_vcs.status.kind_untagged"),
            PathState.Modified => RolaI18N.Get("rorolala_vcs.status.change_modified"),
            PathState.Wrong => RolaI18N.Get("rorolala_vcs.state.wrong"),
            PathState.Renamed => RolaI18N.Get("rorolala_vcs.status.kind_rename_strong"),
            PathState.RenamedLikely => RolaI18N.Get("rorolala_vcs.status.kind_rename_weak"),

            // A merge the reading holds as one waiting to be joined: the words are the panel's for a merge, and
            // the variant it waits for is on the badge of the variant file — the other half of the same record.
            PathState.Merging => RolaI18N.Get("rorolala_vcs.status.kind_merge"),

            // A path the Layout names and the tree does not hold is not an entry of any listing, and a path the
            // reading says nothing about is a file that stands as it was recorded: neither has anything said
            // about it.
            _ => string.Empty,
        };

    /// <summary>
    /// What a merge in progress is said as, which names the variant it is waiting for.
    /// </summary>
    /// <remarks>
    /// The panel's words for a merge name the variant, because that is the one thing about a merge no column of
    /// its row carries — and what a reader who has stopped at a mark on a file wants to know is which variant is
    /// holding it up. The variant is read from the record the entry is the target of, which is the other half of
    /// the record whose variant file wears the same sentence; a file the reading calls merging with no record
    /// naming it as a target is said the way the state alone can say it.
    /// </remarks>
    /// <param name="reading">The reading.</param>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>What to say.</returns>
    private static string Merging(RolaStatus reading, string path) =>
        reading.Merging.FirstOrDefault(
            merge => StatusReadings.Names(reading.Root, merge.Target, path)
        ) is { } merge
            ? RolaI18N.Get("rorolala_vcs.variant.pending", VariantFiles.Head(merge.Variant))
            : Words(PathState.Merging);

    /// <summary>
    /// What a change that is wrong is said as, which is which of the two ways it is wrong.
    /// </summary>
    /// <remarks>
    /// A change is wrong because another account holds it, or because the version it is based on is one
    /// upstream has moved past, and the reading names the paths of each: the state says only that something is
    /// wrong, so the list is what tells the two apart. A path in neither list is a wrong change the reading
    /// came to another way, which is answered with the listing's own word for it rather than with a reason that
    /// may not be the one.
    /// </remarks>
    /// <param name="reading">The reading.</param>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>What to say.</returns>
    private static string Wrong(RolaStatus reading, string path)
    {
        if (reading.Unowned.Any(named => StatusReadings.Names(reading.Root, named, path)))
        {
            return RolaI18N.Get("rorolala_vcs.status.change_wrong_unowned");
        }

        if (reading.WrongBase.Any(named => StatusReadings.Names(reading.Root, named, path)))
        {
            return RolaI18N.Get("rorolala_vcs.status.change_wrong_base");
        }

        return RolaI18N.Get("rorolala_vcs.state.wrong");
    }

    /// <summary>
    /// What a state is drawn as: a dot in its colour, and the word for it said on the dot.
    /// </summary>
    /// <remarks>
    /// The colour is this plugin's own — see <see cref="RorolalaColours"/> — and the line around the mark is a
    /// shade of it chosen by the ground, since a mark that is its own edge on one ground is a smudge on the
    /// other. The words are handed in rather than worked out here, because what a state means is the reading's
    /// to say and what it is drawn as is this mark's: one function answers a colour, the other a sentence.
    /// </remarks>
    /// <param name="state">What the reading says about the entry.</param>
    /// <param name="words">What that state is called.</param>
    /// <returns>What the mark is, which is nothing for a state a listing has no entry for.</returns>
    internal static EntryColumnMark Say(PathState state, string words) =>
        state switch
        {
            PathState.Untagged => Dot(RorolalaColours.Untagged, words),
            PathState.Modified => Dot(RorolalaColours.UnderWay, words),
            PathState.Wrong => Dot(RorolalaColours.Wrong, words),
            PathState.Renamed => Dot(RorolalaColours.Moved, words),
            PathState.RenamedLikely => Dot(RorolalaColours.Moved, words),
            PathState.Merging => Dot(RorolalaColours.UnderWay, words),
            _ => new EntryColumnMark(),
        };

    /// <summary>
    /// What a state is worn as on a tile: a picture in the state's own colour, on the badge's ground.
    /// </summary>
    /// <remarks>
    /// One picture per state, and the two that are alike share one: a file that is only possibly renamed is
    /// renamed as far as what a reader does with it goes, and one that changed and one waiting to be joined
    /// are both content on its way to a version — so the mark is the same and what tells them apart is the
    /// words on it. The colours are the ones the dots are, since what a state is coloured is the state and not
    /// the shape it is drawn in.
    /// </remarks>
    /// <param name="state">What the reading says about the entry.</param>
    /// <param name="words">What that state is called, which is what the badge is hovered for.</param>
    /// <returns>What the badge is, which is nothing for a state a listing has no entry for.</returns>
    internal static EntryMark Badge(PathState state, string words) =>
        state switch
        {
            PathState.Untagged => Worn(Icons.Add, RorolalaColours.Untagged, words),
            PathState.Modified => Worn(Icons.EditNote, RorolalaColours.UnderWay, words),
            PathState.Wrong => Worn(Icons.PriorityHigh, RorolalaColours.Wrong, words),
            PathState.Renamed => Worn(Icons.ArrowRightAlt, RorolalaColours.Moved, words),
            PathState.RenamedLikely => Worn(Icons.ArrowRightAlt, RorolalaColours.Moved, words),
            PathState.Merging => Worn(Icons.EditNote, RorolalaColours.UnderWay, words),
            _ => new EntryMark(),
        };

    /// <summary>One state, as the mark it wears.</summary>
    /// <param name="colour">What the state is coloured.</param>
    /// <param name="said">What it is called, which is what the mark is hovered for.</param>
    /// <returns>The mark.</returns>
    private static EntryColumnMark Dot(Color colour, string said) =>
        new(
            Ink: MarkInk.Quiet,
            Dot: true,
            Hint: said,
            Colour: colour,
            Edge: RorolalaColours.Edge(colour)
        );

    /// <summary>One state, as the badge it wears on a tile.</summary>
    /// <remarks>
    /// No ink role is named, because the badge is painted in the state's own colour rather than in one of the
    /// look's: the colour is the whole of what the picture and the ring around it are drawn in, and a role
    /// beside it would be a second answer about the same mark.
    /// </remarks>
    /// <param name="icon">The key of the picture the set draws for it.</param>
    /// <param name="colour">What the state is coloured.</param>
    /// <param name="words">What that state is called, which is what the badge is hovered for.</param>
    /// <returns>The mark.</returns>
    private static EntryMark Worn(string icon, Color colour, string words) =>
        new(IconKey: icon, Colour: colour, Tagged: true, Text: words);

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the standing column, so this may be the second letting go of the same
    /// set: what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

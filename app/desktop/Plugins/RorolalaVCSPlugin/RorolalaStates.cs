using Avalonia.Media;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to what a file of the work is: the State mark at the left of a directory's list.
/// </summary>
/// <remarks>
/// What it reads is a reading of the work, through <see cref="RolaStatus"/> — nothing is reached for, and a
/// reading is what the tree held when it was made. One reading answers for the whole directory being
/// listed, and one path is asked of it rather than answered from a copy of its lists: which state a path is
/// in is a judgement about the reading, and it is the library's to make.
/// <para>
/// What it contributes is a dot rather than a column of words. A file is in one of a handful of states, and
/// which one is read from the colour before any word of it is read: a column of the same few words beside
/// every name would be a column of text where a mark does, and would take the room the names are read in.
/// The words are the dot's own, said when the pointer rests on it.
/// </para>
/// </remarks>
internal sealed class RorolalaStates : IEntryColumnProvider
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
        // What a state is about is a file of the work, so an entry that names no directory of its own — the
        // way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || _readings.Reading(directory) is not { } status
        )
        {
            return new EntryColumnMark();
        }

        try
        {
            return Say(status.StateOf(entry.Path));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new EntryColumnMark();
        }
    }

    /// <summary>
    /// What a state is drawn as: a dot in its colour, and the word for it said on the dot.
    /// </summary>
    /// <remarks>
    /// The colour is this plugin's own — see <see cref="RorolalaColours"/> — and the line around the mark is a
    /// shade of it chosen by the ground, since a mark that is its own edge on one ground is a smudge on the
    /// other. A state that is wrong says only that it is wrong: which of the two ways it is wrong is the
    /// Overview's to say, where there is room for a reason.
    /// <para>
    /// The words are the dot's, and they are asked for here rather than handed over as keys for the listing
    /// to translate: they are this plugin's own, in this plugin's catalogue, and a key named by a literal
    /// where the gate can see it is a key that cannot go missing without the gate saying so.
    /// </para>
    /// </remarks>
    /// <param name="state">What the reading says about the entry.</param>
    /// <returns>What the mark is, which is nothing for a state a listing has no entry for.</returns>
    internal static EntryColumnMark Say(PathState state) =>
        state switch
        {
            PathState.Untagged => Dot(
                RorolalaColours.Untagged,
                RolaI18N.Get("rorolala_vcs.state.untagged")
            ),
            PathState.Modified => Dot(
                RorolalaColours.UnderWay,
                RolaI18N.Get("rorolala_vcs.state.modified")
            ),
            PathState.Wrong => Dot(RorolalaColours.Wrong, RolaI18N.Get("rorolala_vcs.state.wrong")),
            PathState.Renamed => Dot(RorolalaColours.Moved, RolaI18N.Get("rorolala_vcs.state.renamed")),
            PathState.RenamedLikely => Dot(
                RorolalaColours.Moved,
                RolaI18N.Get("rorolala_vcs.state.renamed_likely")
            ),
            PathState.Merging => Dot(
                RorolalaColours.UnderWay,
                RolaI18N.Get("rorolala_vcs.state.merging")
            ),

            // A path the Layout names and the tree does not hold is not an entry of any listing, and a path
            // the reading says nothing about is a file that stands as it was recorded: neither has a mark
            // beside a name, which is what a mark with nothing in it is.
            _ => new EntryColumnMark(),
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

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the standing column, so this may be the second letting go of the same
    /// set: what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

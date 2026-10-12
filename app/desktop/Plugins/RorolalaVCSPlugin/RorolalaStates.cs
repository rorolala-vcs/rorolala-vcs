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

    /// <summary>What was read for each directory, so a listing costs one read rather than one per row.</summary>
    private readonly Dictionary<string, RolaStatus?> _readings = new(StringComparer.Ordinal);

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the provider.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    public RorolalaStates(ILog log) => _log = log;

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
    public bool Applies(string directory) => Reading(directory) is not null;

    /// <inheritdoc />
    public EntryColumnMark Value(Entry entry)
    {
        // What a state is about is a file of the work, so an entry that names no directory of its own — the
        // way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || Reading(directory) is not { } status
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
            Warn($"what the work says about `{entry.Path}`", error);

            return new EntryColumnMark();
        }
    }

    /// <summary>
    /// What a state is drawn as: a dot in its colour, and the word for it said on the dot.
    /// </summary>
    /// <remarks>
    /// The colour is the role the listing resolves to the theme's resource for it — green for what appeared,
    /// the error red for what must not be recorded, the second colour for what moved, and the one amber for
    /// what is under way: a change of this side's to record, and a merge waiting to be joined. A state that is
    /// wrong says only that it is wrong: which of the two ways it is wrong is the Overview's to say, where
    /// there is room for a reason.
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
            PathState.Untagged => new EntryColumnMark(
                Ink: MarkInk.Add,
                Dot: true,
                Hint: RolaI18N.Get("rorolala_vcs.state.untagged")
            ),
            PathState.Modified => new EntryColumnMark(
                Ink: MarkInk.Warn,
                Dot: true,
                Hint: RolaI18N.Get("rorolala_vcs.state.modified")
            ),
            PathState.Wrong => new EntryColumnMark(
                Ink: MarkInk.Error,
                Dot: true,
                Hint: RolaI18N.Get("rorolala_vcs.state.wrong")
            ),
            PathState.Renamed => new EntryColumnMark(
                Ink: MarkInk.Accent,
                Dot: true,
                Hint: RolaI18N.Get("rorolala_vcs.state.renamed")
            ),
            PathState.RenamedLikely => new EntryColumnMark(
                Ink: MarkInk.Accent,
                Dot: true,
                Hint: RolaI18N.Get("rorolala_vcs.state.renamed_likely")
            ),
            PathState.Merging => new EntryColumnMark(
                Ink: MarkInk.Warn,
                Dot: true,
                Hint: RolaI18N.Get("rorolala_vcs.state.merging")
            ),

            // A path the Layout names and the tree does not hold is not an entry of any listing, and a path
            // the reading says nothing about is a file that stands as it was recorded: neither has a mark
            // beside a name, which is what a mark with nothing in it is.
            _ => new EntryColumnMark(),
        };

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// What was read is a snapshot of the work, and a snapshot is what the tree held at one moment: a file
    /// operation changes what a path is — it may move it, or change what it holds — so the reading is given
    /// back rather than kept across one.
    /// </remarks>
    public void Forget()
    {
        foreach (var status in _readings.Values)
        {
            status?.Dispose();
        }

        _readings.Clear();
    }

    /// <summary>The reading of the work a directory sits in, read once and kept.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The reading, or nothing when the directory is not inside a Workspace that works in a Layout.</returns>
    private RolaStatus? Reading(string directory)
    {
        if (_readings.TryGetValue(directory, out var known))
        {
            return known;
        }

        RolaStatus? status = null;

        try
        {
            status = RolaStatus.Locate(directory);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"how the work under `{directory}` stands", error);
        }

        _readings[directory] = status;

        return status;
    }

    /// <summary>
    /// Says once that something could not be read.
    /// </summary>
    /// <remarks>
    /// A listing that will not appear because a state could not be read is a worse answer than one that
    /// appears without the column, and it is said once because a directory of a thousand entries would
    /// otherwise say the same thing a thousand times.
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

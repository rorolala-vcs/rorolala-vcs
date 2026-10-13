using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to where upstream names a file: the Upstream column of a directory dock's list.
/// </summary>
/// <remarks>
/// An entry is what this side and the Vault share, and the two name it at a path each: the work names it
/// where the file sits here, and the Vault names the same entry where it sits there. The two are the same
/// path until one side moves the file, which is what makes the column worth reading — a path that is not
/// the one beside it is a file that moved in the work.
/// <para>
/// What is read is the copy the Workspace has fetched, through <see cref="OwnershipReadings"/>, which the
/// Ownership column reads through too: nothing is reached for, so the column says what the Vault held when
/// it was last fetched. The column sits beside the name rather than after the listing's own, because what
/// it says is where the entry is — the other name of the same thing — and it is read with the name.
/// </para>
/// <para>
/// A variant file is the one file of the work neither Layout names — it holds content waiting to be joined
/// into a file the Layout does name — so what the column says about it is the variant it holds, in the
/// amber of work under way. That is where a reader finds out that a file they cannot otherwise place is a
/// merge waiting rather than something untracked, and it is said here because this is the column that names
/// a file's other name (see <see cref="VariantFiles"/>).
/// </para>
/// </remarks>
internal sealed class RorolalaUpstream : IEntryColumnProvider
{
    /// <summary>How wide the column starts, which is room for a path rather than for a word.</summary>
    private const double ColumnWidth = 200;

    /// <summary>What the Vault copy says about the entries being listed.</summary>
    private readonly OwnershipReadings _readings;

    /// <summary>What the work itself says, which is what a merge in progress is read from.</summary>
    private readonly StatusReadings _status;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the Vault copy says about the entries being listed.</param>
    /// <param name="status">What the work itself says about them.</param>
    public RorolalaUpstream(OwnershipReadings readings, StatusReadings status)
    {
        _readings = readings;
        _status = status;
    }

    /// <inheritdoc />
    public string Id => "rorolala.upstream";

    /// <inheritdoc />
    public string ColumnKey => "rorolala_vcs.column_upstream";

    /// <inheritdoc />
    public double Width => ColumnWidth;

    /// <inheritdoc />
    public ColumnPlacement Placement => ColumnPlacement.BesideName;

    /// <summary>The column is one of values: what it says about a file is a path.</summary>
    public bool Marks => false;

    /// <summary>
    /// Whether the column is there, which it is where there is an upstream to name a file or a merge to name
    /// one: a directory outside any Vault has no upstream, and a variant file in it is still the one file
    /// nothing else can place.
    /// </summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether the column is shown.</returns>
    public bool Applies(string directory) =>
        _readings.Knows(directory)
        || _status.Reading(directory) is { Merging.Count: > 0 };

    /// <inheritdoc />
    public EntryColumnMark Value(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        // What a variant file is called is read before where upstream names it, because upstream names it
        // nowhere: a path of the Layout's is what this column would otherwise have to say, and a file that
        // has none is a file the column has nothing to write for but the variant it holds.
        if (
            Path.GetDirectoryName(entry.Path) is { Length: > 0 } directory
            && _status.Reading(directory) is { } status
            && VariantFiles.At(status.Merging, status.Root, entry.Path) is { } merge
        )
        {
            return Named(merge);
        }

        return new EntryColumnMark(Text: _readings.Upstream(entry));
    }

    /// <summary>
    /// What a variant file is named as: the variant it holds, in the amber of work under way.
    /// </summary>
    /// <remarks>
    /// The colour is the one a merge is drawn in everywhere else — the state dot beside a name in a listing,
    /// the pill of the Overview's own row — rather than the listing's ordinary ink, because what it says is
    /// the same thing those say: something is waiting to be joined. What the file is for is on the cell's own
    /// words, so nothing is said on hover: a column of values is read, and a value that has to be hovered for
    /// is a value the column did not hold.
    /// </remarks>
    /// <param name="merge">The merge whose variant file it is.</param>
    /// <returns>What the cell says.</returns>
    internal static EntryColumnMark Named(Merge merge)
    {
        ArgumentNullException.ThrowIfNull(merge);

        return new EntryColumnMark(
            Text: RolaI18N.Get("rorolala_vcs.variant.named", VariantFiles.Head(merge.Variant)),
            Colour: RorolalaColours.UnderWay
        );
    }

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the Ownership column, and the state readings with everything else the
    /// work is read for, so this may be the second letting go of either set: what is already let go of is
    /// nothing to let go of again.
    /// </remarks>
    public void Forget()
    {
        _readings.Forget();
        _status.Forget();
    }
}

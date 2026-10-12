using FileSystemPlugin;
using RorolalaDesktop.Contract;

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
/// </remarks>
internal sealed class RorolalaUpstream : IEntryColumnProvider
{
    /// <summary>How wide the column starts, which is room for a path rather than for a word.</summary>
    private const double ColumnWidth = 200;

    /// <summary>What the Vault copy says about the entries being listed.</summary>
    private readonly OwnershipReadings _readings;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the Vault copy says about the entries being listed.</param>
    public RorolalaUpstream(OwnershipReadings readings) => _readings = readings;

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

    /// <inheritdoc />
    public bool Applies(string directory) => _readings.Knows(directory);

    /// <inheritdoc />
    public EntryColumnMark Value(Entry entry) => new(Text: _readings.Upstream(entry));

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the Ownership column, so this may be the second letting go of the same
    /// set: what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

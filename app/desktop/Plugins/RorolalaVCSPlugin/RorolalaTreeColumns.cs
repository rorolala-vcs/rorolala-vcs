using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Status and Staged columns a directory inside a rola-tree shows, where a file's Upstream, Standing
/// and Owner columns are.
/// </summary>
/// <remarks>
/// Inside a tree the Layout names the tree rather than the paths under it, so the three columns about an
/// entry of the work have nothing to say. What the reader is looking at is the tree's own work: whether a
/// path is new or changed, and how much of that the staging area holds. Both are read from the tree's
/// status, which is read once for the listing rather than once per row (see <see cref="TreeReadings"/>).
/// </remarks>
internal static class RorolalaTreeColumns
{
    /// <summary>Contributes the two columns and their badges.</summary>
    /// <param name="readings">The tree statuses the columns read.</param>
    public static void Declare(TreeReadings readings)
    {
        ArgumentNullException.ThrowIfNull(readings);

        var status = new RorolalaTreeStatusColumn(readings);
        var staged = new RorolalaTreeStagedColumn(readings);

        EntryColumns.Register(status);
        EntryColumns.Register(staged);
        EntryMarkProviders.Register(status);
        EntryMarkProviders.Register(staged);
    }
}

/// <summary>One of the two columns a path inside a tree has.</summary>
/// <remarks>
/// It is a column of values rather than of marks: the words are short, and they are what a reader reads
/// before the badge on a tile says the same thing in a colour. The badge is still contributed, because a
/// tile has no column to put the words in.
/// </remarks>
internal abstract class RorolalaTreeColumn : IEntryColumnProvider, IEntryMarkProvider
{
    /// <summary>How wide the columns start, which is room for the longest of their words.</summary>
    protected const double ColumnWidth = 96;

    /// <summary>The tree statuses the column reads.</summary>
    protected readonly TreeReadings Readings;

    /// <summary>Makes the column over the tree readings.</summary>
    /// <param name="readings">The tree statuses the column reads.</param>
    protected RorolalaTreeColumn(TreeReadings readings) => Readings = readings;

    /// <inheritdoc />
    public abstract string Id { get; }

    /// <inheritdoc />
    public abstract string ColumnKey { get; }

    /// <inheritdoc />
    public double Width => ColumnWidth;

    /// <inheritdoc />
    public ColumnPlacement Placement => ColumnPlacement.BeforeValues;

    /// <summary>The column is one of values: what it says about a path is a word.</summary>
    public bool Marks => false;

    /// <summary>
    /// Whether the column is there, which it is for a directory inside a tree and nowhere else.
    /// </summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether the column is shown.</returns>
    public bool Applies(string directory) => RolaTrees.RootOf(directory) is not null;

    /// <inheritdoc />
    public abstract EntryColumnMark Value(Entry entry);

    /// <inheritdoc />
    public abstract EntryMark Mark(Entry entry);

    /// <inheritdoc />
    public void Forget() => Readings.Forget();

    /// <summary>What a tree says about an entry, or nothing while the tree is being read.</summary>
    /// <param name="entry">The path inside the tree.</param>
    /// <returns>What it is and how much of it is staged, or nothing.</returns>
    protected TreeWork? Work(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return RolaTrees.RootOf(entry.Path) is { } tree
            && Readings.Reading(tree) is { } status
                ? RolaTreeWork.Of(status, tree, entry)
                : null;
    }
}

/// <summary>What a path inside a tree is: new, or changed.</summary>
internal sealed class RorolalaTreeStatusColumn : RorolalaTreeColumn
{
    /// <summary>Makes the column over the tree readings.</summary>
    /// <param name="readings">The tree statuses the column reads.</param>
    public RorolalaTreeStatusColumn(TreeReadings readings) : base(readings) { }

    /// <inheritdoc />
    public override string Id => "rorolala.tree.status";

    /// <inheritdoc />
    public override string ColumnKey => "rorolala_vcs.column_tree_status";

    /// <inheritdoc />
    public override EntryColumnMark Value(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Work(entry) is { } work
            ? new EntryColumnMark(
                TextKey: Key(work.Status),
                Colour: Colour(work.Status)
            )
            : new EntryColumnMark();
    }

    /// <inheritdoc />
    public override EntryMark Mark(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Work(entry) is { } work ? Badge(work.Status) : new EntryMark();
    }

    /// <summary>What a status is worn as on a tile.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The picture in the status's colour, on the badge's ground.</returns>
    internal static EntryMark Badge(TreeWorkStatus status) =>
        new(
            IconKey: status == TreeWorkStatus.Added ? Icons.Add : Icons.EditNote,
            Colour: Colour(status),
            Tagged: true,
            HoverKey: HoverKey(status)
        );

    /// <summary>The words a status is said as.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The i18n key of its words.</returns>
    private static string Key(TreeWorkStatus status) =>
        status == TreeWorkStatus.Added
            ? "rorolala_vcs.tree_work.added"
            : "rorolala_vcs.tree_work.modified";

    /// <summary>The sentence a status badge is hovered for.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The i18n key of its explanation.</returns>
    private static string HoverKey(TreeWorkStatus status) =>
        status == TreeWorkStatus.Added
            ? "rorolala_vcs.tree_work.added_hover"
            : "rorolala_vcs.tree_work.modified_hover";

    /// <summary>What a status is coloured.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The green of what is new, or the amber of what changed.</returns>
    private static Avalonia.Media.Color Colour(TreeWorkStatus status) =>
        status == TreeWorkStatus.Added
            ? RorolalaColours.Untagged
            : RorolalaColours.UnderWay;
}

/// <summary>How much of a path's work inside a tree the staging area holds.</summary>
internal sealed class RorolalaTreeStagedColumn : RorolalaTreeColumn
{
    /// <summary>Makes the column over the tree readings.</summary>
    /// <param name="readings">The tree statuses the column reads.</param>
    public RorolalaTreeStagedColumn(TreeReadings readings) : base(readings) { }

    /// <inheritdoc />
    public override string Id => "rorolala.tree.staged";

    /// <inheritdoc />
    public override string ColumnKey => "rorolala_vcs.column_tree_staged";

    /// <inheritdoc />
    public override EntryColumnMark Value(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Work(entry) is { } work
            ? new EntryColumnMark(
                TextKey: Key(work.Stage),
                Colour: Colour(work.Stage)
            )
            : new EntryColumnMark();
    }

    /// <inheritdoc />
    public override EntryMark Mark(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Work(entry) is { } work ? Badge(work.Stage) : new EntryMark();
    }

    /// <summary>What a stage is worn as on a tile.</summary>
    /// <param name="stage">The stage.</param>
    /// <returns>The circle in the stage's colour, on the badge's ground.</returns>
    internal static EntryMark Badge(TreeWorkStage stage) =>
        new(
            IconKey: Icon(stage),
            Colour: Colour(stage),
            Tagged: true,
            HoverKey: HoverKey(stage)
        );

    /// <summary>The words a stage is said as.</summary>
    /// <param name="stage">The stage.</param>
    /// <returns>The i18n key of its words.</returns>
    private static string Key(TreeWorkStage stage) =>
        stage switch
        {
            TreeWorkStage.Staged => "rorolala_vcs.tree_staged.all",
            TreeWorkStage.Partial => "rorolala_vcs.tree_staged.partial",
            _ => "rorolala_vcs.tree_staged.none",
        };

    /// <summary>The sentence a stage badge is hovered for.</summary>
    /// <param name="stage">The stage.</param>
    /// <returns>The i18n key of its explanation.</returns>
    private static string HoverKey(TreeWorkStage stage) =>
        stage switch
        {
            TreeWorkStage.Staged => "rorolala_vcs.tree_staged.all_hover",
            TreeWorkStage.Partial => "rorolala_vcs.tree_staged.partial_hover",
            _ => "rorolala_vcs.tree_staged.none_hover",
        };

    /// <summary>The picture a stage wears: a full, half or empty circle.</summary>
    /// <param name="stage">The stage.</param>
    /// <returns>The icon key.</returns>
    private static string Icon(TreeWorkStage stage) =>
        stage switch
        {
            TreeWorkStage.Staged => Icons.CheckCircle,
            TreeWorkStage.Partial => Icons.RadioButtonChecked,
            _ => Icons.RadioButtonUnchecked,
        };

    /// <summary>What a stage is coloured, when it has a colour of its own.</summary>
    /// <param name="stage">The stage.</param>
    /// <returns>The green of all, the amber of some, or nothing for none.</returns>
    private static Avalonia.Media.Color? Colour(TreeWorkStage stage) =>
        stage switch
        {
            TreeWorkStage.Staged => RorolalaColours.Untagged,
            TreeWorkStage.Partial => RorolalaColours.UnderWay,
            _ => null,
        };
}

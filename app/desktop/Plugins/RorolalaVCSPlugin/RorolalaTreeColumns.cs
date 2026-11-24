using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

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

        // The lamp is registered first so that it stands before the two columns of words: the columns a listing
        // keeps at its left are drawn in the order they were offered, and the colour is read before either.
        EntryColumns.Register(new RorolalaTreeLamp(readings));
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
            IconKey: status switch
            {
                TreeWorkStatus.Added => Icons.Plus,
                TreeWorkStatus.Wrong => Icons.ExclamationMark,
                _ => Icons.NotePencil,
            },
            Colour: Colour(status),
            Tagged: true,
            HoverKey: HoverKey(status)
        );

    /// <summary>The words a status is said as.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The i18n key of its words.</returns>
    private static string Key(TreeWorkStatus status) =>
        status switch
        {
            TreeWorkStatus.Added => "rorolala_vcs.tree_work.added",
            TreeWorkStatus.Wrong => "rorolala_vcs.tree_work.wrong",
            _ => "rorolala_vcs.tree_work.modified",
        };

    /// <summary>The sentence a status badge is hovered for.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The i18n key of its explanation.</returns>
    private static string HoverKey(TreeWorkStatus status) =>
        status switch
        {
            TreeWorkStatus.Added => "rorolala_vcs.tree_work.added_hover",
            TreeWorkStatus.Wrong => "rorolala_vcs.tree_work.wrong_hover",
            _ => "rorolala_vcs.tree_work.modified_hover",
        };

    /// <summary>What a status is coloured.</summary>
    /// <param name="status">The status.</param>
    /// <returns>The green of what is new, the amber of what changed, or the red of a wrong change.</returns>
    private static Avalonia.Media.Color Colour(TreeWorkStatus status) =>
        status switch
        {
            TreeWorkStatus.Added => RorolalaColours.Untagged,
            TreeWorkStatus.Wrong => RorolalaColours.Wrong,
            _ => RorolalaColours.UnderWay,
        };
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
            TreeWorkStage.Partial => Icons.RadioButton,
            _ => Icons.Circle,
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

/// <summary>
/// The lamp a directory inside a tree shows at the left of every row: what happened to the path, in a colour.
/// </summary>
/// <remarks>
/// The two columns beside it say the same thing in words, and a reader of a list of a few dozen rows reads the
/// colour of them before any word of them: this is the mark column a listing has of its own, drawn for the paths of
/// a tree, where the columns about an entry of the Layout have nothing to say.
/// <para>
/// Four colours and one dot rather than a colour and a word, because what a row is, is one of four things: new,
/// changed, named elsewhere by the record, or named by the record and not there. A path that changed and one that
/// holds work the staging area only partly has are both yellow, since the second is a degree of the first rather
/// than a state of its own.
/// </para>
/// </remarks>
internal sealed class RorolalaTreeLamp : IEntryColumnProvider
{
    /// <summary>How wide the column starts, which is room for a dot and the room around it.</summary>
    private const double ColumnWidth = 18;

    /// <summary>The tree statuses the lamp reads.</summary>
    private readonly TreeReadings _readings;

    /// <summary>Makes the lamp over the tree readings.</summary>
    /// <param name="readings">The tree statuses the lamp reads.</param>
    public RorolalaTreeLamp(TreeReadings readings) => _readings = readings;

    /// <inheritdoc />
    public string Id => "rorolala.tree.lamp";

    /// <summary>No key at all, since there is no word that would fit over a column of dots.</summary>
    /// <remarks>
    /// The listing heads a column by the key it names, and a column that names none is headed by nothing, the same
    /// way the listing heads its own picture column. What a dot means is said on the dot itself, which is where a
    /// reader looking at one is looking.
    /// </remarks>
    public string ColumnKey => string.Empty;

    /// <inheritdoc />
    public double Width => ColumnWidth;

    /// <summary>The column stands before everything, the picture included.</summary>
    public ColumnPlacement Placement => ColumnPlacement.Leftmost;

    /// <summary>The column is one of marks: what it says about a path is a colour.</summary>
    public bool Marks => true;

    /// <summary>Whether the column is shown, which is for a directory inside a tree and nowhere else.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether the column is shown.</returns>
    public bool Applies(string directory) => RolaTrees.RootOf(directory) is not null;

    /// <inheritdoc />
    public EntryColumnMark Value(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        // A path the record names and the work does not hold has a row of its own only as a virtual entry, and the
        // tree's status says nothing about it: the status is a reading of the work, and the work does not hold it.
        if (VirtualEntries.Find(entry.Path) is { State: VirtualEntryState.Lost })
        {
            return Dot(RorolalaColours.Wrong, RolaI18N.Get("rorolala_vcs.status.kind_lost"));
        }

        if (RolaTrees.RootOf(entry.Path) is not { } tree || _readings.Reading(tree) is not { } status)
        {
            return new EntryColumnMark();
        }

        return RolaTreeWork.Of(status, tree, entry) is { } work ? Say(work) : new EntryColumnMark();
    }

    /// <inheritdoc />
    public void Forget() => _readings.Forget();

    /// <summary>What a path of a tree is drawn as: a dot in the colour of what happened to it.</summary>
    /// <remarks>
    /// The words are the columns' own, said on the mark: a reader who has stopped at a colour is asking what it
    /// means, and the answer is the sentence the Status column would have written about the same path.
    /// </remarks>
    /// <param name="work">What the tree says about the path.</param>
    /// <returns>The dot and what it is hovered for.</returns>
    private static EntryColumnMark Say(TreeWork work) =>
        Dot(
            work.Status is TreeWorkStatus.Wrong || work.Shape is TreeWorkShape.Removed
                ? RorolalaColours.Wrong
                : work.Shape is TreeWorkShape.Moved
                    ? RorolalaColours.Moved
                    : work.Stage is TreeWorkStage.Partial
                        ? RorolalaColours.UnderWay
                        : work.Shape is TreeWorkShape.Added
                            ? RorolalaColours.Untagged
                            : RorolalaColours.UnderWay,
            Hover(work)
        );

    /// <summary>What the dot is hovered for.</summary>
    /// <param name="work">What the tree says about the path.</param>
    /// <returns>The sentence the colour is read through.</returns>
    private static string Hover(TreeWork work) =>
        RolaI18N.Get(
            work.Status is TreeWorkStatus.Wrong ? "rorolala_vcs.tree_work.wrong_hover"
            : work.Shape is TreeWorkShape.Removed ? "rorolala_vcs.tree_work.removed_hover"
            : work.Shape is TreeWorkShape.Moved ? "rorolala_vcs.tree_work.moved_hover"
            : work.Stage is TreeWorkStage.Partial ? "rorolala_vcs.tree_staged.partial_hover"
            : work.Shape is TreeWorkShape.Added ? "rorolala_vcs.tree_work.added_hover"
            : "rorolala_vcs.tree_work.modified_hover"
        );

    /// <summary>One state, as the dot it wears.</summary>
    /// <param name="colour">What the state is coloured.</param>
    /// <param name="said">What it is called, which is what the dot is hovered for.</param>
    /// <returns>The mark.</returns>
    private static EntryColumnMark Dot(Avalonia.Media.Color colour, string said) =>
        new(
            Ink: MarkInk.Quiet,
            Dot: true,
            Hint: said,
            Colour: colour,
            Edge: RorolalaColours.Edge(colour)
        );
}

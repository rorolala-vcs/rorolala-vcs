using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Overview dock: how the work stands beside the Layout it works from.
/// </summary>
/// <remarks>
/// One table of the changes, read as the file browser reads a directory: a band naming the columns over
/// rows of thirty pixels, told apart by a line. A row is one thing the reading says about one path — what
/// kind of change it is, where it comes from and where it goes, and what became of its content — so the
/// work is read down a column to see what is of a kind, and across a row to see what is of a path.
/// <para>
/// The words are this dock's own rather than the command line's, because the two are read differently: a
/// command's report is read once and acted on by typing, while a panel is looked at. What the two share
/// is the finding, which comes from the C ABI through <see cref="RolaStatus"/> and is read nowhere else.
/// </para>
/// </remarks>
internal sealed class OverviewDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly OverviewView _view;

    /// <summary>Makes the dock.</summary>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="files">What the files may have done, which the reading is made again for.</param>
    /// <param name="refocus">The host's windows being come back to, which the reading is made again for too.</param>
    public OverviewDock(ILog log, IFileChanges files, IRefocus refocus) =>
        _view = new OverviewView(log, files, refocus);

    /// <inheritdoc />
    public Control View => _view;

    /// <summary>
    /// What the dock's own strip offers about what it is showing.
    /// </summary>
    /// <remarks>
    /// One command, and it is the reading itself: a reader who has just saved a file, or who has just been
    /// told by another program that something moved, should not have to wait for the reading the dock makes
    /// on its own. What it does is what the dock does when the files change — read the work again — so
    /// there is nothing here that a reading made by itself would not have done.
    /// <para>
    /// It is drawn as its picture rather than as its word, and the picture is this plugin's own `refresh`:
    /// what the file browser's toolbar wears for reading a directory again is the same mark, and each plugin
    /// drawing its own is what keeps one plugin's keys its own.
    /// </para>
    /// </remarks>
    public IReadOnlyList<DockHeaderCommand> HeaderCommands =>
        [new DockHeaderCommand("rorolala_vcs.status.refresh", _view.Refresh, IconKey: Icons.Refresh)];
}

/// <summary>What where the tree is rooted comes to, as the Overview reads it.</summary>
/// <param name="Facts">The reading to draw, when there is one.</param>
/// <param name="Said">What to say instead, when there is not.</param>
internal sealed record OverviewReading(StatusFacts? Facts, string? Said);

/// <summary>
/// A reading as the table reads it: what to show, without the reading itself.
/// </summary>
/// <remarks>
/// It is what the table is built from, and it is a type of this plugin's own rather than
/// <see cref="RolaStatus"/> so that what a reading is turned into can be read — and checked — without a
/// Workspace to read one from: the native side is a handle that can only be made by looking at a tree.
/// </remarks>
/// <param name="Lost">Paths the Layout names that the tree does not hold.</param>
/// <param name="Untagged">Paths the tree holds that the Layout does not name.</param>
/// <param name="Renamed">Paths that moved.</param>
/// <param name="Modified">Paths that changed where they were, move destinations included.</param>
/// <param name="Unowned">Paths the fetched copy says another account holds.</param>
/// <param name="WrongBase">Paths whose version is behind or apart from the one upstream is at.</param>
/// <param name="Merging">Files a variant is waiting to be joined into.</param>
/// <param name="Broken">Merges that cannot go on, one reason each.</param>
internal sealed record StatusFacts(
    IReadOnlyList<string> Lost,
    IReadOnlyList<string> Untagged,
    IReadOnlyList<Rename> Renamed,
    IReadOnlyList<string> Modified,
    IReadOnlyList<string> Unowned,
    IReadOnlyList<string> WrongBase,
    IReadOnlyList<Merge> Merging,
    IReadOnlyList<BrokenReason> Broken
)
{
    /// <summary>Takes the facts out of a reading.</summary>
    /// <param name="status">The reading.</param>
    /// <returns>What the table is built from.</returns>
    public static StatusFacts Of(RolaStatus status)
    {
        ArgumentNullException.ThrowIfNull(status);

        return new StatusFacts(
            status.Lost,
            status.Untagged,
            status.Renamed,
            status.Modified,
            status.Unowned,
            status.WrongBase,
            status.Merging,
            status.Broken
        );
    }
}

/// <summary>One pill of a row's state: what it says, and the theme resource it is filled with.</summary>
/// <param name="Said">What it is called.</param>
/// <param name="Fill">The theme resource the pill is filled with.</param>
internal sealed record StatusPill(string Said, string Fill);

/// <summary>One row of the table: what the reading says about one path.</summary>
/// <param name="Pills">
/// What the path's state is, one pill each: what kind of change it is, and what became of its content.
/// A change of content alone has no kind, and a kind alone has nothing to say about a content, so a row
/// carries as many pills as it has states — one or two, and more where a merge has something to add.
/// </param>
/// <param name="From">The path it comes from, or nothing when it comes from nowhere.</param>
/// <param name="To">The path it goes to, or nothing when it goes nowhere.</param>
internal sealed record TableRow(IReadOnlyList<StatusPill> Pills, string From, string To);

/// <summary>
/// What the Overview shows: the work as it stands, read where the tree is rooted.
/// </summary>
/// <remarks>
/// The reading is made when the dock appears and whenever something says it may have changed — the tree
/// being rooted elsewhere, the files having been changed, or one of the host's windows being come back to.
/// Nothing polls: a reading is a walk of the tree, and one made on a timer would spend most of its time
/// saying what the last one said.
/// <para>
/// The events it listens to are not all raised on the window's thread — a plugin that changed a file says
/// so from the work it did it on — so what is drawn is drawn through the dispatcher. A control belongs to
/// the thread that made it, and this one is made by the host.
/// </para>
/// </remarks>
internal sealed class OverviewView : UserControl
{
    /// <summary>How wide the line said in place of a reading is allowed to read.</summary>
    private const double Widest = 420;

    /// <summary>How tall a row is, which is the height the browser gives one.</summary>
    private const double RowHeight = 30.0;

    /// <summary>How far a band and its rows are inset, which the browser insets its own by.</summary>
    private const double Inset = 12.0;

    /// <summary>How wide the column saying what each path's state is.</summary>
    /// <remarks>
    /// Wide enough for two pills of the longest pairs the panel draws — a move that is a certainty and a
    /// base upstream has moved past — on one line, and no wider: what a column holds is bounded, and what
    /// is left is what the paths are read in. A pair too long for it wraps, and the row grows.
    /// </remarks>
    private const double StateColumn = 268;

    /// <summary>How round the ends of a pill are, which is half of how tall one is.</summary>
    private const double PillRadius = 10;

    /// <summary>The theme resource for what has gone, or gone wrong.</summary>
    private const string WrongFill = "rorolala.del";

    /// <summary>The theme resource for what has appeared.</summary>
    private const string AppearedFill = "rorolala.add";

    /// <summary>The theme resource for what moved, or is being joined.</summary>
    private const string MovedFill = "rorolala.primary";

    /// <summary>The theme resource for what must be read twice.</summary>
    private const string CarefulFill = "rorolala.warn";

    /// <summary>The theme resource for what has nothing to say about itself.</summary>
    private const string QuietFill = "rorolala.fg.faint";

    /// <summary>The theme resource a line told apart from the next is drawn in.</summary>
    private const string EdgeInk = "rorolala.border";

    /// <summary>The theme resource a band is drawn on, which is the ground a heading sits on.</summary>
    private const string BandGround = "rorolala.bg.sunken";

    /// <summary>How much of a variant's hash is shown.</summary>
    private const int ShortVariant = 7;

    /// <summary>How often the work is read again while the dock is on screen.</summary>
    /// <remarks>
    /// Nothing tells the program that another program changed a file — an artist saving a picture changes
    /// the tree with this window hearing nothing of it — so a reading is asked for on a timer as well as
    /// when something says the files have changed. Five seconds is long enough that a reading is not what
    /// the program spends its time on, and short enough that a save made elsewhere is answered before the
    /// reader has looked away from the row it changes.
    /// </remarks>
    private static readonly TimeSpan Every = TimeSpan.FromSeconds(5);

    /// <summary>The line said when there is no reading to draw.</summary>
    private readonly TextBlock _notice;

    /// <summary>Where the table of a reading goes.</summary>
    private readonly StackPanel _blocks;

    /// <summary>Where a reading that could not be made is said.</summary>
    private readonly ILog _log;

    /// <summary>What the files may have done, which a reading is made again for.</summary>
    private readonly IFileChanges _files;

    /// <summary>The host's windows being come back to, which a reading is made again for too.</summary>
    private readonly IRefocus _refocus;

    /// <summary>What asks for a reading while nothing else does: a file changed by another program.</summary>
    private readonly DispatcherTimer _again;

    /// <summary>What is on screen now, so that a reading saying the same thing is not drawn again.</summary>
    private Shown? _shown;

    /// <summary>Makes the control.</summary>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="files">What the files may have done.</param>
    /// <param name="refocus">The host's windows being come back to.</param>
    public OverviewView(ILog log, IFileChanges files, IRefocus refocus)
    {
        _log = log;
        _files = files;
        _refocus = refocus;

        _notice = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            TextAlignment = TextAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            MaxWidth = Widest,
            Classes = { "muted" },
        };

        _blocks = new StackPanel();

        var root = new Grid();

        root.Children.Add(
            new ScrollViewer
            {
                Content = _blocks,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            }
        );
        root.Children.Add(_notice);

        Content = root;

        _again = new DispatcherTimer { Interval = Every };
        _again.Tick += (_, _) =>
        {
            // A dock that is open but in a tab that is not shown is not being looked at: a reading is a walk
            // of the tree, and one made for a window nobody has in front of them is work for nothing.
            if (IsEffectivelyVisible)
            {
                Draw();
            }
        };

        // Listening while it is on screen rather than for as long as it exists: the host's windows and the
        // file changes outlive every dock, and a dock that was closed has nothing to draw.
        AttachedToVisualTree += (_, _) => Listen(true);
        DetachedFromVisualTree += (_, _) => Listen(false);

        Draw();
    }

    /// <summary>Reads the work again and draws it, which is what the dock's own command asks for.</summary>
    public void Refresh() => Draw();

    /// <summary>
    /// What the Overview makes of a directory the tree is rooted at.
    /// </summary>
    /// <remarks>
    /// A directory that has not been said, or that is the computer — neither of which is somewhere a
    /// Workspace can be found from — is answered the way a directory no Workspace holds is. A Workspace
    /// that holds it and still answers nothing is told apart from that, because the two are different
    /// things to have happened: nothing is being looked at, or the work that is being looked at could not
    /// be read.
    /// </remarks>
    /// <param name="directory">The directory the tree is rooted at.</param>
    /// <param name="log">Where a read that failed is said.</param>
    /// <returns>The reading, or the words to show instead of one.</returns>
    public static OverviewReading Of(string? directory, ILog log)
    {
        ArgumentNullException.ThrowIfNull(log);

        if (directory is not { Length: > 0 } at)
        {
            return new OverviewReading(null, RolaI18N.Get("rorolala_vcs.not_a_workspace"));
        }

        try
        {
            // The native reading is given back as soon as what it said has been taken out of it: the panel
            // wants the lists, and a handle kept for questions nobody asks is a handle kept for nothing.
            using var status = RolaStatus.Locate(at);

            if (status is not null)
            {
                return new OverviewReading(StatusFacts.Of(status), null);
            }

            return new OverviewReading(
                null,
                WorkspaceNotice.Holds(at, log)
                    ? RolaI18N.Get("rorolala_vcs.status_unreadable")
                    : RolaI18N.Get("rorolala_vcs.not_a_workspace")
            );
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            // A run without the native library beside it reads the same as one whose work could not be
            // read, so the two are said in the log rather than by a second line on the window: only one
            // of them is a reason to go looking.
            log.Warn($"how the work under `{at}` stands could not be read: {error.Message}");

            return new OverviewReading(null, RolaI18N.Get("rorolala_vcs.status_unreadable"));
        }
    }

    /// <summary>
    /// The changes a reading holds, one to a row, in the order the table reads them.
    /// </summary>
    /// <remarks>
    /// Four groups, read in the order they are acted on: what is wrong, what is under way, what the Layout
    /// and the tree disagree about, and last the work that is simply to be recorded. Within the wrong ones
    /// another account's comes first, since those are the ones that must not be recorded, and within the
    /// moves a likeness before the bytes, since a guess is the one a reader is likelier to have to act on.
    /// <para>
    /// A row stands in one group, and it is the group of the state that has to be acted on first: a move
    /// whose content is wrong is read among the wrong changes rather than among the moves, because what is
    /// wrong is what a reader has to do something about.
    /// </para>
    /// <para>
    /// A move's destination is already a row among the moves, so it is not also a row among the changes of
    /// content: one file that moved and was edited is one row, not two. What became of its content is said
    /// on the row it is already on.
    /// </para>
    /// </remarks>
    /// <param name="facts">The reading.</param>
    /// <returns>The rows to show.</returns>
    internal static IReadOnlyList<TableRow> Rows(StatusFacts facts)
    {
        ArgumentNullException.ThrowIfNull(facts);

        // Read once and used for every row, since a table is one drawing: what a kind is called is the same
        // word on every row it names. Every one is asked for by a literal so that the gate can see it.
        var lost = RolaI18N.Get("rorolala_vcs.status.kind_lost");
        var untagged = RolaI18N.Get("rorolala_vcs.status.kind_untagged");
        var weak = RolaI18N.Get("rorolala_vcs.status.kind_rename_weak");
        var strong = RolaI18N.Get("rorolala_vcs.status.kind_rename_strong");
        var merge = RolaI18N.Get("rorolala_vcs.status.kind_merge");
        var edited = RolaI18N.Get("rorolala_vcs.status.change_modified");
        var unchanged = RolaI18N.Get("rorolala_vcs.status.change_unchanged");
        var notMine = RolaI18N.Get("rorolala_vcs.status.change_wrong_unowned");
        var stale = RolaI18N.Get("rorolala_vcs.status.change_wrong_base");
        var variant = RolaI18N.Get("rorolala_vcs.status.merge_variant");
        var stopped = RolaI18N.Get("rorolala_vcs.status.merge_broken");

        List<TableRow> wrong = [];
        List<TableRow> merges = [];
        List<TableRow> structural = [];
        List<TableRow> changes = [];

        var destinations = facts.Renamed.Select(rename => rename.To).ToHashSet(StringComparer.Ordinal);
        var behind = facts.WrongBase.ToHashSet(StringComparer.Ordinal);

        wrong.AddRange(
            facts.Unowned.Select(path =>
                new TableRow([new StatusPill(notMine, WrongFill)], path, string.Empty)
            )
        );

        var changed = facts.Modified.Where(path => !destinations.Contains(path)).ToList();

        wrong.AddRange(
            changed
                .Where(behind.Contains)
                .Select(path => new TableRow([new StatusPill(stale, CarefulFill)], path, string.Empty))
        );

        structural.AddRange(
            facts.Lost.Select(path =>
                new TableRow([new StatusPill(lost, WrongFill)], path, string.Empty)
            )
        );

        structural.AddRange(
            facts.Untagged.Select(path =>
                new TableRow([new StatusPill(untagged, AppearedFill)], string.Empty, path)
            )
        );

        // A move is read where the content it carries has to be acted on: one that was edited against a
        // version upstream has moved past is a wrong change before it is a move.
        foreach (var rename in facts.Renamed.Where(rename => !rename.Strong))
        {
            (Misfit(rename) ? wrong : structural).Add(Move(rename, weak));
        }

        foreach (var rename in facts.Renamed.Where(rename => rename.Strong))
        {
            (Misfit(rename) ? wrong : structural).Add(Move(rename, strong));
        }

        merges.AddRange(
            facts.Merging.Select(item =>
                new TableRow(
                    [
                        new StatusPill(merge, MovedFill),
                        new StatusPill($"{variant}{Short(item.Variant)}", QuietFill),
                        .. item.Moved is { Length: > 0 } to
                            ? [new StatusPill(RolaI18N.Get("rorolala_vcs.status.merge_moved", to), QuietFill)]
                            : Array.Empty<StatusPill>(),
                    ],
                    item.Target,
                    item.Path
                )
            )
        );

        merges.AddRange(
            facts.Broken.Select(reason =>
                new TableRow(
                    [
                        // A merge that cannot go on is a thing gone wrong rather than work under way, so its
                        // pill wears the colour of what is wrong rather than the colour of a merge that is.
                        new StatusPill(merge, WrongFill),
                        new StatusPill($"{stopped}{BrokenSaid(reason)}", CarefulFill),
                    ],
                    reason.Target,
                    BrokenPlace(reason)
                )
            )
        );

        changes.AddRange(
            changed
                .Where(path => !behind.Contains(path))
                .Select(path => new TableRow([new StatusPill(edited, MovedFill)], path, string.Empty))
        );

        return [.. wrong, .. merges, .. structural, .. changes];

        // Whether a move's content is a change that has to be acted on rather than one that is simply to be
        // recorded, which is what decides the group it is read in.
        bool Misfit(Rename rename) => rename.Modified && behind.Contains(rename.To);

        // One move, as a row. A move whose file was edited is a change of content as well, so what became of
        // the content is a pill of the move's own row rather than a row of its own.
        TableRow Move(Rename rename, string kind) =>
            new(
                [
                    new StatusPill(kind, MovedFill),
                    !rename.Modified
                        ? new StatusPill(unchanged, QuietFill)
                        : behind.Contains(rename.To)
                            ? new StatusPill(stale, CarefulFill)
                            : new StatusPill(edited, MovedFill),
                ],
                rename.From,
                rename.To
            );
    }

    /// <summary>Starts or stops listening for what a reading goes stale from.</summary>
    /// <param name="on">Whether to listen.</param>
    private void Listen(bool on)
    {
        if (on)
        {
            BaseDirectory.Changed += Draw;
            _files.Changed += Draw;
            _refocus.Regained += Draw;
            _again.Start();
        }
        else
        {
            BaseDirectory.Changed -= Draw;
            _files.Changed -= Draw;
            _refocus.Regained -= Draw;
            _again.Stop();
        }

        Draw();
    }

    /// <summary>Reads the work again and draws it, on the thread the control belongs to.</summary>
    private void Draw() => Dispatcher.UIThread.Post(Drawn);

    /// <summary>Reads the work again and draws it.</summary>
    private void Drawn()
    {
        var reading = Of(BaseDirectory.Current, _log);
        var rows = reading.Facts is { } facts ? Rows(facts) : [];
        var said = Line(reading, rows.Count);

        // What would be drawn is what is already drawn, so nothing is drawn: the table is the whole of the
        // dock, and building it again throws away what the reader has scrolled to, chosen and is pointing at.
        // A reading is asked for on a timer, so all but a few of them say what the last one said.
        if (_shown is { } shown && shown.Said == said && Same(shown.Rows, rows))
        {
            return;
        }

        _shown = new Shown(said, rows);

        _blocks.Children.Clear();

        _notice.Text = said;
        _notice.IsVisible = said is { Length: > 0 };

        if (rows.Count > 0)
        {
            _blocks.Children.Add(Table(rows));
        }
    }

    /// <summary>
    /// What the dock says in the middle, which is said only where the table has nothing to show.
    /// </summary>
    /// <remarks>
    /// A line over a table is a line over the changes, and the two say different things at once: a work with
    /// changes is the table and nothing else, and a work without them is the words — that there is nothing to
    /// confirm, or that a work there is could not be read.
    /// </remarks>
    /// <param name="reading">What was read where the tree is rooted.</param>
    /// <param name="rows">How many rows the reading came to.</param>
    /// <returns>What to say, or nothing when the table is what is drawn.</returns>
    internal static string? Line(OverviewReading reading, int rows)
    {
        ArgumentNullException.ThrowIfNull(reading);

        return rows > 0 ? null : reading.Said ?? RolaI18N.Get("rorolala_vcs.status.clean");
    }

    /// <summary>
    /// Whether two readings would be drawn the same, row for row.
    /// </summary>
    /// <remarks>
    /// The rows are built afresh on every reading, so what tells one reading from another is what the rows
    /// say rather than the values they happen to be. A row holding a list of pills is where that is easy to
    /// get wrong — two lists that say the same thing are not the same list — and getting it wrong means a
    /// table rebuilt for nothing on every reading.
    /// </remarks>
    /// <param name="left">What one reading came to.</param>
    /// <param name="right">What another came to.</param>
    /// <returns>Whether the two are the same drawing.</returns>
    internal static bool Same(IReadOnlyList<TableRow> left, IReadOnlyList<TableRow> right)
    {
        ArgumentNullException.ThrowIfNull(left);
        ArgumentNullException.ThrowIfNull(right);

        return left.Count == right.Count
            && left.Zip(right).All(pair => Same(pair.First, pair.Second));
    }

    /// <summary>Whether two rows would be drawn the same.</summary>
    /// <param name="left">One row.</param>
    /// <param name="right">Another.</param>
    /// <returns>Whether the two are the same drawing.</returns>
    private static bool Same(TableRow left, TableRow right) =>
        left.From == right.From
        && left.To == right.To
        && left.Pills.Count == right.Pills.Count
        && left.Pills.Zip(right.Pills).All(pair => pair.First == pair.Second);

    /// <summary>
    /// The changes as one table, which is the browser's list with a band over it.
    /// </summary>
    /// <remarks>
    /// The band names the columns and the rows carry the changes, and both are laid out by the same
    /// columns — the two fixed ones for what a kind is called and what became of a content, and two that
    /// share what is left for the paths. A path is one line however long it is: what a row cannot show is
    /// on the path itself, and a row that grew to two lines would put every row under it out of step with
    /// the ones beside it.
    /// </remarks>
    /// <param name="rows">The changes.</param>
    /// <returns>The table to show.</returns>
    private static Control Table(IReadOnlyList<TableRow> rows)
    {
        var list = new ListBox
        {
            ItemsSource = rows,
            Background = Brushes.Transparent,
            Padding = new Thickness(0),
            BorderThickness = new Thickness(0),
            ItemTemplate = new FuncDataTemplate<TableRow>((row, _) => Row(row), true),
        };

        // The row keeps the design's height, and its own inset is taken off so that the band and the rows
        // share one edge; the four pixels left between two rows are the line's room.
        list.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MarginProperty, new Thickness(0, 2, 0, 2)),
                    new Setter(Layoutable.MinHeightProperty, RowHeight),
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                },
            }
        );

        var panel = new DockPanel { LastChildFill = true };
        var band = Band();
        DockPanel.SetDock(band, Dock.Top);
        panel.Children.Add(band);
        panel.Children.Add(list);

        return panel;
    }

    /// <summary>The band naming the table's columns, which is where a reading is read from.</summary>
    /// <returns>The band to show.</returns>
    private static Control Band()
    {
        var heads = new Grid { ColumnDefinitions = Definitions() };

        Place(heads, Head(RolaI18N.Get("rorolala_vcs.status.column_state")), 0);
        Place(heads, Head(RolaI18N.Get("rorolala_vcs.status.column_from")), 1);
        Place(heads, Head(RolaI18N.Get("rorolala_vcs.status.column_to")), 2);

        var band = new Border
        {
            Child = heads,
            Padding = new Thickness(Inset, 7),
            MinHeight = RowHeight,
            BorderThickness = new Thickness(0, 0, 0, 1),
        };

        band[!Border.BackgroundProperty] = new DynamicResourceExtension(BandGround);
        band[!Border.BorderBrushProperty] = new DynamicResourceExtension(EdgeInk);

        return band;
    }

    /// <summary>One row of the table: what the path's state is, and the paths it is between.</summary>
    /// <param name="row">The row.</param>
    /// <returns>The row to show.</returns>
    private static Control Row(TableRow row)
    {
        var cells = new Grid { ColumnDefinitions = Definitions() };

        Place(cells, State(row), 0);
        Place(cells, Cell(row.From), 1);
        Place(cells, Cell(row.To), 2);

        var line = new Border
        {
            Child = cells,
            Padding = new Thickness(Inset, 0),
            BorderThickness = new Thickness(0, 0, 0, 1),
        };

        line[!Border.BorderBrushProperty] = new DynamicResourceExtension(EdgeInk);

        return line;
    }

    /// <summary>
    /// What a row's state is, as pills: the colour each is known by, with what it is called written on it.
    /// </summary>
    /// <remarks>
    /// Pills rather than words, because the state is what a reader scans the column for: the colour of a row
    /// is read before its letters are, and every state having one lets a reading be told at a glance from the
    /// kinds it holds and how many of each. What is written on a pill is white, which is what reads on the
    /// colours a state is known by.
    /// <para>
    /// One pill per state rather than one to a row: a move is a move and its content is unchanged, and the
    /// two are two things to know. They flow across the column and wrap where they do not fit, so a row with
    /// more to say grows rather than loses what it has not said.
    /// </para>
    /// <para>
    /// The panel is centred in its cell rather than left to fill it, since what a pill says about itself
    /// places it within its own line and nothing places the line: a panel that filled the cell would stack
    /// its lines from the top, and a row of pills would stand above the path beside it.
    /// </para>
    /// </remarks>
    /// <param name="row">The row.</param>
    /// <returns>The pills to show.</returns>
    private static Control State(TableRow row)
    {
        var state = new WrapPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Center,
        };

        foreach (var pill in row.Pills)
        {
            state.Children.Add(Pill(pill));
        }

        return state;
    }

    /// <summary>One pill: a rounded ground in the colour of its state, with the state written on it.</summary>
    /// <param name="pill">The pill.</param>
    /// <returns>The pill to show.</returns>
    private static Border Pill(StatusPill pill)
    {
        var words = new TextBlock
        {
            Text = pill.Said,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = Brushes.White,
            Classes = { "caption" },
        };

        var made = new Border
        {
            Child = words,
            CornerRadius = new CornerRadius(PillRadius),
            Padding = new Thickness(8, 2),
            // The room a pill keeps around it is its own rather than the panel's, since a panel spaces lines
            // and not the things on them: what wraps onto the next line has to be told from the line above.
            Margin = new Thickness(0, 2, 6, 2),
            VerticalAlignment = VerticalAlignment.Center,
        };

        made[!Border.BackgroundProperty] = new DynamicResourceExtension(pill.Fill);

        return made;
    }

    /// <summary>The columns every row and the band are laid out by.</summary>
    /// <returns>The width of each column.</returns>
    private static ColumnDefinitions Definitions()
    {
        var columns = new ColumnDefinitions();

        columns.Add(new ColumnDefinition(new GridLength(StateColumn)));
        columns.Add(new ColumnDefinition(new GridLength(1, GridUnitType.Star)));
        columns.Add(new ColumnDefinition(new GridLength(1, GridUnitType.Star)));

        return columns;
    }

    /// <summary>Puts one cell in its column.</summary>
    /// <param name="grid">The row or band.</param>
    /// <param name="cell">What the column shows.</param>
    /// <param name="column">Which column it is.</param>
    private static void Place(Grid grid, Control cell, int column)
    {
        Grid.SetColumn(cell, column);
        grid.Children.Add(cell);
    }

    /// <summary>One column's name, over the column it names.</summary>
    /// <param name="said">What it is called.</param>
    /// <returns>The line to show.</returns>
    private static TextBlock Head(string said) =>
        new()
        {
            Text = said.ToUpperInvariant(),
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            Classes = { "label" },
        };

    /// <summary>One cell of a row holding a path, or nothing where the path is not one.</summary>
    /// <param name="said">What it says, which may be nothing.</param>
    /// <returns>The cell to show.</returns>
    private static TextBlock Cell(string said) =>
        new()
        {
            // A path is one line however long it is: what the column cannot show is on the path itself, and a
            // cell that wrapped would put the row's own height out of step with the pills beside it.
            Text = said,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            Classes = { "caption", "mono" },
        };

    /// <summary>Where the variant file of a merge that cannot go on lies, when the reason names it.</summary>
    /// <param name="reason">The reason.</param>
    /// <returns>The path, or nothing.</returns>
    private static string BrokenPlace(BrokenReason reason) =>
        reason.Kind
            is BrokenKind.FileGone
                or BrokenKind.NoIndex
                or BrokenKind.Unreadable
                or BrokenKind.Changed
            ? reason.What
            : string.Empty;

    /// <summary>What a merge that cannot go on is said as.</summary>
    /// <remarks>
    /// The reason crosses as a kind and what it is about, and the words are put to it here: a form is the
    /// language's, and the reading has none.
    /// </remarks>
    /// <param name="reason">The reason.</param>
    /// <returns>What to say.</returns>
    private static string BrokenSaid(BrokenReason reason) =>
        reason.Kind switch
        {
            BrokenKind.NoTarget => RolaI18N.Get("rorolala_vcs.status.broken_no_target", reason.What),
            BrokenKind.NoPlace => RolaI18N.Get("rorolala_vcs.status.broken_no_place", reason.What),
            BrokenKind.FileGone => RolaI18N.Get("rorolala_vcs.status.broken_file_gone", reason.What),
            BrokenKind.NoIndex => RolaI18N.Get("rorolala_vcs.status.broken_no_index", reason.What),
            BrokenKind.NoVariant => RolaI18N.Get(
                "rorolala_vcs.status.broken_no_variant",
                reason.What
            ),
            BrokenKind.Unreadable => RolaI18N.Get(
                "rorolala_vcs.status.broken_unreadable",
                reason.What
            ),
            BrokenKind.Changed => RolaI18N.Get("rorolala_vcs.status.broken_changed", reason.What),
            _ => RolaI18N.Get("rorolala_vcs.status.broken_unknown", reason.What),
        };

    /// <summary>How much of a variant's hash is shown.</summary>
    /// <param name="variant">The hash as the reading gives it.</param>
    /// <returns>The head of it, which is what a person recognises a variant by.</returns>
    private static string Short(string variant) =>
        variant.Length > ShortVariant ? variant[..ShortVariant] : variant;

    /// <summary>What the dock is showing, which is what a reading is compared against.</summary>
    /// <param name="Said">The line said in place of a reading, or nothing when there is one.</param>
    /// <param name="Rows">The rows the reading came to, which are none when it says a work is clean.</param>
    private sealed record Shown(string? Said, IReadOnlyList<TableRow> Rows);
}

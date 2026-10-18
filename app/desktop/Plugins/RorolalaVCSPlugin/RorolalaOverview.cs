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
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="files">What the files may have done, which the reading is made again for.</param>
    /// <param name="refocus">The host's windows being come back to, which the reading is made again for too.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public OverviewDock(IPluginHost host, ILog log, IFileChanges files, IRefocus refocus, ProgressHub hub) =>
        _view = new OverviewView(host, log, files, refocus, hub);

    /// <inheritdoc />
    public Control View => _view;

    /// <summary>
    /// What the dock's own strip offers about what it is showing.
    /// </summary>
    /// <remarks>
    /// One strip, drawn by this plugin, and it holds what reading the work again is as well as the sync: what a
    /// reader reaches for about the whole work belongs in one row, and a refresh that stood apart from the
    /// buttons beside it would be the one thing in the row that is not about the work as a whole.
    /// <para>
    /// It is drawn as its pictures rather than as its words — the reading's own `refresh` first, then this
    /// plugin's three for the sync — because that is what a header is for: each plugin draws its own marks, so
    /// one plugin's keys stay its own.
    /// </para>
    /// <para>
    /// Reading the work again is offered as the strip's own command as well, because a header drawn as a menu —
    /// a dock in a window of its own — has nowhere to put a control: without that, a window holding this dock
    /// would have no way to ask for a reading, which is a way it had before the strip existed.
    /// </para>
    /// </remarks>
    public IReadOnlyList<DockHeaderItem> Header =>
        [
            new DockHeaderView(
                _view.Strip,
                new DockHeaderCommand("rorolala_vcs.status.refresh", _view.Refresh, IconKey: Icons.Refresh)
            ),
        ];
}

/// <summary>What where the tree is rooted comes to, as the Overview reads it.</summary>
/// <param name="Facts">The reading to draw, when there is one.</param>
/// <param name="Said">What to say instead, when there is not.</param>
/// <param name="Status">
/// The reading itself, which what a sync would move is read out of, or nothing. It is the owner's to give
/// back: the view keeps it for as long as its rows are on screen.
/// </param>
internal sealed record OverviewReading(StatusFacts? Facts, string? Said, RolaStatus? Status = null);

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
/// <param name="Sends">Paths a sync would send the version of, being further on than the Vault.</param>
/// <param name="Receives">Paths a sync would bring the Vault's version down to.</param>
/// <param name="Apart">Paths whose two chains have each moved on, which only a forced sync sends.</param>
internal sealed record StatusFacts(
    IReadOnlyList<string> Lost,
    IReadOnlyList<string> Untagged,
    IReadOnlyList<Rename> Renamed,
    IReadOnlyList<string> Modified,
    IReadOnlyList<string> Unowned,
    IReadOnlyList<string> WrongBase,
    IReadOnlyList<Merge> Merging,
    IReadOnlyList<BrokenReason> Broken,
    IReadOnlyList<string> Sends,
    IReadOnlyList<string> Receives,
    IReadOnlyList<string> Apart
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
            status.Broken,
            // The three what a sync would do are the reading's whole list of standings, asked once each: every
            // path the Layout records a version for is in it, whether or not the tree changed it. They are not
            // read from `WrongBase`, which is the changes the tree has made against a base upstream moved past —
            // a file nobody touched but the Vault has moved past is a receive too, and would otherwise have no row.
            [.. status.Ahead],
            [.. status.Behind],
            [.. status.Apart]
        );
    }
}

/// <summary>
/// One state of a row, which is the letters it is read by, how it is coloured, and what may be done about it.
/// </summary>
/// <remarks>
/// Seven, because that is how many the reading may say about a path, and a state is one of them rather than a
/// word composed where a row is made: the letters, the colour and the operations are the state's own answer, so
/// a state added later is a member here and nothing else changes.
/// <para>
/// The colour is this plugin's own — see <see cref="RorolalaColours"/> — rather than a role of the look's,
/// because what a state is coloured is the state: red for what must not be recorded, green for what is not
/// recorded yet, amber for what is under way or has to be read twice, and blue for what is somewhere else than
/// the Layout says.
/// </para>
/// </remarks>
internal enum State
{
    /// <summary>What the Layout does not name and the tree holds.</summary>
    Untagged,

    /// <summary>What the Layout names and the tree has lost.</summary>
    Lost,

    /// <summary>What is not this account's to record.</summary>
    NotMine,

    /// <summary>What is this account's to record but stands on a version upstream has moved past.</summary>
    Stale,

    /// <summary>A move, where the bytes make it one.</summary>
    Renamed,

    /// <summary>A move, where only the likeness suggests it.</summary>
    RenamedLikely,

    /// <summary>A change of content that is this account's to record.</summary>
    Modified,

    /// <summary>A variant waiting to be joined in.</summary>
    Merging,

    /// <summary>A variant that cannot be joined in.</summary>
    Broken,

    /// <summary>This side's version is further on than the Vault's, so a sync sends it.</summary>
    Send,

    /// <summary>The Vault's version is further on, so a sync brings it down.</summary>
    Receive,

    /// <summary>The tree holds it and the Layout does not name it, so a sync creates it upstream.</summary>
    Create,

    /// <summary>The two chains have each moved on, so only a forced sync sends it.</summary>
    Diverged,
}

/// <summary>
/// One pill of a row's state: which state it is, the letters it is read by, and the whole of what it means.
/// </summary>
/// <remarks>
/// One or two letters rather than the state's name, because the letters are what a column of states is scanned
/// by: the names are two to five characters each and a column holding them is the widest thing in the row,
/// where what a reader wants from the column is which state a row is in and not what that state is called. The
/// name and the reason are on the pill's hover, which is where a reader who has stopped at one is looking.
/// <para>
/// The state is carried as well as the letters, because what may be done about a row is read from which states
/// it is in: the letters are how a state is drawn, and the state is what it means.
/// </para>
/// </remarks>
/// <param name="State">Which state it is.</param>
/// <param name="Letters">What the state is known by, which is what the pill carries.</param>
/// <param name="Fill">What the pill is coloured, which is this plugin's colour rather than the look's.</param>
/// <param name="Hint">
/// What the pointer resting on the pill says: the state's name and the whole of what it means, since the
/// letters say only which state it is.
/// </param>
internal sealed record StatusPill(State State, string Letters, Color Fill, string Hint);

/// <summary>One row of the table: what the reading says about one path.</summary>
/// <param name="Pills">
/// What the path's state is, one pill each: what kind of change it is, and what became of its content.
/// A change of content alone has no kind, and a kind alone has nothing to say about a content, so a row
/// carries as many pills as it has states — one or two, and more where a merge has something to add.
/// </param>
/// <param name="Notes">
/// What the row has to say that is not a state: why a merge cannot go on. A note is written rather than
/// coloured, since it says what a row is about rather than what has to be done about it.
/// </param>
/// <param name="From">The path it comes from, or nothing when it comes from nowhere.</param>
/// <param name="To">The path it goes to, or nothing when it goes nowhere.</param>
/// <param name="Operations">
/// What may be done about it, which is what each of its states offers, once each: a row in two states has the
/// buttons of both, because a file that moved and was edited is two things to act on.
/// </param>
/// <param name="Act">
/// Which of the two paths the operations act on, where it is neither of the ones the rule picks — see
/// <see cref="OverviewView.Path"/>. Nothing for every row whose operations act on where its file is now.
/// </param>
internal sealed record TableRow(
    IReadOnlyList<StatusPill> Pills,
    IReadOnlyList<string> Notes,
    string From,
    string To,
    IReadOnlyList<Operation> Operations,
    string? Act = null
);

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

    /// <summary>
    /// How much of the table's width each column takes, in the order they are read.
    /// </summary>
    /// <remarks>
    /// What a state is a width and the two paths are shares of what is left: a state is one or two letters in a
    /// pill of a fixed size, so the room it wants is a measure rather than a share — three pills and the room
    /// between them, which is what a row of states comes to before it wraps. A path is read in full, so the rest
    /// is given to the two of them as shares rather than as widths, and a wider dock is a longer path rather than
    /// a wider column of states.
    /// <para>
    /// The operations are a measure of their own, and the widest measure in the table: a button is a word, and
    /// the words are the command's own — 「还原内容」 and 「获取所有权」 are not shorter for being a table's. They
    /// are the last column rather than the first because they are what a reader does <em>after</em> reading what
    /// a row says: the state, then the paths, then what may be done about them.
    /// </para>
    /// </remarks>
    private static readonly GridLength[] Columns =
    [
        new(StateColumn, GridUnitType.Pixel),
        new(1, GridUnitType.Star),
        new(1, GridUnitType.Star),
        new(OperationsColumn, GridUnitType.Pixel),
    ];

    /// <summary>The colour a mark of one kind is filled with, which is this plugin's own.</summary>
    /// <remarks>
    /// Four colours and nothing else: what is not recorded yet, what is under way, what must not be recorded,
    /// and what is somewhere else than the Layout says. They are named here rather than read from the look,
    /// because what a state is coloured is the state — see <see cref="RorolalaColours"/>.
    /// </remarks>
    private static Color WrongFill => RorolalaColours.Wrong;

    /// <summary>What has appeared.</summary>
    private static Color AppearedFill => RorolalaColours.Untagged;

    /// <summary>What moved.</summary>
    private static Color MovedFill => RorolalaColours.Moved;

    /// <summary>What is not settled: what is under way, and what has to be read twice.</summary>
    private static Color UnsettledFill => RorolalaColours.UnderWay;

    /// <summary>The theme resource for what has nothing to say about itself, which a note is read in.</summary>
    private const string QuietFill = PaletteKeys.Faint;

    /// <summary>The theme resource a line told apart from the next is drawn in.</summary>
    private const string EdgeInk = PaletteKeys.Splitter;

    /// <summary>The theme resource a band is drawn on, which is the ground a heading sits on.</summary>
    private const string BandGround = PaletteKeys.Dark;

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

    /// <summary>The host, which is what a row's buttons reach to run and to report.</summary>
    private readonly IPluginHost _host;

    /// <summary>
    /// The row a connection was begun from, or nothing while none is.
    /// </summary>
    /// <remarks>
    /// Held by the row rather than by a button, because what is connected to what is a fact about two rows and
    /// not about the control that was pressed: the table is drawn again under a reader — by the reading on its
    /// timer — and a connection kept in a button would be lost the moment the button was thrown away.
    /// <para>
    /// What is held is the path rather than the row, since a row is built afresh on every reading and two rows
    /// that say the same thing are not the same object: the path is what a row is about, and a path is what the
    /// command line takes.
    /// </para>
    /// </remarks>
    private string? _connecting;

    /// <summary>
    /// Whether one of the table's operations is going, which is what every one of its buttons is held for.
    /// </summary>
    /// <remarks>
    /// One flag rather than one per row, because what it is about is the tree: every operation button of the
    /// table acts on the work under the base, and two modes over one work at once would be two runs the second
    /// of which acts on what the first has half done. The buttons are drawn from it, so it is enough to draw the
    /// table again when it changes.
    /// </remarks>
    private bool _busy;

    /// <summary>
    /// Whether a fetch is going, which is what a refresh begins and what a second refresh is kept from.
    /// </summary>
    /// <remarks>
    /// A fetch already running is the one a second press would be: what is fetched is the same copy, and two
    /// exchanges for one reading are two where one answers.
    /// </remarks>
    /// <summary>Whether a running command is one whose end needs a reading, which is what a fetch and a sync are.</summary>
    /// <remarks>
    /// Either of them writes the copy every reading of the work is made from, while the other commands this plugin
    /// runs — a claim, a release, a mode of <c>rola align</c> — write the Layout and are followed by the files being
    /// said to have changed. A reading taken while a sync is carrying content down would say what the tree held in
    /// the middle of it, which is neither what it held nor what it holds.
    /// </remarks>
    private bool _fetching;

    /// <summary>Whether the hub had something going when it last said it had changed.</summary>
    /// <remarks>
    /// What tells a run ending from a run saying what it is doing: the hub says both the same way, and the reading
    /// is held for the first and not the second.
    /// </remarks>
    private bool _running;

    /// <summary>The reading the rows on screen were made from, or nothing before one was made.</summary>
    /// <remarks>
    /// The count a sync row is hovered for is the one thing about it the reading keeps per path, so it is read from
    /// here as the row is drawn rather than carried in the row: a row built by <see cref="Rows"/> has no reading to
    /// ask, and the reading is a snapshot that is the same one however many rows are drawn from it.
    /// <para>
    /// It is owned here: a reading that is replaced — or one a newer reading says the same as, and so is not drawn
    /// — is given back rather than left to the native side.
    /// </para>
    /// </remarks>
    private RolaStatus? _reading;

    /// <summary>Whether a reading is already waiting to be made, so a burst of asks is one reading.</summary>
    private bool _drawAsked;

    /// <summary>What is on screen now, so that a reading saying the same thing is not drawn again.</summary>
    private Shown? _shown;

    /// <summary>Makes the control.</summary>
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="files">What the files may have done.</param>
    /// <param name="refocus">The host's windows being come back to.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public OverviewView(IPluginHost host, ILog log, IFileChanges files, IRefocus refocus, ProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(hub);

        _host = host;
        _log = log;
        _files = files;
        _refocus = refocus;
        _hub = hub;

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

        // The sync's own state, which the strip in the dock's header draws from; what the runs of every command
        // say is gathered in the hub, which the panel down the right draws from and which every place a command
        // may be run from — the strip, an entry's menu — writes to.
        _sync = new SyncWatcher(_host, _log, hub);
        _strip = new SyncStrip(_host, _sync, () => BaseDirectory.Current, Refresh);
        _panel = new ProgressPanel(hub);

        var root = new Grid();

        // Two columns rather than one: what a sync is doing stands down the right of the reading, so that the
        // work and the run are read at once. The panel's column is nothing while no run is going, and the table
        // has the whole width then — see `Split`.
        root.ColumnDefinitions.Add(_tableColumn);
        root.ColumnDefinitions.Add(_panelColumn);

        var body = new ScrollViewer
        {
            Content = _blocks,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        };

        // The reading and the words that stand in for one share the left column, one at a time: a work with
        // changes is the table, and one without them is the words.
        var reading = new Grid { Children = { body, _notice } };

        Grid.SetColumn(_panel, 1);
        root.Children.Add(reading);
        root.Children.Add(_panel);

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
        AttachedToVisualTree += (_, _) =>
        {
            Listen(true);

            // What there is to carry is asked when the dock appears rather than on the timer the reading is made
            // on: a plan costs a fetch of the Vault's Layout, and a listing left open would ask every five
            // seconds for as long as it was looked at.
            _sync.Plan(BaseDirectory.Current);

            // The width follows what is running for as long as the dock is looked at: a hub outlives every dock,
            // and a handler left on it when the dock closes would keep the whole view alive for nothing.
            _hub.Changed += Sizing;
        };

        DetachedFromVisualTree += (_, _) =>
        {
            Listen(false);
            _hub.Changed -= Sizing;
        };

        // What a command is doing decides whether the panel has anywhere to stand: with nothing shown, the room
        // down the right is given back to the table rather than left as an empty third of the dock.
        Split(hub.Any);
        _running = hub.Any;

        Draw();
    }

    /// <summary>How the width is shared while a run is going: the reading to the panel.</summary>
    private const double TableShare = 7;

    /// <summary>The panel's share of the width while a run is going, which is a third of the dock.</summary>
    private const double PanelShare = 3;

    /// <summary>
    /// Gives the panel its share of the width while a run is going, and takes it back when none is.
    /// </summary>
    /// <remarks>
    /// The share is a third of the dock, which is what a row of a bar and a path reads in, and the reading keeps
    /// the rest. With no run going there is nothing down the right, so the reading has the whole width rather
    /// than an empty third of it.
    /// </remarks>
    /// <param name="running">Whether a run is going.</param>
    private void Split(bool running)
    {
        _tableColumn.Width = new GridLength(running ? TableShare : 1, GridUnitType.Star);
        _panelColumn.Width = new GridLength(running ? PanelShare : 0, GridUnitType.Star);
    }

    /// <summary>Redraws the width the panel stands in, which every change to the runs may have moved.</summary>
    /// <param name="sender">What raised it.</param>
    /// <param name="changed">The event.</param>
    private void Sizing(object? sender, EventArgs changed)
    {
        _ = sender;
        _ = changed;

        Split(_hub.Any);

        // A run that has just ended is the fetch or the sync the reading was held for: what it wrote is what the
        // work is read from now, and nothing else will say so — see <see cref="Drawn"/>.
        if (_running && !_hub.Any)
        {
            _running = false;
            Draw();
            _sync.Plan(BaseDirectory.Current);
        }

        _running = _hub.Any;
    }

    /// <summary>What the dock's header carries, which is the sync strip with the reading's own refresh in it.</summary>
    /// <remarks>
    /// One control rather than a run of commands, because what is in it is more than buttons: three that belong
    /// together, three whose colour answers a key the user is holding, and a dropdown of what a run is to leave
    /// out. The host puts it in its header and asks nothing about what is inside it.
    /// </remarks>
    public Control Strip => _strip;

    /// <summary>What the reading is drawn from, and what a sync is watched by.</summary>
    private readonly SyncWatcher _sync;

    /// <summary>What every command this plugin runs reports itself to, which the width of the panel follows.</summary>
    private readonly ProgressHub _hub;

    /// <summary>The strip the dock's header carries.</summary>
    private readonly SyncStrip _strip;

    /// <summary>What every command this plugin runs is drawn as, down the right of the reading.</summary>
    private readonly ProgressPanel _panel;

    /// <summary>The column the reading takes, which is the whole width while no run is going.</summary>
    private readonly ColumnDefinition _tableColumn = new(new GridLength(TableShare, GridUnitType.Star));

    /// <summary>The column the panel takes, which is nothing while no run is going.</summary>
    private readonly ColumnDefinition _panelColumn = new(new GridLength(0, GridUnitType.Star));

    /// <summary>
    /// Fetches the Vault's Layout and reads the work again, which is what the dock's own command asks for.
    /// </summary>
    /// <remarks>
    /// What is fetched is the copy every reading of the work is made from, so a reading made before it would say
    /// what the Vault held when the copy was last brought over. The run is shown down the panel as a card of its
    /// own, and the reading and what a sync would carry are asked for once it is over. A directory no Workspace
    /// holds has nothing to fetch, so it is only read again.
    /// </remarks>
    public void Refresh()
    {
        // A fetch or a sync is what reads the work already: a second press while one is going is the one that is
        // going, and its own completion is what draws. Asking again would be a reading of the state the run is
        // halfway through, and would be drawn as if it were the state after it.
        if (_fetching || _sync.Running)
        {
            return;
        }

        if (BaseDirectory.Current is { Length: > 0 } at && WorkspaceNotice.Holds(at, _log))
        {
            BeginFetch(at);

            return;
        }

        Draw();
        _sync.Plan(BaseDirectory.Current);
    }

    /// <summary>Runs a fetch, and reads the work again once it is over.</summary>
    /// <remarks>
    /// The card is opened before the work is handed off, so that the panel has it to draw from the first turn —
    /// see the hub for why the runs are gathered rather than kept where they were begun. The run says nothing of
    /// how far it is, so its card is drawn as one still moving: a fetch reports no progress of its own.
    /// </remarks>
    /// <param name="directory">Where the work is rooted.</param>
    private void BeginFetch(string directory)
    {
        // A fetch already going is the one a second press would be, and a sync brings the same copy as it goes:
        // what is on screen is read again either way, and the run in hand leaves the copy where it stands.
        if (_fetching || _sync.Running)
        {
            return;
        }

        // The window is told which program to run by the run that opened it, and a window opened on its own has
        // none: there is nothing to fetch with, so what there is to read is read.
        if (Environment.GetEnvironmentVariable(Sync.Program) is not { Length: > 0 } program)
        {
            Draw();
            _sync.Plan(directory);

            return;
        }

        _fetching = true;

        var run = _hub.Begin("rorolala_vcs.progress.fetch");

        _ = Task.Run(() =>
        {
            var (done, said) = Fetch.Run(program, directory);

            Dispatcher.UIThread.Post(() =>
            {
                _fetching = false;

                // The card is let go of here rather than when the run is over, because the panel keeps it for a
                // few seconds after: what ended is drawn as ended before it is taken away.
                _hub.End(run);

                // What was fetched is what every reading is made from, so what the columns answered about the
                // entries is let go of with the files, and the listing and the reading are read again.
                _host.Files.Touch();

                if (!done)
                {
                    // Drawn as every other command's failure is, and not as one line of raw text: what the
                    // command wrote says which of its lines went wrong and which can be acted on, and the window
                    // draws those marks rather than the words they are written with.
                    _host.Dialogs.Present(
                        RolaI18N.Get("rorolala_vcs.fetch.failed"),
                        () =>
                            ReportView.Of(
                                ReportLines.Parse(said),
                                key => _host.Icons.Find(key),
                                ReportView.Ink
                            )
                    );
                }

                Draw();
                _sync.Plan(directory);
            });
        });
    }

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

        RolaStatus? status = null;

        try
        {
            // What is read is handed back to the caller rather than given back here: a row's count is asked of
            // the reading as it is drawn, so the reading has to live as long as the rows do. It is given back
            // when the reading is replaced or discarded — see <see cref="Show"/> and <see cref="Drawn"/>.
            status = RolaStatus.Locate(at);

            if (status is not null)
            {
                return new OverviewReading(StatusFacts.Of(status), null, status);
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
            // A reading that could not be taken out whole is not one to keep: what is left would be a handle
            // nobody can reach, and a row asking it a question would be asking nothing.
            status?.Dispose();

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
        // word on every row it names. Every one is asked for by a literal so that the gate can see it — and the
        // letters are the pill's own, so what a state is known by is stated where the state is made rather than
        // read from the catalogue.
        var stopped = RolaI18N.Get("rorolala_vcs.status.merge_broken");

        List<TableRow> wrong = [];
        List<TableRow> merges = [];
        List<TableRow> structural = [];
        List<TableRow> changes = [];
        List<TableRow> syncing = [];

        var destinations = facts.Renamed.Select(rename => rename.To).ToHashSet(StringComparer.Ordinal);
        var behind = facts.WrongBase.ToHashSet(StringComparer.Ordinal);
        var apart = facts.Apart.ToHashSet(StringComparer.Ordinal);

        // Every path the three above speak of, so that what a sync would move is read for the rest: a path two of
        // them name is one row, and a row is what says what is to be done about it.
        var moving = new HashSet<string>(StringComparer.Ordinal);
        moving.UnionWith(facts.Lost);
        moving.UnionWith(facts.Untagged);
        moving.UnionWith(facts.Renamed.Select(rename => rename.From));
        moving.UnionWith(destinations);
        moving.UnionWith(facts.Modified);
        moving.UnionWith(facts.Unowned);

        wrong.AddRange(
            facts.Unowned.Select(path =>
                new TableRow([Pill(State.NotMine)], [], path, string.Empty, OverviewOperations.Of(State.NotMine))
            )
        );

        var changed = facts.Modified.Where(path => !destinations.Contains(path)).ToList();

        wrong.AddRange(
            changed
                .Where(behind.Contains)
                .Select(path =>
                    new TableRow([Pill(State.Stale)], [], path, string.Empty, OverviewOperations.Of(State.Stale))
                )
        );

        structural.AddRange(
            facts.Lost.Select(path =>
                new TableRow([Pill(State.Lost)], [], path, string.Empty, OverviewOperations.Of(State.Lost))
            )
        );

        // What the tree holds and the Layout does not name is a path with nothing on the other side of it, so it
        // is written where a path stands rather than where one goes: `From` and `To` are not a direction here,
        // they are the two columns a path may stand in, and the states that are about one path use the first.
        structural.AddRange(
            facts.Untagged.Select(path =>
                new TableRow([Pill(State.Untagged)], [], path, string.Empty, OverviewOperations.Of(State.Untagged))
            )
        );

        // A move is read where the content it carries has to be acted on: one that was edited against a
        // version upstream has moved past is a wrong change before it is a move.
        foreach (var rename in facts.Renamed.Where(rename => !rename.Strong))
        {
            (Misfit(rename) ? wrong : structural).Add(Move(rename, State.RenamedLikely));
        }

        foreach (var rename in facts.Renamed.Where(rename => rename.Strong))
        {
            (Misfit(rename) ? wrong : structural).Add(Move(rename, State.Renamed));
        }

        merges.AddRange(
            facts.Merging.Select(item =>
                new TableRow(
                    [MergePill(item.Variant)],
                    // The variant and the two files are on the pill and in the columns, so the row has nothing
                    // left to note: what a reader wants of it is which variant is waiting, which is on the pill,
                    // and where the two files are, which is in the columns.
                    [],
                    // The variant file comes first and the file it is for second, which is the direction the
                    // content is to move: the variant's content is joined into the file the Layout names. What
                    // the row's one button acts on is the variant file — the one a merge is ended at — so the
                    // row says so rather than leaving the rule to pick the other column.
                    VariantFiles.Place(item),
                    item.Target,
                    OverviewOperations.Of(State.Merging),
                    Act: VariantFiles.Place(item)
                )
            )
        );

        merges.AddRange(
            facts.Broken.Select(reason =>
            {
                // The reason is what names the variant file, and a reason about the variant itself rather than
                // about its file names none: a row with no path is one nothing can be done from, which is why
                // it is offered no button.
                var place = BrokenPlace(reason);

                return new TableRow(
                    // A merge that cannot go on is a thing gone wrong rather than work under way, so its pill
                    // wears the colour of what is wrong rather than the colour of a merge that is — and what is
                    // wrong with it is in the note, where there is room for the reason.
                    [Pill(State.Broken)],
                    [$"{stopped}{BrokenSaid(reason)}"],
                    // Read the same way round as a merge that can go on: the variant file first, and the file it
                    // is for second.
                    place,
                    reason.Target,
                    place is { Length: > 0 } ? OverviewOperations.Of(State.Broken) : [],
                    Act: place is { Length: > 0 } ? place : null
                );
            })
        );

        changes.AddRange(
            changed
                .Where(path => !behind.Contains(path))
                .Select(path =>
                    new TableRow([Pill(State.Modified)], [], path, string.Empty, OverviewOperations.Of(State.Modified))
                )
        );

        // What a sync would do with a path already named above is on the row it is already on; what is left is
        // the work that is in step with nothing wrong about it — which is the ordinary state of a file, and the
        // one a reader opens the Overview to see is safe to sync.
        syncing.AddRange(
            facts
                .Sends.Where(path => !moving.Contains(path))
                .Select(path => Sync(State.Send, path, behind))
        );
        syncing.AddRange(
            facts
                .Receives.Where(path => !moving.Contains(path))
                .Select(path => Sync(State.Receive, path, behind))
        );
        syncing.AddRange(
            facts
                .Apart.Where(path => !moving.Contains(path))
                .Select(path => Sync(State.Diverged, path, behind))
        );

        return [.. wrong, .. merges, .. structural, .. changes, .. syncing];

        // One path of what a sync would move, as a row of its own. It offers no button: what is to be done about it
        // is the sync the strip in the header runs, and a button here would be a second way to ask for the same
        // run — one that would have to say what it carries, which the strip already says.
        //
        // The path is written where it comes from rather than where it goes: nothing goes anywhere here, and the
        // two columns are the sides of a change rather than a direction, so a path with nothing on the other side
        // of it is in the first. What is behind is a base gone wrong as well, so it wears the pill saying so beside
        // the one saying which way the work moves.
        TableRow Sync(State move, string path, HashSet<string> stale) =>
            new(
                stale.Contains(path) && move != State.Diverged
                    ? new[] { Pill(move), Pill(State.Stale) }
                    : new[] { Pill(move) },
                [],
                path,
                string.Empty,
                []
            );

        // Whether a move's content is a change that has to be acted on rather than one that is simply to be
        // recorded, which is what decides the group it is read in.
        bool Misfit(Rename rename) => rename.Modified && behind.Contains(rename.To);

        // One move, as a row. A move whose file was edited is a change of content as well, so what became of
        // the content is a pill of the move's own row rather than a row of its own — and a move that changed
        // nothing says nothing beyond being a move, since there is no change of content to report.
        //
        // The buttons are both states' where it is both: what a reader has to do about a file that moved and
        // was edited is both things, and a row offering one of them would be a row that could not be settled
        // from where it is read.
        TableRow Move(Rename rename, State kind) =>
            new(
                [
                    Pill(kind),
                    .. rename.Modified
                        ? new[] { Pill(behind.Contains(rename.To) ? State.Stale : State.Modified) }
                        : [],
                ],
                [],
                rename.From,
                rename.To,
                rename.Modified
                    ? OverviewOperations.Of([kind, behind.Contains(rename.To) ? State.Stale : State.Modified])
                    : OverviewOperations.Of(kind)
            );
    }

    /// <summary>
    /// One state, as the pill a row wears for it.
    /// </summary>
    /// <remarks>
    /// The letters are stated here rather than read from the catalogue, because they are not words: one or two
    /// characters a reader learns once and then scans for, and a translation that changed them would change what
    /// a column is read by. What is translated is the hint, which is what the letters stand for.
    /// </remarks>
    /// <param name="state">The state.</param>
    /// <returns>The pill.</returns>
    private static StatusPill Pill(State state) =>
        state switch
        {
            State.Untagged => new StatusPill(state, "A", AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_untagged")),
            State.Lost => new StatusPill(state, "D", WrongFill, RolaI18N.Get("rorolala_vcs.status.kind_lost")),
            State.NotMine => new StatusPill(state, "E", WrongFill, RolaI18N.Get("rorolala_vcs.status.change_wrong_unowned")),
            State.Stale => new StatusPill(state, "E", WrongFill, RolaI18N.Get("rorolala_vcs.status.change_wrong_base")),
            State.Renamed => new StatusPill(state, "R", MovedFill, RolaI18N.Get("rorolala_vcs.status.kind_rename_strong")),
            State.RenamedLikely => new StatusPill(state, "R?", MovedFill, RolaI18N.Get("rorolala_vcs.status.kind_rename_weak")),
            State.Modified => new StatusPill(state, "M", UnsettledFill, RolaI18N.Get("rorolala_vcs.status.change_modified")),
            State.Send => new StatusPill(state, "S↑", AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_send")),
            State.Receive => new StatusPill(state, "S↓", AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_receive")),
            State.Create => new StatusPill(state, "S+", AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_create")),
            State.Diverged => new StatusPill(state, "Sx", WrongFill, RolaI18N.Get("rorolala_vcs.status.kind_diverged")),
            _ => new StatusPill(state, "J", WrongFill, RolaI18N.Get("rorolala_vcs.status.kind_merge")),
        };

    /// <summary>
    /// What a path a sync would move is hovered for: which way the work moves, how far apart the two versions are
    /// where the index says, and what a sync comes to afterwards.
    /// </summary>
    /// <remarks>
    /// The count is asked of the reading rather than taken from the row, because it is the one thing about a
    /// standing that the reading keeps per path: what a pill carries is the direction, and how far is in the words.
    /// A count the chain does not give is not said at all — what the Vault is at is ordinarily a version this side
    /// never fetched — since a zero would read as being in step, which is the one thing the row is not about.
    /// </remarks>
    /// <param name="state">Which of the four the path is in.</param>
    /// <param name="path">The path.</param>
    /// <param name="at">Where the reading was made, whose paths are relative to it.</param>
    /// <param name="reading">The reading, or nothing when it could not be made.</param>
    /// <returns>The words for the pointer resting on it.</returns>
    internal static string SyncSaid(State state, string path, string? at, RolaStatus? reading)
    {
        ArgumentNullException.ThrowIfNull(path);

        ulong? distance = null;

        if (reading is not null && at is { Length: > 0 })
        {
            try
            {
                // The path is joined to the root the way every other path of this table is: what a reading names
                // is relative to the Workspace, and what a row carries is as the system spells it.
                distance = RorolalaSync.Distance(reading, System.IO.Path.Combine(at, path));
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                // A count that could not be read is a count not said: what the pill carries is the direction,
                // and that is read whether or not the chain is here to add to it.
                _ = error;
            }
        }

        return state switch
        {
            State.Send => distance is { } ahead
                ? RolaI18N.Get("rorolala_vcs.status.sync_ahead", ahead)
                : RolaI18N.Get("rorolala_vcs.status.kind_send"),
            State.Receive => distance is { } behind
                ? RolaI18N.Get("rorolala_vcs.status.sync_behind", behind)
                : RolaI18N.Get("rorolala_vcs.status.kind_receive"),
            State.Diverged => RolaI18N.Get("rorolala_vcs.status.kind_diverged"),
            _ => RolaI18N.Get("rorolala_vcs.status.kind_create"),
        };
    }

    /// <summary>The pill a merge in progress wears, which names the variant it is waiting for.</summary>
    /// <remarks>
    /// A pill of its own rather than <see cref="Pill(State)"/>, because what it is hovered for is the one
    /// thing about a merge that the row's columns do not say: which variant it is. The columns carry the two
    /// files and nothing carries the hash once the note is gone, and a reader who has stopped at the pill is
    /// asking exactly that.
    /// </remarks>
    /// <param name="variant">The variant waiting, as the hash it is named by.</param>
    /// <returns>The pill.</returns>
    private static StatusPill MergePill(string variant) =>
        new(
            State.Merging,
            "J",
            UnsettledFill,
            RolaI18N.Get("rorolala_vcs.variant.pending", VariantFiles.Head(variant))
        );

    /// <summary>
    /// The states a row is in, read from the pills it wears.
    /// </summary>
    /// <remarks>
    /// Read off the pills rather than carried beside them, since the pills are already the row's answer to which
    /// states it is in: a second list that could disagree with them would be a row whose buttons acted on
    /// something other than what it says.
    /// </remarks>
    /// <param name="row">The row.</param>
    /// <returns>The states, in the order the pills are read.</returns>
    private static IReadOnlyList<State> Choices(TableRow row) =>
        [.. row.Pills.Select(pill => pill.State)];

    /// <summary>
    /// What the buttons of a row are made of, which is a fresh set on every drawing.
    /// </summary>
    /// <remarks>
    /// Held rather than made where a row is, because what a button does needs the host — and the rows are built
    /// by a static method that has no host and no reason to want one. The owner is what the buttons read as they
    /// are drawn: the path the run is made in and what to tell when the files have changed.
    /// </remarks>
    /// <param name="row">The row the buttons are for.</param>
    /// <param name="at">Where the run is made, which is what the paths in the row are read from.</param>
    /// <param name="host">The host, which is what a button reaches to run and to report.</param>
    /// <param name="view">The control the buttons belong to, which is what a connection is remembered on.</param>
    /// <returns>The buttons, or nothing drawn when the row has no operations.</returns>
    private static Control Cell(TableRow row, string at, IPluginHost host, OverviewView view)
    {
        var operations = new WrapPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Center,
        };

        foreach (var operation in row.Operations)
        {
            operations.Children.Add(view.NewButton(operation, row, at, host));
        }

        return operations;
    }

    /// <summary>
    /// One operation as a button.
    /// </summary>
    /// <remarks>
    /// A button rather than a menu, because what may be done about a row is one to three things and a row of
    /// things to press says them at once: a menu would hide what a row offers behind a click, and a row that has
    /// to be opened to be read is a row that is read twice.
    /// <para>
    /// What a press does depends on what is being connected: a connection is two clicks and the second one runs
    /// the operation, where every other button runs when it is pressed. While a connection is on, every button
    /// that is not a side of it is disabled — including the other buttons of the row it began from — so that a
    /// reader who has begun one cannot also ask for something else, which would be two runs over one path.
    /// </para>
    /// <para>
    /// The two sides of it are the two that stand out, and what tells them is what they say rather than how
    /// they are painted: the button that began the connection says that it is waiting, and every button it
    /// could be finished on says that it is an answer (see <see cref="Label"/>). The rest keep their own words
    /// and are not dimmed — the look has no faded state, and inventing one here would be a colour the theme
    /// does not hold — but they do not answer: a disabled button takes no press and takes no pointer, so a
    /// reader finds the ones that name themselves before they find the ones that do not.
    /// </para>
    /// </remarks>
    /// <param name="operation">What the button does.</param>
    /// <param name="row">The row it is on.</param>
    /// <param name="at">Where the run is made, which is what the row's paths are read from.</param>
    /// <param name="host">The host, which is what the button reaches to run and to report.</param>
    /// <returns>The button.</returns>
    private Button NewButton(Operation operation, TableRow row, string at, IPluginHost host)
    {
        var path = Path(row);
        var pairing = OverviewOperations.IsPairing(operation);

        // Both of these are asked of a connection button and of no other, and `pairing` is part of both rather
        // than only of the candidate: every button of a row acts on the row's one path, so a button that is not
        // a connection is `begun` the moment a connection is begun on its row, and would be drawn as the button
        // waiting for a second press rather than as the thing it does.
        var begun = pairing && _connecting is { Length: > 0 } begunAt && begunAt == path;
        var candidate = pairing
            && _connecting is { Length: > 0 } from
            && from != path
            && OverviewOperations.Pairs(Choices(row), StatesAt(from, _shown?.Rows ?? []));
        var running = _connecting is not null;

        var button = new Button
        {
            Content = RolaI18N.Get(Label(operation, begun, candidate)),
            Classes = { "ghost" },
            VerticalAlignment = VerticalAlignment.Center,
            // One width for every button, whatever it says and whether or not it is part of a connection. A
            // button sized by its words is a button that changes size when its words do, and the words of the
            // two sides of a connection are the ones that change: the whole column would jump sideways at the
            // press, and the two rows being paired would jump by different amounts.
            Width = ButtonWidth,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, 2, ButtonGap, 2),
            // Everything is pressable while nothing is being connected or run; while a connection is in
            // hand, only the two sides of it are — which is the row's own button again, so that a second
            // press takes it back; and while an operation is going, nothing answers at all.
            IsEnabled = !_busy && (!running || begun || candidate),
        };

        button.Click += (_, _) =>
        {
            if (pairing)
            {
                Connect(operation, row, at, host);
                return;
            }

            Ask(operation, path, at, host);
        };

        return button;
    }

    /// <summary>
    /// What a button says, which is not always what it does.
    /// </summary>
    /// <remarks>
    /// A connection is a question — which of these two is which — and the answer is given by pressing one of
    /// them. So the buttons that are part of the question stop saying what they do and start saying what they
    /// are: the one that began it says that it is waiting, and each one it could be finished on says that it is
    /// an answer. Every other button on the table says what it does, and the two flags are the connection
    /// button's own (see <see cref="NewButton"/>) rather than the row's, since a row's buttons all act on the
    /// row's one path and a flag that went by the path would catch them all.
    /// <para>
    /// Words rather than a mark, because the two sides of a connection are two rows that may be far apart and
    /// may look alike: what a reader has to be able to tell is which of them they started from and which of them
    /// they may finish on, and 「…」 and 「组合」 say that where a tint has to be noticed first.
    /// </para>
    /// </remarks>
    /// <param name="operation">The operation the button is for.</param>
    /// <param name="begun">Whether it is the button the connection began from.</param>
    /// <param name="candidate">Whether it is one the connection could be finished on.</param>
    /// <returns>The key its words are read from.</returns>
    private static string Label(Operation operation, bool begun, bool candidate) =>
        begun ? "rorolala_vcs.operations.connecting"
        : candidate ? "rorolala_vcs.operations.pair"
        : OverviewOperations.Key(operation);

    /// <summary>
    /// What a connection button does when it is pressed: begins one, takes one back, or finishes one.
    /// </summary>
    /// <remarks>
    /// A press on the row a connection began from takes it back, which is what a reader who has changed their
    /// mind wants and is also what keeps the button from being pressed twice into a connection of a row with
    /// itself. A press on a row that is the other side of it runs the operation, with the two paths.
    /// </remarks>
    /// <param name="operation">The operation, which is always a connection here.</param>
    /// <param name="row">The row it was pressed on.</param>
    /// <param name="at">Where the run is made.</param>
    /// <param name="host">The host.</param>
    private void Connect(Operation operation, TableRow row, string at, IPluginHost host)
    {
        var path = Path(row);
        var begun = _connecting is { Length: > 0 } begunAt && begunAt == path;

        if (begun || _connecting is not { Length: > 0 } from)
        {
            // Begun, or begun again after a reading threw the row away: either way the next press is the first
            // of a new connection rather than the second of a stale one.
            _connecting = begun ? null : path;

            // The rows say the same thing they did, so this is a drawing of what is already read: reading the
            // work again would be a reading spent on a button, and one that might disagree with what is on
            // screen about a connection the reader is in the middle of making.
            Redraw();
            return;
        }

        // The lost path is the one that moved, so it is the one the command is given and the other is what it
        // moved to — whichever of the two rows was clicked first.
        var lost = Choices(row).Contains(State.Lost) ? path : from;
        var gain = lost == path ? from : path;

        _connecting = null;
        Perform(operation, lost, at, gain, host);

        // The run says the files may have changed, so the reading that follows will say something else. What is
        // drawn now is that reading and not these rows: the two rows of a connection are about to become one
        // move, and drawing them as they were would be drawing a question that has been answered.
        //
        // What is let go of after the run is begun rather than before, so that the table the run holds its
        // buttons on is the one on screen: a table with nothing read under it is one no button could be held on.
        _shown = null;
    }

    /// <summary>
    /// Runs one of a row's operations, holding every button of the table while it goes.
    /// </summary>
    /// <remarks>
    /// The operations are the command line's and are not waited for. Two of them over one work at once would be
    /// two runs whose effect is each other's to undo — a mode is not known to be safe against a second mode over
    /// the same tree — so a run takes the whole table: its own row's buttons and every other row's, and the
    /// button it was pressed on along with them. They answer again when it is over.
    /// <para>
    /// The busy flag is written and cleared on the window's thread, which is what draws: the run begins on the
    /// thread that pressed the button, and ends on the one it was made on, so what is told it is over is posted.
    /// </para>
    /// </remarks>
    /// <param name="operation">What is to be done.</param>
    /// <param name="path">The path it is done to, read where the run is made.</param>
    /// <param name="at">Where the run is made.</param>
    /// <param name="paired">The other side of a connection, or nothing for an operation with one path.</param>
    /// <param name="host">The host.</param>
    private void Perform(Operation operation, string path, string at, string? paired, IPluginHost host)
    {
        Hold(true);

        OverviewOperations.Run(
            operation,
            path,
            at,
            paired,
            host,
            () => Dispatcher.UIThread.Post(() => Hold(false))
        );
    }

    /// <summary>
    /// Draws the table's operation buttons pressable or not, which is every operation button while one is going.
    /// </summary>
    /// <remarks>
    /// What the buttons say has not changed — only whether they answer — so the table is drawn from the rows
    /// already on screen rather than read again: a reading spent on a button, and one that might disagree with
    /// what the reader is looking at.
    /// </remarks>
    /// <param name="busy">Whether an operation is going.</param>
    private void Hold(bool busy)
    {
        if (_busy == busy)
        {
            return;
        }

        _busy = busy;
        Redraw();
    }

    /// <summary>
    /// Asks about an operation that cannot be taken back, and runs it when the answer is yes.
    /// </summary>
    /// <remarks>
    /// Putting content back writes over what the file holds now, and what it held is not in the store: the
    /// Layout names a version, and what was changed between them is work that no command can recover. So it is
    /// asked about rather than done — the one operation of the six whose effect a reader might not have meant.
    /// Cancelling a merge is asked about for the same reason and one of its own: it takes the variant file
    /// away, and what that file holds is not kept anywhere the file is not — the record says which variant it
    /// was checked in as, and the content that is to be joined comes back only from the Vault.
    /// <para>
    /// The other operations are not asked about, and that is deliberate: a move that is taken back is a move
    /// made again, a loss that is confirmed is a record of what already happened, and a claim is answered by
    /// the Vault. None of them destroys anything the reader has no other copy of.
    /// </para>
    /// </remarks>
    /// <param name="operation">The operation.</param>
    /// <param name="path">The path it is done to.</param>
    /// <param name="at">Where the run is made.</param>
    /// <param name="host">The host, whose window the question is put over.</param>
    private void Ask(Operation operation, string path, string at, IPluginHost host)
    {
        var asked = operation switch
        {
            Operation.RestoreContent => new Dialog(
                RolaI18N.Get("rorolala_vcs.operations.restore_confirm_title"),
                RolaI18N.Get("rorolala_vcs.operations.restore_confirm_message", path),
                () => Perform(operation, path, at, null, host)
            ),
            Operation.CancelMerge => new Dialog(
                RolaI18N.Get("rorolala_vcs.operations.cancel_merge_confirm_title"),
                RolaI18N.Get("rorolala_vcs.operations.cancel_merge_confirm_message", path),
                () => Perform(operation, path, at, null, host)
            ),
            _ => null,
        };

        if (asked is null)
        {
            Perform(operation, path, at, null, host);
            return;
        }

        host.Dialogs.Show(asked);
    }

    /// <summary>
    /// What a row is about, which is the path its operations act on.
    /// </summary>
    /// <remarks>
    /// A row that stands for one path has that path in one of its two columns and nothing in the other, and
    /// which of the two it is depends on the state: a loss is a path the Layout names, a gain is one the tree
    /// holds. A move is the one row with both, and what a move's operations act on is where the file is now —
    /// that is the path a reader is looking at, and the one the command can find.
    /// <para>
    /// A merge is the other row with both, and it is the other way round: the two columns read the variant
    /// file being joined and the file it is being joined into, and what a merge's operation acts on is the
    /// variant file — the one a merge is ended at, and the one the command is run on. That is why a row may
    /// say which of its paths to act on rather than leaving it to the rule.
    /// </para>
    /// </remarks>
    /// <param name="row">The row.</param>
    /// <returns>The path, or empty for a row that names none.</returns>
    private static string Path(TableRow row) =>
        row.Act is { Length: > 0 } act ? act
        : row.To is { Length: > 0 } to ? to
        : row.From;

    /// <summary>The states of the row a path is on, or none when no row names it.</summary>
    /// <param name="path">The path.</param>
    /// <param name="rows">The rows the table is showing.</param>
    /// <returns>The states, which are none for a path no row is about.</returns>
    private static IReadOnlyList<State> StatesAt(string path, IReadOnlyList<TableRow> rows) =>
        rows.FirstOrDefault(row => Path(row) == path) is { } row ? Choices(row) : [];

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

    /// <summary>
    /// Redraws what is already on screen, without reading the work again.
    /// </summary>
    /// <remarks>
    /// For what changes what the table looks like without changing what it says: beginning a connection, and
    /// taking one back. Reading the work again for that would be a reading spent on nothing — and worse, it
    /// would be a reading that might disagree, since the work may have moved since the last one.
    /// </remarks>
    private void Redraw()
    {
        if (_shown is { } shown)
        {
            Show(shown.Said, shown.Rows, _reading);
        }
    }

    /// <summary>Reads the work again and draws it, on the thread the control belongs to.</summary>
    /// <remarks>
    /// Asks for one reading at a time: the timer, a change of the files and a press of the Refresh button are
    /// all the same question, and a press beside a tick would otherwise be two readings of one work. A call
    /// that arrives while one is waiting is the one that is waiting.
    /// </remarks>
    private void Draw()
    {
        if (!Dispatcher.UIThread.CheckAccess())
        {
            Dispatcher.UIThread.Post(Draw);

            return;
        }

        if (_drawAsked)
        {
            return;
        }

        _drawAsked = true;
        Dispatcher.UIThread.Post(Drawn);
    }

    /// <summary>Reads the work again and draws it.</summary>
    private void Drawn()
    {
        _drawAsked = false;

        // A reading made while a sync is carrying content down says what the tree held in the middle of the run, and
        // the rows drawn from it would be out of step with the copy the same run is writing. What is on screen is
        // read again after it, which the hub's own event says — and nothing is read here at all, since it would be a
        // reading spent on an answer that cannot be drawn.
        if (_sync.Running)
        {
            return;
        }

        var reading = Of(BaseDirectory.Current, _log);
        var rows = reading.Facts is { } facts ? Rows(facts) : [];
        var said = Line(reading, rows.Count);

        // What would be drawn is what is already drawn, so nothing is drawn: the table is the whole of the
        // dock, and building it again throws away what the reader has scrolled to, chosen and is pointing at.
        // A reading is asked for on a timer, so all but a few of them say what the last one said.
        //
        // What the rows say is not the whole of what the table looks like: a connection in hand is the view's
        // rather than the reading's, and the row it is on is not rebuilt when a button is pressed — see
        // <see cref="Redraw"/>. So a reading that says what the last one said is still drawn when the table on
        // screen was drawn with no connection in it, which is the one case where the two differ.
        if (
            _shown is { } shown
            && shown.Said == said
            && Same(shown.Rows, rows)
            && _connecting is null
        )
        {
            // The reading was made for a table that is already on screen: what it holds is given back rather
            // than kept beside the one the rows were drawn from.
            reading.Status?.Dispose();

            return;
        }

        Show(said, rows, reading.Status);
    }

    /// <summary>
    /// Puts a reading on screen, which is what both ways of drawing come to.
    /// </summary>
    /// <remarks>
    /// One place rather than two, because the two callers differ only in where the rows came from: the reading
    /// on its timer, and a redraw of the rows already on screen. Everything below is about the drawing rather
    /// than about the reading.
    /// </remarks>
    /// <param name="said">What is said in the middle, or nothing when the table is what is shown.</param>
    /// <param name="rows">The rows to show.</param>
    /// <param name="reading">The reading the rows were made from, which a sync row's count is read from.</param>
    private void Show(string? said, IReadOnlyList<TableRow> rows, RolaStatus? reading)
    {
        var previous = _reading;

        _shown = new Shown(said, rows);
        _reading = reading;

        // The rows the old reading answered for are being replaced, so its handle is given back with them.
        // A redraw of the same rows passes the same reading, which is still the one on screen.
        if (!ReferenceEquals(previous, reading))
        {
            previous?.Dispose();
        }

        // A connection whose row is no longer in the table is nothing to finish, and a reader is not told about
        // it: the work moved under them, and what the button was about is gone. What is left is the table as the
        // reading now says it, which is what a reader is looking at either way.
        if (_connecting is { Length: > 0 } connecting && !rows.Any(row => Path(row) == connecting))
        {
            _connecting = null;
        }

        _blocks.Children.Clear();

        _notice.Text = said;
        _notice.IsVisible = said is { Length: > 0 };

        if (rows.Count > 0)
        {
            _blocks.Children.Add(Table(rows, BaseDirectory.Current ?? string.Empty, _host, this));
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
        && left.Pills.Zip(right.Pills).All(pair => pair.First == pair.Second)
        && left.Notes.SequenceEqual(right.Notes, StringComparer.Ordinal);

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
    /// <param name="at">Where the run is made, which is what a row's paths are read from.</param>
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    /// <param name="view">The control the rows belong to, which is what a connection is remembered on.</param>
    /// <returns>The table to show.</returns>
    private static Control Table(IReadOnlyList<TableRow> rows, string at, IPluginHost host, OverviewView view)
    {
        var list = new ListBox
        {
            ItemsSource = rows,
            Background = Brushes.Transparent,
            Padding = new Thickness(0),
            BorderThickness = new Thickness(0),
            ItemTemplate = new FuncDataTemplate<TableRow>((row, _) => Row(row, at, host, view), true),
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
        Place(heads, Head(RolaI18N.Get("rorolala_vcs.status.column_operations")), 3);

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

    /// <summary>One row of the table: what the path's state is, the paths it is between, and what to do about it.</summary>
    /// <param name="row">The row.</param>
    /// <param name="at">Where the run is made, which is what the row's paths are read from.</param>
    /// <param name="host">The host, which is what a button reaches to run and to report.</param>
    /// <param name="view">The control the row belongs to, which is what a connection is remembered on.</param>
    /// <returns>The row to show.</returns>
    private static Control Row(TableRow row, string at, IPluginHost host, OverviewView view)
    {
        var cells = new Grid { ColumnDefinitions = Definitions() };

        Place(cells, States(row, at, view), 0);
        Place(cells, Cell(row.From), 1);
        Place(cells, Cell(row.To), 2);
        Place(cells, Cell(row, at, host, view), 3);

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
    /// <param name="at">Where the reading was made, whose paths a sync row's count is read against.</param>
    /// <param name="view">The control the row belongs to, which is what a sync row's words are read from.</param>
    /// <returns>The pills to show.</returns>
    private static Control States(TableRow row, string at, OverviewView view)
    {
        var state = new WrapPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Center,
        };

        foreach (var pill in row.Pills)
        {
            state.Children.Add(
                pill.State is State.Send or State.Receive or State.Create or State.Diverged
                    ? Pill(pill, SyncSaid(pill.State, row.From, at, view._reading))
                    : Pill(pill)
            );
        }

        // What the row says that is not a state is written rather than coloured, and written after the states:
        // a note is what a row is about, where a pill is what has to be done about it.
        foreach (var note in row.Notes)
        {
            if (note is { Length: > 0 })
            {
                state.Children.Add(Note(note));
            }
        }

        return state;
    }

    /// <summary>One note: what a row has to say that is not a state, in the ink the paths are read in.</summary>
    /// <param name="said">What it says.</param>
    /// <returns>The note to show.</returns>
    private static TextBlock Note(string said)
    {
        var note = new TextBlock
        {
            Text = said,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 2, 6, 2),
            Classes = { "caption", "mono" },
        };

        note[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(QuietFill);

        return note;
    }

    /// <summary>
    /// One pill: a rounded tag of its letters, in the state's own colour, with the whole of it said on hover.
    /// </summary>
    /// <remarks>
    /// The line and the letters are the state's colour and the ground behind them is that colour laid thin, so
    /// a pill is one thing said rather than a colour with a label stuck on it: the tint is what tells the pill
    /// from the row it stands on, and the colour entire is what makes it a mark. Laid thin rather than mixed
    /// down, so that one pill reads on the light look and the dark one without being told which it is on — what
    /// stands behind the colour is whatever the row is drawn on.
    /// <para>
    /// Every pill is the same size, whatever its letters: they are read as a column of marks rather than as
    /// words, and a column whose marks grew and shrank with what they said would be one a reader has to measure
    /// rather than scan. One letter and two both fit the width, so what a state is known by costs the column
    /// nothing.
    /// </para>
    /// </remarks>
    /// <param name="pill">The pill.</param>
    /// <param name="said">What the pointer rests on it for, where it is not what the pill was made with.</param>
    /// <returns>The pill to show.</returns>
    private static Border Pill(StatusPill pill, string? said = null)
    {
        var words = new TextBlock
        {
            Text = pill.Letters,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            // The state's own colour rather than a shade of it, because the ground is that colour laid thin:
            // words in a shade would be a third colour in a tag that is meant to be one.
            Foreground = new SolidColorBrush(pill.Fill),
            // Heavier than the words around it, since a pill is a state said rather than a value read. A bold
            // face is also what makes one letter read as a mark rather than as the beginning of a word.
            FontWeight = FontWeight.Bold,
            Classes = { "caption" },
        };

        var made = new Border
        {
            Child = words,
            // One size for every state: the letters are the column's marks, and a mark that changed size with
            // what it said would be a column that has to be measured rather than read.
            Width = PillWidth,
            Height = PillHeight,
            CornerRadius = TagRadius,
            Background = new SolidColorBrush(WithAlpha(pill.Fill, PillWash)),
            BorderThickness = new Thickness(1),
            BorderBrush = new SolidColorBrush(pill.Fill),
            // The room a pill keeps around it is its own rather than the panel's, since a panel spaces lines
            // and not the things on them: what wraps onto the next line has to be told from the line above.
            Margin = new Thickness(0, 2, PillGap, 2),
            VerticalAlignment = VerticalAlignment.Center,
        };

        // What the letters stand for is on the pill, at once: a reader who has stopped at one is asking what it
        // is, and nothing is said by making them wait that the pill's own colour has not said already.
        ToolTip.SetTip(made, said ?? pill.Hint);
        ToolTip.SetShowDelay(made, 0);

        return made;
    }

    /// <summary>
    /// How thin the colour behind a pill is laid.
    /// </summary>
    /// <remarks>
    /// Enough that the pill is a shape rather than a word in a row, and no more: past a fifth or so the tint
    /// stops being a tint of the row and starts being a surface of its own, which is the loudness a table of
    /// states does not want.
    /// </remarks>
    private const byte PillWash = 0x22;

    /// <summary>How round a pill is, which is the design's radius for a small control.</summary>
    private static readonly CornerRadius TagRadius = new(5);

    /// <summary>
    /// How large a pill is, and how far one stands from the next.
    /// </summary>
    /// <remarks>
    /// Wide enough for the two letters a state may be known by and no wider, since the marks are read across
    /// rather than word by word; as tall as the words beside them, so that a row of states and the paths it is
    /// about are read along one line. The gap is the pill's own rather than the panel's, because a panel spaces
    /// lines and not the things on a line.
    /// </remarks>
    private const double PillWidth = 34;

    /// <inheritdoc cref="PillWidth" />
    private const double PillHeight = 18;

    /// <inheritdoc cref="PillWidth" />
    private const double PillGap = 6;

    /// <summary>
    /// How wide the column of states is: three pills and the room between them.
    /// </summary>
    /// <remarks>
    /// Three because that is what a row of states comes to before it wraps: a move whose file was edited is two,
    /// and three is the most a reader is given before the rest goes on the next line. A width rather than a
    /// share of the dock, because a column of one- to two-letter marks has nothing to gain from a wider window
    /// — the room belongs to the paths, which is where it is read.
    /// </remarks>
    private const double StateColumn = (3 * PillWidth) + (3 * PillGap);

    /// <summary>
    /// How wide a button is, and how far one stands from the next.
    /// </summary>
    /// <remarks>
    /// Wide enough for the longest words a button ever carries — 「获取所有权」 — and no wider, since the column
    /// is taken from the paths beside it. The width is stated rather than left to the words, because the words
    /// change while a connection is being made and the column must not: what a reader is doing is reading three
    /// buttons that say what they are part of, and a button that moved out from under the pointer between the
    /// two presses would be a button that has to be found twice.
    /// </remarks>
    private const double ButtonWidth = 84;

    /// <inheritdoc cref="ButtonWidth" />
    private const double ButtonGap = 4;

    /// <summary>
    /// How wide the column of operations is: three buttons and the room between them.
    /// </summary>
    /// <remarks>
    /// Three because that is the most a row is ever offered — a loss wants rebuilding, connecting and confirming,
    /// and a likely move wants confirming, taking back and connecting — so the column holds them all on one line
    /// rather than wrapping the third under the others. A row with more than fits still wraps, which is what the
    /// WrapPanel is for and what makes a row grow rather than lose a button.
    /// </remarks>
    private const double OperationsColumn = (3 * ButtonWidth) + (3 * ButtonGap);

    /// <summary>One colour with the alpha a pill's ground is laid at, over whatever the row is drawn on.</summary>
    /// <param name="colour">The state's colour.</param>
    /// <param name="alpha">How thin to lay it.</param>
    /// <returns>The colour to fill with.</returns>
    private static Color WithAlpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);

    /// <summary>The columns every row and the band are laid out by.</summary>
    /// <returns>The width of each column.</returns>
    private static ColumnDefinitions Definitions()
    {
        var columns = new ColumnDefinitions();

        foreach (var width in Columns)
        {
            columns.Add(new ColumnDefinition(width));
        }

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

    /// <summary>What the dock is showing, which is what a reading is compared against.</summary>
    /// <param name="Said">The line said in place of a reading, or nothing when there is one.</param>
    /// <param name="Rows">The rows the reading came to, which are none when it says a work is clean.</param>
    private sealed record Shown(string? Said, IReadOnlyList<TableRow> Rows);
}

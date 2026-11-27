using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Shapes;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Media.Transformation;
using Avalonia.Styling;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using Path = Avalonia.Controls.Shapes.Path;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Overview dock: how the work stands beside the Layout it works from.
/// </summary>
/// <remarks>
/// One table of the changes, read as the file browser reads a directory: a band naming the columns over
/// rows of thirty pixels, told apart by a line. A row is one thing the reading says about one path, what
/// kind of change it is, where it comes from and where it goes, and what became of its content, so the
/// work is read down a column to see what is of a kind, and across a row to see what is of a path.
/// <para>
/// The words are this dock's own rather than the command line's, because the two are read differently: a
/// command's report is read once and acted on by typing, while a panel is looked at. What the two share
/// is the finding, which comes from the C ABI through <see cref="StatusReading"/> and is read nowhere else.
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
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="sync">What a sync would carry, shared with the Version Graph.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout, shared with the Version Graph.</param>
    public OverviewDock(
        IPluginHost host,
        ILog log,
        IFileChanges files,
        IProgressHub hub,
        SyncWatcher sync,
        VaultFetch fetch
    ) => _view = new OverviewView(host, log, files, hub, sync, fetch);

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
    /// It is drawn as its pictures rather than as its words, the reading's own `refresh` first, then this
    /// plugin's three for the sync, because that is what a header is for: each plugin draws its own marks, so
    /// one plugin's keys stay its own.
    /// </para>
    /// <para>
    /// Reading the work again is offered as the strip's own command as well, because a header drawn as a menu,
    /// a dock in a window of its own, has nowhere to put a control: without that, a window holding this dock
    /// would have no way to ask for a reading, which is a way it had before the strip existed.
    /// </para>
    /// </remarks>
    public IReadOnlyList<DockHeaderItem> Header =>
        [
            new DockHeaderView(
                _view.Strip,
                new DockHeaderCommand("rorolala_vcs.status.refresh", _view.Refresh, IconKey: Icons.ArrowClockwise)
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
internal sealed record OverviewReading(StatusFacts? Facts, string? Said, StatusReading? Status = null);

/// <summary>
/// A reading as the table reads it: what to show, without the reading itself.
/// </summary>
/// <remarks>
/// It is what the table is built from, and it is a type of this plugin's own rather than
/// <see cref="StatusReading"/> so that what a reading is turned into can be read, and checked, without a
/// Workspace to read one from: the native side is a handle that can only be made by looking at a tree.
/// </remarks>
/// <param name="Lost">Paths the Layout names that the tree does not hold.</param>
/// <param name="Untagged">Paths the tree holds that the Layout does not name.</param>
/// <param name="Ignored">Paths the tree holds that the Layout does not name and the ignore rules govern.</param>
/// <param name="Renamed">Paths that moved.</param>
/// <param name="Modified">Paths that changed where they were, move destinations included.</param>
/// <param name="Unowned">Paths the fetched copy says another account holds.</param>
/// <param name="WrongBase">Paths whose version is behind or apart from the one upstream is at.</param>
/// <param name="Merging">Files a variant is waiting to be joined into.</param>
/// <param name="Broken">Merges that cannot go on, one reason each.</param>
/// <param name="Sends">Paths a sync would send the version of, being further on than the Vault.</param>
/// <param name="Receives">Paths a sync would bring the Vault's version down to.</param>
/// <param name="Apart">Paths whose two chains have each moved on, which only a forced sync sends.</param>
/// <param name="Unclaimed">Paths the fetched copy leaves unheld, which changed here.</param>
internal sealed record StatusFacts(
    IReadOnlyList<string> Lost,
    IReadOnlyList<string> Untagged,
    IReadOnlyList<string> Ignored,
    IReadOnlyList<Rename> Renamed,
    IReadOnlyList<string> Modified,
    IReadOnlyList<string> Unowned,
    IReadOnlyList<string> WrongBase,
    IReadOnlyList<Merge> Merging,
    IReadOnlyList<BrokenReason> Broken,
    IReadOnlyList<string> Sends,
    IReadOnlyList<string> Receives,
    IReadOnlyList<string> Apart,
    IReadOnlyList<string>? Unclaimed = null
)
{
    /// <summary>Takes the facts out of a reading.</summary>
    /// <param name="status">The reading.</param>
    /// <returns>What the table is built from.</returns>
    public static StatusFacts Of(StatusReading status)
    {
        ArgumentNullException.ThrowIfNull(status);

        return new StatusFacts(
            status.Lost,
            status.Untagged,
            [.. status.Ignored],
            status.Renamed,
            status.Modified,
            status.Unowned,
            status.WrongBase,
            status.Merging,
            status.Broken,
            // The three what a sync would do are the reading's whole list of standings, asked once each: every
            // path the Layout records a version for is in it, whether or not the tree changed it. They are not
            // read from `WrongBase`, which is the changes the tree has made against a base upstream moved past,
            // a file nobody touched but the Vault has moved past is a receive too, and would otherwise have no row.
            [.. status.Ahead],
            [.. status.Behind],
            [.. status.Apart],
            Unclaimed: status.Unclaimed
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
/// The colour is this plugin's own, see <see cref="RorolalaColours"/>, rather than a role of the look's,
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

    /// <summary>What this account may record once it claims it, which nobody holds yet.</summary>
    Unclaimed,

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
/// <param name="IconKey">The key of the picture the badge draws.</param>
/// <param name="Fill">What the pill is coloured, which is this plugin's colour rather than the look's.</param>
/// <param name="Hint">
/// What the pointer resting on the pill says: the state's name and the whole of what it means, since the
/// picture says only which state it is.
/// </param>
internal sealed record StatusPill(State State, string IconKey, Color Fill, string Hint);

/// <summary>One row of the table: what the reading says about one path.</summary>
/// <param name="Pills">
/// What the path's state is, one pill each: what kind of change it is, and what became of its content.
/// A change of content alone has no kind, and a kind alone has nothing to say about a content, so a row
/// carries as many pills as it has states, one or two, and more where a merge has something to add.
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
/// Which of the two paths the operations act on, where it is neither of the ones the rule picks, see
/// <see cref="OverviewView.Path"/>. Nothing for every row whose operations act on where its file is now.
/// </param>
/// <param name="Ignored">
/// Whether the row is a path the ignore rules govern rather than one the reading reports as work. Such a
/// row is only drawn when the ignored are being shown, and its two path columns are drawn in the ink kept
/// for what is not the work, which is what tells a reader at a glance that nothing is to be done with it.
/// </param>
/// <param name="Tally">
/// Whether the row is one a sync would move, which shows how far apart the two versions are instead of a
/// note.
/// </param>
internal sealed record TableRow(
    IReadOnlyList<StatusPill> Pills,
    IReadOnlyList<string> Notes,
    string From,
    string To,
    IReadOnlyList<Operation> Operations,
    string? Act = null,
    bool Ignored = false,
    bool Tally = false
);

/// <summary>One group of a reading: its heading, and the rows under it.</summary>
/// <param name="Key">The stable name the group is remembered by, which collapse is kept against.</param>
/// <param name="Label">What its heading says.</param>
/// <param name="Rows">The rows under it.</param>
/// <param name="Ignored">Whether the group is the paths the ignore rules govern.</param>
internal sealed record RowGroup(
    string Key,
    string Label,
    IReadOnlyList<TableRow> Rows,
    bool Ignored = false
);

/// <summary>
/// What the Overview shows: the work as it stands, read where the tree is rooted.
/// </summary>
/// <remarks>
/// The reading is made when the dock appears and whenever something says it may have changed, the tree
/// being rooted elsewhere, the files having been changed, or one of the host's windows being come back to.
/// Nothing polls: a reading is a walk of the tree, and one made on a timer would spend most of its time
/// saying what the last one said.
/// <para>
/// The events it listens to are not all raised on the window's thread, a plugin that changed a file says
/// so from the work it did it on, so what is drawn is drawn through the dispatcher. A control belongs to
/// the thread that made it, and this one is made by the host.
/// </para>
/// </remarks>
internal sealed class OverviewView : UserControl
{
    /// <summary>How wide the line said in place of a reading is allowed to read.</summary>
    private const double Widest = 420;



    /// <summary>The colour a mark of one kind is filled with, which is the look's semantic state.</summary>
    /// <remarks>
    /// Four colours and nothing else: what is not recorded yet, what is under way, what must not be recorded,
    /// and what is somewhere else than the Layout says. They are read from the look's semantic set rather than
    /// kept here, because what a state is coloured is the state, see <see cref="RorolalaColours"/>.
    /// </remarks>
    private static Color WrongFill => RorolalaColours.Wrong;

    /// <summary>What has appeared.</summary>
    private static Color AppearedFill => RorolalaColours.Untagged;

    /// <summary>What moved.</summary>
    private static Color MovedFill => RorolalaColours.Moved;

    /// <summary>What is not settled: what is under way, and what has to be read twice.</summary>
    private static Color UnsettledFill => RorolalaColours.UnderWay;

    /// <summary>What is waiting for a variant to be joined into it.</summary>
    private static Color MergeFill => RorolalaColours.StatusDelta;

    /// <summary>The theme resource for what has nothing to say about itself, which a note is read in.</summary>
    /// <remarks>
    /// The second step of the ink rather than the third: a note is a sentence about the work and is
    /// read, where the third step is kept for what is drawn past. A table of changes has several notes
    /// in a column, so a step too faint is a column the reader has to work at.
    /// </remarks>
    private const string QuietFill = PaletteKeys.Muted;

    /// <summary>
    /// The theme resource a path the ignore rules govern is read in.
    /// </summary>
    /// <remarks>
    /// The look's second ink rather than its third, for the reason the notes use it: an ignored path is
    /// still a path the reader may want to read, and a step that fails at 11 px is a step that reads as
    /// disabled whether or not it is.
    /// </remarks>
    private const string QuietInk = PaletteKeys.Muted;

    /// <summary>The theme resource a line told apart from the next is drawn in.</summary>
    private const string EdgeInk = PaletteKeys.Splitter;

    /// <summary>How often the work is read again while the dock is on screen.</summary>
    /// <remarks>
    /// Nothing tells the program that another program changed a file, an artist saving a picture changes
    /// the tree with this window hearing nothing of it, so a reading is asked for on a timer as well as
    /// when something says the files have changed. Five seconds is long enough that a reading is not what
    /// the program spends its time on, and short enough that a save made elsewhere is answered before the
    /// reader has looked away from the row it changes.
    /// </remarks>
    private static readonly TimeSpan Every = TimeSpan.FromSeconds(5);

    /// <summary>The line said when there is no reading to draw.</summary>
    private readonly TextBlock _notice;

    /// <summary>
    /// Where the table of a reading goes.
    /// </summary>
    /// <remarks>
    /// One cell rather than a stack, and a cell the table fills: the reading is the view rather than a block of
    /// it, and what is under its last row is the table's own room rather than the window's ground showing
    /// through a table that stopped where its rows did.
    /// </remarks>
    private readonly Grid _tableCell;

    /// <summary>Where a reading that could not be made is said.</summary>
    private readonly ILog _log;

    /// <summary>What the files may have done, which a reading is made again for.</summary>
    private readonly IFileChanges _files;

    /// <summary>What asks for a reading while nothing else does: a file changed by another program.</summary>
    private readonly DispatcherTimer _again;

    /// <summary>The host, which is what a row's buttons reach to run and to report.</summary>
    private readonly IPluginHost _host;

    /// <summary>
    /// The row a connection was begun from, or nothing while none is.
    /// </summary>
    /// <remarks>
    /// Held by the row rather than by a button, because what is connected to what is a fact about two rows and
    /// not about the control that was pressed: the table is drawn again under a reader, by the reading on its
    /// timer, and a connection kept in a button would be lost the moment the button was thrown away.
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
    /// of which acts on what the first has half done. The buttons on screen are told when it changes rather
    /// than the table being built again, so that a run costs the list no scroll and no flash.
    /// </remarks>
    private bool _busy;

    /// <summary>The operation buttons on screen, so that a run holds them without the list being built again.</summary>
    private readonly List<(Button Button, bool Begun, bool Candidate)> _buttons = [];

    /// <summary>Whether a redraw is already posted, so a burst of changes is one drawing.</summary>
    private bool _redrawAsked;

    /// <summary>Whether a track begun from the To track group is going, so a second Enter starts no second run.</summary>
    private bool _tracking;

    /// <summary>The one fetch of the Vault's Layout, shared with the Version Graph.</summary>
    private readonly VaultFetch _fetch;

    /// <summary>The line under the reading that says which Vault is fetched and how the last fetch went.</summary>
    private readonly FetchStatusBar _status;

    /// <summary>The Vault the work tracks, or the Workspace's default one, as the status bar names it.</summary>
    private string _vaultName = string.Empty;

    /// <summary>The address the Vault answers at, as the status bar shows it.</summary>
    private string _vaultAddress = string.Empty;

    /// <summary>Which reading of the Vault's name and address is the newest, so an old one does not land.</summary>
    private int _vaultReading;

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
    /// It is owned here: a reading that is replaced, or one a newer reading says the same as, and so is not drawn
    ///, is given back rather than left to the native side.
    /// </para>
    /// </remarks>
    private StatusReading? _reading;

    /// <summary>Whether a reading is already waiting to be made, so a burst of asks is one reading.</summary>
    private bool _drawAsked;

    /// <summary>Whether a reading of the work is being made on its own thread, so one is going at a time.</summary>
    private bool _readingGoing;

    /// <summary>Whether a reading was asked for while one was being made, so the one after it is drawn.</summary>
    private bool _readAgain;

    /// <summary>What is on screen now, so that a reading saying the same thing is not drawn again.</summary>
    private Shown? _shown;

    /// <summary>The groups the reader has folded shut, by their stable key.</summary>
    private readonly HashSet<string> _collapsed = new(StringComparer.Ordinal) { "ignored" };

    /// <summary>
    /// The directory and the sync-relevant standings the last plan was asked over, so a reading that moves
    /// either asks again.
    /// </summary>
    private string? _plannedFor;

    /// <summary>Makes the control.</summary>
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    /// <param name="log">Where a reading that could not be made is said.</param>
    /// <param name="files">What the files may have done.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="sync">What a sync would carry, shared with the Version Graph.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout, shared with the Version Graph.</param>
    public OverviewView(
        IPluginHost host,
        ILog log,
        IFileChanges files,
        IProgressHub hub,
        SyncWatcher sync,
        VaultFetch fetch
    )
    {
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(sync);
        ArgumentNullException.ThrowIfNull(fetch);

        _host = host;
        _log = log;
        _files = files;
        _hub = hub;
        _sync = sync;
        _fetch = fetch;

        _notice = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            TextAlignment = TextAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            MaxWidth = Widest,
            Classes = { "muted" },
        };

        _tableCell = new Grid();
        _status = new FetchStatusBar();

        // The sync's own state, which the strip in the dock's header draws from; what the runs of every command
        // say is gathered in the hub, which the floating capsules draw from and which every place a command
        // may be run from (the strip, an entry's menu) writes to.
        _strip = new SyncStrip(_host, _sync, () => BaseDirectory.Current, Refresh);

        var root = new Grid { RowDefinitions = new RowDefinitions("*,35") };

        // One column: what a command is doing floats over the bottom right of the reading rather than taking a
        // column of it, so the table keeps its width whether a run is going or not.
        //
        // Nothing scrolls the whole of the reading: the list of rows scrolls inside itself, see `Table`, and a
        // table that fills the dock and is then put in a scroller would be measured with no end to fill and so
        // would fill nothing, which is the height this view had before the table was one of its own.
        var reading = new Grid { Children = { _tableCell, _notice } };

        root.Children.Add(reading);
        Grid.SetRow(_status, 1);
        root.Children.Add(_status);

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

            // The status bar is what says which Vault is fetched: read its name and address when the dock
            // appears and when the base moves, and follow what the fetch itself says about its state.
            _fetch.Changed += Fetched;
            BaseDirectory.Changed += ReadVault;
            ReadVault();

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
            _fetch.Changed -= Fetched;
            BaseDirectory.Changed -= ReadVault;
            _hub.Changed -= Sizing;
        };

        // A dock in a tab that is shown again is being looked at again, which is a moment to ask what
        // there is to carry, see `BecameVisible`.
        PropertyChanged += BecameVisible;

        // A run that has just ended is the fetch or the sync the reading was held for, and the capsule of one that
        // has ended fades out of the stack while the reading is read again.
        _running = hub.Any;

        Draw();
    }

    /// <summary>Asks what there is to carry when the dock is shown again, and reads the work again with it.</summary>
    /// <remarks>
    /// A dock in a tab that was not shown is not being looked at, and the timer that reads the work does not run
    /// for it. A command run elsewhere (the terminal's <c>rola track</c>, say) is not one this plugin is told
    /// about by the files, so coming back to the Overview is the moment what a sync would carry is asked again
    /// rather than left as it was when the tab was left. It is one ask per becoming visible rather than a second
    /// timer: a plan costs a fetch of the Vault's Layout, and one made while nobody is looking is work for
    /// nothing.
    /// </remarks>
    /// <param name="sender">What raised it.</param>
    /// <param name="changed">The property and its values.</param>
    private void BecameVisible(object? sender, AvaloniaPropertyChangedEventArgs changed)
    {
        _ = sender;

        if (changed.Property != IsVisibleProperty || changed.NewValue is not true)
        {
            return;
        }

        Draw();
        _sync.Plan(BaseDirectory.Current);
    }

    /// <summary>Reads the work again when the runs end, which is what a fetch or a sync writes the reading from.</summary>
    private void Sizing()
    {
        // A run that has just ended is the fetch or the sync the reading was held for: what it wrote is what the
        // work is read from now, and nothing else will say so, see <see cref="Drawn"/>.
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

    /// <summary>What every command this plugin runs reports itself to, which the capsule stack follows.</summary>
    private readonly IProgressHub _hub;

    /// <summary>The strip the dock's header carries.</summary>
    private readonly SyncStrip _strip;

    /// <summary>
    /// Fetches the Vault's Layout and reads the work again, which is what the dock's own command asks for.
    /// </summary>
    /// <remarks>
    /// What is fetched is the copy every reading of the work is made from, so a reading made before it would say
    /// what the Vault held when the copy was last brought over. The run is shown as a capsule in the stack, and the
    /// reading and what a sync would carry are asked for once it is over. A directory no Workspace
    /// holds has nothing to fetch, so it is only read again.
    /// <para>
    /// A sync is what brings the same copy, so a refresh while one is going asks nothing: the run's own end is
    /// what reads the work again.
    /// </para>
    /// </remarks>
    public void Refresh()
    {
        if (_sync.Running)
        {
            return;
        }

        if (BaseDirectory.Current is { Length: > 0 } at && WorkspaceNotice.Holds(at, _log))
        {
            _fetch.Fetch(
                at,
                () =>
                {
                    Draw();
                    _sync.Plan(at);
                }
            );

            return;
        }

        Draw();
        _sync.Plan(BaseDirectory.Current);
    }

    /// <summary>
    /// What the Overview makes of a directory the tree is rooted at.
    /// </summary>
    /// <remarks>
    /// A directory that has not been said, or that is the computer, neither of which is somewhere a
    /// Workspace can be found from, is answered the way a directory no Workspace holds is. A Workspace
    /// that holds it and still answers nothing is told apart from that, because the two are different
    /// things to have happened: nothing is being looked at, or the work that is being looked at could not
    /// be read.
    /// </remarks>
    /// <param name="directory">The directory the tree is rooted at.</param>
    /// <param name="log">Where a read that failed is said.</param>
    /// <returns>The reading, or the words to show instead of one.</returns>
    // The scope holds only the reading, and the view keeps the reading for as long as its rows are on screen:
    // disposing the scope here would give back what Show answers for, so the reading is the one owner.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the scope holds only the reading, which the view keeps and gives back in Show"
    )]
    public static OverviewReading Of(string? directory, ILog log)
    {
        ArgumentNullException.ThrowIfNull(log);

        if (directory is not { Length: > 0 } at)
        {
            return new OverviewReading(null, RolaI18N.Get("rorolala_vcs.not_a_workspace"));
        }

        StatusReading? status = null;

        try
        {
            // What is read is handed back to the caller rather than given back here: a row's count is asked of
            // the reading as it is drawn, so the reading has to live as long as the rows do. It is given back
            // when the reading is replaced or discarded, see <see cref="Show"/> and <see cref="Drawn"/>.
            //
            // The scope is not kept: disposing it would give the same reading back, and what this view lets go
            // of when it is done with a reading is the reading itself.
            var scope = new StatusScope(at);
            scope.Read();
            status = scope.Status;

            return new OverviewReading(StatusFacts.Of(status), null, status);
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.Unavailable or RolaReadErrorKind.NoWorkspace)
        {
            // No Workspace, or a Workspace working in no Layout, is the absence the V1 call answered with
            // nothing rather than a read that failed: whether a Workspace holds the base is what tells the
            // two lines apart, and nothing is said in the log for either.
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
    /// <para>
    /// A path the ignore rules govern is not work, the reading keeps the two lists apart, so its row is
    /// read in its own group, which the view folds shut by default.
    /// </para>
    /// </remarks>
    /// <param name="facts">The reading.</param>
    /// <returns>The rows to show.</returns>
    internal static IReadOnlyList<RowGroup> Rows(StatusFacts facts)
    {
        ArgumentNullException.ThrowIfNull(facts);

        // Read once and used for every row, since a table is one drawing: what a kind is called is the same
        // word on every row it names. Every one is asked for by a literal so that the gate can see it, and the
        // letters are the pill's own, so what a state is known by is stated where the state is made rather than
        // read from the catalogue.
        var stopped = RolaI18N.Get("rorolala_vcs.status.merge_broken");

        List<TableRow> wrong = [];
        List<TableRow> merges = [];
        List<TableRow> structural = [];
        List<TableRow> changes = [];
        List<TableRow> syncing = [];
        List<TableRow> governed = [];

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
        moving.UnionWith(facts.Unclaimed ?? []);

        wrong.AddRange(
            facts.Unowned.Select(path =>
                new TableRow([Pill(State.NotMine)], [], path, string.Empty, OverviewOperations.Of(State.NotMine))
            )
        );

        wrong.AddRange(
            (facts.Unclaimed ?? []).Select(path =>
                new TableRow([Pill(State.Unclaimed)], [], path, string.Empty, OverviewOperations.Of(State.Unclaimed))
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
                    // the row's one button acts on is the variant file, the one a merge is ended at, so the
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
                    // wears the colour of what is wrong rather than the colour of a merge that is, and what is
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
        // the work that is in step with nothing wrong about it, which is the ordinary state of a file, and the
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

        // What the ignore rules govern is what the tree gained and the reading kept apart from the
        // work. It is drawn last, because it is not work: a reader scans past it rather than acting on
        // it, and a row that stood among the changes would read as one of them. Nothing is offered to
        // do with it, a recording refuses a path the rules govern, so the row carries no buttons.
        governed.AddRange(
            facts.Ignored.Select(path =>
                new TableRow([Pill(State.Untagged)], [], path, string.Empty, [], Ignored: true)
            )
        );

        List<RowGroup> groups = [];

        if (wrong.Count + merges.Count > 0)
        {
            groups.Add(
                new RowGroup(
                    "confirm",
                    RolaI18N.Get("rorolala_vcs.status.group_confirm"),
                    [.. wrong, .. merges]
                )
            );
        }

        if (structural.Count + changes.Count > 0)
        {
            groups.Add(
                new RowGroup(
                    "track",
                    RolaI18N.Get("rorolala_vcs.status.group_track"),
                    [.. structural, .. changes]
                )
            );
        }

        if (syncing.Count > 0)
        {
            groups.Add(new RowGroup("sync", RolaI18N.Get("rorolala_vcs.status.group_sync"), syncing));
        }

        if (governed.Count > 0)
        {
            groups.Add(
                new RowGroup(
                    "ignored",
                    RolaI18N.Get("rorolala_vcs.status.group_ignored"),
                    governed,
                    Ignored: true
                )
            );
        }

        return groups;

        // One path of what a sync would move, as a row of its own. It offers no button: what is to be done about it
        // is the sync the strip in the header runs, and a button here would be a second way to ask for the same
        // run, one that would have to say what it carries, which the strip already says.
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
                [],
                Tally: true
            );

        // Whether a move's content is a change that has to be acted on rather than one that is simply to be
        // recorded, which is what decides the group it is read in.
        bool Misfit(Rename rename) => rename.Modified && behind.Contains(rename.To);

        // One move, as a row. A move whose file was edited is a change of content as well, so what became of
        // the content is a pill of the move's own row rather than a row of its own, and a move that changed
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
            State.Untagged => new StatusPill(state, Icons.PlusBold, AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_untagged")),
            State.Lost => new StatusPill(state, Icons.ExclamationMarkBold, WrongFill, RolaI18N.Get("rorolala_vcs.status.kind_lost")),
            State.NotMine => new StatusPill(state, Icons.LockBold, RorolalaColours.Disallow, RolaI18N.Get("rorolala_vcs.status.change_wrong_unowned")),
            State.Unclaimed => new StatusPill(state, Icons.LockOpen, RorolalaColours.Allow, RolaI18N.Get("rorolala_vcs.status.change_wrong_unclaimed")),
            State.Stale => new StatusPill(state, Icons.WarningCircle, WrongFill, RolaI18N.Get("rorolala_vcs.status.change_wrong_base")),
            State.Renamed => new StatusPill(state, Icons.ArrowRightBold, MovedFill, RolaI18N.Get("rorolala_vcs.status.kind_rename_strong")),
            State.RenamedLikely => new StatusPill(state, Icons.ArrowRightBold, MovedFill, RolaI18N.Get("rorolala_vcs.status.kind_rename_weak")),
            State.Modified => new StatusPill(state, Icons.NotePencilBold, UnsettledFill, RolaI18N.Get("rorolala_vcs.status.change_modified")),
            State.Merging => new StatusPill(state, Icons.GitMergeBold, MergeFill, RolaI18N.Get("rorolala_vcs.status.kind_merge")),
            State.Broken => new StatusPill(state, Icons.ExclamationMarkBold, WrongFill, RolaI18N.Get("rorolala_vcs.status.kind_merge")),
            State.Send => new StatusPill(state, Icons.ArrowUpBold, AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_send")),
            State.Receive => new StatusPill(state, Icons.ArrowDownBold, AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_receive")),
            State.Create => new StatusPill(state, Icons.PlusBold, AppearedFill, RolaI18N.Get("rorolala_vcs.status.kind_create")),
            _ => new StatusPill(state, Icons.GitForkBold, WrongFill, RolaI18N.Get("rorolala_vcs.status.kind_diverged")),
        };

    /// <summary>
    /// What a path a sync would move is hovered for: which way the work moves, how far apart the two versions are
    /// where the index says, and what a sync comes to afterwards.
    /// </summary>
    /// <remarks>
    /// The count is asked of the reading rather than taken from the row, because it is the one thing about a
    /// standing that the reading keeps per path: what a pill carries is the direction, and how far is in the words.
    /// A count the chain does not give is not said at all, what the Vault is at is ordinarily a version this side
    /// never fetched, since a zero would read as being in step, which is the one thing the row is not about.
    /// </remarks>
    /// <param name="state">Which of the four the path is in.</param>
    /// <param name="path">The path.</param>
    /// <param name="at">Where the reading was made, whose paths are relative to it.</param>
    /// <param name="reading">The reading, or nothing when it could not be made.</param>
    /// <returns>The words for the pointer resting on it.</returns>
    internal static string SyncSaid(State state, string path, string? at, StatusReading? reading)
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
            Icons.GitMergeBold,
            MergeFill,
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
    /// Held rather than made where a row is, because what a button does needs the host, and the rows are built
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

        if (Recordable(row) is { } file)
        {
            // The row's own track, to the left of what is done about it: a run of one file, where the heading's
            // button runs every file the group holds.
            operations.Children.Add(view.NewTrackButton(file, host));
        }

        foreach (var operation in row.Operations)
        {
            operations.Children.Add(view.NewButton(operation, row, at, host));
        }

        return operations;
    }

    /// <summary>The path a track from a row would record, or nothing when the row is not one a track records.</summary>
    /// <param name="row">The row.</param>
    /// <returns>The path, or nothing.</returns>
    private static string? Recordable(TableRow row) =>
        row.From is { Length: > 0 } file
        && row.Pills.Any(pill => pill.State is State.Modified or State.Untagged)
            ? file
            : null;

    /// <summary>A row's own track, as the picture button it wears.</summary>
    /// <remarks>
    /// It stands before what the row offers to put right, since recording the change is what a reader means
    /// first; it opens the same message field the heading's button does, for the one file it is on.
    /// </remarks>
    /// <param name="file">The file the run records.</param>
    /// <param name="host">The host, which is what a failure is shown through.</param>
    /// <returns>The button.</returns>
    private Button NewTrackButton(string file, IPluginHost host)
    {
        var button = new Button
        {
            Content = Glyph(host, Icons.RadioButtonBold, RorolalaColours.Front, ActionGlyph),
            Width = ActionSize,
            Height = ActionSize,
            Padding = new Thickness(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            Margin = new Thickness(0, 0, ButtonGap, 0),
            IsEnabled = !_busy,
        };
        button[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );

        ToolTip.SetTip(button, RolaI18N.Get("rorolala_vcs.version_graph.track_tip"));
        button.Click += (_, _) => AskTrack([file], host, button);

        _buttons.Add((button, false, false));

        return button;
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
    /// that is not a side of it is disabled, including the other buttons of the row it began from, so that a
    /// reader who has begun one cannot also ask for something else, which would be two runs over one path.
    /// </para>
    /// <para>
    /// The two sides of it are the two that stand out, and what tells them is what they say rather than how
    /// they are painted: the button that began the connection says that it is waiting, and every button it
    /// could be finished on says that it is an answer (see <see cref="Label"/>). The rest keep their own words
    /// and are not dimmed, the look has no faded state, and inventing one here would be a colour the theme
    /// does not hold, but they do not answer: a disabled button takes no press and takes no pointer, so a
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
            && OverviewOperations.Pairs(Choices(row), StatesAt(from, _shown is { } seen ? Flat(seen.Groups) : []));
        var running = _connecting is not null;

        var (icon, danger) = OperationIcon(operation);
        var ink = danger ? RorolalaColours.Error : RorolalaColours.Front;

        var button = new Button
        {
            Content = Glyph(host, icon, ink, ActionGlyph),
            Width = ActionSize,
            Height = ActionSize,
            Padding = new Thickness(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            BorderThickness = new Thickness(begun || candidate ? 2 : 1),
            CornerRadius = new CornerRadius(0),
            Margin = new Thickness(0, 0, ButtonGap, 0),
            // Everything is pressable while nothing is being connected or run; while a connection is in
            // hand, only the two sides of it are, which is the row's own button again, so that a second
            // press takes it back; and while an operation is going, nothing answers at all.
            IsEnabled = !_busy && (!running || begun || candidate),
        };

        if (danger)
        {
            button.Background = new SolidColorBrush(WithAlpha(RorolalaColours.Error, 0x24));
            button.BorderBrush = new SolidColorBrush(RorolalaColours.Error);
        }
        else
        {
            button[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
            button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);
        }

        // The two sides of a connection are not told apart by their words any more, so the line around
        // them says it: amber where the connection began, green where it can be finished.
        if (begun)
        {
            button.BorderBrush = new SolidColorBrush(RorolalaColours.Warning);
        }
        else if (candidate)
        {
            button.BorderBrush = new SolidColorBrush(RorolalaColours.Success);
        }

        ToolTip.SetTip(button, RolaI18N.Get(Label(operation, begun, candidate)));
        ToolTip.SetShowDelay(button, 0);

        _buttons.Add((button, begun, candidate));

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

    /// <summary>What an operation wears: the picture of the button, and whether it discards the local copy.</summary>
    /// <param name="operation">The operation.</param>
    /// <returns>The picture's key, and whether it is the wrong-colour one.</returns>
    private static (string Icon, bool Danger) OperationIcon(Operation operation) =>
        operation switch
        {
            Operation.RestoreContent or Operation.RestoreMove => (Icons.TrashBold, true),
            Operation.Rebuild => (Icons.ArrowClockwise, false),
            Operation.ConfirmRename or Operation.ConfirmLost => (Icons.CheckCircleBold, false),
            Operation.TakeOwnership => (Icons.LockBold, false),
            Operation.CancelMerge => (Icons.ExclamationMarkBold, false),
            _ => (Icons.ArrowRightBold, false),
        };

    /// <summary>One picture drawn as a mask over an ink, which is how every mark is drawn.</summary>
    /// <param name="host">The host, whose icon library holds the picture.</param>
    /// <param name="key">The picture's key.</param>
    /// <param name="ink">What the picture is drawn in.</param>
    /// <param name="size">How large it is.</param>
    /// <returns>The picture to show.</returns>
    private static Border Glyph(IPluginHost host, string key, Color ink, double size)
    {
        var glyph = new Border
        {
            Width = size,
            Height = size,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(ink),
        };

        if (host.Icons.Find(key) is { } picture)
        {
            glyph.OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform };
            RenderOptions.SetBitmapInterpolationMode(glyph, BitmapInterpolationMode.HighQuality);
        }

        return glyph;
    }

    /// <summary>A sync row's count: the arrow of its direction and how many versions apart the two sides are.</summary>
    /// <param name="row">The row.</param>
    /// <param name="at">Where the reading was made, which the path is relative to.</param>
    /// <param name="host">The host, whose icon library holds the pictures.</param>
    /// <param name="view">The control the row belongs to, which holds the reading.</param>
    /// <returns>The count to show.</returns>
    private static Control Tally(TableRow row, string at, IPluginHost host, OverviewView view)
    {
        var state = row.Pills.Count > 0 ? row.Pills[0].State : State.Send;
        var (icon, colour) = state switch
        {
            State.Receive => (Icons.ArrowDownBold, RorolalaColours.Fail),
            State.Diverged => (Icons.GitForkBold, RorolalaColours.Fail),
            _ => (Icons.ArrowUpBold, RorolalaColours.Success),
        };

        var number = new TextBlock
        {
            Text = DistanceNow(row.From, at, view._reading)?.ToString(CultureInfo.InvariantCulture)
                ?? "?",
            VerticalAlignment = VerticalAlignment.Center,
            FontSize = 12,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(colour),
        };

        var inner = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { Glyph(host, icon, colour, ActionGlyph), number },
        };

        return new Border
        {
            Child = inner,
            Width = 42,
            Height = 28,
            CornerRadius = new CornerRadius(0),
            Background = new SolidColorBrush(WithAlpha(colour, 0x24)),
            BorderThickness = new Thickness(1),
            BorderBrush = new SolidColorBrush(colour),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 4, 0),
        };
    }

    /// <summary>How many versions apart the two sides are, as the reading says, or nothing when it does not know.</summary>
    /// <param name="path">The path, as the reading names it.</param>
    /// <param name="at">Where the reading was made.</param>
    /// <param name="reading">The reading, or nothing.</param>
    /// <returns>The distance, or nothing.</returns>
    private static ulong? DistanceNow(string path, string at, StatusReading? reading)
    {
        if (reading is null || at is not { Length: > 0 })
        {
            return null;
        }

        try
        {
            return RorolalaSync.Distance(reading, System.IO.Path.Combine(at, path));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _ = error;
            return null;
        }
    }

    /// <summary>
    /// What a button says, which is not always what it does.
    /// </summary>
    /// <remarks>
    /// A connection is a question, which of these two is which, and the answer is given by pressing one of
    /// them. So the buttons that are part of the question stop saying what they do and start saying what they
    /// are: the one that began it says that it is waiting, and each one it could be finished on says that it is
    /// an answer. Every other button on the table says what it does, and the two flags are the connection
    /// button's own (see <see cref="NewButton"/>) rather than the row's, since a row's buttons all act on the
    /// row's one path and a flag that went by the path would catch them all.
    /// <para>
    /// Words rather than a mark, because the two sides of a connection are two rows that may be far apart and
    /// may look alike: what a reader has to be able to tell is which of them they started from and which of them
    /// they may finish on, and 「连接中」 and 「组合」 say that where a tint has to be noticed first.
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
        // moved to, whichever of the two rows was clicked first.
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
    /// two runs whose effect is each other's to undo, a mode is not known to be safe against a second mode over
    /// the same tree, so a run takes the whole table: its own row's buttons and every other row's, and the
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
    /// What the buttons say has not changed, only whether they answer, so the table is drawn from the rows
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

        // The buttons are told rather than drawn again: an operation going changes whether they answer and
        // nothing else, and rebuilding the list for it is a scroll lost and a frame spent for one flag.
        var running = _connecting is not null;

        foreach (var (button, begun, candidate) in _buttons)
        {
            button.IsEnabled = !_busy && (!running || begun || candidate);
        }
    }

    /// <summary>
    /// Asks about an operation that cannot be taken back, and runs it when the answer is yes.
    /// </summary>
    /// <remarks>
    /// Putting content back writes over what the file holds now, and what it held is not in the store: the
    /// Layout names a version, and what was changed between them is work that no command can recover. So it is
    /// asked about rather than done, the one operation of the six whose effect a reader might not have meant.
    /// Cancelling a merge is asked about for the same reason and one of its own: it takes the variant file
    /// away, and what that file holds is not kept anywhere the file is not, the record says which variant it
    /// was brought in as, and the content that is to be joined comes back only from the Vault.
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
    /// holds. A move is the one row with both, and what a move's operations act on is where the file is now,
    /// that is the path a reader is looking at, and the one the command can find.
    /// <para>
    /// A merge is the other row with both, and it is the other way round: the two columns read the variant
    /// file being joined and the file it is being joined into, and what a merge's operation acts on is the
    /// variant file, the one a merge is ended at, and the one the command is run on. That is why a row may
    /// say which of its paths to act on rather than leaving it to the rule.
    /// </para>
    /// </remarks>
    /// <param name="row">The row.</param>
    /// <returns>The path, or empty for a row that names none.</returns>
    private static string Path(TableRow row) =>
        row.Act is { Length: > 0 } act ? act
        : row.To is { Length: > 0 } to ? to
        : row.From;

    /// <summary>The rows of a set of groups, in the order they are read.</summary>
    /// <param name="groups">The groups.</param>
    /// <returns>Every row under them.</returns>
    private static IReadOnlyList<TableRow> Flat(IReadOnlyList<RowGroup> groups) =>
        [.. groups.SelectMany(group => group.Rows)];

    /// <summary>The states of the row a path is on, or none when no row names it.</summary>
    /// <param name="path">The path.</param>
    /// <param name="rows">The rows the table is showing.</param>
    /// <returns>The states, which are none for a path no row is about.</returns>
    private static IReadOnlyList<State> StatesAt(string path, IReadOnlyList<TableRow> rows) =>
        rows.FirstOrDefault(row => Path(row) == path) is { } row ? Choices(row) : [];

    /// <summary>Draws what the fetch says about itself in the status bar.</summary>
    private void Fetched() => _status.Show(_fetch.State, _vaultName, _vaultAddress);

    /// <summary>Reads the Vault the work tracks, or the Workspace's default, for the status bar.</summary>
    /// <remarks>
    /// The name and the address are the command line's answer rather than a file this plugin reads: the Workspace
    /// keeps them in its own configuration, and what a Vault is called is what the command that reaches it calls it.
    /// The read is made off the window's thread, since it is a run of `rola` beside the one the fetch loop makes.
    /// </remarks>
    private void ReadVault()
    {
        if (!Dispatcher.UIThread.CheckAccess())
        {
            Dispatcher.UIThread.Post(ReadVault);

            return;
        }

        if (BaseDirectory.Current is not { Length: > 0 } at)
        {
            _vaultName = string.Empty;
            _vaultAddress = string.Empty;
            Fetched();

            return;
        }

        var version = ++_vaultReading;

        _ = Task.Run(() =>
        {
            var name = string.Empty;
            var address = string.Empty;

            try
            {
                var layouts = LayoutEntriesCommand.Layouts(at);
                var vaults = LayoutEntriesCommand.Vaults(at);
                var tracked = layouts.Layouts.FirstOrDefault(layout => layout.Current)?.Tracking;
                var vault =
                    tracked is { Length: > 0 }
                        ? vaults.Vaults.FirstOrDefault(one =>
                            string.Equals(one.Name, tracked, StringComparison.Ordinal)
                        )
                        : null;

                vault ??= vaults.Vaults.FirstOrDefault(one => one.Default);

                name = vault?.Name ?? tracked ?? string.Empty;
                address = vault?.Address ?? string.Empty;
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Info($"the Vault the status bar names could not be read: {error.Message}");
            }

            Dispatcher.UIThread.Post(() =>
            {
                if (version != _vaultReading)
                {
                    return;
                }

                _vaultName = name;
                _vaultAddress = address;
                Fetched();
            });
        });
    }

    /// <summary>Starts or stops listening for what a reading goes stale from.</summary>
    /// <param name="on">Whether to listen.</param>
    private void Listen(bool on)
    {
        if (on)
        {
            BaseDirectory.Changed += Touched;
            _files.Changed += Touched;
            _again.Start();
        }
        else
        {
            BaseDirectory.Changed -= Touched;
            _files.Changed -= Touched;
            _again.Stop();
        }

        Draw();
    }

    /// <summary>
    /// Reads the work again and asks what there is to carry, which a change of the files may have moved.
    /// </summary>
    /// <remarks>
    /// What the sync buttons offer is the Vault's answer beside this side's, and a version recorded here is
    /// one the Vault has not been told of: the plan is asked again with the reading, or a work that has gained
    /// something to send keeps the buttons of the answer before it, grey, and saying there is nothing to
    /// carry. The reading on the timer asks nothing of the Vault, so this is the only ask a change makes;
    /// <see cref="SyncWatcher.Plan"/> coalesces, so a burst of changes is one run of the command.
    /// </remarks>
    private void Touched()
    {
        Draw();
        _sync.Plan(BaseDirectory.Current);
    }

    /// <summary>
    /// Redraws what is already on screen, without reading the work again.
    /// </summary>
    /// <remarks>
    /// For what changes what the table looks like without changing what it says: beginning a connection, and
    /// taking one back. Reading the work again for that would be a reading spent on nothing, and worse, it
    /// would be a reading that might disagree, since the work may have moved since the last one.
    /// </remarks>
    private void Redraw()
    {
        if (_redrawAsked)
        {
            return;
        }

        _redrawAsked = true;

        // A burst of redraws is one drawing: a command that restores many files says each of them changed,
        // and rebuilding the list for each would be one flash per file.
        Dispatcher.UIThread.Post(() =>
        {
            _redrawAsked = false;

            if (_shown is { } shown)
            {
                Show(shown.Said, shown.Groups, _reading);
            }
        });
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
    /// <remarks>
    /// The reading itself is made on a thread of its own: it is a walk of the whole tree with a version chain walked for
    /// every path that records one, and it is asked for on a timer, a reading made here would stop the window for as long
    /// as the walk took, every five seconds, for as long as the dock was open. What is asked for here is the read; what
    /// comes back draws it (<see cref="Landed"/>).
    /// </remarks>
    private void Drawn()
    {
        _drawAsked = false;

        // A reading made while a sync is carrying content down says what the tree held in the middle of the run, and
        // the rows drawn from it would be out of step with the copy the same run is writing. What is on screen is
        // read again after it, which the hub's own event says, and nothing is read here at all, since it would be a
        // reading spent on an answer that cannot be drawn.
        if (_sync.Running)
        {
            return;
        }

        // One reading at a time: what asks during one is answered by the reading after it, which is the one about the
        // work as it is by then.
        if (_readingGoing)
        {
            _readAgain = true;

            return;
        }

        _readingGoing = true;

        var directory = BaseDirectory.Current;

        _ = Task.Run(() =>
        {
            // Read here rather than inside what is posted: a reading built in the lambda that runs on the window's
            // thread would be the walk this is here to keep off it.
            var reading = Of(directory, _log);

            Dispatcher.UIThread.Post(() => Landed(reading));
        });
    }

    /// <summary>Draws what a reading that landed came to, or asks for the one after it.</summary>
    /// <remarks>
    /// A reading is asked for again rather than drawn when something was asked for while it was being made: what it
    /// says may be older than the change that asked, and drawing it would be describing the work as it no longer is.
    /// A reading that arrives in the middle of a sync is given back for the reason <see cref="Drawn"/> gives.
    /// </remarks>
    /// <param name="reading">What was read.</param>
    private void Landed(OverviewReading reading)
    {
        _readingGoing = false;

        if (_readAgain)
        {
            _readAgain = false;

            reading.Status?.Dispose();
            Draw();

            return;
        }

        if (_sync.Running)
        {
            reading.Status?.Dispose();

            return;
        }

        var facts = reading.Facts;
        var groups = facts is null ? [] : Rows(facts);
        var said = Line(reading, groups.Sum(group => group.Rows.Count));

        // What a sync would carry is asked whenever the reading says the standings it is about have moved, see
        // `PlanIfMoved`. It is asked before the same-answer check below, since the plan may be stale even when
        // the rows are not.
        if (facts is not null)
        {
            PlanIfMoved(facts);
        }

        // What the reading itself names is work the buttons may be pressed for, whatever a plan has or has
        // not been able to say: a table that says "Sync Down" beside three grey buttons is the one thing
        // this must not draw.
        _sync.Observe(
            facts is { } read
                && (read.Sends.Count > 0 || read.Receives.Count > 0 || read.Apart.Count > 0)
        );

        // What would be drawn is what is already drawn, so nothing is drawn: the table is the whole of the
        // dock, and building it again throws away what the reader has scrolled to, chosen and is pointing at.
        // A reading is asked for on a timer, so all but a few of them say what the last one said.
        //
        // What the rows say is not the whole of what the table looks like: a connection in hand is the view's
        // rather than the reading's, and the row it is on is not rebuilt when a button is pressed, see
        // <see cref="Redraw"/>. So a reading that says what the last one said is still drawn when the table on
        // screen was drawn with no connection in it, which is the one case where the two differ.
        if (
            _shown is { } shown
            && shown.Said == said
            && Same(shown.Groups, groups)
            && _connecting is null
        )
        {
            // The reading was made for a table that is already on screen: what it holds is given back rather
            // than kept beside the one the rows were drawn from.
            reading.Status?.Dispose();

            return;
        }

        Show(said, groups, reading.Status);
    }

    /// <summary>Asks what a sync would carry when the reading says the standings it is about have moved.</summary>
    /// <remarks>
    /// The reading is made on a timer and sees the copy of the Vault's Layout that a command run elsewhere, the
    /// terminal's <c>rola fetch</c> or <c>rola track</c>, say, may have moved without telling this plugin that
    /// the files changed. What a plan is about is those standings, so a reading that changes them is the moment
    /// to ask; a reading that says what it said before costs nothing, and a plan costs a fetch of the Vault's
    /// Layout.
    /// </remarks>
    /// <param name="facts">The reading's standings.</param>
    private void PlanIfMoved(StatusFacts facts)
    {
        var signature = string.Join(
            '\u0001',
            BaseDirectory.Current ?? string.Empty,
            string.Join('\u0000', facts.Sends),
            string.Join('\u0000', facts.Receives),
            string.Join('\u0000', facts.Apart)
        );

        if (string.Equals(signature, _plannedFor, StringComparison.Ordinal))
        {
            return;
        }

        _plannedFor = signature;
        _sync.Plan(BaseDirectory.Current);
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
    /// <param name="groups">The groups to show.</param>
    /// <param name="reading">The reading the rows were made from, which a sync row's count is read from.</param>
    private void Show(string? said, IReadOnlyList<RowGroup> groups, StatusReading? reading)
    {
        var previous = _reading;

        _shown = new Shown(said, groups);
        _reading = reading;

        // The rows the old reading answered for are being replaced, so its handle is given back with them.
        // A redraw of the same rows passes the same reading, which is still the one on screen.
        if (!ReferenceEquals(previous, reading))
        {
            previous?.Dispose();
        }

        var rows = Flat(groups);

        // A connection whose row is no longer in the table is nothing to finish, and a reader is not told about
        // it: the work moved under them, and what the button was about is gone. What is left is the table as the
        // reading now says it, which is what a reader is looking at either way.
        if (_connecting is { Length: > 0 } connecting && !rows.Any(row => Path(row) == connecting))
        {
            _connecting = null;
        }

        _tableCell.Children.Clear();

        _notice.Text = said;
        _notice.IsVisible = said is { Length: > 0 };

        _buttons.Clear();

        if (rows.Count > 0)
        {
            _tableCell.Children.Add(Table(groups, BaseDirectory.Current ?? string.Empty, _host, this));
        }
    }

    /// <summary>
    /// What the dock says in the middle, which is said only where the table has nothing to show.
    /// </summary>
    /// <remarks>
    /// A line over a table is a line over the changes, and the two say different things at once: a work with
    /// changes is the table and nothing else, and a work without them is the words, that there is nothing to
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
    /// get wrong, two lists that say the same thing are not the same list, and getting it wrong means a
    /// table rebuilt for nothing on every reading.
    /// </remarks>
    /// <param name="left">What one reading came to.</param>
    /// <param name="right">What another came to.</param>
    /// <returns>Whether the two are the same drawing.</returns>
    internal static bool Same(IReadOnlyList<RowGroup> left, IReadOnlyList<RowGroup> right)
    {
        ArgumentNullException.ThrowIfNull(left);
        ArgumentNullException.ThrowIfNull(right);

        return left.Count == right.Count
            && left
                .Zip(right)
                .All(pair =>
                    pair.First.Key == pair.Second.Key
                    && pair.First.Rows.Count == pair.Second.Rows.Count
                    && pair.First.Rows.Zip(pair.Second.Rows).All(row => Same(row.First, row.Second))
                );
    }

    /// <summary>Whether two rows would be drawn the same.</summary>
    /// <param name="left">One row.</param>
    /// <param name="right">Another.</param>
    /// <returns>Whether the two are the same drawing.</returns>
    private static bool Same(TableRow left, TableRow right) =>
        left.From == right.From
        && left.To == right.To
        && left.Ignored == right.Ignored
        && left.Pills.Count == right.Pills.Count
        && left.Pills.Zip(right.Pills).All(pair => pair.First == pair.Second)
        && left.Notes.SequenceEqual(right.Notes, StringComparer.Ordinal);

    /// <summary>
    /// The reading as a list of groups, each under its heading.
    /// </summary>
    /// <remarks>
    /// Not a table: a row is about one path and the few rows with two say so in the one cell, so there is no
    /// column of shapes to line up. The groups are headed and counted, and a heading folds its rows away,
    /// which is what keeps a long reading short enough to scan.
    /// </remarks>
    /// <param name="groups">The groups, in the order they are read.</param>
    /// <param name="at">Where the run is made, which is what a row's paths are read from.</param>
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    /// <param name="view">The control the rows belong to, which is what a connection and the folded groups are held on.</param>
    /// <returns>The list to show.</returns>
    private static Control Table(
        IReadOnlyList<RowGroup> groups,
        string at,
        IPluginHost host,
        OverviewView view
    )
    {
        var panel = new StackPanel();

        for (var index = 0; index < groups.Count; index++)
        {
            panel.Children.Add(Section(groups[index], at, host, view, index == 0));
        }

        return new ScrollViewer
        {
            Content = panel,
            Background = Brushes.Transparent,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
    }

    /// <summary>One group: a heading that folds its rows away, and the rows under it.</summary>
    /// <param name="group">The group.</param>
    /// <param name="at">Where the run is made.</param>
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    /// <param name="view">The control the rows belong to, which the folded groups are held on.</param>
    /// <param name="first">Whether it is the first group, which the strip above already divides.</param>
    /// <returns>The section to show.</returns>
    private static Control Section(
        RowGroup group,
        string at,
        IPluginHost host,
        OverviewView view,
        bool first
    )
    {
        var rows = new StackPanel();

        foreach (var row in group.Rows)
        {
            rows.Children.Add(Row(row, at, host, view));
        }

        // The room after the last row, which folds away with the rows rather than holding the group open.
        rows.Children.Add(new Border { Height = 6 });

        var arrow = new Path
        {
            Data = Geometry.Parse("M 1,0 L 5,4 L 1,8"),
            StrokeThickness = 1.5,
            StrokeLineCap = PenLineCap.Round,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransformOrigin = RelativePoint.Center,
            RenderTransform = TransformOperations.Parse("rotate(0deg)"),
        };
        arrow[!Shape.StrokeProperty] = new DynamicResourceExtension(PaletteKeys.Faint);
        arrow.Transitions = new Transitions
        {
            new TransformOperationsTransition
            {
                Property = Visual.RenderTransformProperty,
                Duration = TimeSpan.FromMilliseconds(160),
                Easing = new CubicEaseOut(),
            },
        };

        var caret = new Border
        {
            Width = 14,
            VerticalAlignment = VerticalAlignment.Center,
            Child = arrow,
        };

        var label = new TextBlock
        {
            Text = group.Label.ToUpperInvariant(),
            VerticalAlignment = VerticalAlignment.Center,
            FontWeight = FontWeight.SemiBold,
            FontSize = 12,
        };

        var count = new TextBlock
        {
            Text = group.Rows.Count.ToString(CultureInfo.InvariantCulture),
            VerticalAlignment = VerticalAlignment.Center,
            FontSize = 12,
        };
        count[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(QuietInk);

        var inner = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { caret, label, count },
        };

        // The files a track from this group would record: the changes that have one, not a loss a track
        // cannot bring back nor a move that is confirmed rather than recorded.
        var track = group.Key == "track"
            ? group.Rows.Select(Recordable).OfType<string>().ToArray()
            : [];

        Control heading = inner;
        Button? trackButton = null;

        if (track.Length > 0)
        {
            trackButton = TrackButton(track, host, view);

            var bar = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };
            Place(bar, inner, 0);
            Place(bar, trackButton, 1);
            heading = bar;
        }

        var motion = new Transitions
        {
            new DoubleTransition
            {
                Property = Layoutable.MaxHeightProperty,
                Duration = TimeSpan.FromMilliseconds(160),
                Easing = new CubicEaseOut(),
            },
            new DoubleTransition
            {
                Property = Visual.OpacityProperty,
                Duration = TimeSpan.FromMilliseconds(160),
                Easing = new CubicEaseOut(),
            },
        };

        var folded = view._collapsed.Contains(group.Key);

        rows.ClipToBounds = true;
        rows.Transitions = motion;

        if (folded)
        {
            rows.MaxHeight = 0;
            rows.Opacity = 0;
        }

        arrow.RenderTransform = TransformOperations.Parse(folded ? "rotate(0deg)" : "rotate(90deg)");

        var head = new Border
        {
            Child = new Border
            {
                Padding = new Thickness(20, 0),
                HorizontalAlignment = HorizontalAlignment.Stretch,
                Child = heading,
            },
            Height = 36,
            Background = Brushes.Transparent,
            Cursor = new Cursor(StandardCursorType.Hand),
        };

        // The hover the look gives a row, and no press state at all: the press is the fold rather than a
        // second way the row is coloured.
        var wash = RorolalaColours.DarkLook
            ? Color.FromArgb(0x14, 0xFF, 0xFF, 0xFF)
            : Color.FromArgb(0x14, 0x00, 0x00, 0x00);

        head.PointerEntered += (_, _) => head.Background = new SolidColorBrush(wash);
        head.PointerExited += (_, _) => head.Background = Brushes.Transparent;

        head.PointerPressed += (_, pressed) =>
        {
            if (!pressed.GetCurrentPoint(head).Properties.IsLeftButtonPressed)
            {
                return;
            }

            // The track button is the group's own, not the heading's: a press on it asks for a track rather
            // than folding the rows.
            if (trackButton is not null && Within(pressed.Source, trackButton))
            {
                return;
            }

            if (!view._collapsed.Remove(group.Key))
            {
                view._collapsed.Add(group.Key);
            }

            var now = view._collapsed.Contains(group.Key);
            Fold(rows, motion, now);
            arrow.RenderTransform = TransformOperations.Parse(
                now ? "rotate(0deg)" : "rotate(90deg)"
            );
            pressed.Handled = true;
        };

        var section = new StackPanel { Children = { head, rows } };

        var framed = new Border
        {
            Child = section,
            BorderThickness = new Thickness(0, first ? 0 : 1, 0, 0),
        };
        framed[!Border.BorderBrushProperty] = new DynamicResourceExtension(EdgeInk);

        return framed;
    }

    /// <summary>The button a To track group offers, which records every file under it as one version.</summary>
    /// <remarks>
    /// A plain button in the look's own dark face rather than a filled black one: it stands in a heading and
    /// is read beside the words it belongs to, so it wears the same face and line as the row buttons below.
    /// </remarks>
    /// <param name="files">The files a track would record.</param>
    /// <param name="host">The host, which is what a failure is shown through.</param>
    /// <param name="view">The control the prompt is opened from.</param>
    /// <returns>The button.</returns>
    private static Button TrackButton(IReadOnlyList<string> files, IPluginHost host, OverviewView view)
    {
        var word = new TextBlock
        {
            Text = RolaI18N.Get("rorolala_vcs.status.track_all"),
            FontSize = 11,
            FontWeight = FontWeight.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
        };
        word[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);

        var button = new Button
        {
            Content = word,
            Width = 72,
            Height = 24,
            Padding = new Thickness(0),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            Margin = new Thickness(12, 0, 0, 0),
        };
        button[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );

        ToolTip.SetTip(button, RolaI18N.Get("rorolala_vcs.version_graph.track_tip"));
        button.Click += (_, _) => view.AskTrack(files, host, button);

        return button;
    }

    /// <summary>Asks for the message a track from the To track group records the group's files with.</summary>
    /// <remarks>
    /// The field is the bubble a name is typed in rather than a window of its own: Enter records and Escape
    /// leaves, and a field that has been left is the same answer as Escape. Nothing is forced here: this is
    /// the ordinary track, and a change that would be refused is said rather than recorded anyway.
    /// </remarks>
    /// <param name="files">The files the run records.</param>
    /// <param name="host">The host, which is what a failure is shown through.</param>
    /// <param name="anchor">The button the prompt stands under.</param>
    internal void AskTrack(IReadOnlyList<string> files, IPluginHost host, Control anchor)
    {
        if (files.Count == 0 || BaseDirectory.Current is not { Length: > 0 } directory)
        {
            return;
        }

        var box = new TextBox
        {
            Width = 260,
            MinHeight = 34,
            Padding = new Thickness(12, 0),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
            Background = Brushes.Transparent,
            PlaceholderText = RolaI18N.Get("rorolala_vcs.version_graph.track_hint"),
        };

        // The flyout's presenter is the one frame: a field with a border and a ring of its own would be a
        // field inside the bubble rather than a message in it.
        box.Styles.Add(
            new Style(selector => selector.Class(":focus").Template().Name("border"))
            {
                Setters =
                {
                    new Setter(Border.BorderThicknessProperty, new Thickness(0)),
                    new Setter(Border.BoxShadowProperty, default(BoxShadows)),
                },
            }
        );

        var flyout = new Flyout { Placement = PlacementMode.Bottom, Content = box };
        var closed = false;

        void Close()
        {
            if (!closed)
            {
                closed = true;
                flyout.Hide();
            }
        }

        // Taken on the way down, before the field reads them: a field that took Enter for itself would be a
        // message nobody could finish, and the same for the key that leaves it.
        box.AddHandler(
            InputElement.KeyDownEvent,
            (_, pressed) =>
            {
                if (pressed.Key == Avalonia.Input.Key.Enter)
                {
                    pressed.Handled = true;

                    var message = box.Text?.Trim() ?? string.Empty;

                    if (message.Length == 0)
                    {
                        return;
                    }

                    Close();
                    RunTrack(directory, files, message, host);
                }
                else if (pressed.Key == Avalonia.Input.Key.Escape)
                {
                    pressed.Handled = true;
                    Close();
                }
            },
            RoutingStrategies.Tunnel
        );

        box.LostFocus += (_, _) => Close();
        flyout.ShowAt(anchor);

        // The field is focused after the flyout has made its own window, so that the keyboard is in it.
        Dispatcher.UIThread.Post(() => box.Focus());
    }

    /// <summary>Records the group's files as one version, with the message that was typed.</summary>
    /// <param name="directory">Where the work is rooted.</param>
    /// <param name="files">The files to record.</param>
    /// <param name="message">What the version says.</param>
    /// <param name="host">The host, which is what a failure is shown through.</param>
    private void RunTrack(string directory, IReadOnlyList<string> files, string message, IPluginHost host)
    {
        if (_tracking)
        {
            return;
        }

        _tracking = true;

        var run = _hub.Begin("rorolala_vcs.progress.track");

        _ = Task.Run(() =>
        {
            var (done, said) = TrackCommand.Run(directory, files, message);

            Dispatcher.UIThread.Post(() =>
            {
                _tracking = false;
                _hub.End(run);
                host.Files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        host.Dialogs,
                        host.Icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.track_failed"),
                        said
                    );
                }
            });
        });
    }

    /// <summary>Whether a pressed visual is the track button or something inside it.</summary>
    /// <param name="source">Where the press landed.</param>
    /// <param name="ancestor">What it may be inside.</param>
    /// <returns>Whether it is.</returns>
    private static bool Within(object? source, Visual ancestor)
    {
        for (var at = source as Visual; at is not null; at = at.GetVisualParent())
        {
            if (at == ancestor)
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>Folds a group's rows away or open, animating the height they take and their ink.</summary>
    /// <param name="rows">The rows to fold.</param>
    /// <param name="motion">What the fold is animated with.</param>
    /// <param name="folded">Whether the rows are being folded shut.</param>
    private static void Fold(StackPanel rows, Transitions motion, bool folded)
    {
        if (folded)
        {
            rows.MaxHeight = rows.Bounds.Height;
            Dispatcher.UIThread.Post(() =>
            {
                rows.MaxHeight = 0;
                rows.Opacity = 0;
            });

            return;
        }

        // The height it is going to take is not known while it is folded, so it is measured without the
        // transition in the way, and the limit is put back before the animation is told to run.
        rows.Transitions = null;
        rows.MaxHeight = double.PositiveInfinity;
        rows.Measure(new Size(rows.Bounds.Width, double.PositiveInfinity));
        var height = rows.DesiredSize.Height;
        rows.MaxHeight = 0;
        rows.Transitions = motion;

        Dispatcher.UIThread.Post(() =>
        {
            rows.MaxHeight = height;
            rows.Opacity = 1;
        });
    }

    /// <summary>One row's paths, as one cell: a move is the one row with two, joined by an arrow.</summary>
    /// <param name="row">The row.</param>
    /// <param name="at">Where the reading was made.</param>
    /// <param name="host">The host, whose windows the browser is reached through.</param>
    /// <returns>The cell to show.</returns>
    private static Control Paths(TableRow row, string at, IPluginHost host)
    {
        var panel = new Grid
        {
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 0, 12, 0),
        };

        if (row.To is { Length: > 0 } to)
        {
            panel.ColumnDefinitions = new ColumnDefinitions("*,Auto,*");

            Place(panel, Cell(row.From, row.Ignored, at, host), 0);

            var arrow = new TextBlock
            {
                Text = "\u2192",
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(6, 0, 6, 0),
            };
            arrow[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(QuietInk);
            Place(panel, arrow, 1);

            Place(panel, Cell(to, row.Ignored, at, host), 2);
        }
        else
        {
            panel.ColumnDefinitions = new ColumnDefinitions("*");
            Place(panel, Cell(row.From, row.Ignored, at, host), 0);
        }

        return panel;
    }

    /// <summary>A row's notes, which say what is not a state, such as why a merge cannot go on.</summary>
    /// <param name="row">The row.</param>
    /// <returns>The notes to show.</returns>
    private static Control Note(TableRow row)
    {
        var notes = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 12, 0),
        };

        foreach (var note in row.Notes)
        {
            if (note is { Length: > 0 })
            {
                notes.Children.Add(Note(note));
            }
        }

        return notes;
    }

    /// <summary>The columns a row is laid out by: the states, the paths, the note, and the actions.</summary>
    /// <returns>The width of each column.</returns>
    private static ColumnDefinitions RowColumns()
    {
        var columns = new ColumnDefinitions();

        columns.Add(new ColumnDefinition(GridLength.Auto));
        columns.Add(new ColumnDefinition(new GridLength(1, GridUnitType.Star)));
        columns.Add(new ColumnDefinition(GridLength.Auto));
        columns.Add(new ColumnDefinition(GridLength.Auto));

        return columns;
    }

    /// <summary>One row: its states, its paths, what it has to say, and what to do about it.</summary>
    /// <param name="row">The row.</param>
    /// <param name="at">Where the run is made, which is what the row's paths are read from.</param>
    /// <param name="host">The host, which is what a button reaches to run and to report.</param>
    /// <param name="view">The control the row belongs to, which is what a connection is remembered on.</param>
    /// <returns>The row to show.</returns>
    private static Control Row(TableRow row, string at, IPluginHost host, OverviewView view)
    {
        var cells = new Grid { ColumnDefinitions = RowColumns() };

        Place(cells, States(row, at, host, view), 0);
        Place(cells, Paths(row, at, host), 1);
        Place(cells, row.Tally ? Tally(row, at, host, view) : Note(row), 2);
        Place(cells, Cell(row, at, host, view), 3);

        var line = new Border
        {
            Child = cells,
            Padding = new Thickness(40, 5, 20, 5),
            MinHeight = 33,
            Background = Brushes.Transparent,
        };

        // The look's hover wash while the pointer is on the row, which is what tells a reader which row a
        // button belongs to.
        var wash = RorolalaColours.DarkLook
            ? Color.FromArgb(0x14, 0xFF, 0xFF, 0xFF)
            : Color.FromArgb(0x14, 0x00, 0x00, 0x00);

        line.PointerEntered += (_, _) => line.Background = new SolidColorBrush(wash);
        line.PointerExited += (_, _) => line.Background = Brushes.Transparent;

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
    /// <param name="host">The host, whose icon library holds the pictures the states are drawn as.</param>
    /// <param name="view">The control the row belongs to, which is what a sync row's words are read from.</param>
    /// <returns>The pills to show.</returns>
    private static Control States(TableRow row, string at, IPluginHost host, OverviewView view)
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
                    ? Pill(pill, host, SyncSaid(pill.State, row.From, at, view._reading))
                    : Pill(pill, host)
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
    /// One state, as the bead a row wears: the look's face tinted with the state's colour, a ring of that
    /// colour, and the state's picture drawn in it.
    /// </summary>
    /// <remarks>
    /// The bead is the look's face tinted with the state's colour rather than the colour entire, so a column
    /// of marks is read at a glance without shouting; the ring is the same colour stronger, which tells the
    /// bead from the row under it; and the picture stands in the state's own colour. The picture is a mask
    /// over that ink rather than a coloured bitmap, so one picture serves every colour it is asked for.
    /// </remarks>
    /// <param name="pill">The pill.</param>
    /// <param name="host">The host, whose icon library holds the picture the state is drawn as.</param>
    /// <param name="said">What the pointer rests on it for, where it is not what the pill was made with.</param>
    /// <returns>The bead to show.</returns>
    private static Border Pill(StatusPill pill, IPluginHost host, string? said = null)
    {
        var dark = RorolalaColours.DarkLook;
        var towards = dark ? Colors.White : Colors.Black;
        var ground = RorolalaColours.Blend(RorolalaColours.Bright, pill.Fill, 0.35);
        var glyphInk = RorolalaColours.Blend(pill.Fill, towards, dark ? 0.25 : 0.30);

        var glyph = new Border
        {
            Width = BeadGlyph,
            Height = BeadGlyph,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(glyphInk),
        };

        if (host.Icons.Find(pill.IconKey) is { } picture)
        {
            glyph.OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform };
            RenderOptions.SetBitmapInterpolationMode(glyph, BitmapInterpolationMode.HighQuality);
        }

        var bead = new Border
        {
            Child = glyph,
            Width = BeadSize,
            Height = BeadSize,
            CornerRadius = new CornerRadius(BeadSize / 2),
            Background = new SolidColorBrush(ground),
            BorderThickness = new Thickness(1),
            BorderBrush = new SolidColorBrush(RorolalaColours.Blend(ground, towards, 0.10)),
            Margin = new Thickness(0, 2, PillGap, 2),
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(bead, said ?? pill.Hint);
        ToolTip.SetShowDelay(bead, 0);

        return bead;
    }

    /// <summary>How large a bead is, and how large the picture inside it.</summary>
    private const double BeadSize = 22;

    /// <inheritdoc cref="BeadSize" />
    private const double BeadGlyph = 13;

    /// <summary>How far one bead stands from the next.</summary>
    private const double PillGap = 6;

    /// <summary>How large an action button is, and how large the picture inside it.</summary>
    private const double ActionSize = 28;

    /// <inheritdoc cref="ActionSize" />
    private const double ActionGlyph = 14;

    /// <summary>How far one action button stands from the next.</summary>
    private const double ButtonGap = 4;

    /// <summary>One colour with the alpha a pill's ground is laid at, over whatever the row is drawn on.</summary>
    /// <param name="colour">The state's colour.</param>
    /// <param name="alpha">How thin to lay it.</param>
    /// <returns>The colour to fill with.</returns>
    private static Color WithAlpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);

    /// <summary>Puts one cell in its column.</summary>
    /// <param name="grid">The row or band.</param>
    /// <param name="cell">What the column shows.</param>
    /// <param name="column">Which column it is.</param>
    private static void Place(Grid grid, Control cell, int column)
    {
        Grid.SetColumn(cell, column);
        grid.Children.Add(cell);
    }

    /// <summary>One cell of a row holding a path, or nothing where the path is not one.</summary>
    /// <remarks>
    /// A path is a way into the work: pressing one asks the browser to show the place it names, which is what makes a
    /// row read in the Overview something the user can act on where it is, the file's own directory, the file chosen,
    /// and the version graph following it. A row has one side or two, and the side a change is not on is a cell of
    /// nothing that does nothing.
    /// </remarks>
    /// <param name="said">What it says, which may be nothing.</param>
    /// <param name="quiet">
    /// Whether the cell is one of a row that is not the work's, which is drawn in the ink kept for what
    /// is not: the path is still there to be read, and it is read past rather than acted on.
    /// </param>
    /// <param name="at">Where the reading was made, which a path of the work's is relative to.</param>
    /// <param name="host">The host, whose windows the browser is reached through.</param>
    /// <returns>The cell to show.</returns>
    internal static TextBlock Cell(string said, bool quiet, string at, IPluginHost host)
    {
        var cell = new TextBlock
        {
            // A path is one line however long it is: what the column cannot show is on the path itself, and a
            // cell that wrapped would put the row's own height out of step with the pills beside it.
            Text = said,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            FontSize = 12.5,
            Classes = { "caption", "mono" },
        };

        if (quiet)
        {
            // The look's own faint ink rather than a colour of this plugin's, for the reason the rule and the
            // band are: what is not the work is drawn in whatever the look spends on what is not read as work.
            cell[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(QuietInk);
        }

        if (OverviewPaths.Place(said, at) is not { } place)
        {
            return cell;
        }

        // A path that can be gone to says so with the pointer: what the row is offering is a step into the work, and
        // a path drawn like the words beside it would be read as text rather than as a way there. The ground is a
        // transparent brush rather than none, because a control with nothing to fill is not under the pointer at all,
        // and the cells around it are where a press would land instead.
        cell.Cursor = new Cursor(StandardCursorType.Hand);
        cell.Background = Brushes.Transparent;
        ToolTip.SetTip(cell, RolaI18N.Get("rorolala_vcs.status.show_path"));

        // Let go of rather than pressed: a press on a row is what chooses it, and a press begun in the cell and
        // carried away is a drag of the entry rather than a request to go and look at it.
        cell.AddHandler(
            InputElement.PointerReleasedEvent,
            (_, released) =>
            {
                if (released.InitialPressMouseButton != MouseButton.Left)
                {
                    return;
                }

                released.Handled = true;

                host.Windows.Reveal(
                    new DockRequest(
                        global::FileSystemPlugin.FileSystemPlugin.DirectoryDockNameId,
                        place
                    )
                );
            },
            RoutingStrategies.Bubble
        );

        return cell;
    }

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

    /// <summary>What is on screen now, which is what a reading is compared against.</summary>
    /// <param name="Said">The line said in place of a reading, or nothing when there is one.</param>
    /// <param name="Groups">The groups the reading came to, which are none when it says a work is clean.</param>
    private sealed record Shown(string? Said, IReadOnlyList<RowGroup> Groups);
}

/// <summary>
/// Where a column's path is in the program's own terms: a place the browser can be asked to show.
/// </summary>
/// <remarks>
/// A reading names its paths relative to the Workspace it was made over, so what a browser is asked for is the path
/// joined to that place and spelled as the system spells it, which is also how the table reads a path when it asks the
/// version chain how far it is from upstream. An address is a place already and is left as it stands, since a payload
/// has no grammar here and only the plugin that answered it knows what is inside; a column with no path on one side of
/// a row has nothing to show.
/// </remarks>
internal static class OverviewPaths
{
    /// <summary>The place a column's path names, or nothing when it names none a browser can be asked for.</summary>
    /// <param name="said">What the cell says.</param>
    /// <param name="at">Where the reading was made, which the path is relative to.</param>
    /// <returns>The place, in the words the browser takes.</returns>
    public static string? Place(string said, string at)
    {
        ArgumentNullException.ThrowIfNull(said);
        ArgumentNullException.ThrowIfNull(at);

        if (said.Length == 0)
        {
            return null;
        }

        if (LogicalAddress.TryRead(said, out _))
        {
            return said;
        }

        if (at.Length == 0)
        {
            return null;
        }

        try
        {
            return System.IO.Path.GetFullPath(
                System.IO.Path.Combine(at, said.Replace('/', System.IO.Path.DirectorySeparatorChar))
            );
        }
        catch (Exception error)
            when (error is ArgumentException or NotSupportedException or PathTooLongException)
        {
            // A path the system will not read as one is not a place anything can be shown at, and the row is still
            // worth reading: what is left out is the step into it, not the row.
            return null;
        }
    }
}

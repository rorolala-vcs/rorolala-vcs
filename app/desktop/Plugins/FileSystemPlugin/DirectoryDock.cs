using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>The Directory dock: one directory's entries, read at a zoom.</summary>
internal sealed class DirectoryDock : IDockView, IDockOpened, IDockTitle
{
    /// <summary>The control the dock shows.</summary>
    private readonly DirectoryControl _view;

    /// <summary>Makes a directory dock.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="shared">The answers every location shares.</param>
    /// <param name="browser">The one location, which this dock is a view of until it is taken out of step.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    /// <param name="logical">What a place's own copy or cut has put within reach of a paste.</param>
    public DirectoryDock(IPluginHost host, Shared shared, Browser browser, Clip clip, LogicalClip logical) =>
        _view = new DirectoryControl(host, shared, browser, clip, logical);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header => [];

    /// <inheritdoc />
    /// <remarks>
    /// A browser is one dock looked at in several windows, so the name it was registered under says what kind of dock
    /// it is and nothing about which place it is showing: two of them are two tabs called "Directories". What the
    /// control answers is where it is, which is what a tab can say one dock apart from another by.
    /// </remarks>
    public string? Title => _view.Title;

    /// <inheritdoc />
    public string? Detail => _view.Detail;

    /// <inheritdoc />
    public event Action? TitleChanged
    {
        add => _view.TitleChanged += value;
        remove => _view.TitleChanged -= value;
    }

    /// <inheritdoc />
    public void Restored(IDockState state) => _view.Restored(state);

    /// <inheritdoc />
    /// <remarks>
    /// A dock is told what to show at most a second time: once when it is made, where a place of the caller's choosing
    /// is this window's own place and the dock is taken out of step to be there, and again whenever a view asks for a
    /// place to be shown in the dock the user is working in, which is a step inside the dock's own work, and so moves
    /// the whole where the dock follows it rather than making a place of its own. The two answers are
    /// <see cref="DirectoryControl.OpenAt"/> and <see cref="DirectoryControl.Reveal"/>.
    /// </remarks>
    public void Opened(DockRequest request)
    {
        var again = _told;
        _told = true;

        if (request.StartAt is not { Length: > 0 } at)
        {
            return;
        }

        if (again)
        {
            _view.Reveal(at);
        }
        else
        {
            _view.OpenAt(at);
        }
    }

    /// <summary>
    /// Whether this dock has already been told what to show, which tells an opening from a showing.
    /// </summary>
    /// <remarks>
    /// A dock opened with nothing in particular (restored from the layout, or asked for from the Window menu) is
    /// told once and about nothing, and what follows it is a showing rather than an opening: this is what says so.
    /// </remarks>
    private bool _told;
}

/// <summary>
/// The directory's control: the location's entries, as a list or a grid, at a zoom.
/// </summary>
/// <remarks>
/// Neither navigation nor the tree is here: back, forward, up, refresh and the address are a dock of
/// their own, and so is the tree. What is here is one directory, read at a size, and the size
/// is the one thing about this dock that is its own by default, the directory is every dock's, until this
/// one is taken out of step and given a location of its own.
/// <para>
/// The arrangement is not a second choice beside the size: entries read too small to be tiles are read as
/// rows of names, and entries read large enough are read as tiles, so one scale decides both and there is
/// nothing to keep but the scale. A switch between two arrangements would let a dock be a grid of tiny
/// pictures, which is a grid nobody asked for.
/// </para>
/// </remarks>
[SuppressMessage(
    "Design",
    "CA1001:Types that own disposable fields should be disposable",
    Justification = "a control is not disposed by anything: this dock's own location is let go of where it is replaced, which is the one moment it is this dock's to keep"
)]
internal sealed class DirectoryControl : UserControl
{
    /// <summary>The zoom the dock opens at, as a percentage of the size an icon is at its own.</summary>
    private const double Opening = 100.0;

    /// <summary>How little and how much a dock may be zoomed, which is where a turn of the wheel stops.</summary>
    private const double Least = 50.0;
    private const double Most = 200.0;

    /// <summary>
    /// How far one notch of the wheel moves the zoom.
    /// </summary>
    /// <remarks>
    /// A step coarse on purpose: an icon is read from the desktop once per size it is asked for, so a wheel that
    /// crept would be a zoom that asked for a picture nearly every notch and kept one for each. At this step a
    /// notch is worth several sizes, which is also what makes a hand cross the range in a few turns of it.
    /// </remarks>
    private const double Wheel = 20.0;

    /// <summary>Above this zoom the entries are tiles; at it or below they are rows.</summary>
    private const double GridAbove = 60.0;

    /// <summary>The dock key the zoom is kept under.</summary>
    private const string ZoomKey = "zoom";

    /// <summary>The dock key the choice about following the whole is kept under.</summary>
    private const string SyncKey = "sync";

    /// <summary>
    /// The dock key the place this dock was looking at is kept under.
    /// </summary>
    /// <remarks>
    /// Empty for a dock that follows the whole, which is the state a dock that was taken back into step leaves behind:
    /// where such a dock is is where the whole is, and a directory value here would be a place of its own.
    /// </remarks>
    private const string LocationKey = "location";

    /// <summary>
    /// The dock key the arrangement was kept under before a zoom decided it.
    /// </summary>
    /// <remarks>
    /// Read and never written. A dock that kept <c>grid</c> or <c>list</c> here is a dock whose user had
    /// chosen an arrangement, and opening at the zoom that arrangement amounts to is what keeps their choice
    /// rather than replacing it with the default. It is written no more because the zoom says the same thing
    /// and more, and two keys saying it is how they come to disagree.
    /// </remarks>
    private const string LayoutKey = "layout";

    /// <summary>What the arrangement used to be kept as, when there were two to choose from.</summary>
    private const string Grid = "grid";
    private const string List = "list";

    /// <summary>The answers every location shares, which a dock out of step goes on reading.</summary>
    private readonly Shared _shared;

    /// <summary>The one location, which this dock is a view of until it is taken out of step.</summary>
    private readonly Browser _whole;

    /// <summary>The host, which the views are made over.</summary>
    private readonly IPluginHost _host;

    /// <summary>What a copy or a cut has put within reach of a paste.</summary>
    private readonly Clip _clip;

    /// <summary>What a place's own copy or cut has put within reach of a paste.</summary>
    private readonly LogicalClip _logicalClip;

    /// <summary>What the views do, so that every view of the location behaves alike.</summary>
    private BrowserActions _actions;

    /// <summary>
    /// The zoom the entries are read at, as a percentage.
    /// </summary>
    /// <remarks>
    /// A number rather than the slider that used to hold it: the slider was what the band at the foot was
    /// for, and the one way of turning the zoom that is left is the wheel, which had to read and write the
    /// slider to move it. What the zoom is, is a step of the range the constants below state, and it is kept
    /// under the dock's own state like the rest of what a dock remembers.
    /// </remarks>
    private double _zoom = Opening;

    /// <summary>
    /// The switch that shows what the rules hide, and the dropdown that chooses the rules.
    /// </summary>
    /// <remarks>
    /// Two controls because they answer two questions: the switch is whether the user is looking at what is
    /// hidden, the entries then shown faded, and the dropdown is what is hidden in the first place. Both
    /// write the plugin's own settings rather than the dock's state, so one switch serves every dock and the
    /// choice outlives the dock it was made in.
    /// <para>
    /// The switch is a tool with a picture rather than a word: an open eye while the hidden entries are shown,
    /// a struck-through one while they are not, and the primary, the colour the program says a thing chosen is
    ///, behind it while it is on. Its word is the tooltip's, because a picture says what a picture says and
    /// the name of the choice is still worth having.
    /// </para>
    /// </remarks>
    private readonly Button _show = new();

    /// <summary>The eye the show switch wears, which is turned on and off.</summary>
    private Border? _showGlyph;

    /// <summary>The two pictures the eye turns between, in the order they are turned.</summary>
    private (Bitmap Shown, Bitmap Hidden)? _eyes;

    /// <summary>The dropdown that switches which providers hide what.</summary>
    private readonly Button _rules = new();

    /// <summary>One row per provider, so that the mode each rule is in follows the settings.</summary>
    private readonly List<(IEntryHideProvider Provider, Button Row, Border Glyph)> _hiders = [];

    /// <summary>The three pictures a rule's mode is drawn with, in the order the mode is read.</summary>
    private (Bitmap Show, Bitmap Gray, Bitmap Hide)? _marks;

    /// <summary>
    /// Whether this dock follows the whole, or looks at a directory of its own.
    /// </summary>
    /// <remarks>
    /// A tool with a picture for the same reason the switch above is: a closed link while this dock follows the
    /// whole, a broken one while it looks at a directory of its own, and the primary behind it while it
    /// follows.
    /// </remarks>
    private readonly Button _sync = new();

    /// <summary>The link the sync switch wears, which is closed or broken.</summary>
    private Border? _syncGlyph;

    /// <summary>The two pictures the link turns between, in the order they are turned.</summary>
    private (Bitmap Closed, Bitmap Broken)? _links;

    /// <summary>
    /// What the dock keeps beside the address, which is put at the far end of the toolbar.
    /// </summary>
    /// <remarks>
    /// Built once and handed to every toolbar this dock makes, because the toolbar is told which location to
    /// read rather than being made again (see <see cref="NavigationBar.Reading"/>), and a control that was
    /// handed over once cannot be handed over twice while it is still a child of the bar it left.
    /// </remarks>
    private readonly StackPanel _trailing = new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = 12,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>
    /// The toolbar, kept so that being taken out of step hands it the other location.
    /// </summary>
    /// <remarks>
    /// The one row this dock draws above its entries. There is none below them: the zoom is the dock's own
    /// state, turned with the wheel and kept, and a band at the foot holding a slider for it was a band the
    /// entries could not be read in, the listing is what a dock is for, so it takes the whole of it.
    /// </remarks>
    private readonly NavigationBar _bar;

    /// <summary>Whichever arrangement the zoom amounts to.</summary>
    private readonly ContentControl _content = new();

    /// <summary>The view being read, which is what a copy or a paste in this dock is about.</summary>
    private EntryView<Entry>? _view;

    /// <summary>
    /// The listing of a place, while this dock is looking at one.
    /// </summary>
    /// <remarks>
    /// One or the other of this and <see cref="_view"/> is what is being read. Both are views of the very same
    /// arrangements, a place is read by the rows and the tiles a directory is, over a different listing
    /// (<see cref="Places"/> rather than <see cref="Files"/>), so which of them there is follows from what the
    /// location says, and how it is drawn does not.
    /// </remarks>
    private EntryView<LogicalEntry>? _logical;

    /// <summary>
    /// The location this dock was given, or nothing while it follows the whole.
    /// </summary>
    /// <remarks>
    /// Held because a location that was made for this dock is this dock's to let go of: what it holds of the
    /// plugin's is a subscription to the answers every dock shares, and a location nobody lets go of is a
    /// listing that goes on being told about changes it will never draw.
    /// </remarks>
    private Browser? _own;

    /// <summary>The location being listened to, so that one left behind is stopped being listened to.</summary>
    private Browser? _watched;

    /// <summary>What the dock kept, and where the next of it is kept.</summary>
    private IDockState? _state;

    /// <summary>The entries as this dock last drew them, so that a reread can be told from a step.</summary>
    private IReadOnlyList<Entry>? _drawn;

    /// <summary>The place this dock last drew, so that a reread of the same one can be told from a step.</summary>
    private LogicalPlace? _drawnPlace;

    /// <summary>
    /// What this dock is looking at, which is what its tab and its window are called.
    /// </summary>
    /// <remarks>
    /// The registered name, "Directories", says what kind of dock this is and not which place it is showing, so two
    /// of them are two tabs called the same. What is said here is the place itself: the real path of the directory, the
    /// address of a place a plugin answers for, and "This PC" where there is no path at all. The whole of it is kept
    /// beside as the detail, which is what a tab's tip and the top of its menu carry, and it is the same string, since
    /// a path is already the whole answer and a shorter name would be a second name to keep in step.
    /// </remarks>
    public string? Title { get; private set; }

    /// <summary>Where this dock is, in full, for the tip of its tab and the top of its menu.</summary>
    public string? Detail { get; private set; }

    /// <summary>Raised when <see cref="Title"/> or <see cref="Detail"/> says something else.</summary>
    public event Action? TitleChanged;

    /// <summary>
    /// Whether a listing was asked to be drawn while a row's name was being typed, so it is drawn once the box goes.
    /// </summary>
    /// <remarks>
    /// A name is edited in a row of the view, so drawing the listing again would replace the view, the row and the box
    /// with everything typed in it. The ask is owed rather than dropped: what was read while the name was being typed
    /// is what the listing has to show once the name is settled.
    /// </remarks>
    private bool _deferred;

    /// <summary>
    /// Whether a name is being typed in the listing being read, which is the one state a redraw cannot interrupt.
    /// </summary>
    /// <remarks>
    /// One answer rather than two, because which of the two views is drawn follows from what the location is looking
    /// at: a dock that kept the two apart would have to say which of them is the one being typed in.
    /// </remarks>
    private bool Editing => _view?.Editing == true || _logical?.Editing == true;

    /// <summary>
    /// The directory the dock last drew, so that a choice is only carried across a rebuild of the same one.
    /// </summary>
    /// <remarks>
    /// Stepping into another directory builds the view again as much as a reread does, and a choice is about the
    /// files of the directory it was made in: carrying it into another one would choose whatever happened to be
    /// named the same there, and the way up, which is named the same everywhere, would be chosen in every
    /// directory the user stepped into.
    /// </remarks>
    private string? _drawnAt;

    /// <summary>The file this dock was told to show, until the drawing that goes to its directory has used it.</summary>
    /// <remarks>
    /// A showing is delivered as a navigation of the whole location and a choice in the listing that navigation
    /// draws, and the two cannot be said in one call: the listing is what knows its rows, and it is made by the
    /// navigation. What is said here is the path, and the drawing that finds it chooses it once (see
    /// <see cref="Showing"/>).
    /// </remarks>
    private string? _revealing;

    /// <summary>
    /// What this dock is looking at now.
    /// </summary>
    /// <remarks>
    /// Its own location once it has been taken out of step, and the whole's until then, which is why nothing
    /// here reads the whole directly but this: a dock out of step that went on reading the whole would go on
    /// following it, and not following it is what being out of step means.
    /// </remarks>
    private Browser Location => _own ?? _whole;

    /// <summary>Makes the directory's control.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="shared">The answers every location shares.</param>
    /// <param name="whole">The one location, which this dock is a view of until it is taken out of step.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    /// <param name="logical">What a place's own copy or cut has put within reach of a paste.</param>
    public DirectoryControl(
        IPluginHost host,
        Shared shared,
        Browser whole,
        Clip clip,
        LogicalClip logical
    )
    {
        _host = host;
        _clip = clip;
        _logicalClip = logical;
        _shared = shared;
        _whole = whole;
        _actions = new BrowserActions(host, whole, clip);

        // Listening while it is on screen rather than for as long as it exists: a dock that was
        // closed is not a view of anything, and one that was dragged to another region is taken off
        // the tree and put back, which is a departure and a return for a dock that never closed.
        AttachedToVisualTree += (_, _) =>
        {
            // What a place cut is drawn faded, and the rows are repainted for it: the offer is one for every dock, so a
            // cut made in one is faded in the next one drawn rather than only in the one it was made in.
            logical.Changed += RepaintPlace;
            ChosenFiles.Cleared += ClearedSelection;
            Watch(Location);
            Draw();
        };
        DetachedFromVisualTree += (_, _) =>
        {
            logical.Changed -= RepaintPlace;
            ChosenFiles.Cleared -= ClearedSelection;
            Watch(null);
        };

        // Tunnelled, so that the zoom is reached before whatever the pointer is over reads the wheel as a
        // scroll: a zoom that stopped working as soon as the entries were worth scrolling would be a zoom
        // that stopped working where it is wanted most.
        AddHandler(PointerWheelChangedEvent, Wheeled, RoutingStrategies.Tunnel);

        // And on the way down for the same reason: F5 is answered wherever in the dock the keyboard is, so that
        // a listing that happened to have the keyboard would not be the only place it worked.
        AddHandler(KeyDownEvent, Keyed, RoutingStrategies.Tunnel);

        FillHidden();
        FillSync();

        // What the toolbar keeps at its far end, in the order it reads: whether this dock follows the whole,
        // then a rule, then the eye that shows what the hide rules take out and the more button that opens
        // those rules. The eye and the more button are two answers about one subject and stand together, and
        // the rule is what tells them from the link: the four are one run rather than four separate tools.
        var rule = new Border
        {
            Width = 1,
            Height = 16,
            Margin = new Thickness(2, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };

        rule[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        var hides = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { _show, _rules },
        };
        var tools = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { _sync, rule, hides },
        };

        _trailing.Children.Add(tools);

        // A dock is a place the keyboard can be put, which is how the shell hands it to the thing being read: what
        // this dock answers is answered at the top of itself, so the shell putting the keyboard here is what makes
        // the keys work after this dock is chosen, the keyboard was on the tab that was clicked, and that tab
        // belongs to the shell's strip rather than to the dock.
        Focusable = true;
        GotFocus += (_, e) =>
        {
            if (ReferenceEquals(e.Source, this))
            {
                Seated();
            }
        };

        // The toolbar is the dock's own rather than a dock beside it: the arrows and the address act on the
        // location being read, and they belong over the entries they act on. What the dock keeps
        // beside them is handed in, so that the toolbar does not have to know what a dock remembers.
        _bar = new NavigationBar(_host, Location, _trailing) { Left = Seated };

        // The entries take everything under the toolbar. What is left at the foot is nothing, which is what
        // this is: a dock is a view of a directory, and the room a band at the bottom would take is room the
        // listing is read in.
        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(_bar, Dock.Top);
        panel.Children.Add(_bar);
        panel.Children.Add(_content);

        Content = panel;

        Draw();
    }

    /// <summary>
    /// Looks at a directory of this dock's own, which is what it was opened onto.
    /// </summary>
    /// <remarks>
    /// The switch is turned over and the dock taken out of step, rather than the whole location being sent
    /// somewhere: a dock opened onto a directory is one window onto that place, and moving the whole would
    /// take every other dock with it, including the one the user was working in when they asked for this.
    /// <para>
    /// The location is made here rather than by the toolbar, because what the switch does is start from where
    /// the whole is and this starts from a directory that was named. The two are the same state either way,
    /// which is why what follows is the one way out of step rather than a second one.
    /// </para>
    /// </remarks>
    /// <param name="at">The directory or the address of the place to look at.</param>
    public void OpenAt(string at)
    {
        // A directory that cannot be read is refused before a location is made over it: what a location is over a
        // directory, and one that cannot be read is not one. The dock then goes on following the whole, which is the
        // one place that is always there. An address is not a directory and is not asked of the filesystem: whether it
        // leads anywhere is the plugin's answer, and an address nobody answers is a place to be at rather than a step
        // that failed.
        var address = LogicalAddress.TryRead(at, out _);

        if (!address && !System.IO.Directory.Exists(Browser.Expand(at)))
        {
            _host.Log.Warn($"`{at}` cannot be looked at, so the dock stayed where the whole is looking");

            return;
        }

        _own?.Dispose();
        _own = new Browser(
            _host.Log,
            _shared,
            address ? at : Browser.Expand(at),
            _host.LogicalViews
        );

        // What is done to an entry is done in the directory this dock is looking at, so the actions are made over
        // the location again here: left as they were, a menu would act on the whole's directory, opening a folder
        // from here would send the docks beside this one there and leave this one where it is, which is the one
        // thing a dock out of step must not do. Asked of `Location` rather than of `_own`, since the two are one
        // answer (see `Location`), and made here as it is in `InStep` because this is the same state reached by
        // being opened onto a directory rather than by the switch.
        _actions = new BrowserActions(_host, Location, _clip);

        _bar.Reading(_own);
        Watch(_own);

        // The switch says what this dock is rather than leading it, which is the same rule its click follows.
        Follows(false);
        Draw();
    }

    /// <summary>
    /// Shows a place in this dock: the directory holding a path, and the path itself where it is a file of it.
    /// </summary>
    /// <remarks>
    /// The dock navigates as the user's own steps do, the whole where it follows the whole, and itself where it is
    /// out of step, rather than taking a place of its own as being opened onto a directory does (see
    /// <see cref="OpenAt"/>). A path shown from elsewhere is a step inside the dock's own work, and a showing that
    /// made the dock stop following the work would be a navigation that changed what the dock is.
    /// <para>
    /// A file is shown by its directory with its row chosen, which is what puts the version graph on it. One whose
    /// file is gone, or which the hide rules leave out of the listing, is shown by its directory alone; one whose
    /// directory is not there either is said rather than stepped to, so the dock stays where the user left it.
    /// </para>
    /// </remarks>
    /// <param name="at">The path to show: a directory, a file in one, or an address.</param>
    public void Reveal(string at)
    {
        // An address is a place like any other, and the browser knows how to go to one: what a place holds is the
        // answerer's business, and a showing has nothing to choose in it (see `Browser.Go`).
        if (LogicalAddress.TryRead(at, out _))
        {
            if (!Location.Go(at))
            {
                _host.Log.Warn($"`{at}` cannot be shown, so the dock stayed where it is looking");
            }

            return;
        }

        var full = Path.GetFullPath(Browser.Expand(at));

        if (System.IO.Directory.Exists(full))
        {
            _ = Location.Go(full);

            return;
        }

        var holding = Path.GetDirectoryName(full);

        if (holding is not { Length: > 0 } || !System.IO.Directory.Exists(holding))
        {
            Refused(full);

            return;
        }

        _revealing = System.IO.File.Exists(full) ? full : null;

        if (!Location.Go(holding))
        {
            _revealing = null;
            Refused(full);
        }
    }

    /// <summary>
    /// Takes what this dock was: the zoom it was left at, and whether it followed the whole.
    /// </summary>
    /// <remarks>
    /// Read before the dock is drawn, so that a directory that was left as tiles comes back as tiles rather
    /// than being drawn as rows and rearranging under the first frame. The host makes the view and then tells
    /// it what it was, which is why there is anything to do here at all.
    /// </remarks>
    /// <param name="state">What the dock kept, and where the next of it is kept.</param>
    public void Restored(IDockState state)
    {
        _state = state;

        if (Kept(state) is { } zoom)
        {
            Zoom = zoom;
        }

        // Where the dock was comes first, because it decides which location is being looked at: a dock that was left at
        // a place comes back at it, and one that was left in step is drawn at wherever the whole is.
        var at = state.Read(LocationKey);

        if (at is { Length: > 0 })
        {
            OpenAt(at);

            return;
        }

        // And for the same reason again: a dock left out of step comes back out of step, at a directory of its
        // own that nothing keeps, the location is not written into the layout, rather than
        // being drawn in step and switching under the first frame.
        if (bool.TryParse(state.Read(SyncKey), out var inStep))
        {
            Sync(inStep);
        }

        Draw();
    }

    /// <summary>
    /// Sets the switch and the dropdown up: what is shown, and what the rules hide.
    /// </summary>
    /// <remarks>
    /// Both are the plugin's rather than the dock's: they write the settings every dock and the preference
    /// panel read, so a switch turned here is turned in the next dock as well, and a dock that kept either
    /// to itself would list something the next one did not.
    /// </remarks>
    private void FillHidden()
    {
        _eyes = (
            Icons.Picture(Icons.Visibility),
            Icons.Picture(Icons.VisibilityOff)
        );
        _marks = (
            Icons.Picture(Icons.RadioButtonUnchecked),
            Icons.Picture(Icons.RadioButtonChecked),
            Icons.Picture(Icons.Circle)
        );

        _show.Classes.Add("tool");
        _show.VerticalAlignment = VerticalAlignment.Center;
        ToolTip.SetTip(_show, RolaI18N.Get("rorolala_file_system.hidden"));
        _showGlyph = Icons.Inked(_eyes.Value.Shown, Icons.Tool);
        _show.Content = _showGlyph;

        // Written rather than read here: a dock attached to the tree is the one that drew the rows, so it is
        // the one the shell tells to look again, a dock in a region nobody can see has no rows to redraw.
        _show.Click += (_, _) => _shared.ShowHidden = !_shared.ShowHidden;

        _rules.VerticalAlignment = VerticalAlignment.Center;
        _rules.Classes.Add("tool");
        _rules.Content = Icons.Inked(Icons.Picture(Icons.MoreVert), Icons.Tool);
        _rules.Flyout = new Flyout { Content = HiddenBoxes() };
        AutomationProperties.SetName(_rules, RolaI18N.Get("rorolala_file_system.hides.providers"));

        RefreshHidden();
    }

    /// <summary>
    /// Says on the switch what it shows: the eye that is open or struck through, the ink that goes with it, and
    /// the fill the look gives a tool that is on.
    /// </summary>
    /// <param name="shown">Whether what the rules hide is being shown.</param>
    private void Show(bool shown)
    {
        if (_showGlyph is null || _eyes is not { } eyes)
        {
            return;
        }

        _showGlyph.OpacityMask = new ImageBrush(shown ? eyes.Shown : eyes.Hidden)
        {
            Stretch = Stretch.Uniform,
        };

        // Read much larger than it is shown, so the mask is reducing the picture: the high filter is what
        // keeps the eye's stroke whole on the way down.
        Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
            _showGlyph,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );

        // The look's class for a tool that is on, which is the same word for every tool the program draws.
        _show.Classes.Set("on", shown);
    }

    /// <summary>Builds the rows the dropdown holds, one per provider.</summary>
    private StackPanel HiddenBoxes()
    {
        var boxes = new StackPanel { Spacing = 2, Width = 240 };

        foreach (var provider in HideRegistry.All)
        {
            var glyph = Icons.Inked(Icons.Picture(Icons.RadioButtonUnchecked), Icons.Tool);
            var label = new TextBlock
            {
                Text = RolaI18N.Get(provider.LabelKey),
                VerticalAlignment = VerticalAlignment.Center,
                TextTrimming = TextTrimming.CharacterEllipsis,
                FontSize = 13,
            };
            var content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                Children = { glyph, label },
            };
            var row = new Button
            {
                Content = content,
                Classes = { "ghost" },
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Padding = new Thickness(8, 4),
                MinHeight = 28,
            };

            row.Click += (_, _) => Cycle(provider);
            ToolTip.SetTip(row, RolaI18N.Get(provider.LabelKey));
            AutomationProperties.SetName(row, RolaI18N.Get(provider.LabelKey));

            _hiders.Add((provider, row, glyph));
            boxes.Children.Add(row);
        }

        return boxes;
    }

    /// <summary>Puts one rule into its next mode: shown, faded, then hidden.</summary>
    /// <param name="provider">The rule to turn.</param>
    private void Cycle(IEntryHideProvider provider)
    {
        var next = _shared.HideRegistry.State(provider.Id) switch
        {
            HideState.Show => HideState.Gray,
            HideState.Gray => HideState.Hide,
            _ => HideState.Show,
        };

        // The hiding setting is one the plugin listens to, and the faded one is not declared. A change that
        // only moves a rule out of the hiding set therefore has nobody to tell the listing, so it is told here.
        if (!_shared.HideRegistry.SetState(provider.Id, next))
        {
            _shared.Hidden();
        }

        RefreshHidden();
    }

    /// <summary>
    /// Says on the dropdown what is in force, and brings its rows and the switch in step with the settings.
    /// </summary>
    /// <remarks>
    /// Read here rather than remembered, because the settings are changed in the preference panel and in other
    /// docks as well as in this one: a dock drawing what it last wrote would disagree with the listing beside
    /// it the moment another dock wrote something else.
    /// </remarks>
    private void RefreshHidden()
    {
        var hiding = _shared.HideRegistry.Selection();
        var gray = _shared.HideRegistry.Gray();

        foreach (var (provider, _, glyph) in _hiders)
        {
            var state = hiding.Contains(provider.Id)
                ? HideState.Hide
                : gray.Contains(provider.Id)
                    ? HideState.Gray
                    : HideState.Show;

            glyph.OpacityMask = new ImageBrush(Mark(state))
            {
                Stretch = Stretch.Uniform,
            };
        }

        Show(_shared.ShowHidden);

        var named = HideRegistry
            .All.Where(provider => hiding.Contains(provider.Id) || gray.Contains(provider.Id))
            .Select(provider => RolaI18N.Get(provider.LabelKey))
            .ToArray();

        var summary = named.Length switch
        {
            0 => RolaI18N.Get("rorolala_file_system.hidden_none"),
            1 => named[0],
            _ => RolaI18N.Get("rorolala_file_system.hidden_many", named.Length),
        };

        ToolTip.SetTip(_rules, summary);
    }

    /// <summary>The picture a rule's mode wears.</summary>
    /// <param name="state">The mode.</param>
    /// <returns>The picture.</returns>
    private Bitmap Mark(HideState state) =>
        state switch
        {
            HideState.Show => _marks!.Value.Show,
            HideState.Gray => _marks!.Value.Gray,
            _ => _marks!.Value.Hide,
        };

    /// <summary>
    /// Sets the sync toggle up, and what turning it does.
    /// </summary>
    /// <remarks>
    /// It is the one choice about where a dock is looking that is the dock's own, which is why it sits with
    /// what the dock keeps rather than in the toolbar's tools: a direction is everybody's, and being in step
    /// with it is this dock's answer.
    /// </remarks>
    private void FillSync()
    {
        _links = (
            Icons.Picture(Icons.Link),
            Icons.Picture(Icons.LinkOff)
        );

        _sync.Classes.Add("tool");
        _sync.VerticalAlignment = VerticalAlignment.Center;
        ToolTip.SetTip(_sync, RolaI18N.Get("rorolala_file_system.sync"));
        _syncGlyph = Icons.Inked(_links.Value.Closed, Icons.Tool);
        _sync.Content = _syncGlyph;

        // Clicking turns it over: in step means looking where the whole looks, so a click out of step is a
        // click back into it. What it follows is asked of where this dock is looking rather than remembered
        // here, since that is where being in step is kept.
        _sync.Click += (_, _) => Sync(_own is not null);

        // In step until it is said otherwise, which is what a dock opened fresh is: the whole is where a run
        // starts, and a dock that opened somewhere of its own would be a second place to find on the first
        // frame. Only what the switch says is set here, the toolbar this would otherwise reach for is not
        // made yet.
        Follows(true);
    }

    /// <summary>
    /// Follows the whole or looks at a directory of this dock's own, and says so on the switch.
    /// </summary>
    /// <remarks>
    /// The one way to change which it follows, so that the state and the switch cannot come apart: a click that
    /// changed what the dock looks at and left the picture saying the other thing would be a switch that never
    /// went out.
    /// </remarks>
    /// <param name="inStep">Whether this dock is to follow the whole.</param>
    private void Sync(bool inStep)
    {
        InStep(inStep);
        Follows(inStep);
    }

    /// <summary>
    /// Says on the switch whether this dock follows the whole: the link that is closed or broken, the ink that
    /// goes with it, and the fill the look gives a tool that is on.
    /// </summary>
    /// <param name="inStep">Whether this dock follows the whole.</param>
    private void Follows(bool inStep)
    {
        if (_syncGlyph is null || _links is not { } links)
        {
            return;
        }

        _syncGlyph.OpacityMask = new ImageBrush(inStep ? links.Closed : links.Broken)
        {
            Stretch = Stretch.Uniform,
        };

        // Read much larger than it is shown, so the mask is reducing the picture: the high filter is what
        // keeps the link's stroke whole on the way down.
        Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
            _syncGlyph,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );

        _sync.Classes.Set("on", inStep);
    }

    /// <summary>
    /// Follows the whole again, or looks at a directory of this dock's own.
    /// </summary>
    /// <remarks>
    /// Leaving starts from where the whole is looking, because that is where this dock is looking when it
    /// leaves: a dock that appeared somewhere else entirely would be a second place to find rather than a
    /// place this one went to.
    /// <para>
    /// Coming back throws that location away, which is why a location is let go of rather than dropped
    /// (<see cref="Browser.Dispose"/>): what it holds of the plugin's outlives it.
    /// </para>
    /// </remarks>
    /// <param name="inStep">Whether this dock is to follow the whole.</param>
    private void InStep(bool inStep)
    {
        // Written rather than saved, as the zoom and the hiding are: the dock keeps what it was and the host
        // writes the layout.
        _state?.Write(SyncKey, inStep.ToString(CultureInfo.InvariantCulture));

        var was = Location;

        Watch(null);
        _own?.Dispose();
        _own = inStep ? null : new Browser(_host.Log, _shared, was.Current, _host.LogicalViews);
        _actions = new BrowserActions(_host, Location, _clip);

        _bar.Reading(Location);
        Watch(Location);

        // Only when it is another location: coming back in step while already in step is not a place to go,
        // and drawing again would only throw the listing away for the one it already has.
        if (!ReferenceEquals(was, Location))
        {
            Draw();
        }
    }

    /// <summary>
    /// Listens to a location, so that what it holds is drawn again when it changes, or listens to none.
    /// </summary>
    /// <remarks>
    /// One at a time, and while the dock is on screen: a dock out of step is a view of its own location and not
    /// of the whole's, so listening to both would redraw it for a change it is no longer following. What the
    /// toolbar listens to is its own affair and moves with it (see <see cref="NavigationBar.Reading"/>).
    /// </remarks>
    /// <param name="location">What to listen to, or nothing to stop listening.</param>
    private void Watch(Browser? location)
    {
        if (ReferenceEquals(_watched, location))
        {
            return;
        }

        if (_watched is not null)
        {
            _watched.Changed -= Update;
        }

        _watched = location;

        if (_watched is not null)
        {
            _watched.Changed += Update;
        }
    }

    /// <summary>
    /// The zoom the entries are read at, kept within the range the constants state.
    /// </summary>
    /// <remarks>
    /// Set rather than assigned where anything wants another zoom, and written down as soon as it is: what a
    /// dock kept is the host's to save, and a zoom turned with the wheel is the dock keeping something. The
    /// redraw is here for the same reason it was on the slider, the arrangement is what the zoom amounts to,
    /// so a step across the line between rows and tiles is a listing drawn again. A zoom that is already the
    /// one asked for draws nothing, which is what keeps a dock being restored from drawing its listing twice.
    /// </remarks>
    private double Zoom
    {
        get => _zoom;
        set
        {
            var zoom = Math.Clamp(value, Least, Most);

            if (zoom == _zoom)
            {
                return;
            }

            _zoom = zoom;
            _state?.Write(ZoomKey, _zoom.ToString(CultureInfo.InvariantCulture));

            // A name being typed is the view's own edit, and drawing again would take the box with it: the zoom is
            // kept, and the listing is drawn at it once the name is settled.
            if (Editing)
            {
                _deferred = true;

                return;
            }

            Draw();
        }
    }

    /// <summary>
    /// Takes the keyboard back into the listing, which is where a dock's keys are answered from.
    /// </summary>
    /// <remarks>
    /// The listing and not the dock itself: the keys are taken at the top of the dock and handed to the view being
    /// read, so a view that is not focused is a view that never receives one. Deferred to the next turn of the
    /// loop, because the listing it goes into has just been made and has not been laid out yet, a keyboard asked
    /// of a control that is not arranged yet is a keyboard it forgets, the same reason the address defers its own
    /// selection.
    /// </remarks>
    private void Seated() =>
        Dispatcher.UIThread.Post(() =>
        {
            if (_view is { } view)
            {
                view.Listen();
            }

            _logical?.Listen();
        });

    /// <summary>
    /// Reads the keys a dock answers for the whole of itself.
    /// </summary>
    /// <remarks>
    /// Read again on <c>F5</c>, which is the dock's own key, and the three the clipboard answers, which are
    /// taken here rather than by the listing so that they are answered wherever the keyboard is in the dock,
    /// after the zoom was turned as much as after an entry was clicked.
    /// <para>
    /// Nothing is answered while a row's name is being typed, whether or not the box has the keyboard: the box is
    /// this dock's own edit, and a key meant for the name (<c>Delete</c>, <c>F2</c>, the next folder) answered by
    /// the listing because the box lost the keyboard is a row deleted, a second box or a second folder, and none of
    /// them is what the hand reaching for the name meant.
    /// </para>
    /// </remarks>
    /// <param name="sender">The dock.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        if (Editing)
        {
            return;
        }

        if (Keys.Again(e, _whole, _host.Log))
        {
            return;
        }

        if (_view is { } view)
        {
            if (Keys.Forget(e, view.Delete, _host.Log))
            {
                return;
            }

            if (Keys.Rename(e, view.Rename, _host.Log))
            {
                return;
            }

            _ = Keys.Clipboard(
                e,
                new Clipboard(view.Copy, view.Cut, view.Paste, view.NewFolder),
                _host.Log
            );

            return;
        }

        // A place answers the clipboard keys too, and with its own offer: what is copied there is a place's entry and
        // not a file, so the two clipboards are two offers rather than one. What may be named or removed there is the
        // place's to say, a folder of its own is the case it exists for, and a row it does not answer for is left alone.
        if (_logical is { } place)
        {
            if (Keys.Forget(e, place.Delete, _host.Log) || Keys.Rename(e, place.Rename, _host.Log))
            {
                return;
            }

            _ = Keys.Clipboard(
                e,
                new Clipboard(place.Copy, place.Cut, place.Paste, place.NewFolder),
                _host.Log
            );
        }
    }

    /// <summary>Draws a place's rows again after what is on offer to a paste changed.</summary>
    private void RepaintPlace() => _logical?.Repaint();

    /// <summary>Draws the listing with nothing chosen, which is what a view of the work taking the choice away means.</summary>
    private void ClearedSelection()
    {
        ChosenDirectory.Clear();
        _view?.Choose([]);
    }

    /// <summary>
    /// Takes a turn of the wheel with control held as a step of zoom.
    /// </summary>
    /// <remarks>
    /// A turn without control is left alone for whatever is under the pointer to read, which is what makes
    /// the wheel scroll the entries and only zoom them when it is asked to.
    /// </remarks>
    private void Wheeled(object? sender, PointerWheelEventArgs e)
    {
        if (!e.KeyModifiers.HasFlag(KeyModifiers.Control))
        {
            return;
        }

        Zoom += Math.Sign(e.Delta.Y) * Wheel;
        e.Handled = true;
    }

    /// <summary>Draws the entries again when they are not the ones it drew.</summary>
    /// <remarks>
    /// Every read of a directory is a new list, so one list being another is what a step, a refresh
    /// and a change of base all look like from here. Two changes are not that, switching what is shown, and
    /// switching a rule that matches nothing in this directory, and they are the ones the rows are drawn
    /// again for: the entries are the same ones, and how they are drawn is not.
    /// <para>
    /// A place is told from a step by the answer itself, and by neither the list nor the address: the same address
    /// read again is a new answer of the same shape, which is a listing to draw again, while a step to another
    /// place is a different answer entirely, and a step between a place and a directory is the same question
    /// asked of the two modes.
    /// </para>
    /// </remarks>
    private void Update()
    {
        // The switch and the dropdown follow the settings as well as leading them: what is hidden is one
        // choice for every dock, so one opened after another was switched shows what that one shows rather
        // than its own last word on it.
        RefreshHidden();

        // A name being typed is the view's own edit, and the box is a child of the view drawing it: what was read
        // while the name is being typed is owed to the listing that follows the edit (see `_deferred`).
        if (Editing)
        {
            _deferred = true;

            return;
        }

        if (Location.Place is not null)
        {
            Draw();

            return;
        }

        // A dock that has just come out of a place has no drawn listing to compare: what it was drawing is not
        // a list of entries, and the directory it is now looking at has to be drawn whatever it holds.
        if (_drawnPlace is not null)
        {
            Draw();

            return;
        }

        if (ReferenceEquals(_drawn, Location.Shown))
        {
            _view?.Repaint();

            return;
        }

        Draw();
    }

    /// <summary>Draws what was owed while a row's name was being typed, once the row is no longer an edit.</summary>
    /// <remarks>
    /// Asked of the dock rather than drawn by the view: what was read while the name was being typed is the dock's to
    /// read, and drawing it is the dock's to do, the view that told this one has no idea what the location is worth
    /// now, and one view cannot put another in the dock.
    /// <para>
    /// Drawn rather than read again and compared, because what was owed need not be a change of what the location
    /// holds: a zoom turned or a hiding switched while the name was being typed is the same entries drawn another
    /// way, and a comparison of the lists would find nothing to draw.
    /// </para>
    /// </remarks>
    private void Resumed()
    {
        if (!_deferred)
        {
            return;
        }

        _deferred = false;

        RefreshHidden();
        Draw();
    }

    /// <summary>Draws what is there, at the zoom this dock reads it.</summary>
    private void Draw()
    {
        // A drawing supersedes one that was owed while a name was being typed: what this draws is read now.
        _deferred = false;

        // Whether the keyboard is inside the view about to be replaced. It matters because replacing the view is
        // what a step and a zoom change both do, and the control that had the keyboard is then no longer in the
        // tree, so every key after walking into another directory would go unanswered, with nothing anywhere
        // saying why. A view replaced while the keyboard was somewhere else (the zoom, say) leaves it there.
        var on = Keys.On(this);
        var seating = ((Control?)_view ?? _logical) is { } was && Keys.Holds(on, was);

        // A place is read by the same two arrangements a directory is, over a listing of its own: whether an entry
        // is a file is not what decides how it is laid out, and a dock that stopped honouring the zoom at a place
        // would be a dock whose zoom means something only some of the time.
        if (Location.Place is { } place)
        {
            var source = new Places(_host, Location, place, _logicalClip);

            EntryView<LogicalEntry> listing = Zoom > GridAbove
                ? new GridBrowser<LogicalEntry>(_host, source, Icons.SizeAt(Zoom))
                : new ListBrowser<LogicalEntry>(_host, source);

            listing.EditEnded += Resumed;
            _logical = listing;
            _view = null;
            _content.Content = listing;

            _host.Log.Info($"drew the place `{place.Address}`: {listing.Shown} entries");

            if (seating)
            {
                Seated();
            }

            _drawnPlace = place;
            _drawn = null;
            _drawnAt = null;

            Retitled();
            KeepLocation();

            return;
        }

        _logical = null;
        _drawnPlace = null;

        // What was chosen goes with it, for the same reason the tree keeps what was open: a read that found the
        // files it found before is not a reason to stop working with them. Only within one
        // directory, stepping elsewhere is a different listing, and its choice starts empty.
        var chosen = _drawnAt == Location.Current ? _view?.Chosen() ?? [] : [];

        EntryView<Entry> drawn = Zoom > GridAbove
            ? new GridBrowser<Entry>(
                _host,
                new Files(_host, Location, _actions, _clip, _logicalClip),
                Icons.SizeAt(Zoom)
            )
            : new ListBrowser<Entry>(
                _host,
                new Files(_host, Location, _actions, _clip, _logicalClip)
            );
        drawn.EditEnded += Resumed;
        _view = drawn;

        _content.Content = drawn;
        drawn.Choose(chosen);

        // A place this dock was told to show is shown by its directory with the row chosen: what asked for it wanted
        // the file, and a dock that stopped at the directory would leave the user to find it again among the rows.
        var showed = Showing(drawn);

        // A listing rebuilt on another directory starts with nothing chosen, and no selection event
        // says so because nothing changed in the view that was just made: said here, but only when the
        // keyboard was in the view being replaced or the dock itself walked elsewhere, so that a dock
        // refreshing behind another one does not take the choice away from the listing being worked in.
        if (
            seating
            || showed
            || chosen.Count > 0
            || (_drawn is not null && _drawnAt != Location.Current)
        )
        {
            ChosenEntries.Publish(drawn.Chosen());
        }

        // Said here rather than left to the view: whether the listing was drawn again is the other half of
        // "did a change reach this dock", and a report of one that did not cannot tell the two halves apart.
        _host.Log.Info($"drew the listing of `{Location.Current}`: {drawn.Shown} entries");

        // A showing puts the keyboard in the listing it chose a row of: the dock was brought forward to be worked in,
        // and a choice that the keyboard cannot act on is a row the user has to find again.
        if (seating || showed)
        {
            Seated();
        }

        _drawn = Location.Shown;
        _drawnAt = Location.Current;

        Retitled();
        KeepLocation();
    }

    /// <summary>
    /// Chooses the file this dock was last told to show, where this drawing is the one that has it.
    /// </summary>
    /// <remarks>
    /// Read and let go of here rather than kept: a showing is one navigation and one choice, and a path left over would
    /// be chosen again in the next drawing of the same directory, a step back to where the user was, say, silently
    /// moving their choice.
    /// </remarks>
    /// <param name="drawn">The listing that was just drawn.</param>
    /// <returns>Whether a row was chosen for a showing.</returns>
    private bool Showing(EntryView<Entry> drawn)
    {
        var showing = _revealing;

        _revealing = null;

        if (
            showing is not { } path
            || !string.Equals(Path.GetDirectoryName(path), Location.Current, StringComparison.Ordinal)
        )
        {
            return false;
        }

        var entry = Location.Shown.FirstOrDefault(item =>
            string.Equals(item.Path, path, StringComparison.Ordinal)
        );

        // A path whose file is gone, or which a hide rule leaves out of the listing, is shown by its directory alone:
        // what the user asked for is where it is, and a dock that refused to move over a row it cannot draw would be
        // refusing the half of the showing it can do.
        if (entry is not { Kind: EntryKind.File })
        {
            return false;
        }

        drawn.Reveal(entry);

        // The one answer a view of a file's history follows: a row chosen from elsewhere is the file the program is
        // about in exactly the way a click is, and a second notion of "the file last went to" would be a second one
        // to keep in step.
        ChosenEntry.Publish(entry);

        return true;
    }

    /// <summary>Says that a path asked to be shown is nowhere, and leaves the dock where the user left it.</summary>
    /// <param name="path">The path.</param>
    private void Refused(string path)
    {
        _host.Log.Warn($"`{path}` cannot be shown: neither it nor the directory holding it is there");

        _host.Dialogs.Notify(
            new Notification(
                NotificationLevel.Warning,
                RolaI18N.Get("rorolala_file_system.not_there_title"),
                () =>
                    new TextBlock
                    {
                        Text = RolaI18N.Get("rorolala_file_system.not_there", path),
                        TextWrapping = TextWrapping.Wrap,
                        MaxWidth = 420,
                    }
            )
        );
    }

    /// <summary>Takes the name of where this dock is looking, and says it when it has changed.</summary>
    /// <remarks>
    /// Taken as the listing is drawn rather than as the location changes, because the drawing is when the place is read:
    /// a place's title is the provider's answer for the address being read, and a dock told where to look is drawn
    /// again for it. Nothing is said when nothing changed, a dock drawn again for a file change is the same dock in
    /// the same place, and a tab said again for it would be a tab redrawn in the middle of a file operation.
    /// </remarks>
    private void Retitled()
    {
        var (title, detail) = Where();

        if (
            string.Equals(Title, title, StringComparison.Ordinal)
            && string.Equals(Detail, detail, StringComparison.Ordinal)
        )
        {
            return;
        }

        Title = title;
        Detail = detail;
        TitleChanged?.Invoke();
    }

    /// <summary>What this dock is called, and where that is in full.</summary>
    /// <remarks>
    /// A directory inside the tree's base is named by its part under that base, which is what a tab needs to tell two
    /// places apart without spending the tab strip on a drive letter. The detail stays the whole path, since a tip and a
    /// window title have room for it and the address bar already answers navigation with its own crumbs.
    /// <para>
    /// A place is named by its address in both, because an address is not a path and has no base to be short of. The
    /// computer and a directory outside the base are named by the whole of what they are for the same reason: there is
    /// nothing to shorten them against.
    /// </para>
    /// </remarks>
    /// <returns>The title and the detail.</returns>
    private (string Title, string Detail) Where()
    {
        // A place is named by its address, which is the whole of where it is: what kind of place it is and which place
        // of that kind. What the provider calls it is a reading of the address rather than another place, and the real
        // path is the address.
        if (Location.Place is { } place)
        {
            var address = place.Address.ToString();

            return (address, address);
        }

        var at = Location.Current;

        // The computer is the place that is no path at all, so it is named rather than read out of one.
        if (at.Length == 0)
        {
            var computer = RolaI18N.Get("rorolala_file_system.computer");

            return (computer, computer);
        }

        var root = Location.BaseDir;

        if (root.Length > 0 && Same(root, at))
        {
            return (Leaf(at), at);
        }

        return root.Length > 0 && Under(root, at) ? (Path.GetRelativePath(root, at), at) : (at, at);
    }

    /// <summary>Whether two paths name the same directory, by the comparison the platform uses.</summary>
    /// <param name="left">One path.</param>
    /// <param name="right">The other path.</param>
    /// <returns>Whether they are the same directory.</returns>
    private static bool Same(string left, string right) =>
        string.Equals(
            Path.TrimEndingDirectorySeparator(Path.GetFullPath(left)),
            Path.TrimEndingDirectorySeparator(Path.GetFullPath(right)),
            PathComparison
        );

    /// <summary>Whether a path is strictly below a directory.</summary>
    /// <param name="root">The directory.</param>
    /// <param name="path">The path to test.</param>
    /// <returns>Whether the path is below the directory and not the directory itself.</returns>
    private static bool Under(string root, string path)
    {
        var basePath = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root));
        var full = Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));

        if (string.Equals(basePath, full, PathComparison))
        {
            return false;
        }

        var relative = Path.GetRelativePath(basePath, full);

        return !Path.IsPathRooted(relative)
            && !string.Equals(relative, "..", StringComparison.Ordinal)
            && !relative.StartsWith($"..{Path.DirectorySeparatorChar}", StringComparison.Ordinal);
    }

    /// <summary>The last part of a directory's path, or the path itself where it has no last part.</summary>
    /// <param name="directory">The directory.</param>
    /// <returns>The leaf, or the whole path for a root.</returns>
    private static string Leaf(string directory)
    {
        var full = Path.TrimEndingDirectorySeparator(Path.GetFullPath(directory));
        var leaf = Path.GetFileName(full);

        return leaf.Length > 0 ? leaf : full;
    }

    /// <summary>How paths are compared on the platform the program is running on.</summary>
    private static StringComparison PathComparison =>
        OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;

    /// <summary>
    /// Writes where this dock is, so that a dock left at a place comes back to it.
    /// </summary>
    /// <remarks>
    /// The words the location is written in rather than a directory: an address is not a path, and a dock left at one
    /// would otherwise come back at whatever directory the whole happens to be looking at. Empty while the dock follows
    /// the whole (where it is, then, is where the whole is) so that a dock that was taken back into step does not go
    /// on restoring the place it left.
    /// </remarks>
    private void KeepLocation() => _state?.Write(LocationKey, _own is null ? string.Empty : Location.Current);

    /// <summary>
    /// The zoom this dock was left at, written down either as a zoom or, from before there were zooms, as
    /// the arrangement it used to be read in.
    /// </summary>
    /// <param name="state">What the dock kept.</param>
    /// <returns>The zoom, or nothing when it kept none.</returns>
    private static double? Kept(IDockState state) =>
        double.TryParse(state.Read(ZoomKey), NumberStyles.Float, CultureInfo.InvariantCulture, out var zoom)
            ? Math.Clamp(zoom, Least, Most)
            : state.Read(LayoutKey) switch
            {
                Grid => Opening,
                List => Least,
                _ => null,
            };
}

using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>
/// The File Navigation dock: the directories under the base, as a tree.
/// </summary>
/// <remarks>
/// It is a dock of its own rather than a layout the directory's dock can be put into.
/// A tree is not another way of reading one directory; it is a way of walking the ones under a place,
/// and it is the only view that is rooted somewhere the location is not — so it is not rearranged by
/// stepping through directories, and it is worth having on screen beside one that is.
/// </remarks>
internal sealed class TreeDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly TreeControl _view;

    /// <summary>Makes the file navigation dock.</summary>
    /// <param name="host">The host, for logging what the tree cannot do.</param>
    /// <param name="browser">The one location and base, which the tree is rooted at.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    /// <param name="logical">What a place's own copy or cut has put within reach of a paste.</param>
    public TreeDock(IPluginHost host, Browser browser, Clip clip, LogicalClip? logical = null) =>
        _view = new TreeControl(host, browser, new BrowserActions(host, browser, clip), clip, logical);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header => [];
}

/// <summary>
/// The tree's control: the base, and everything under it.
/// </summary>
/// <remarks>
/// The tree is rooted at the base rather than at the location, which is what a base is for: what a
/// user has opened stays open while the location moves under it, and a step is read when it is opened
/// so that a directory with many directories under it costs a listing only when it is looked at.
/// <para>
/// One button sits over it, which takes the root to the top of the platform — the drive list on
/// Windows, the root on Unix — and roots the tree there: the way back to a place you can start
/// choosing a new base from.
/// </para>
/// </remarks>
internal sealed class TreeControl : UserControl
{
    /// <summary>Where the browser is, and where its base is.</summary>
    private readonly Browser _browser;

    /// <summary>The host, which the tree is made over.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the rows do, which is what every other view of the location does.</summary>
    private readonly BrowserActions _actions;

    /// <summary>What a copy or a cut has put within reach of a paste, which a cut step is drawn faded from.</summary>
    private readonly Clip _clip;

    /// <summary>A row's pictures, badges, menu and opening, which are a directory listing's answers.</summary>
    private readonly Files _listing;

    /// <summary>The steps that were open the last time this program ran.</summary>
    private readonly TreeMemory _memory;

    /// <summary>
    /// The tree, and nothing else, which is the whole of the dock.
    /// </summary>
    /// <remarks>
    /// Held apart from the ground it is drawn on so that what the dock draws is one thing: what is put here is
    /// the tree, and a change to how the dock is dressed does not have to know that.
    /// </remarks>
    private readonly ContentControl _content = new();

    /// <summary>
    /// What the tree was last read against, so that a change to any of it is a tree to read again.
    /// </summary>
    /// <remarks>
    /// The base, the switch that shows what is hidden, and the providers in force: the first is what the
    /// tree is rooted at, and the other two decide which rows there are under it. A tree is read a step at
    /// a time and keeps what it read, so a hiding that changed is not something a repaint reaches — the rows
    /// themselves come and go — which is why it is compared here rather than faded where they are drawn.
    /// </remarks>
    private (string Base, bool Shown, IReadOnlySet<string> Rules)? _drawn;

    /// <summary>Makes the tree's control.</summary>
    /// <param name="host">The host, for what a drop cannot do.</param>
    /// <param name="browser">The one location and base, which the tree is rooted at.</param>
    /// <param name="actions">What the rows do.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    /// <param name="logical">What a place's own copy or cut has put within reach of a paste.</param>
    public TreeControl(
        IPluginHost host,
        Browser browser,
        BrowserActions actions,
        Clip clip,
        LogicalClip? logical = null
    )
    {
        _host = host;
        _browser = browser;
        _actions = actions;
        _clip = clip;
        _memory = new TreeMemory(host.Cache.Directory);
        _listing = new Files(host, browser, actions, clip, logical);

        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            _browser.Reread += Draw;
            Draw();
        };
        DetachedFromVisualTree += (_, _) =>
        {
            _browser.Changed -= Update;
            _browser.Reread -= Draw;
        };

        // Taken on the way down, like the keys a listing answers: F5 is a key the dock answers wherever in it the
        // keyboard is, rather than only where a row was clicked.
        AddHandler(KeyDownEvent, Keyed, RoutingStrategies.Tunnel);

        // A dock is a place the keyboard can be put, which is how the shell hands it to the tree being read: what
        // this dock answers is answered at the top of itself, so the shell putting the keyboard here is what makes
        // the keys work after this dock is chosen.
        Focusable = true;
        GotFocus += (_, e) =>
        {
            if (ReferenceEquals(e.Source, this))
            {
                Seated();
            }
        };

        // The tree is the whole dock: no row above it, because what a row there held is offered where the
        // tree is read instead — the empty space's own menu carries the way back to the top of the platform
        // (see `Actions.Empty`) — and a row that only holds one button is a row the steps are not drawn in.
        //
        // The sidebar ground: the tree sits on the elevated surface, which is what reads as a pane of the
        // window rather than as a bare stretch of its ground. The edge against the dock beside it is not
        // drawn here, because the dock area draws the line between two docks itself and draws it the same
        // wherever two meet: one drawn here as well would stand a pixel inside that one and read as two.
        var ground = new Border { Child = _content };
        ground[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);

        Content = ground;

        Draw();
    }

    /// <summary>Roots the tree again when the base or the hiding is not what it was read against.</summary>
    private void Update()
    {
        if (
            _drawn is not { } drawn
            || !string.Equals(drawn.Base, _browser.BaseDir, StringComparison.Ordinal)
            || drawn.Shown != _browser.ShowHidden
            || !drawn.Rules.SetEquals(_browser.HideRegistry.Selection())
        )
        {
            Draw();

            return;
        }

        // Nothing to read again, but what a provider answered may have landed since the rows were drawn: a badge
        // is asked for when a row is made, so a strip refilled only by a rebuild would be a badge that appears and
        // then vanishes until the next rebuild. What is asked again is the badges, not the directories.
        (_content.Content as TreeBrowser)?.RepaintBadges();
    }

    /// <summary>
    /// Reads the keys a dock answers for the whole of itself.
    /// </summary>
    /// <remarks>
    /// Read again on <c>F5</c>, and the three the clipboard answers, over the step that is chosen: a tree is a
    /// view of directories, and a directory is what a copy, a cut and a paste are about as much as an entry of a
    /// listing is.
    /// </remarks>
    /// <param name="sender">The dock.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        if (Keys.Again(e, _browser, _host.Log))
        {
            return;
        }

        if (_content.Content is TreeBrowser tree)
        {
            if (Keys.Forget(e, tree.Delete, _host.Log))
            {
                return;
            }

            _ = Keys.Clipboard(e, new Clipboard(tree.Copy, tree.Cut, tree.Paste), _host.Log);
        }
    }

    /// <summary>Keeps what a step says about being open, for the next run to open it again.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The step, and whether it is open.</param>
    private void Stepped(object? sender, StepEventArgs e) => _memory.Remember(e.Path, e.Open);

    /// <summary>
    /// Builds the tree over the base, keeping the steps that were open and what was chosen.
    /// </summary>
    /// <remarks>
    /// The steps and the choice are asked of the tree being replaced rather than kept here, because they are
    /// what that tree was drawn with: a directory whose contents have moved is a tree to build again — a step
    /// may have come or gone — and a user who has walked down into a step should not be put back at the base for
    /// it, nor lose the step the clipboard's keys would act on.
    /// </remarks>
    private void Draw()
    {
        var was = _content.Content as TreeBrowser;

        // Whether the keyboard is inside the tree about to be replaced, for the reason a listing is asked about as
        // well: a view replaced from under the keyboard takes it away, and this dock answers its keys at the top of
        // itself — so a tree rebuilt while it had the keyboard would answer nothing afterwards.
        var seating = was is { } old && Keys.Holds(Keys.On(this), old);

        // A tree built for the first time is one nothing in this run says anything about, so it opens what
        // was open when the program last ran: a user is put back where they were rather than at the base.
        IReadOnlyList<string> opened = was is null ? _memory.Opened(_browser.BaseDir) : was.Opened();

        var tree = new TreeBrowser(_host, _browser, _browser.BaseDir, _actions, _clip, _listing);
        tree.StepChanged += Stepped;
        _content.Content = tree;
        tree.Reopen(opened, was?.Chosen());

        if (seating)
        {
            Seated();
        }

        _drawn = (
            _browser.BaseDir,
            _browser.ShowHidden,
            _browser.HideRegistry.Selection()
        );
    }

    /// <summary>
    /// Takes the keyboard into the tree, which is where this dock's keys are answered from.
    /// </summary>
    /// <remarks>
    /// Deferred to the next turn of the loop, because the tree it goes into has just been made and is not laid out
    /// yet — a keyboard asked of a control that is not arranged yet is a keyboard it forgets. Not the
    /// tree itself but the row that is chosen, when there is one: what a keyboard on the tree reaches is that row.
    /// </remarks>
    private void Seated() =>
        Dispatcher.UIThread.Post(() => (_content.Content as TreeBrowser)?.Listen());
}

/// <summary>The directories under one directory, as a tree opened downward.</summary>
/// <remarks>
/// A step is read when it is first opened, so a directory with many directories under it costs a
/// listing only when the user looks there. Whether a step offers an expander at all is settled before
/// that, and costs one entry of the step: a step with nothing under it is given no children, and the
/// base theme draws no chevron for one that has none.
/// </remarks>
internal sealed class TreeBrowser : UserControl
{
    /// <summary>Where the browser is, which the tree reads and switches.</summary>
    private readonly Browser _browser;

    /// <summary>The host, for what a drag cannot do.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the rows do, which is what every other view of the location does.</summary>
    private readonly BrowserActions _actions;

    /// <summary>What a copy or a cut has put within reach of a paste, which a cut step is drawn faded from.</summary>
    private readonly Clip _clip;

    /// <summary>A row's pictures, badges, menu and opening, which are a directory listing's answers.</summary>
    private readonly Files _listing;

    /// <summary>The tree itself, which the open steps are gathered from when it is built again.</summary>
    private readonly TreeView _tree;

    /// <summary>
    /// What says that a step was opened or closed, which is what the tree is remembered by.
    /// </summary>
    /// <remarks>
    /// Said as it happens rather than gathered when the tree is built again: a run that ends with a step open
    /// never builds the tree again, and that step is the one worth remembering.
    /// </remarks>
    public event EventHandler<StepEventArgs>? StepChanged;

    /// <summary>
    /// The directory the tree is rooted at.
    /// </summary>
    /// <remarks>
    /// Kept because the root is a row of the tree like any other and yet is not a step of it: it is what the
    /// tree stands on, so removing it is not offered and a chosen root is not what the delete key acts on.
    /// </remarks>
    private readonly string _root;

    /// <summary>How a drag is taken here: which step it would land in, and which row is lit.</summary>
    private readonly Drops _drops;

    /// <summary>The card that follows a drag, drawn over the tree while the pointer is in it.</summary>
    private readonly Ghost _ghost;

    /// <summary>Every row's badge strip, so that what it holds can be asked for again.</summary>
    private readonly List<(string Path, StackPanel Strip)> _worn = [];

    /// <summary>Every step on screen, so that the one a drag is over can be lit.</summary>
    /// <remarks>
    /// A step is recorded when it is made and never taken out: a tree read a step at a time keeps the ones it
    /// has read, closed or not, so a row that was recorded is still the row the tree draws — and what is read
    /// is bounded by what a user opened.
    /// </remarks>
    private readonly List<(string Path, Border Row)> _rows = [];

    /// <summary>The row wearing the drop mark, so that it can be taken off again.</summary>
    private Border? _marked;

    /// <summary>
    /// The step that is chosen, or nothing where none is.
    /// </summary>
    /// <remarks>
    /// Kept here rather than read back from the toolkit's selection, because what choosing a row does here is go
    /// there: a row is a step, and whether the toolkit also calls it selected is not something the clipboard's
    /// keys can act on. It is remembered across a rebuild along with the open steps, since being read again is
    /// what a rebuilt tree is.
    /// </remarks>
    private string? _chosen;

    /// <summary>The press a drag of a step would begin from, while it may still become one.</summary>
    private PointerPressedEventArgs? _pressed;

    /// <summary>
    /// The steps that are to be read again when they are next opened.
    /// </summary>
    /// <remarks>
    /// A step reads the directories under it once and holds them, which is what keeps a tree over a deep
    /// directory cheap. A directory made under a step is the one thing that makes what it holds wrong, so the
    /// step is named here and reads again the next time it is opened. Steps are named by path rather than by
    /// row because the row may not exist yet when the change is made — a step that was never opened has no row.
    /// </remarks>
    private readonly HashSet<string> _stale = new(StringComparer.Ordinal);

    /// <summary>
    /// The newest reading claimed for each step, so that a reading another has superseded does not land.
    /// </summary>
    /// <remarks>
    /// A step read again — one a directory was made in is closed and opened again — may have a reading on its
    /// way from before, and both would draw rows: the one that lands is the newest claim, and the older is
    /// dropped rather than left to rebuild what it found.
    /// </remarks>
    private readonly Dictionary<string, int> _readings = new(StringComparer.Ordinal);

    /// <summary>
    /// The steps still to be opened and the step still to be chosen, while what holds them is being read.
    /// </summary>
    /// <remarks>
    /// Kept rather than given up on, because a reading made while the rules are being read is made a moment
    /// after it is asked for: a tree built again would otherwise come back with nothing open whenever the
    /// repository it is rooted in had not been read yet (see <see cref="Resume"/>).
    /// </remarks>
    private readonly List<string> _opening = [];
    private string? _choosing;

    /// <summary>
    /// The box a step's name is being typed in, while one is, and nothing otherwise.
    /// </summary>
    /// <remarks>
    /// Held rather than looked up, because whether a name is being edited is a state of the view: a box found
    /// in the tree would be as likely to be the search field of something else, and the row it is in is what
    /// has to be replaced when the editing ends.
    /// </remarks>
    private TextBox? _editor;

    /// <summary>The name the box was put in place of, so that cancelling puts it back.</summary>
    private TextBlock? _label;

    /// <summary>
    /// How many turns a box is asked for before it is given up on.
    /// </summary>
    /// <remarks>
    /// A handful, because the turns it waits for are the reading of a directory and the lay-out of the row it
    /// makes: a few more than those are needed costs nothing, and asking forever would be a loop that never
    /// ends for a directory this view is not drawing at all — one made in another dock's location, say.
    /// </remarks>
    private const int Tries = 6;

    /// <summary>
    /// The class a step's name wears, which is the one a listing's name wears.
    /// </summary>
    /// <remarks>
    /// The same word in both views because it is the same thing: what a rename edits is the label a row draws
    /// its name in, and the two views are two drawings of the same directories.
    /// </remarks>
    private const string EntryName = "entry-name";

    /// <summary>Where that press landed and which step it was on, while it may still become a drag.</summary>
    private Point _slip;
    private string? _slipped;

    /// <summary>
    /// Whether that press became a drag.
    /// </summary>
    /// <remarks>
    /// A step of the tree is both a place and a thing: choosing one goes there, and dragging one carries it
    /// there. Which of the two a press was is settled by whether it turned into a drag before it was let go, and
    /// the answer is kept until the next press so that the release of a drag is not also a step.
    /// </remarks>
    private bool _dragged;

    /// <summary>Makes the tree rooted at a directory.</summary>
    /// <param name="host">The host, for what a drop or a drag cannot do.</param>
    /// <param name="browser">Where the browser is, which the tree reads and switches.</param>
    /// <param name="root">The directory the tree is rooted at.</param>
    /// <param name="actions">What a directory does when it is chosen or right-clicked.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    /// <param name="listing">The listing a row is drawn and treated by.</param>
    public TreeBrowser(
        IPluginHost host,
        Browser browser,
        string root,
        BrowserActions actions,
        Clip clip,
        Files listing
    )
    {
        _browser = browser;
        _host = host;
        _actions = actions;
        _clip = clip;
        _listing = listing;
        _root = root;
        _drops = new Drops(host, browser, Mark);

        var tree = new TreeView { ContextMenu = actions.Empty(this) };
        tree.Items.Add(Node(root));
        _tree = tree;

        // A drag is taken over the whole tree and not over the rows alone: the space beside and below them is
        // not a directory, and a drag let go there is refused by saying so rather than by saying nothing.
        DragDrop.SetAllowDrop(this, true);
        AddHandler(DragDrop.DragEnterEvent, DraggedOver);
        AddHandler(DragDrop.DragOverEvent, DraggedOver);
        AddHandler(DragDrop.DragLeaveEvent, DraggedOff);
        AddHandler(DragDrop.DropEvent, Dropped);

        // A step is read once and holds what it read, so a folder made in one this view is showing has to be
        // asked for by this view: the browser reading every directory again is what a listing turns into rows
        // and what a tree does not, since a tree only reads the steps a user opened.
        _actions.OnMade = Made;

        // Taken on the way down, before the row reads them, so that a drag can begin from the press that would
        // otherwise only choose the step: nothing here is marked handled, so choosing still happens as well.
        AddHandler(PointerPressedEvent, Pressed, RoutingStrategies.Tunnel);
        AddHandler(PointerMovedEvent, Moved, RoutingStrategies.Tunnel);
        AddHandler(PointerReleasedEvent, Released, RoutingStrategies.Tunnel);

        // Redrawn while on screen, like the listing beside it: a dock that was closed is not a view of anything
        // to keep in step. The box a new directory's name is typed in is heard for the same reason and only
        // while on screen: a dock that was closed is not a view of anything to name either.
        AttachedToVisualTree += (_, _) =>
        {
            _clip.Changed += Repaint;
            _actions.Naming += Named;
        };
        DetachedFromVisualTree += (_, _) =>
        {
            _clip.Changed -= Repaint;
            _actions.Naming -= Named;
        };

        var over = new Canvas { IsHitTestVisible = false };
        _ghost = new Ghost(over);

        var grid = new Grid();
        grid.Children.Add(tree);

        // The base is a row of the tree whether or not it holds anything under it, so a base that holds
        // nothing still shows that one row; the word says so rather than leaving the pane reading as a
        // tree that has not finished loading.
        if (!HoldsAny(root))
        {
            grid.Children.Add(Empty());
        }

        grid.Children.Add(over);

        Content = grid;
    }

    /// <summary>Says whether a drag may land here, and lights the step it would land in.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOver(object? sender, DragEventArgs e)
    {
        _ghost.Following(e.GetPosition(this));

        var land = Landing(e);

        Mark(land?.Row);
        e.DragEffects = land is null ? DragDropEffects.None : Drops.Over(e);
        e.Handled = e.DragEffects != DragDropEffects.None;
    }

    /// <summary>Takes the drop mark off when a drag leaves, and the card with it.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOff(object? sender, DragEventArgs e)
    {
        _drops.Off();
        _ghost.Unghost();
    }

    /// <summary>Lets a drag go into the step it was over, and takes the card off.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The drop.</param>
    private void Dropped(object? sender, DragEventArgs e)
    {
        _ghost.Unghost();
        _drops.Dropped(Landing(e), e);
    }

    /// <summary>
    /// Where a drag would land: the directory of the step it is over.
    /// </summary>
    /// <remarks>
    /// The step itself and not the space around it — a tree holds the steps a user opened, and the space beside
    /// and below them names nothing to go into, so a drag let go there is let go nowhere rather than being read
    /// as the base. The computer is a place to choose a drive from rather than a directory, so a step standing
    /// for it is not somewhere a drag can go either.
    /// </remarks>
    /// <param name="e">The drag.</param>
    private Land? Landing(DragEventArgs e) =>
        e.DataTransfer.Contains(DataFormat.File) &&
        Step(e.GetPosition(this)) is { } path &&
        !Browser.IsComputer(path)
            ? new Land(path, path)
            : null;

    /// <summary>
    /// Takes a press as the beginning of a drag of the step it landed on.
    /// </summary>
    /// <remarks>
    /// Nothing is decided until the pointer has moved far enough to say the hand meant a drag rather than a click
    /// (<see cref="Drag.Slip"/>), so a press that only chooses a step still chooses it. The computer is not
    /// carried: it stands for the drives rather than being one of them.
    /// </remarks>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The press.</param>
    private void Pressed(object? sender, PointerPressedEventArgs e)
    {
        _dragged = false;
        _pressed = null;
        _slipped = null;

        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            return;
        }

        var at = e.GetPosition(this);

        if (Step(at) is not { } path || Browser.IsComputer(path))
        {
            return;
        }

        _pressed = e;
        _slip = at;
        _slipped = path;
    }

    /// <summary>Starts the drag of a step once the press has slipped far enough to mean one.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The move.</param>
    private void Moved(object? sender, PointerEventArgs e)
    {
        if (_slipped is not { } path || _pressed is not { } from)
        {
            return;
        }

        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            return;
        }

        var slid = e.GetPosition(this) - _slip;

        if (Math.Abs(slid.X) < Drag.Slip && Math.Abs(slid.Y) < Drag.Slip)
        {
            return;
        }

        _pressed = null;
        _slipped = null;
        _dragged = true;

        var entry = Being(path);

        Drag.Away(_browser, from, [entry], entry, _host.Log.Error);
    }

    /// <summary>Lets a press that never became a drag go, so that it is read as the step it was.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The release.</param>
    private void Released(object? sender, PointerReleasedEventArgs e)
    {
        _pressed = null;
        _slipped = null;
    }

    /// <summary>The step under a point, or nothing where no step is there.</summary>
    /// <param name="at">The point, in this view's own coordinates.</param>
    private string? Step(Point at)
    {
        for (var visual = this.GetVisualAt(at) as Visual; visual is not null; visual = visual.GetVisualParent())
        {
            if (visual is TreeViewItem item && item.Tag is string path)
            {
                return path;
            }
        }

        return null;
    }

    /// <summary>
    /// Puts the keyboard in the tree, which is where this dock's keys are answered from.
    /// </summary>
    /// <remarks>
    /// The tree itself and not a row: a row is built into it, and what a keyboard on the tree reaches is the step
    /// that is chosen under it — and being built again is exactly what a tree that lost the keyboard looks like
    /// from here.
    /// </remarks>
    public void Listen() => _tree.Focus();

    /// <summary>
    /// The steps that are open, outermost first.
    /// </summary>
    /// <remarks>
    /// In the order they are drawn, so that a step is opened before the steps under it are asked for: what is
    /// under a step is not read until it is opened, and a step that was never read holds no row to find.
    /// </remarks>
    /// <returns>The paths of the steps that are open.</returns>
    public IReadOnlyList<string> Opened()
    {
        var opened = new List<string>();
        Walk(_tree.Items, opened);

        return opened;
    }

    /// <summary>The step that is chosen, or nothing where none is.</summary>
    public string? Chosen() => _chosen;

    /// <summary>
    /// Shows a directory that has just been made.
    /// </summary>
    /// <remarks>
    /// A step holds what it read when it was opened, so one that is already open goes on showing the steps it
    /// held before the folder was made: the step it was made in is named as stale, which is what makes opening
    /// it read the directories under it again. The name to be typed is handed over by the actions rather than
    /// here (see <see cref="Named"/>), because opening the step is what reads it and the reading is what has
    /// to type the name.
    /// </remarks>
    /// <param name="into">The directory it was made in.</param>
    private void Made(string into)
    {
        _stale.Add(into);

        if (Find(_tree.Items, into) is not { } step)
        {
            // Not on screen, so there is nothing to show and nothing to type in: the step was never opened,
            // and opening it later reads the directory as it is then, which is with the folder in it.
            return;
        }

        // Closed and opened again, which is two turns of the loop rather than one: the step has to be told it
        // is closed before it can be told it is open, or a step that is already open is not opened at all. A
        // step that was never opened is opened the once, and one that is empty — which a step with only a
        // placeholder in it is — is not opened onto nothing.
        var holds = HoldsAny(into);

        step.IsExpanded = false;

        if (holds)
        {
            step.IsExpanded = true;
        }
    }

    /// <summary>
    /// Puts a box in the row of a directory that has just been made, once that row is drawn.
    /// </summary>
    /// <remarks>
    /// The row of a directory just made arrives a turn or so after it is made: the tree is read again when the
    /// files change, that reading is itself deferred, and the row is only built once the step holding it is
    /// opened. So the box is asked for again on each of a few turns until the row is there — a box asked for
    /// once, now, would be a box asked for before there is anything to put it in, which is exactly what a tree
    /// did when this was not retried.
    /// </remarks>
    /// <param name="entry">The directory whose name is to be typed.</param>
    private void Named(Entry entry)
    {
        Try(entry, Tries);
    }

    /// <summary>Asks for a box in a row, and asks again on the next turn while the row is not drawn.</summary>
    /// <param name="entry">The directory whose name is to be typed.</param>
    /// <param name="left">How many turns are left to ask on.</param>
    private void Try(Entry entry, int left)
    {
        if (left <= 0)
        {
            return;
        }

        Dispatcher.UIThread.Post(
            () =>
            {
                // A box already open is a box this is not to take the place of: the user may have begun typing
                // in it, and the step a folder was made in is not always the step that was asked about.
                if (_editor is not null)
                {
                    return;
                }

                if (Beginning(entry))
                {
                    return;
                }

                Try(entry, left - 1);
            },
            DispatcherPriority.Background
        );
    }

    /// <summary>
    /// Opens the steps that were open, in the order they are given, and chooses what was chosen.
    /// </summary>
    /// <remarks>
    /// One at a time and outermost first, because opening a step is what reads the steps under it: a step asked
    /// for before its parent is open is a step with no row to find yet. What is not drawn yet is kept rather
    /// than given up on, because a reading of a step whose rules have not been read is made a moment after
    /// this asks for it (<see cref="Resume"/>).
    /// <para>
    /// The choice is restored with them because it is what the clipboard's keys act on: a tree that
    /// came back from being read again with nothing chosen would answer <c>Ctrl+C</c>, <c>Ctrl+X</c> and
    /// <c>Ctrl+V</c> by doing nothing at all, which reads as the keys being broken.
    /// </para>
    /// </remarks>
    /// <param name="opened">The paths of the steps to open, outermost first.</param>
    /// <param name="chosen">The step to choose, or nothing where none was.</param>
    public void Reopen(IReadOnlyList<string> opened, string? chosen)
    {
        _opening.Clear();
        _opening.AddRange(opened);
        _choosing = chosen;

        Resume(landed: false);
    }

    /// <summary>
    /// Opens what can be opened of the steps a rebuilt tree is to bring back, and chooses what was chosen.
    /// </summary>
    /// <remarks>
    /// A step whose row is not drawn is either one the reading holding it has not reached — which is what this
    /// waits for, and why the reading calls this again when it lands — or one that is not there at all, which
    /// is dropped. <paramref name="landed"/> is what tells the two apart: it is said by the reading that has
    /// just finished, since the tree cannot answer whether a row is missing or merely not made yet.
    /// <para>
    /// This is called by the reading as well as by the rebuild, so it is re-entered as each step is opened in
    /// turn: what each call opens is taken off the list before it is opened, and the whole of the list is
    /// looked at again by whichever call is left holding it.
    /// </para>
    /// </remarks>
    /// <param name="landed">Whether a reading has just finished, so that a step still missing is gone.</param>
    private void Resume(bool landed)
    {
        while (_opening.Count > 0)
        {
            if (Find(_tree.Items, _opening[0]) is not { } item)
            {
                if (!landed)
                {
                    return;
                }

                _opening.RemoveAt(0);

                continue;
            }

            var path = _opening[0];
            _opening.RemoveAt(0);

            // Whatever this opened, the step after it has no row until the reading of this one has landed:
            // the reading says when it has, and this is called again by it.
            landed = false;

            if (!item.IsExpanded)
            {
                item.IsExpanded = true;
            }
        }

        // The choice is put back once its row is drawn, which is after the reading holding it has landed.
        if (_choosing is { } choose && Find(_tree.Items, choose) is { } pick)
        {
            _chosen = choose;
            pick.IsSelected = true;
            _choosing = null;
        }
    }

    /// <summary>Gathers the open steps under some items, outermost first.</summary>
    /// <param name="items">The items to walk.</param>
    /// <param name="opened">Where the paths are gathered.</param>
    private static void Walk(ItemCollection items, List<string> opened)
    {
        foreach (var item in items.OfType<TreeViewItem>())
        {
            if (item.Tag is string path && item.IsExpanded)
            {
                opened.Add(path);
                Walk(item.Items, opened);
            }
        }
    }

    /// <summary>The row standing for a directory, or nothing where the tree has no such row.</summary>
    /// <param name="items">The items to look under.</param>
    /// <param name="path">The directory to look for.</param>
    private static TreeViewItem? Find(ItemCollection items, string path)
    {
        foreach (var item in items.OfType<TreeViewItem>())
        {
            if (item.Tag is string here && string.Equals(here, path, StringComparison.Ordinal))
            {
                return item;
            }

            if (Find(item.Items, path) is { } under)
            {
                return under;
            }
        }

        return null;
    }

    /// <summary>Paints the row standing for a directory, or takes the paint off.</summary>
    /// <param name="directory">The directory being pointed at, or nothing.</param>
    private void Mark(object? directory)
    {
        Unmark();

        if (directory is not string path)
        {
            return;
        }

        foreach (var (step, row) in _rows)
        {
            if (!string.Equals(step, path, StringComparison.Ordinal))
            {
                continue;
            }

            // Bound rather than read, so that the mark follows a colour changed while the program runs, the way
            // the band a frame is drawn with does.
            row[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Selection);
            _marked = row;

            return;
        }
    }

    /// <summary>Takes the drop mark off whatever was wearing it.</summary>
    private void Unmark()
    {
        if (_marked is { } row)
        {
            // Opaque again rather than empty, because the band the row is drawn on is what a click on the space
            // after a name answers: a background taken away would take that click with it.
            row.Background = Brushes.Transparent;
            _marked = null;
        }
    }

    /// <summary>
    /// Chooses a step, which is what the clipboard's keys act on.
    /// </summary>
    /// <remarks>
    /// Going there is the caller's, because choosing and going are one gesture here: what is done here is
    /// remembering it, and having the row drawn as the chosen one so that what a copy or a cut is about is what the
    /// eye says it is.
    /// <para>
    /// What is chosen is also what the rest of the program is told is chosen: a directory listing publishes its own
    /// choice and the entry that was clicked to a catalogue the version graph reads, and a tree is the same view of the
    /// same files, so a file chosen here is a file the graph comes to as well.
    /// </para>
    /// </remarks>
    /// <param name="path">The step to choose.</param>
    private void Choose(string path)
    {
        _chosen = path;

        if (Find(_tree.Items, path) is { } item)
        {
            item.IsSelected = true;
        }

        var entry = Being(path);

        _listing.Chosen([entry]);
        _listing.Clicked(entry);
    }

    /// <summary>Copies the chosen step to the clipboard.</summary>
    public void Copy()
    {
        if (Chosen() is { } path)
        {
            _actions.Copy(this, [Being(path)]);
        }
    }

    /// <summary>Cuts the chosen step to the clipboard, which a paste then moves.</summary>
    public void Cut()
    {
        if (Chosen() is { } path)
        {
            _actions.Cut(this, [Being(path)]);
        }
    }

    /// <summary>
    /// Pastes what is on the clipboard into the chosen step.
    /// </summary>
    /// <remarks>
    /// Into the step that is chosen and not into the base: a tree is walked by choosing steps, so the step a user
    /// last chose is the directory a paste here is about. With nothing chosen there is nowhere to paste, and
    /// nothing happens.
    /// </remarks>
    public void Paste()
    {
        if (Chosen() is { } path)
        {
            _actions.Paste(this, path);
        }
    }

    /// <summary>Removes the chosen step, asking first unless told not to.</summary>
    /// <remarks>
    /// The root is left out: it is the row the tree stands on rather than a step of it, and removing it would
    /// leave the tree rooted at a directory that is no longer there.
    /// </remarks>
    /// <param name="ask">Whether to put the question to the user first.</param>
    public void Delete(bool ask)
    {
        if (Chosen() is { } path && !string.Equals(path, _root, StringComparison.Ordinal))
        {
            _actions.Remove([Being(path)], ask);
        }
    }

    /// <summary>
    /// Makes a directory inside a step of the tree, and opens the step so that it can be seen.
    /// </summary>
    /// <remarks>
    /// Into the step the menu was opened on, which is what a step is: a directory. A menu opened on the space
    /// beside the steps has no such directory, which is why this is offered on a row and not on the tree's own
    /// space — the tree draws the steps under a base rather than one directory, and a folder is made in a
    /// directory.
    /// </remarks>
    /// <param name="into">The directory to make it in.</param>
    public void NewFolder(string into) => _actions.NewFolder(into);

    /// <summary>
    /// Puts the name of a step into a box, where the tree draws it.
    /// </summary>
    /// <remarks>
    /// The row is found by the path in its own tag rather than by what is chosen, because a rename is asked
    /// for about one entry and the choice is not always that entry — a folder just made is renamed without
    /// ever being clicked.
    /// </remarks>
    /// <param name="entry">The directory whose name is being edited.</param>
    public void Rename(Entry entry) => Named(entry);

    /// <summary>Puts the name of a step into a box, where its row and its label are already drawn.</summary>
    /// <param name="entry">The directory whose name is being edited.</param>
    /// <returns>Whether the box was made.</returns>
    private bool Beginning(Entry entry)
    {
        var label = _rows
            .Where(row => string.Equals(row.Path, entry.Path, StringComparison.Ordinal))
            .Select(row => row.Row)
            .SelectMany(row => row.GetVisualDescendants())
            .OfType<TextBlock>()
            .FirstOrDefault(text => text.Classes.Contains(EntryName));

        if (label?.Parent is not Panel parent)
        {
            return false;
        }

        var name = Path.GetFileName(Path.TrimEndingDirectorySeparator(entry.Path)) ?? string.Empty;
        var box = new TextBox
        {
            Text = name,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The box takes the place the name had, in the panel the name sat in, rather than the row being built
        // again around it: the row is the one the tree holds, and swapping it would take the step with it.
        var at = parent.Children.IndexOf(label);
        parent.Children[at] = box;

        _editor = box;
        _label = label;
        _actions.Renaming = entry;

        // Taken on the way down, so that Enter and Escape are read before the box's own reading of them: a
        // field that took Enter for itself would be a name nobody could finish.
        box.AddHandler(KeyDownEvent, (_, args) => Edited(args), RoutingStrategies.Tunnel);
        box.LostFocus += (_, _) => Committed();

        box.Focus();
        box.SelectionStart = 0;
        box.SelectionEnd = Names.Chosen(entry.Kind, name);

        return true;
    }

    /// <summary>Reads a key in the box a name is being typed in.</summary>
    /// <param name="e">The key.</param>
    private void Edited(KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            e.Handled = true;
            Committed();

            return;
        }

        if (e.Key == Key.Escape)
        {
            e.Handled = true;
            Cancelled();
        }
    }

    /// <summary>Takes the name that was typed as the step's new name.</summary>
    /// <remarks>
    /// The keyboard goes back to the tree with it, as it does in a listing: the box was where it was, and the
    /// keys a dock answers are answered from the rows, so a keyboard left in a box that is gone is a dock whose
    /// keys do nothing.
    /// </remarks>
    private void Committed()
    {
        if (_editor is not { } box || _actions.Renaming is not { } entry)
        {
            return;
        }

        var name = box.Text ?? string.Empty;
        Stop();

        _actions.Rename(entry, name);
        Listen();
    }

    /// <summary>Leaves the name as it was.</summary>
    /// <remarks>
    /// The label the box was put in place of is put back, because taking it out is what made room for the box:
    /// leaving the box there with the old name in it would leave a field where a name is drawn.
    /// </remarks>
    private void Cancelled()
    {
        if (_editor is { } box)
        {
            if (box.Parent is Panel panel && _label is { } label && panel.Children.IndexOf(box) is var at && at >= 0)
            {
                panel.Children[at] = label;
            }

            Stop();
            Listen();
        }
    }

    /// <summary>Takes the box out of the row and forgets what was being renamed.</summary>
    private void Stop()
    {
        _editor = null;
        _label = null;
        _actions.Renaming = null;
    }

    /// <summary>
    /// Fades a row whose step is hidden or cut, so that it reads as a listing would draw it.
    /// </summary>
    /// <remarks>
    /// Cut wins, as it does in a listing: being on its way out is what the user just did. A hidden row is
    /// only ever drawn while the switch that shows what is hidden is on, and it is drawn fainter so that
    /// the eye passes over it rather than being left out of the tree.
    /// </remarks>
    /// <param name="row">The row.</param>
    /// <param name="path">The step it stands for.</param>
    private void Fade(Border row, string path) =>
        row.Opacity = _clip.IsCut(path)
            ? _clip.Faded
            : Hides(path)
                ? EntryView.HiddenOpacity
                : 1.0;

    /// <summary>Whether a provider in force hides a directory, read against the base the tree is rooted at.</summary>
    /// <remarks>
    /// Rooted at the base and not at the directory itself, which is what makes one tree read by one set of
    /// rules: a repository nested inside the base is not one the tree asks on its own behalf (see
    /// <see cref="IEntryHideProvider.Hides(Entry, string)"/>).
    /// </remarks>
    /// <param name="directory">The directory to consider.</param>
    /// <returns>Whether a provider in force hides it.</returns>
    private bool Hides(string directory) =>
        _browser.Hides(new Entry(directory, EntryKind.Directory), _root);

    /// <summary>Whether a directory is one the rules leave showing, or the switch shows what they hide.</summary>
    /// <param name="directory">The directory to consider.</param>
    /// <returns>Whether it belongs in the tree.</returns>
    private bool Shown(string directory) => _browser.ShowHidden || !Hides(directory);

    /// <summary>Draws the cut state again on every row on screen.</summary>
    private void Repaint()
    {
        foreach (var (path, row) in _rows)
        {
            Fade(row, path);
        }
    }

    /// <summary>
    /// What the tree says where the base holds nothing under it.
    /// </summary>
    /// <remarks>
    /// It takes no pointer of its own: the space it sits over is still the tree's, and a word that ate the
    /// right-click there would take the tree's own menu with it.
    /// </remarks>
    private static Control Empty() =>
        new StackPanel
        {
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Children =
            {
                new TextBlock
                {
                    Text = "\U0001F5C2",
                    FontSize = 34,
                    Classes = { "faint" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
                new TextBlock
                {
                    Text = RolaI18N.Get("rorolala_file_system.empty"),
                    Classes = { "muted" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
            },
        };

    /// <summary>One directory as a step of the tree, reading its children when first opened.</summary>
    /// <remarks>
    /// Whether a step may be opened is a question for the filesystem, and the rules that answer it are a
    /// repository's to read — which is a program run. It is asked where it can be answered without one: a
    /// step is drawn with the mark that says it may be opened and corrected a moment later, rather than a
    /// window that stops drawing until a program has answered (see <see cref="Settle"/>).
    /// </remarks>
    /// <param name="path">The directory the step stands for.</param>
    private TreeViewItem Node(string path)
    {
        var item = new TreeViewItem { Tag = path };
        var read = false;

        // A step with nothing under it is given no child at all, and that is what leaves it without an
        // expander: one that cannot be opened must not be offered as though it could. The step that
        // has something under it gets a child that is never shown, so that it can be opened at all;
        // it is replaced the first time the step is. What it holds is also what says whether there is
        // anything worth offering to close under it (see Header).
        if (HideRegistry.Ready(_root))
        {
            Settle(item, path, HoldsAny(path));
        }
        else
        {
            // The mark is worn until the rules have been read, and what the rows are is not said before then:
            // a step holding nothing loses it again when the answer is in. A step that is opened in the
            // meantime is left alone, because what its rows are is the reading's to say.
            Waiting(item);
            item.Header = Header(new Entry(path, EntryKind.Directory), item, () => Collapse(item));

            _ = Task.Run(() =>
            {
                HideRegistry.Prepare(_root);

                Dispatcher.UIThread.Post(() =>
                {
                    if (!read)
                    {
                        Settle(item, path, HoldsAny(path));
                    }
                });
            });
        }

        item.Expanded += (_, _) =>
        {
            // Said before the reading: whether a step is open is what the next run opens again, and that is
            // true of a step that is opened and read later as much as of one read now.
            StepChanged?.Invoke(this, new StepEventArgs(path, open: true));

            // Read the first time it is opened, and again only when something under the step has changed on
            // disk: a step holds what it read, and a reading of it is what the steps it holds are made from.
            if (read && !_stale.Remove(path))
            {
                return;
            }

            read = true;

            var token = Claim(path);

            if (HideRegistry.Ready(_root))
            {
                Read(item, path);

                return;
            }

            // The rules a reading is made by are a repository's, and reading them is a program run: it is made
            // off the thread the rows are drawn on, and the rows are made when it is done.
            Waiting(item);

            _ = Task.Run(() =>
            {
                HideRegistry.Prepare(_root);

                Dispatcher.UIThread.Post(() =>
                {
                    if (Current(path, token))
                    {
                        Read(item, path);
                    }
                });
            });
        };

        item.Collapsed += (_, _) => StepChanged?.Invoke(this, new StepEventArgs(path, open: false));

        return item;
    }

    /// <summary>
    /// Says what a step holds: the child that makes it something to open, and what choosing it offers.
    /// </summary>
    /// <param name="item">The step's own row.</param>
    /// <param name="path">The directory the step stands for.</param>
    /// <param name="holds">Whether the rules leave anything under it.</param>
    private void Settle(TreeViewItem item, string path, bool holds)
    {
        if (holds)
        {
            if (item.Items.Count == 0)
            {
                Waiting(item);
            }
        }
        else
        {
            item.Items.Clear();
        }

        // Faded here rather than where the row is made, because what a row is faded by is the rules, and the
        // rules are read by a provider that may have to reach for the filesystem: a row made before they are
        // in hand is drawn plain and faded the moment they are (see `Node`).
        var row = Header(
            new Entry(path, EntryKind.Directory),
            item,
            holds ? () => Collapse(item) : null
        );
        Fade(row, path);
        item.Header = row;
    }

    /// <summary>
    /// Gives a step the child that is never shown, which is what makes it something that can be opened.
    /// </summary>
    /// <param name="item">The step's own row.</param>
    private static void Waiting(TreeViewItem item)
    {
        item.Items.Clear();
        item.Items.Add(new TreeViewItem { Header = new TextBlock { Text = "\u2026", Classes = { "faint" } } });
    }

    /// <summary>Reads the directories under a step and draws them as the steps they are.</summary>
    /// <param name="item">The step's own row.</param>
    /// <param name="path">The directory to read.</param>
    private void Read(TreeViewItem item, string path)
    {
        item.Items.Clear();

        foreach (var child in Children(path))
        {
            // A directory is a step, which opens onto its own entries; a file is where the walking stops.
            item.Items.Add(child.Kind is EntryKind.Directory ? Node(child.Path) : Leaf(child));
        }

        // What a rebuild is to bring back is opened as the reading holding it lands: what is under a step is
        // not known until it has been read, and a step asked for before then has no row to find.
        Resume(landed: true);
    }

    /// <summary>
    /// Claims the reading of a step, so that a reading already on its way is not the one that lands.
    /// </summary>
    /// <param name="path">The step being read.</param>
    /// <returns>The claim, which <see cref="Current"/> is asked about when a reading lands.</returns>
    private int Claim(string path)
    {
        var token = _readings.GetValueOrDefault(path) + 1;
        _readings[path] = token;

        return token;
    }

    /// <summary>Whether a claim is still the newest one made for a step.</summary>
    /// <param name="path">The step being read.</param>
    /// <param name="token">The claim the reading was made under.</param>
    /// <returns>Whether the reading it belongs to is the one to land.</returns>
    private bool Current(string path, int token) => _readings.GetValueOrDefault(path) == token;

    /// <summary>
    /// One row: its picture, the badges worn on it, its name, and what choosing it does.
    /// </summary>
    /// <remarks>
    /// A row is an entry like a directory listing's row: what it is drawn with, what its menu holds and what opens
    /// it are that listing's answers, so the two views of one directory agree about an entry rather than each
    /// drawing it.
    /// <para>
    /// A press on a directory switches the location there and then, rather than waiting for a second one: a tree
    /// is walked by choosing steps, and choosing one can only mean going to it. The step is left open with it, so
    /// the walking is one press per step. What closes a step is its chevron — which the theme draws beside the
    /// header as a control of its own, so a press on the chevron is not a press on the row and never reaches this
    /// handler — and the menu's close-everything. A file is chosen, and opened by a second press, because going to
    /// a file is nothing a tree can do.
    /// </para>
    /// <para>
    /// What is pressed is the band the row is drawn in and not the word written in it, which is why the
    /// band is given a background at all: a panel with none is not hit anywhere its children are not, so
    /// a press on the space after a name would select the row and leave the location where it was.
    /// </para>
    /// <para>
    /// The base theme opens and closes a step on a double click of a row as well, which leaves the chevron
    /// as one of two ways rather than the way. That gesture is taken from it here, and taken from the row
    /// rather than from the tree: the base theme's handler is on the row's own presenter — above it and
    /// below the tree — and a gesture taken by a handler on the row is a gesture that has already been taken
    /// by the time the presenter is reached.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry the row names.</param>
    /// <param name="step">The step the row is the header of, which a press on the row opens.</param>
    /// <param name="collapse">What closes every step under the row, or nothing where it has no steps under it.</param>
    private Border Header(Entry entry, TreeViewItem? step, Action? collapse)
    {
        var path = entry.Path;
        var name = _listing.Name(entry);
        var deletable = !string.Equals(path, _root, StringComparison.Ordinal);

        // Filled when it opens rather than when the row is made, which is how a listing's menu is made as
        // well (see `EntryView.Prepared`). What a menu holds is asked of the plugins, and what they answer
        // — the ownership commands among them — is read from the filesystem: a tree builds a row for every
        // step under an opened one, so a menu made here is a menu paid for on every row nobody opened.
        var menu = new ContextMenu();
        menu.Opening += (_, _) =>
        {
            // Filled by the same actions a listing fills its menu with, with the base row left out of what may be removed.
            _actions.Fill(menu, this, [entry], deletable);

            // Offered by every row with steps under it, which is the same row that is given an expander, since
            // closing them all is worth offering where there is more than one to close. What is not offered is
            // the other half — opening them all — since that would read every directory under the row, which is
            // what a tree read a step at a time is for avoiding.
            if (collapse is not null)
            {
                menu.Items.Add(BrowserActions.Item("rorolala_file_system.collapse_all", collapse));
            }
        };

        // The menu is opened by the control the row is in rather than by the row, since one of the
        // things it does — putting a path on the clipboard — is reached through a control on screen.
        // A tree row is as wide as the tree, so what speaks for an entry is worn at the row's right rather than
        // on the icon's corner: a tree is a list of names, and a badge drawn over a picture in it would be a mark
        // on the picture rather than an answer about the entry.
        var badges = Worn(entry);

        DockPanel.SetDock(badges, Dock.Right);

        _worn.RemoveAll(one => string.Equals(one.Path, path, StringComparison.Ordinal));
        _worn.Add((path, badges));

        var panel = new DockPanel { LastChildFill = true };

        panel.Children.Add(badges);

        panel.Children.Add(
            new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                Children =
                {
                    _listing.Icon(entry, Icons.Row),
                    new TextBlock
                    {
                        Text = name.Length > 0 ? name : path,
                        Classes = { EntryName },
                        VerticalAlignment = VerticalAlignment.Center,
                    },
                },
            }
        );

        var row = new Border
        {
            Background = Brushes.Transparent,
            ContextMenu = menu,
            Child = panel,
        };

        row.Tapped += (_, _) =>
        {
            // A press that became a drag is not also a step: the drag took the pointer, and a hand letting go of
            // one is putting something down rather than choosing a place.
            if (_dragged)
            {
                return;
            }

            Choose(path);

            if (entry.Kind is EntryKind.Directory)
            {
                // A press on a directory is the whole of walking: the location goes there, and the step is left
                // open onto what it holds, so the step after it is one the user can see and press. A press on a
                // row never puts a step away — what closes one is its chevron, or the menu's close-everything —
                // so a tree is never folded up by the press that went into it.
                //
                // Opened before the location moves rather than after it: going to the directory the location is
                // already in reads it again, and a tree read again is built again out of the steps that were open
                // when it was asked (see `TreeControl.Draw`). An opening made after that belongs to the tree that
                // is already gone, and the step the user pressed comes back closed.
                if (step is not null)
                {
                    step.IsExpanded = true;
                }

                _browser.Go(path);
            }
            else if (Path.GetDirectoryName(path) is { Length: > 0 } directory)
            {
                // A file is not a place to go: what a click on one takes the location to is the directory it is
                // in, which is the half of walking a file has. Opening the file is the second click's.
                _browser.Go(directory);
            }
        };

        // A double click opens a step in the base theme, and a tree here is walked by choosing one: the gesture
        // is taken from it, and taken from the row rather than from the tree, because the theme's handler is on
        // the row's own presenter and a gesture taken by a handler on the row is already taken by the time the
        // presenter is reached. A double click in the box a name is being typed in is the box's, though, since
        // that is how a word of a name is chosen.
        row.DoubleTapped += (_, e) =>
        {
            if (e.Source is TextBox)
            {
                return;
            }

            // A directory is walked into by choosing it, so a second click on one is not also a step; a file is
            // opened the way the listing opens one, by the program the plugin answered for it with.
            if (entry.Kind is EntryKind.File)
            {
                _listing.Open(entry);
            }

            e.Handled = true;
        };

        // A step's header is made again when what it holds changes — the way to close it is offered once it has
        // something to close — and the row it had is then no longer the row it is drawn with: keeping both would
        // leave the search for a row a name is edited in finding a row nothing is drawn in.
        _rows.RemoveAll(one => string.Equals(one.Path, path, StringComparison.Ordinal));
        _rows.Add((path, row));

        return row;
    }

    /// <summary>What the contributions said about an entry, drawn as a strip at the row's right.</summary>
    /// <remarks>
    /// The same badges a tile wears on its icon's corner, in the same order, drawn where a row has room for them.
    /// Nothing is made when nothing speaks for the entry, so a row without a badge is a row with nothing added.
    /// </remarks>
    /// <param name="entry">The entry the row is for.</param>
    /// <returns>The strip, which is kept so that what it holds can be asked for again.</returns>
    private StackPanel Worn(Entry entry)
    {
        var strip = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Right,
        };

        Wear(strip, entry);

        return strip;
    }

    /// <summary>Puts what speaks for an entry into the strip at its row's right.</summary>
    /// <param name="strip">The strip to fill.</param>
    /// <param name="entry">The entry the row is for.</param>
    private void Wear(StackPanel strip, Entry entry)
    {
        strip.Children.Clear();

        foreach (var badge in _listing.Badges(entry))
        {
            strip.Children.Add(Icons.Worn(badge, Icons.Row));
        }

        // A row nothing speaks for wears nothing: a strip left in the layout would be room taken from a name for an
        // answer that is not there.
        strip.IsVisible = strip.Children.Count > 0;
    }

    /// <summary>Asks every row again what speaks for it, without reading anything again.</summary>
    /// <remarks>
    /// What the tree holds is asked of the catalogue the same way a listing asks when it is staged, so a reading that
    /// lands is a badge that appears on the row it belongs to rather than on the next rebuild of the tree.
    /// </remarks>
    public void RepaintBadges()
    {
        foreach (var (path, strip) in _worn)
        {
            Wear(strip, Being(path));
        }
    }

    /// <summary>A row for a file, which opens onto nothing.</summary>
    /// <param name="entry">The file the row names.</param>
    /// <returns>The row.</returns>
    private TreeViewItem Leaf(Entry entry)
    {
        // The row is made before its header, since a header is told which step it is: a file has none, because
        // nothing opens under a file.
        var item = new TreeViewItem { Tag = entry.Path };
        item.Header = Header(entry, step: null, collapse: null);

        return item;
    }

    /// <summary>Whether a path is a directory or a file, as the tree knows it when asked.</summary>
    /// <remarks>
    /// Asked of the filesystem rather than carried on a row: a path is a directory until it is not, and a row that
    /// remembered what it was when it was drawn would be a row acting on what is no longer there.
    /// </remarks>
    /// <param name="path">The path.</param>
    /// <returns>The entry it is.</returns>
    private static Entry Being(string path) =>
        new(path, System.IO.Directory.Exists(path) ? EntryKind.Directory : EntryKind.File);

    /// <summary>
    /// Closes every step under a row, and the row itself.
    /// </summary>
    /// <remarks>
    /// Walked rather than closed by setting the one row: closing a step hides the steps under it whether
    /// or not they are closed, so opening it again would show the whole walk the reader had taken down
    /// through it, which is the state this is asked for to leave. Only the steps that were opened are
    /// there to walk, which is what a tree read a step at a time has.
    /// </remarks>
    /// <param name="row">The row to close everything under.</param>
    private static void Collapse(TreeViewItem row)
    {
        row.IsExpanded = false;

        foreach (var step in row.Items.OfType<TreeViewItem>())
        {
            Collapse(step);
        }
    }

    /// <summary>Whether a directory holds any directory the rules leave showing.</summary>
    /// <remarks>
    /// Read to the first entry rather than counted to the last: the whole of a large directory is not
    /// worth reading to answer what one entry already answers. What is skipped is what a listing would
    /// leave out, so a step whose only steps under it are hidden is a step without an expander rather than
    /// one that opens onto nothing. The computer is the exception, holding nothing but the few drives,
    /// which are read outright.
    /// </remarks>
    /// <param name="path">The directory the step stands for.</param>
    private bool HoldsAny(string path)
    {
        if (Browser.IsComputer(path))
        {
            return Children(path).Count > 0;
        }

        try
        {
            // A directory holding nothing but files still opens onto them, which is what a file tree is for.
            return Children(path).Count > 0;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    /// <summary>The entries directly under one that are shown, or the drives when it is the computer.</summary>
    /// <remarks>
    /// Directories come first and each kind is by name, which is the order a file tree is read in: what a user walks
    /// down into stands above what they stop at. Nothing else decides it, so a row keeps its place from one reading
    /// to the next.
    /// </remarks>
    /// <param name="path">The directory to read.</param>
    private IReadOnlyList<Entry> Children(string path) =>
        Browser.IsComputer(path)
            ? [.. Browser.Read(path).Where(entry => Shown(entry.Path)).OrderBy(entry => entry.Kind).ThenBy(entry => entry.Path, StringComparer.OrdinalIgnoreCase)]
            : Entries(path);

    /// <summary>
    /// The directories directly under one that are shown, then the files, or nothing when it cannot be read.
    /// </summary>
    /// <remarks>
    /// A file is a row as a directory is: the tree is a view of what is under a place rather than of the places
    /// under it, and a folder whose only content is files still opens onto them. What is skipped is what a listing
    /// would leave out, so the tree and the directory beside it show the same entries.
    /// </remarks>
    /// <param name="path">The directory to read.</param>
    /// <returns>The entries, directories first.</returns>
    private IReadOnlyList<Entry> Entries(string path)
    {
        try
        {
            IEnumerable<Entry> directories = System.IO.Directory
                .EnumerateDirectories(path)
                .Where(Shown)
                .OrderBy(directory => Path.GetFileName(directory), StringComparer.OrdinalIgnoreCase)
                .Select(directory => new Entry(directory, EntryKind.Directory));

            IEnumerable<Entry> files = System.IO.Directory
                .EnumerateFiles(path)
                .Where(Shown)
                .OrderBy(file => Path.GetFileName(file), StringComparer.OrdinalIgnoreCase)
                .Select(file => new Entry(file, EntryKind.File));

            return [.. directories, .. files];
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }
}
/// <summary>What a step says when it is opened or closed.</summary>
/// <param name="path">The directory the step stands for.</param>
/// <param name="open">Whether it is open now.</param>
internal sealed class StepEventArgs(string path, bool open) : EventArgs
{
    /// <summary>The directory the step stands for.</summary>
    public string Path { get; } = path;

    /// <summary>Whether it is open now.</summary>
    public bool Open { get; } = open;
}
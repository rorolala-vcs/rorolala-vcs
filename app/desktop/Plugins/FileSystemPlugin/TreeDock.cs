using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.LogicalTree;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Menus;

namespace FileSystemPlugin;

/// <summary>
/// The File Navigation dock: the directories under the base, as a tree.
/// </summary>
/// <remarks>
/// It is a dock of its own rather than a layout the directory's dock can be put into.
/// A tree is not another way of reading one directory; it is a way of walking the ones under a place,
/// and it is the only view that is rooted somewhere the location is not, so it is not rearranged by
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
/// One button sits over it, which takes the root to the top of the platform, the drive list on
/// Windows, the root on Unix, and roots the tree there: the way back to a place you can start
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
    /// The base, the switch that shows what is hidden, the providers in force and the set of virtual entries:
    /// the first is what the tree is rooted at, and the rest decide which rows there are under it. A tree is
    /// read a step at a time and keeps what it read, so a hiding that changed or an entry that arrived or went
    /// is not something a repaint reaches, the rows themselves come and go, which is why it is compared here
    /// rather than faded where they are drawn.
    /// </remarks>
    private (string Base, bool Shown, IReadOnlySet<string> Rules, IReadOnlySet<string> Gray, int Shape)? _drawn;

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
            _browser.Changed += UpdateOwed;
            _browser.Reread += AgainOwed;
            _browser.Shared.TouchedIn += ChangedIn;

            // The constructor has already built the tree once. Building it again on the way into the
            // window would replace the rows the reader is about to see for the same rows, which is a
            // whole tree laid out twice for one opening.
            if (_content.Content is not TreeBrowser)
            {
                Draw();
            }
        };
        DetachedFromVisualTree += (_, _) =>
        {
            _browser.Changed -= UpdateOwed;
            _browser.Reread -= AgainOwed;
            _browser.Shared.TouchedIn -= ChangedIn;
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
        // tree is read instead, the empty space's own menu carries the way back to the top of the platform
        // (see `Actions.Empty`), and a row that only holds one button is a row the steps are not drawn in.
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

    /// <summary>Says that what the tree is worth may have changed, once for the frame.</summary>
    private void UpdateOwed() => Frame.Post(Update);

    /// <summary>Says that the files behind the tree may have changed, once for the frame.</summary>
    private void AgainOwed() => Frame.Post(Again);

    /// <summary>
    /// Says that the files of one directory changed, which the step over it is read again for.
    /// </summary>
    /// <remarks>
    /// The step is read again rather than the tree: a directory another program wrote to is one step of the tree
    /// that is wrong, and reading the tree whole for it would make every row on screen again.
    /// </remarks>
    /// <param name="directory">The directory that changed.</param>
    private void ChangedIn(string directory) =>
        Frame.Post(() =>
        {
            if (_content.Content is TreeBrowser tree)
            {
                tree.Again(directory);
            }
        });

    /// <summary>Roots the tree again when the base or the hiding is not what it was read against.</summary>
    private void Update()
    {
        if (
            _drawn is not { } drawn
            || !string.Equals(drawn.Base, _browser.BaseDir, StringComparison.Ordinal)
            || drawn.Shown != _browser.ShowHidden
            || !drawn.Rules.SetEquals(_browser.HideRegistry.Selection())
            || !drawn.Gray.SetEquals(_browser.HideRegistry.Gray())
            || drawn.Shape != VirtualEntries.Shape
        )
        {
            Draw();

            return;
        }

        // Nothing to read again, but what a provider answered may have landed since the rows were drawn: a badge
        // is asked for when a row is made, so a strip refilled only by a rebuild would be a badge that appears and
        // then vanishes until the next rebuild. What is asked again is the badges and the words under the row,
        // not the directories.
        if (_content.Content is TreeBrowser tree)
        {
            tree.RepaintBadges();
            tree.RepaintColours();
        }
    }

    /// <summary>
    /// Reads the keys a dock answers for the whole of itself.
    /// </summary>
    /// <remarks>
    /// Read again on <c>F5</c>, and the clipboard's three, <c>Delete</c>, <c>F2</c> and <c>Ctrl+Shift+N</c>, over
    /// the step that is chosen: a tree is a view of directories, and a directory is what a copy, a cut and a paste
    /// are about as much as an entry of a listing is.
    /// </remarks>
    /// <param name="sender">The dock.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        // A name being typed is the box's, not the tree's: Ctrl, Alt and the tree's own keys belong to the
        // field while it is there, and Escape is the field's way out (see `Edited`).
        var tree = _content.Content as TreeBrowser;

        if (tree?.Editing == true)
        {
            return;
        }

        if (Keys.Again(e, _browser, _host.Log))
        {
            return;
        }

        if (tree is not null)
        {
            if (
                e.Key == Key.A
                && e.KeyModifiers.HasFlag(KeyModifiers.Control)
                && !e.KeyModifiers.HasFlag(KeyModifiers.Alt)
            )
            {
                tree.SelectAll();
                e.Handled = true;

                return;
            }

            if (Keys.Forget(e, tree.Delete, _host.Log))
            {
                return;
            }

            if (Keys.Rename(e, tree.Rename, _host.Log))
            {
                return;
            }

            _ = Keys.Clipboard(
                e,
                new Clipboard(tree.Copy, tree.Cut, tree.Paste, tree.NewFolder),
                _host.Log
            );
        }
    }

    /// <summary>Keeps what a step says about being open, for the next run to open it again.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The step, and whether it is open.</param>
    private void Stepped(object? sender, StepEventArgs e) => _memory.Remember(e.Path, e.Open);

    /// <summary>
    /// Reads what the tree is showing again, when the files have changed.
    /// </summary>
    /// <remarks>
    /// What is read again is the tree that is on screen, not a new one: a tree is a view the reader has scrolled
    /// and walked into, and a tree built again comes back with its own scroll at the top, which is the change of
    /// the view's height a reader sees as the chosen step paging up or down under the pointer. The steps that are
    /// open and the step that is chosen are kept by the tree itself, which is what makes reading it again one
    /// drawing of the same view rather than a replacement of it.
    /// <para>
    /// A tree built again is still what a tree rooted elsewhere, or read by other rules, is: those change which
    /// rows there are under the base rather than what the rows hold, and are drawn by <see cref="Draw"/>.
    /// </para>
    /// </remarks>
    private void Again()
    {
        if (_content.Content is TreeBrowser tree)
        {
            tree.Refresh();

            return;
        }

        Draw();
    }

    /// <summary>
    /// Builds the tree over the base, keeping the steps that were open and what was chosen.
    /// </summary>
    /// <remarks>
    /// The steps and the choice are asked of the tree being replaced rather than kept here, because they are
    /// what that tree was drawn with: a tree rooted elsewhere, or one read by other rules, is a tree to build
    /// again, the rows under the base are different rows, and a user who has walked down into a step should
    /// not be put back at the base for it, nor lose the step the clipboard's keys would act on.
    /// <para>
    /// What the files having changed is not: that is the same tree over the same base, read again in place by
    /// <see cref="Again"/>, which is what keeps its scroll.
    /// </para>
    /// </remarks>
    private void Draw()
    {
        var was = _content.Content as TreeBrowser;

        // The tree being replaced keeps the reader's height when it is the same base read under another
        // answer: a rule, a hiding, a virtual entry. A tree rooted somewhere else starts at the top.
        var keepScroll =
            was is not null
            && _drawn is { } kept
            && string.Equals(kept.Base, _browser.BaseDir, StringComparison.Ordinal);
        var offset =
            keepScroll
                ? was?.GetVisualDescendants().OfType<ScrollViewer>().FirstOrDefault()?.Offset ?? default
                : default;

        // Whether the keyboard is inside the tree about to be replaced, for the reason a listing is asked about as
        // well: a view replaced from under the keyboard takes it away, and this dock answers its keys at the top of
        // itself, so a tree rebuilt while it had the keyboard would answer nothing afterwards.
        var seating = was is { } old && Keys.Holds(Keys.On(this), old);

        // A tree built for the first time is one nothing in this run says anything about, so it opens what
        // was open when the program last ran: a user is put back where they were rather than at the base.
        IReadOnlyList<string> opened = was is null ? _memory.Opened(_browser.BaseDir) : was.Opened();

        var tree = new TreeBrowser(_host, _browser, _browser.BaseDir, _actions, _clip, _listing);
        tree.StepChanged += Stepped;
        _content.Content = tree;
        tree.Reopen(opened, was?.Chosen());

        if (keepScroll)
        {
            Dispatcher.UIThread.Post(
                () =>
                {
                    if (tree.GetVisualDescendants().OfType<ScrollViewer>().FirstOrDefault() is { } scroll)
                    {
                        scroll.Offset = offset;
                    }
                },
                DispatcherPriority.Render
            );
        }

        if (seating)
        {
            Seated();
        }

        _drawn = (
            _browser.BaseDir,
            _browser.ShowHidden,
            _browser.HideRegistry.Selection(),
            _browser.HideRegistry.Gray(),
            VirtualEntries.Shape
        );
    }

    /// <summary>
    /// Takes the keyboard into the tree, which is where this dock's keys are answered from.
    /// </summary>
    /// <remarks>
    /// Deferred to the next turn of the loop, because the tree it goes into has just been made and is not laid out
    /// yet, a keyboard asked of a control that is not arranged yet is a keyboard it forgets. Not the
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

    /// <summary>The layer the coloured rows are washed in, behind the tree.</summary>
    /// <remarks>
    /// A wash is the row's ground rather than a mark over it, so it stands under the rows: over them it would tint
    /// the pictures and the names as well as the ground, and a row's hover and choice would fight it.
    /// </remarks>
    private readonly Canvas _washes = new()
    {
        Name = "TreeWashes",
        IsHitTestVisible = false,
        ClipToBounds = true,
    };

    /// <summary>What was drawn in the block layer, so that a layout that changed nothing draws nothing.</summary>
    private IReadOnlyList<TreeBlock> _drawnBlocks = [];

    /// <summary>Whether the block layer is being written, so that writing it does not write it again.</summary>
    private bool _paintingBlocks;

    /// <summary>Whether a block refresh is already posted, so a resize posts one rather than one per row.</summary>
    /// <remarks>
    /// The lines are written after the layout pass that moved the rows, not inside it. A row's position read
    /// while the tree is still arranging is a position it is about to leave, and clearing the layer for that
    /// answer is the line disappearing for a frame.
    /// </remarks>
    private bool _blocksPosted;

    /// <summary>Whether a guide repaint is already posted, so a colour change repaints the guides once.</summary>
    private bool _guidesPosted;

    /// <summary>One washed block: the directory it belongs to, its depth, its row, and its colour.</summary>
    /// <param name="Path">The directory's path, which says what is inside it.</param>
    /// <param name="Level">How deep the directory is in the tree.</param>
    /// <param name="Row">The step's own line, which is what the wash covers.</param>
    /// <param name="Wash">The wash behind the row, which is what the row is read on.</param>
    private readonly record struct TreeBlock(string Path, int Level, Rect Row, Color Wash);

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

    /// <summary>What each row's badge strip was filled with, so a repaint with the same answer changes nothing.</summary>
    private readonly Dictionary<string, IReadOnlyList<EntryBadge>> _wornBadges =
        new(StringComparer.Ordinal);

    /// <summary>What each row made is, so that a block does not have to ask the filesystem again.</summary>
    private readonly Dictionary<string, EntryKind> _kinds = new(StringComparer.Ordinal);

    /// <summary>Every step on screen, so that the one a drag is over can be lit.</summary>
    /// <remarks>
    /// A step is recorded when it is made and never taken out: a tree read a step at a time keeps the ones it
    /// has read, closed or not, so a row that was recorded is still the row the tree draws, and what is read
    /// is bounded by what a user opened.
    /// </remarks>
    private readonly List<(string Path, Border Row)> _rows = [];

    /// <summary>The row wearing the drop mark, so that it can be taken off again.</summary>
    private Border? _marked;

    /// <summary>
    /// The steps that are chosen, in the order the tree shows them.
    /// </summary>
    /// <remarks>
    /// Kept here rather than read back from the toolkit's selection, because what choosing a row does here is go
    /// there: a row is a step, and the clipboard's keys act on the whole choice. It is remembered across a rebuild
    /// along with the open steps, since being read again is what a rebuilt tree is.
    /// </remarks>
    private IReadOnlyList<string> _chosen = [];

    /// <summary>Where the last click landed, which a shift click measures its run from.</summary>
    private int _anchor = -1;

    /// <summary>Whether this view is putting a choice back, so that the toolkit's event is not a user choosing.</summary>
    private bool _selecting;

    /// <summary>
    /// Whether the press landed on a step already in a multiple choice, which a drag carries whole.
    /// </summary>
    /// <remarks>
    /// The toolkit collapses the selection to the row under the pointer as the press is made, which reads as a
    /// new choice and would make a drag of a chosen group a drag of one. The choice is read before that and put
    /// back while the press is still one that could become a drag; a release that never becomes one chooses the
    /// row alone, which is what a plain click means.
    /// </remarks>
    private bool _pressedInChoice;

    /// <summary>The choice as it stood when that press landed.</summary>
    private IReadOnlyList<string> _pressedChoice = [];

    /// <summary>The press a drag of a step would begin from, while it may still become one.</summary>
    private PointerPressedEventArgs? _pressed;

    /// <summary>
    /// The steps that are to be read again when they are next opened.
    /// </summary>
    /// <remarks>
    /// A step reads the directories under it once and holds them, which is what keeps a tree over a deep
    /// directory cheap. A directory made under a step is the one thing that makes what it holds wrong, so the
    /// step is named here and reads again the next time it is opened. Steps are named by path rather than by
    /// row because the row may not exist yet when the change is made, a step that was never opened has no row.
    /// </remarks>
    private readonly HashSet<string> _stale = new(StringComparer.Ordinal);

    /// <summary>The directories this tree has shown to the watch, so that none is shown twice or left showing.</summary>
    private readonly HashSet<string> _watched = new(StringComparer.Ordinal);

    /// <summary>
    /// The newest reading claimed for each step, so that a reading another has superseded does not land.
    /// </summary>
    /// <remarks>
    /// A step read again, one a directory was made in is closed and opened again, may have a reading on its
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
    private IReadOnlyList<string>? _choosing;

    /// <summary>
    /// Whether the rows are being read again in place, so that what the toolkit calls selected is no choice.
    /// </summary>
    /// <remarks>
    /// Reading the rows again takes them out from under the tree's own selection, which is published as a choice
    /// the reader made. A choice that went out that way would have the listing and the version graph let go of
    /// what they were reading for a step nobody unchose.
    /// </remarks>
    private bool _refreshing;

    /// <summary>The height to put back after a reread, while a restore is waiting.</summary>
    private Vector? _scrollToKeep;

    /// <summary>The bubble a step's name is being typed in, while one is, and nothing otherwise.</summary>
    /// <remarks>
    /// The bubble stands under the name rather than in its place: the row is not rebuilt for an edit, and the
    /// field is where the keyboard is, so the tree's keys wait while a name is being typed.
    /// </remarks>
    private RenameBubble? _bubble;

    /// <summary>Whether a step's name is being typed, which is when the tree's keys are the box's.</summary>
    public bool Editing => _bubble is not null;

    /// <summary>
    /// How many turns a box is asked for before it is given up on.
    /// </summary>
    /// <remarks>
    /// A handful, because the turns it waits for are the reading of a directory and the lay-out of the row it
    /// makes: a few more than those are needed costs nothing, and asking forever would be a loop that never
    /// ends for a directory this view is not drawing at all, one made in another dock's location, say.
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

    /// <summary>
    /// The word said where the base holds nothing under it, drawn over the tree's own space.
    /// </summary>
    /// <remarks>
    /// One control rather than one made where the base is read, because what it says is read again whenever the
    /// base's own level is: a word added once and never taken away would outlive the empty base it was added for,
    /// and sit over a base that has since been given something.
    /// </remarks>

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

        var tree = new TreeView
        {
            SelectionMode = SelectionMode.Multiple,
            Focusable = true,
            ContextMenu = actions.Empty(this),
        };
        tree.Items.Add(Node(root));
        ScrollViewer.SetHorizontalScrollBarVisibility(tree, ScrollBarVisibility.Disabled);
        _tree = tree;

        // A mark covers the step it is on and every step under it, and both a layout and a scroll move where
        // that block stands: the layer is written again whenever either has moved, and only when the blocks
        // themselves have changed.
        _tree.LayoutUpdated += (_, _) => PostPaintBlocks();
        _tree.AddHandler(
            ScrollViewer.ScrollChangedEvent,
            (_, _) => PostPaintBlocks(),
            RoutingStrategies.Bubble
        );

        // What the tree calls selected is what the keys act on, and it is not always a row's own press: the
        // arrows move the toolkit's selection without touching a row, and a reading that lands puts the choice
        // back by selecting the row itself. Both are a step drawn as chosen, so both have to be the one the
        // keys are about rather than a step that was chosen earlier (see `Selected`).
        _tree.SelectionChanged += (_, _) => Selected();

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

        // Ctrl+A is the tree's own selection, which the toolkit does not provide on its own: the whole of what is
        // on screen is chosen, not the step the keyboard happens to be on. Taken here as well as at the dock, since
        // the keyboard may be on the tree or on a row within it.
        AddHandler(KeyDownEvent, ChosenAll, RoutingStrategies.Tunnel);

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

            // A tree going away is every step it drew no longer on screen, and the watch is what says so.
            UnwatchingAll();
        };

        var over = new Canvas { IsHitTestVisible = false };
        _ghost = new Ghost(over);

        var grid = new Grid();
        grid.Children.Add(_washes);
        grid.Children.Add(tree);
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
        e.DragEffects = land is null ? DragDropEffects.None : Over(e);
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
    /// The step itself and not the space around it, a tree holds the steps a user opened, and the space beside
    /// and below them names nothing to go into, so a drag let go there is let go nowhere rather than being read
    /// as the base. The computer is a place to choose a drive from rather than a directory, so a step standing
    /// for it is not somewhere a drag can go either.
    /// <para>
    /// A drag that is not files is a place's own object, and what a directory does with one is not this view's to
    /// decide: it is offered to what was registered, and a step nothing takes is a step the drop is not offered on.
    /// The files are read first, because a drag of this program's own files carries the payload as well, and it is a
    /// file operation whatever a place made of it.
    /// </para>
    /// </remarks>
    /// <param name="e">The drag.</param>
    private Land? Landing(DragEventArgs e)
    {
        if (Step(e.GetPosition(this)) is not { } path || Browser.IsComputer(path))
        {
            return null;
        }

        if (e.DataTransfer.Contains(DataFormat.File))
        {
            return new Land(path, path);
        }

        // A place's drag is a thing being put *in* a folder, so a step naming a file is not a place to let one
        // go: a listing refuses a drop on a file row the same way (see `Files.Landing`).
        return e.DataTransfer.TryGetValue(Drag.Payload) is { } logical
            && Directory.Exists(path)
            && _drops.Takes(path, logical)
                ? new Land(path, path)
                : null;
    }

    /// <summary>What letting go on a step would do.</summary>
    /// <remarks>
    /// A place's drag is a copy whichever key is held: what it brings is not the files the source holds, and nothing
    /// is moved out of anywhere by taking it, so the answer is the one that leaves the source where it is. A drag of
    /// files is the platform's own, and the key held says which of the two it is.
    /// </remarks>
    /// <param name="e">The drag.</param>
    /// <returns>What the platform is told.</returns>
    private static DragDropEffects Over(DragEventArgs e) =>
        e.DataTransfer.Contains(DataFormat.File) ? Drops.Over(e) : DragDropEffects.Copy;

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
        _pressedInChoice = false;
        _pressedChoice = [];

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
        _pressedInChoice = KeepsChoice(_chosen, path, e.KeyModifiers);
        _pressedChoice = _pressedInChoice ? _chosen : [];
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
        _pressedInChoice = false;
        _pressedChoice = [];

        var lead = Being(path);
        var carrying = Carrying([.. _chosen.Select(Being)], lead);

        Drag.Away(_browser, from, carrying, lead, _host.Log.Error);
    }

    /// <summary>Lets a press that never became a drag go, so that it is read as the step it was.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The release.</param>
    private void Released(object? sender, PointerReleasedEventArgs e)
    {
        _pressed = null;
        _slipped = null;
        _pressedInChoice = false;
        _pressedChoice = [];
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
    /// that is chosen under it, and being built again is exactly what a tree that lost the keyboard looks like
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

    /// <summary>The steps that are chosen, in the order the tree shows them.</summary>
    public IReadOnlyList<string> Chosen() => _chosen;

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
        // step that was never opened is opened the once, and one that is empty, which a step with only a
        // placeholder in it is, is not opened onto nothing.
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
    /// opened. So the box is asked for again on each of a few turns until the row is there, a box asked for
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
                if (_bubble is not null)
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
    /// <param name="chosen">The steps to choose, or nothing where none was.</param>
    public void Reopen(IReadOnlyList<string> opened, IReadOnlyList<string>? chosen)
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
    /// A step whose row is not drawn is either one the reading holding it has not reached, which is what this
    /// waits for, and why the reading calls this again when it lands, or one that is not there at all, which
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

        // The choice is put back once its rows are drawn, which is after the reading holding them has landed. A
        // path whose row is not drawn once a reading has landed is one that is gone, and the reading that says a
        // step is missing says it of this one too: it is let go of rather than kept for a row that will never be
        // drawn.
        if (_choosing is { } choosing)
        {
            var found = choosing
                .Select(path => (Path: path, Item: Find(_tree.Items, path)))
                .Where(one => one.Item is not null)
                .ToArray();

            if (found.Length == choosing.Count || landed)
            {
                _selecting = true;

                try
                {
                    foreach (var (_, item) in found)
                    {
                        item!.IsSelected = true;
                    }
                }
                finally
                {
                    _selecting = false;
                }

                _chosen = [.. found.Select(one => one.Path)];
                _anchor = _chosen.Count > 0 ? IndexOf(_chosen[0]) : -1;
                _choosing = null;

                _listing.Chosen([.. _chosen.Select(Being)]);
            }
        }
    }

    /// <summary>
    /// Reads what every step the tree is showing holds again, keeping the tree, its steps and its choice.
    /// </summary>
    /// <remarks>
    /// The tree is the same control rather than a new one, which is what keeps what a reader has scrolled to: a
    /// tree built again comes back with its scroll at the top, and a view whose scroll moved under the reader is
    /// one where the chosen step is no longer where it was seen. What is read again is the rows, a directory
    /// whose contents have moved is read from disk, and what is put back is what was open and what was chosen,
    /// which is what those rows are read against.
    /// <para>
    /// The root is read whether or not it is open, since it is the one step every other step hangs from; a step
    /// under it is read by being opened again, exactly as a tree built from nothing is (<see cref="Reopen"/>).
    /// What a step held before is dropped with the row that held it: the rows are about to be made again, and one
    /// kept beside the row it was drawn as would answer about a tree nothing is drawn from.
    /// </para>
    /// <para>
    /// The choice is put back once the rows are laid out, and not while they are being made: a row that is chosen
    /// before it is arranged is a row the toolkit brings into view from a position it does not have yet, which
    /// moves the scroll the reader is reading at. A step the reader has chosen is one they are looking at, so
    /// putting the choice back where it is already is a scroll asked for and not made.
    /// </para>
    /// </remarks>
    public void Refresh()
    {
        var opened = Opened();
        var chosen = Chosen();
        var scroll = _tree.GetVisualDescendants().OfType<ScrollViewer>().FirstOrDefault();

        _scrollToKeep ??= scroll?.Offset;

        // A box a name is being typed in does not outlive the row it was put in, and the row is about to be read
        // again: the name is given up rather than left half-typed in a row nothing holds.
        if (_bubble is not null)
        {
            Stop();
        }

        // A reading on its way was asked for by a tree that is no longer drawn: its claim is moved past the
        // newest one made, so that it lands as nothing rather than reopening a row nothing holds.
        foreach (var path in _readings.Keys.ToArray())
        {
            _readings[path]++;
        }

        _stale.Clear();
        UnwatchingAll();
        _refreshing = true;

        try
        {
            if (_tree.Items.OfType<TreeViewItem>().FirstOrDefault() is not { } root)
            {
                return;
            }

            Read(root, _root);

            // What the base holds is settled again with the rows: a base that was empty when it was first read is
            // a base that may have been given something since, and the word over the tree is about what it holds
            // now rather than about what it held then. It is also the row that says whether the base can be opened
            // at all, which a base read as empty is drawn without.
            Settle(root, _root, root.Items.Count > 0);
            Reopen(opened, chosen: null);
        }
        finally
        {
            _refreshing = false;
        }

        if (chosen.Count > 0)
        {
            ChooseOnceDrawn(chosen);
        }

        // The same tree read again is the same rows in the same places: the reader stays at the height
        // they had scrolled to, after the rows and the choice have been put back.
        if (_scrollToKeep is { } offset)
        {
            Dispatcher.UIThread.Post(
                () =>
                {
                    if (_tree.GetVisualDescendants().OfType<ScrollViewer>().FirstOrDefault() is { } now)
                    {
                        now.Offset = offset;
                    }

                    _scrollToKeep = null;
                },
                DispatcherPriority.Render
            );
        }
    }

    /// <summary>
    /// Reads one step again, when the files in it have changed on disk.
    /// </summary>
    /// <remarks>
    /// What is read is the directory the step stands for, and the steps that were open under it: a tree read
    /// whole for a change in one directory is every step on screen made again, and the reader's place in it
    /// made again with the rows.
    /// <para>
    /// A directory that is not on screen is not read at all. A step that is not drawn reads its directory when it
    /// is opened, which is after the change, so nothing is owed to it.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory that changed.</param>
    public void Again(string directory)
    {
        if (Find(_tree.Items, directory) is not { } step)
        {
            return;
        }

        var opened = Opened()
            .Where(one => Pathing.Same(one, directory) || Pathing.Inside(one, directory))
            .ToArray();
        var chosen = Chosen();

        foreach (var one in opened)
        {
            Unwatching(one);
        }

        Unwatching(directory);

        // Named as stale as well as read now, so a step that is closed and opened again before the next change
        // reads the directory rather than what it held when this ran.
        _stale.Add(directory);

        Read(step, directory);
        Reopen(opened, chosen);
    }

    /// <summary>Shows a step's directory to the watch, from the moment it is open.</summary>
    /// <param name="path">The directory the step stands for.</param>
    private void Watching(string path)
    {
        if (_watched.Add(path))
        {
            _browser.Shared.Showing(path);
        }
    }

    /// <summary>Stops showing a step's directory, which is no longer drawn.</summary>
    /// <param name="path">The directory the step stood for.</param>
    private void Unwatching(string path)
    {
        if (_watched.Remove(path))
        {
            _browser.Shared.Hiding(path);
        }
    }

    /// <summary>Stops showing every directory this tree showed, which is a tree going away or read whole.</summary>
    private void UnwatchingAll()
    {
        foreach (var path in _watched.ToArray())
        {
            Unwatching(path);
        }
    }

    /// <summary>
    /// Chooses the steps a reader had chosen, once the rows read again have been laid out.
    /// </summary>
    /// <remarks>
    /// What is chosen is put back only if it is still what is chosen: a reader who has chosen another step while
    /// the rows were being made has chosen that one, and a reading that put the old choice back would be the view
    /// undoing a choice rather than restoring one. A step whose row is not there once the rows are laid out is one
    /// the tree no longer holds (a step that was open is opened again by the reading, so a gone row is gone) and
    /// it is let go of rather than kept for a row that will never be drawn.
    /// </remarks>
    /// <param name="chosen">The steps that were chosen when the rows were read again.</param>
    private void ChooseOnceDrawn(IReadOnlyList<string> chosen) =>
        Dispatcher.UIThread.Post(
            () =>
            {
                if (!Same(_chosen, chosen))
                {
                    return;
                }

                var found = chosen
                    .Select(path => (Path: path, Item: Find(_tree.Items, path)))
                    .Where(one => one.Item is not null)
                    .ToArray();

                if (found.Length != chosen.Count)
                {
                    _chosen = [.. found.Select(one => one.Path)];
                    _listing.Chosen([.. _chosen.Select(Being)]);

                    return;
                }

                _selecting = true;

                try
                {
                    foreach (var (_, item) in found)
                    {
                        item!.IsSelected = true;
                    }
                }
                finally
                {
                    _selecting = false;
                }
            },
            DispatcherPriority.Loaded
        );

    /// <summary>The rows on screen, in the order the tree draws them.</summary>
    /// <remarks>
    /// A closed step contributes itself and nothing under it: a range is what the user can see, and a range that
    /// reached into a closed step would choose rows no press could have shown.
    /// </remarks>
    /// <returns>The drawn rows, outermost first.</returns>
    private IReadOnlyList<TreeViewItem> VisibleRows()
    {
        var rows = new List<TreeViewItem>();

        if (_tree.Items.OfType<TreeViewItem>().FirstOrDefault() is { } root)
        {
            Flatten(root, rows);
        }

        return rows;
    }

    /// <summary>Gathers a row and, where it is open, the rows under it.</summary>
    /// <param name="item">The row to gather.</param>
    /// <param name="rows">Where the rows are gathered.</param>
    private static void Flatten(TreeViewItem item, List<TreeViewItem> rows)
    {
        rows.Add(item);

        if (!item.IsExpanded)
        {
            return;
        }

        foreach (var under in item.Items.OfType<TreeViewItem>())
        {
            Flatten(under, rows);
        }
    }

    /// <summary>Schedules the coloured lines for after the layout that moved the rows.</summary>
    /// <remarks>
    /// A resize raises a layout update for every row it moves, and the rows are still being arranged while it
    /// does. Reading their places then and clearing the layer for an answer that is already stale is what makes
    /// a line vanish for a frame; one posted pass reads the places the rows settled at.
    /// </remarks>
    private void PostPaintBlocks()
    {
        if (_blocksPosted)
        {
            return;
        }

        _blocksPosted = true;

        Frame.Post(() =>
        {
            _blocksPosted = false;
            PaintBlocks();
        });
    }

    /// <summary>Draws a wash behind every row a colour reaches.</summary>
    /// <remarks>
    /// A step the colour is its own is washed more strongly than one that only inherits it, so that the folder
    /// reads as the place the colour belongs to and the rows under it as being inside that place. The wash is the
    /// ground rather than a mark over the row, which is why it is drawn under the tree.
    /// </remarks>
    private void PaintBlocks()
    {
        if (_paintingBlocks)
        {
            return;
        }

        _paintingBlocks = true;

        try
        {
            var rows = VisibleRows().ToArray();
            var tops = new double[rows.Length];
            var high = new double[rows.Length];
            var incomplete = false;

            for (var at = 0; at < rows.Length; at++)
            {
                high[at] = rows[at].Bounds.Height;

                if (rows[at].TranslatePoint(new Point(0, 0), _washes) is { } origin)
                {
                    tops[at] = origin.Y;
                }
                else
                {
                    // A row with a size but no place on the layer is still being laid out: keeping what is
                    // there is better than clearing it for a place that is about to be answered.
                    incomplete = true;
                }
            }

            if (incomplete)
            {
                return;
            }

            var blocks = new List<TreeBlock>();

            for (var at = 0; at < rows.Length; at++)
            {
                var item = rows[at];

                if (
                    item.Tag is not string path
                    || item.Bounds.Width <= 0
                    || item.Bounds.Height <= 0
                )
                {
                    continue;
                }

                var kind = _kinds.GetValueOrDefault(path, EntryKind.Directory);
                var entry = new Entry(path, kind);

                if (FolderColours.Band(entry) is not { } band)
                {
                    continue;
                }

                // The step's own colour is the one it was given; a row that only inherits one is washed more
                // lightly, which is what makes the marked folder read as where the colour belongs.
                var own = FolderColours.Mark(entry) is not null;

                // The wash covers the row from its own top to the top of the row below it, rather than the
                // header alone: the room under a step belongs to the step, and a wash stopping at the header
                // leaves one pixel of the ground between two coloured rows.
                var header = item
                    .GetVisualDescendants()
                    .OfType<Border>()
                    .FirstOrDefault(one => one.Name == "SelectionBorder");

                if (
                    header is null
                    || header.TranslatePoint(new Point(0, 0), _washes) is not { } top
                )
                {
                    continue;
                }

                var bottom = at + 1 < rows.Length ? tops[at + 1] : tops[at] + high[at];

                blocks.Add(
                    new TreeBlock(
                        path,
                        item.Level,
                        new Rect(top.X, tops[at], header.Bounds.Width, Math.Max(0, bottom - tops[at])),
                        WithAlpha(band, own ? OwnWash : InheritedWash)
                    )
                );
            }

            if (SameBlocks(blocks, _drawnBlocks))
            {
                return;
            }

            _drawnBlocks = blocks;
            _washes.Children.Clear();

            foreach (var block in blocks)
            {
                var ground = new Border
                {
                    IsHitTestVisible = false,
                    Width = block.Row.Width,
                    Height = block.Row.Height,
                    Background = new SolidColorBrush(block.Wash),
                };

                Canvas.SetLeft(ground, block.Row.X);
                Canvas.SetTop(ground, block.Row.Y);
                _washes.Children.Add(ground);
            }
        }
        finally
        {
            _paintingBlocks = false;
        }
    }

    /// <summary>A colour at a share of its strength, which is how a wash is made from it.</summary>
    /// <param name="colour">The colour.</param>
    /// <param name="alpha">How much of it shows, from nothing to all of it.</param>
    /// <returns>The colour with that alpha.</returns>
    private static Color WithAlpha(Color colour, double alpha) =>
        Color.FromArgb(
            (byte)Math.Round(Math.Clamp(alpha, 0, 1) * 255),
            colour.R,
            colour.G,
            colour.B
        );

    /// <summary>One colour taken a share of the way towards another.</summary>
    /// <param name="from">The colour that stays.</param>
    /// <param name="towards">The colour it is pulled towards.</param>
    /// <param name="share">How far, from none of it to all of it.</param>
    /// <returns>The mixed colour, fully opaque.</returns>
    private static Color Mix(Color from, Color towards, double share)
    {
        var at = Math.Clamp(share, 0, 1);

        return Color.FromRgb(
            (byte)Math.Round((from.R * (1 - at)) + (towards.R * at)),
            (byte)Math.Round((from.G * (1 - at)) + (towards.G * at)),
            (byte)Math.Round((from.B * (1 - at)) + (towards.B * at))
        );
    }

    /// <summary>Whether two sets of blocks are the same answer.</summary>
    /// <param name="left">One set.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether every block is where the other set has it.</returns>
    private static bool SameBlocks(IReadOnlyList<TreeBlock> left, IReadOnlyList<TreeBlock> right)
    {
        if (left.Count != right.Count)
        {
            return false;
        }

        for (var at = 0; at < left.Count; at++)
        {
            if (
                !string.Equals(left[at].Path, right[at].Path, StringComparison.Ordinal)
                || left[at].Level != right[at].Level
                || left[at].Wash != right[at].Wash
                || Math.Abs(left[at].Row.X - right[at].Row.X) > 0.5
                || Math.Abs(left[at].Row.Y - right[at].Row.Y) > 0.5
                || Math.Abs(left[at].Row.Width - right[at].Row.Width) > 0.5
                || Math.Abs(left[at].Row.Height - right[at].Row.Height) > 0.5
            )
            {
                return false;
            }
        }

        return true;
    }

    /// <summary>The place a path has among the rows on screen, or nothing where it is not drawn.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The place, or <c>-1</c>.</returns>
    private int IndexOf(string path)
    {
        var rows = VisibleRows();

        for (var at = 0; at < rows.Count; at++)
        {
            if (string.Equals(rows[at].Tag as string, path, StringComparison.Ordinal))
            {
                return at;
            }
        }

        return -1;
    }

    /// <summary>Whether two choices name the same steps in the same order.</summary>
    /// <param name="left">One choice.</param>
    /// <param name="right">The other choice.</param>
    /// <returns>Whether they are the same answer.</returns>
    private static bool Same(IReadOnlyList<string> left, IReadOnlyList<string> right)
    {
        if (left.Count != right.Count)
        {
            return false;
        }

        for (var at = 0; at < left.Count; at++)
        {
            if (!string.Equals(left[at], right[at], StringComparison.Ordinal))
            {
                return false;
            }
        }

        return true;
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
    /// remembering it, and having the rows drawn as chosen so that what a copy or a cut is about is what the eye
    /// says it is.
    /// <para>
    /// What is chosen is also what the rest of the program is told is chosen: a directory listing publishes its own
    /// choice and the entry that was clicked to a catalogue the version graph reads, and a tree is the same view of the
    /// same files, so a file chosen here is a file the graph comes to as well.
    /// </para>
    /// </remarks>
    /// <param name="path">The step to choose.</param>
    private void Choose(string path)
    {
        if (_chosen.Count != 1 || !string.Equals(_chosen[0], path, StringComparison.Ordinal))
        {
            SelectOnly(path);
        }

        _listing.Clicked(Being(path));
    }

    /// <summary>Makes one step the whole choice, which is what a plain press on a row does.</summary>
    /// <param name="path">The step to choose.</param>
    private void SelectOnly(string path)
    {
        _selecting = true;

        try
        {
            _tree.SelectedItems?.Clear();

            if (Find(_tree.Items, path) is { } item)
            {
                _tree.SelectedItems?.Add(item);
            }
        }
        finally
        {
            _selecting = false;
        }

        _chosen = [path];
        _anchor = IndexOf(path);
        _listing.Chosen([Being(path)]);
    }

    /// <summary>Keeps what the tree calls selected and what the keys act on the same steps.</summary>
    /// <remarks>
    /// A press on a row is <see cref="Choose"/>, which is the whole of an unmodified click. A modified press and the
    /// arrows are the toolkit's selection, and a step the eye reads as chosen has to be the step a key acts on, or
    /// Delete removes what was chosen one gesture ago. What is published is the choice alone and not a click, since a
    /// listing's arrows publish only that much.
    /// </remarks>
    private void Selected()
    {
        // Nothing while a reading is putting the choice back: `Resume` is the one that says what is chosen then, and
        // a selection it is making is not a user choosing anything. Nor while the rows are being read again in
        // place, where the selection goes only because the rows it was on are being made again.
        if (_refreshing || _selecting || _choosing is not null)
        {
            return;
        }

        var picked = _tree.SelectedItems?
            .OfType<TreeViewItem>()
            .Select(item => item.Tag as string)
            .OfType<string>()
            .ToArray() ?? [];

        // A row the tree only shows is not part of a choice: the toolkit can still put one under the keyboard
        // or in a range, so what it chose is put back without the rows that cannot be acted on.
        if (picked.Any(one => VirtualEntries.Find(one) is not null))
        {
            Restore([.. picked.Where(one => VirtualEntries.Find(one) is null)]);

            return;
        }

        if (Same(_chosen, picked))
        {
            return;
        }

        // The toolkit collapses a multiple choice to the row a press landed on. When that row was already in the
        // choice, the press may be the beginning of a drag of the whole of it, so the choice is put back here and
        // stays until the release either begins the drag or chooses the row alone.
        if (_pressedInChoice && picked.Length == 1 && _pressedChoice.Contains(picked[0], StringComparer.Ordinal))
        {
            Restore(_pressedChoice);

            return;
        }

        _chosen = picked;
        _anchor = picked.Length > 0 ? IndexOf(picked[^1]) : -1;

        _listing.Chosen([.. picked.Select(Being)]);
    }

    /// <summary>Puts back a choice the toolkit collapsed, while a press on it may become a drag of the whole.</summary>
    /// <param name="choice">The choice to put back.</param>
    private void Restore(IReadOnlyList<string> choice)
    {
        _selecting = true;

        try
        {
            _tree.SelectedItems?.Clear();

            foreach (var path in choice)
            {
                if (Find(_tree.Items, path) is { } item)
                {
                    _tree.SelectedItems?.Add(item);
                }
            }
        }
        finally
        {
            _selecting = false;
        }

        _anchor = choice.Count > 0 ? IndexOf(choice[0]) : -1;
        _listing.Chosen([.. choice.Select(Being)]);
    }

    /// <summary>Chooses every row the tree is showing.</summary>
    public void SelectAll()
    {
        // A row the tree only shows is not chosen by choosing everything, which is the rule a listing's
        // control-all follows.
        var rows = VisibleRows()
            .Where(row => row.Tag is not string path || VirtualEntries.Find(path) is null)
            .ToArray();

        _selecting = true;

        try
        {
            _tree.SelectedItems?.Clear();

            foreach (var row in rows)
            {
                _tree.SelectedItems?.Add(row);
            }
        }
        finally
        {
            _selecting = false;
        }

        _chosen = [.. rows.Select(row => row.Tag as string).OfType<string>()];
        _anchor = 0;

        _listing.Chosen([.. _chosen.Select(Being)]);
    }

    /// <summary>Takes Ctrl+A as choosing every row on screen.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The key.</param>
    private void ChosenAll(object? sender, KeyEventArgs e)
    {
        if (_bubble is not null)
        {
            return;
        }

        if (
            e.Key == Key.A
            && e.KeyModifiers.HasFlag(KeyModifiers.Control)
            && !e.KeyModifiers.HasFlag(KeyModifiers.Alt)
        )
        {
            SelectAll();
            e.Handled = true;
        }
    }

    /// <summary>The chosen steps as entries, in the order the tree shows them.</summary>
    /// <param name="withoutRoot">Whether to leave out the root and anything holding it.</param>
    /// <returns>The entries.</returns>
    private Entry[] ChosenEntries(bool withoutRoot = false) =>
        [
            .. _chosen
                .Where(path => !withoutRoot || !HoldsRoot(path))
                .Select(Being),
        ];

    /// <summary>Copies the chosen steps to the clipboard.</summary>
    public void Copy()
    {
        if (ChosenEntries() is { Length: > 0 } entries)
        {
            _actions.Copy(this, entries);
        }
    }

    /// <summary>Cuts the chosen steps to the clipboard, which a paste then moves.</summary>
    public void Cut()
    {
        if (ChosenEntries() is { Length: > 0 } entries)
        {
            _actions.Cut(this, entries);
        }
    }

    /// <summary>
    /// Pastes what is on the clipboard into the chosen directory.
    /// </summary>
    /// <remarks>
    /// Into the first chosen directory, because a directory is what a paste is into; a choice of files only is pasted
    /// beside them, into the directory holding the first. With nothing chosen the base is the place, which is the
    /// directory the tree is rooted at and the only one a tree always names.
    /// </remarks>
    public void Paste()
    {
        var chosen = ChosenEntries();

        var into = chosen.FirstOrDefault(entry => entry.Kind == EntryKind.Directory) is { } directory
            ? directory.Path
            : chosen
                .Select(entry => Path.GetDirectoryName(entry.Path))
                .FirstOrDefault(path => path is { Length: > 0 }) ?? _root;

        _actions.Paste(this, into);
    }

    /// <summary>Removes the chosen steps, asking first unless told not to.</summary>
    /// <remarks>
    /// The root is left out: it is the row the tree stands on rather than a step of it, and removing it would
    /// leave the tree rooted at a directory that is no longer there. A step holding the root is left out for the
    /// same reason, since removing it would take the root with it, and a tree that was rooted somewhere else
    /// earlier in the run can still have chosen one.
    /// <para>
    /// Both refusals are said in the log rather than passed over in silence: the key is taken and answered above
    /// this (see `Keys.Forget`), so a press that removed nothing is otherwise indistinguishable from a press that never
    /// arrived at all.
    /// </para>
    /// </remarks>
    /// <param name="ask">Whether to put the question to the user first.</param>
    public void Delete(bool ask)
    {
        var chosen = ChosenEntries();

        if (chosen.Length == 0)
        {
            _host.Log.Info("delete: nothing is chosen in the tree");

            return;
        }

        var removable = chosen.Where(entry => !HoldsRoot(entry.Path)).ToArray();

        if (removable.Length != chosen.Length)
        {
            _host.Log.Info("delete: the tree's own root, or a step holding it, is not removed");
        }

        if (removable.Length > 0)
        {
            _actions.Remove(removable, ask);
        }
    }

    /// <summary>Whether removing a path would take the directory the tree stands on with it.</summary>
    /// <remarks>
    /// The tree's own root and everything over it: a chosen step is read off a tree that was drawn, and a tree that
    /// has been rooted elsewhere since still holds the paths it was drawn with.
    /// </remarks>
    /// <param name="path">The path that would be removed.</param>
    /// <returns>Whether the tree is rooted at it or under it.</returns>
    private bool HoldsRoot(string path) =>
        _root.Length >= path.Length
        && _root.StartsWith(path, StringComparison.Ordinal)
        && (_root.Length == path.Length || Divider(_root[path.Length]));

    /// <summary>Whether a character is one a path is joined with.</summary>
    /// <param name="character">The character.</param>
    /// <returns>Whether it divides one part of a path from the next.</returns>
    private static bool Divider(char character) =>
        character == Path.DirectorySeparatorChar || character == Path.AltDirectorySeparatorChar;

    /// <summary>
    /// Makes a directory inside a step of the tree, and opens the step so that it can be seen.
    /// </summary>
    /// <remarks>
    /// Into the step the menu was opened on, which is what a step is: a directory. A menu opened on the space
    /// beside the steps has no such directory, which is why this is offered on a row and not on the tree's own
    /// space, the tree draws the steps under a base rather than one directory, and a folder is made in a
    /// directory.
    /// </remarks>
    /// <param name="into">The directory to make it in.</param>
    /// <param name="variant">What the folder is to become, or nothing for a plain folder.</param>
    public void NewFolder(string into, FolderVariant? variant = null) =>
        _actions.NewFolder(into, variant);

    /// <summary>Makes a directory in the chosen directory, or in the base when none is chosen.</summary>
    /// <remarks>
    /// The tree's own answer to <c>Ctrl+Shift+N</c>, which a listing answers in the directory it is showing. The
    /// tree shows directories under a base, so the one a new folder goes in is the chosen one, and the base is
    /// what a tree with nothing chosen is standing on.
    /// </remarks>
    public void NewFolder()
    {
        var into =
            ChosenEntries().FirstOrDefault(entry => entry.Kind == EntryKind.Directory)?.Path
            ?? _root;

        NewFolder(into);
    }

    /// <summary>
    /// Puts the name of a step into a box, where the tree draws it.
    /// </summary>
    /// <remarks>
    /// The row is found by the path in its own tag rather than by what is chosen, because a rename is asked
    /// for about one entry and the choice is not always that entry, a folder just made is renamed without
    /// ever being clicked.
    /// </remarks>
    /// <param name="entry">The directory whose name is being edited.</param>
    public void Rename(Entry entry) => Named(entry);

    /// <summary>Renames the one chosen step, which is the tree's answer to <c>F2</c>.</summary>
    /// <remarks>
    /// One step, because a box replaces one row's name: a choice of several has no one name to put in it, and
    /// the root is left out for the reason it is left out of a delete, the tree stands on it rather than holding
    /// it as a step.
    /// </remarks>
    public void Rename()
    {
        var chosen = ChosenEntries().Where(entry => !HoldsRoot(entry.Path)).ToArray();

        if (chosen is [var entry])
        {
            Rename(entry);

            return;
        }

        _host.Log.Info(
            chosen.Length == 0
                ? "rename: nothing is chosen in the tree"
                : "rename: choose one entry to rename"
        );
    }

    /// <summary>Puts a bubble under the name of a step, where its row and its label are already drawn.</summary>
    /// <param name="entry">The directory whose name is being edited.</param>
    /// <returns>Whether the bubble was made.</returns>
    private bool Beginning(Entry entry)
    {
        if (_bubble is not null)
        {
            return true;
        }

        var label = _rows
            .Where(row => string.Equals(row.Path, entry.Path, StringComparison.Ordinal))
            .Select(row => row.Row)
            .SelectMany(row => row.GetVisualDescendants())
            .OfType<TextBlock>()
            .FirstOrDefault(text => text.Classes.Contains(EntryName));

        if (label is null)
        {
            return false;
        }

        var name = Path.GetFileName(Path.TrimEndingDirectorySeparator(entry.Path)) ?? string.Empty;

        _actions.Renaming = entry;
        _bubble = new RenameBubble(
            label,
            name,
            Names.Chosen(entry.Kind, name),
            Committed,
            Cancelled
        );

        return true;
    }

    /// <summary>Takes the name that was typed as the step's new name.</summary>
    /// <remarks>
    /// The keyboard goes back to the tree with it, as it does in a listing: the field was where it was, and
    /// the keys a dock answers are answered from the rows, so a keyboard left in a field that is gone is a
    /// dock whose keys do nothing.
    /// </remarks>
    /// <param name="name">The name that was typed.</param>
    private void Committed(string name)
    {
        if (_actions.Renaming is not { } entry)
        {
            Stop();

            return;
        }

        Stop();
        _actions.Rename(entry, name);
        Listen();
    }

    /// <summary>Leaves the name as it was and closes the bubble.</summary>
    private void Cancelled()
    {
        Stop();
        Listen();
    }

    /// <summary>Takes the bubble down and forgets what was being renamed.</summary>
    private void Stop()
    {
        _bubble?.Dismiss();
        _bubble = null;
        _actions.Renaming = null;
    }

    /// <summary>
    /// Fades a row whose step is drawn in gray or cut, so that it reads as a listing would draw it.
    /// </summary>
    /// <remarks>
    /// Cut wins, as it does in a listing: being on its way out is what the user just did. A step a rule hides
    /// is only ever drawn while the switch that shows what is hidden is on, and it is drawn fainter so that
    /// the eye passes over it rather than being left out of the tree.
    /// </remarks>
    /// <param name="row">The row.</param>
    /// <param name="path">The step it stands for.</param>
    private void Fade(Border row, string path) =>
        row.Opacity = _clip.IsCut(path)
            ? _clip.Faded
            : Dims(path)
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

    /// <summary>Whether a directory is one the rules draw in gray, read against the base.</summary>
    /// <param name="directory">The directory to consider.</param>
    /// <returns>Whether it is drawn faded.</returns>
    private bool Dims(string directory) =>
        _browser.Dims(new Entry(directory, EntryKind.Directory), _root);

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

    /// <summary>One directory as a step of the tree, reading its children when first opened.</summary>
    /// <remarks>
    /// Whether a step may be opened is a question for the filesystem, and the rules that answer it are a
    /// repository's to read, which is a program run. It is asked where it can be answered without one: a
    /// step is drawn with the mark that says it may be opened and corrected a moment later, rather than a
    /// window that stops drawing until a program has answered (see <see cref="Settle"/>).
    /// </remarks>
    /// <param name="path">The directory the step stands for.</param>
    private TreeViewItem Node(string path)
    {
        var item = new TreeViewItem
        {
            Tag = path,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };

        _kinds[path] = EntryKind.Directory;

        Guides(item);

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
            // The directory a step holds is drawn from the moment it is open, so it is watched from then: what
            // another program puts in it is a change the rows under it have to be read again for.
            Watching(path);

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

        item.Collapsed += (_, _) =>
        {
            Unwatching(path);
            StepChanged?.Invoke(this, new StepEventArgs(path, open: false));
        };

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
            // A directory is a step, which opens onto its own entries; a file is where the walking stops. A
            // directory the tree only shows is where the walking stops too: there is nothing under it to read.
            item.Items.Add(
                child.Kind is EntryKind.Directory && VirtualEntries.Find(child.Path) is null
                    ? Node(child.Path)
                    : Leaf(child)
            );
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
    /// the walking is one press per step. What closes a step is its chevron, which the theme draws beside the
    /// header as a control of its own, so a press on the chevron is not a press on the row and never reaches this
    /// handler, and the menu's close-everything. A file is chosen, and opened by a second press, because going to
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
    /// rather than from the tree: the base theme's handler is on the row's own presenter, above it and
    /// below the tree, and a gesture taken by a handler on the row is a gesture that has already been taken
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
        var virtualEntry = VirtualEntries.Find(path);

        // Filled when it opens rather than when the row is made, which is how a listing's menu is made as
        // well (see `EntryView.Prepared`). What a menu holds is asked of the plugins, and what they answer
        //, the ownership commands among them, is read from the filesystem: a tree builds a row for every
        // step under an opened one, so a menu made here is a menu paid for on every row nobody opened. A row
        // the tree only shows has no menu, for the reason a listing's inert row has none.
        ContextMenu? menu = null;

        if (virtualEntry is null)
        {
            menu = new ContextMenu();
            menu.Opening += (_, _) =>
            {
                // A right click inside the choice acts on the whole of it, with the row it landed on first; one outside
                // it acts on that row alone, which is what a click that did not select anything is about. The base row
                // is left out of what may be removed, and so is a step holding it.
                var chosen = _chosen.Count > 0 && _chosen.Contains(path) ? _chosen : [path];
                var order = VisibleRows()
                    .Select((row, at) => (Path: row.Tag as string, At: at))
                    .Where(one => one.Path is not null)
                    .ToDictionary(one => one.Path!, one => one.At, StringComparer.Ordinal);

                var entries = chosen
                    .OrderBy(one => string.Equals(one, path, StringComparison.Ordinal) ? -1 : order.GetValueOrDefault(one))
                    .Select(Being)
                    .ToArray();

                // Offered by every row with steps under it, which is the same row that is given an expander, since
                // closing them all is worth offering where there is more than one to close. What is not offered is
                // the other half, opening them all, since that would read every directory under the row, which is
                // what a tree read a step at a time is for avoiding. It goes in the same menu as the entry, under
                // the view category, because it is about how the tree is showing a place rather than about the place.
                IReadOnlyList<MenuLeaf>? extra =
                    collapse is null
                        ? null
                        :
                        [
                            new MenuLeaf(
                                ["menu.entry.view", "menu.entry.collapse_all"],
                                30,
                                collapse,
                                "rorolala_file_system.collapse_all"
                            ),
                        ];

                _actions.Fill(
                    menu,
                    this,
                    entries,
                    entries.All(one => !HoldsRoot(one.Path)),
                    extra: extra
                );
            };
        }

        // The menu is opened by the control the row is in rather than by the row, since one of the
        // things it does, putting a path on the clipboard, is reached through a control on screen.
        // A tree row is as wide as the tree, so what speaks for an entry is worn at the row's right rather than
        // on the icon's corner: a tree is a list of names, and a badge drawn over a picture in it would be a mark
        // on the picture rather than an answer about the entry.
        // The strip already standing for this row is given up before the new one is made, because what a strip
        // wears is compared against what was worn for the row: a new strip compared against the old answer would be
        // taken for one that already says it, and stay empty until something asked the row again.
        _worn.RemoveAll(one => string.Equals(one.Path, path, StringComparison.Ordinal));
        _wornBadges.Remove(path);

        var badges = Worn(entry);
        badges.Name = "entry-badges";
        badges.Margin = new Thickness(0, 0, 6, 0);

        _worn.Add((path, badges));

        // A directory is the thing a tree is made of, so its step is its name: a folder picture beside every
        // step would be the same answer on every row, and the chevron already says which rows open.
        var icon = entry.Kind is EntryKind.Directory ? null : _listing.Icon(entry, Icons.Row);

        if (icon is not null)
        {
            icon.Name = "entry-icon";
        }

        var label = new TextBlock
        {
            Text = name.Length > 0 ? name : path,
            Name = "entry-name",
            Classes = { EntryName },
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            MinWidth = 0,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            ClipToBounds = true,
        };

        WearName(entry, label);

        // The name column is the one that gives way, and a TextBlock in a DockPanel's last-child fill can
        // draw past the width it was arranged in and cover the badges. The badges are measured before the
        // name, so the name is trimmed rather than the strip clipped or pushed out of the panel.
        var panel = new Grid
        {
            ColumnDefinitions =
                icon is null ? new ColumnDefinitions("*,Auto") : new ColumnDefinitions("Auto,*,Auto"),
            ColumnSpacing = 8,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // A file has no arrow of its own, so its picture stands where the name of the directory that
        // holds it does rather than one indent step further in. The depth is still read from the rows
        // above, and a directory's own row keeps its arrow and its name where they were.
        if (icon is not null)
        {
            panel.Margin = new Thickness(-Indent, 0, 0, 0);
        }

        if (icon is null)
        {
            Grid.SetColumn(label, 0);
            Grid.SetColumn(badges, 1);
            panel.Children.Add(label);
            panel.Children.Add(badges);
        }
        else
        {
            Grid.SetColumn(icon, 0);
            Grid.SetColumn(label, 1);
            Grid.SetColumn(badges, 2);
            panel.Children.Add(icon);
            panel.Children.Add(label);
            panel.Children.Add(badges);
        }

        var row = new Border
        {
            Background = Brushes.Transparent,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            ContextMenu = menu,
            Child = panel,
        };

        row.Tapped += (_, tapped) =>
        {
            // A row the tree only shows is not a place to walk into and not a thing to open: the press is
            // taken here so that neither the row nor the tree reads it as one.
            if (virtualEntry is not null)
            {
                return;
            }

            // A press that became a drag is not also a step: the drag took the pointer, and a hand letting go of
            // one is putting something down rather than choosing a place.
            if (_dragged)
            {
                return;
            }

            // A modified press is the toolkit's selection and nothing else: `Ctrl` toggles one row and `Shift` takes
            // a range, and going to the place under either would take the whole tree with it for a gesture that was
            // about choosing what to act on.
            if (
                tapped.KeyModifiers.HasFlag(KeyModifiers.Control)
                || tapped.KeyModifiers.HasFlag(KeyModifiers.Shift)
            )
            {
                return;
            }

            Choose(path);

            if (entry.Kind is EntryKind.Directory)
            {
                // A press on a directory is the whole of walking: the location goes there, and the step is left
                // open onto what it holds, so the step after it is one the user can see and press. A press on a
                // row never puts a step away (what closes one is its chevron, or the menu's close-everything)
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
            if (virtualEntry is not null || e.Source is TextBox)
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

        // A step's header is made again when what it holds changes, the way to close it is offered once it has
        // something to close, and the row it had is then no longer the row it is drawn with: keeping both would
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
        var badges = _listing.Badges(entry);

        // A strip already saying this is left alone. The call comes on every change the browser reports,
        // including one that changed another directory, and clearing and rebuilding every row's strip
        // for an answer that did not change is a relayout the eye reads as a flicker.
        if (_wornBadges.TryGetValue(entry.Path, out var was) && was.SequenceEqual(badges))
        {
            return;
        }

        _wornBadges[entry.Path] = badges;
        strip.Children.Clear();

        foreach (var badge in badges)
        {
            strip.Children.Add(Icons.WornAt(badge, Icons.Badge));
        }

        // Moved rather than arranged: the room the theme leaves under the row's line belongs to the line and
        // to the steps below it, and the badges are the one thing in the row that is centred in the step.
        strip.RenderTransform = new TranslateTransform(0, BadgeLift);

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

    /// <summary>Draws the coloured rows and the guides again, without reading anything again.</summary>
    /// <remarks>
    /// A provider's answer is read when a block is drawn, so a rule that appeared since the last drawing is one
    /// this asks for: the rows themselves are unchanged, and only what is behind them and the lines beside them
    /// are not.
    /// </remarks>
    public void RepaintColours()
    {
        PostPaintBlocks();
        PostPaintGuides();
    }

    /// <summary>Schedules the guide layer's colours for after the layout that moved the rows.</summary>
    private void PostPaintGuides()
    {
        if (_guidesPosted)
        {
            return;
        }

        _guidesPosted = true;

        Frame.Post(() =>
        {
            _guidesPosted = false;

            foreach (var item in VisibleRows())
            {
                DrawGuides(item);

                // The name leans towards the colour the row stands in, which is asked of the providers, so it is
                // painted here beside the lines rather than only where the row was built: a colour that arrived
                // since then reaches the words as well as the ground behind them.
                if (item.Tag is string path && NameOf(item) is { } written)
                {
                    WearName(
                        new Entry(path, _kinds.GetValueOrDefault(path, EntryKind.Directory)),
                        written
                    );
                }
            }
        });
    }

    /// <summary>The label a row's name is written in, or nothing while the row has no header yet.</summary>
    /// <param name="item">The row.</param>
    /// <returns>The name label.</returns>
    private static TextBlock? NameOf(TreeViewItem item) =>
        item.GetVisualDescendants()
            .OfType<TextBlock>()
            .FirstOrDefault(one => string.Equals(one.Name, "entry-name", StringComparison.Ordinal));

    /// <summary>Paints a row's name in the ink it is read in.</summary>
    /// <remarks>
    /// A name inside a coloured part of the tree leans towards that colour without leaving the ink: the tree is
    /// read at a glance, and a row whose colour only stood behind the words would still be a grey line of text
    /// under a folder that is not. An ink the listing names for the entry wins, since that ink is about the entry
    /// rather than about where it stands.
    /// </remarks>
    /// <param name="entry">The entry the name belongs to.</param>
    /// <param name="label">The label the name is written in.</param>
    private void WearName(Entry entry, TextBlock label)
    {
        if (_listing.NameInk(entry) is { Length: > 0 } ink)
        {
            label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(ink);

            return;
        }

        if (BandOf(entry.Path) is { } band)
        {
            label.Foreground = new SolidColorBrush(Mix(_host.Palette.Front, band, TintedName));

            return;
        }

        label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
    }

    /// <summary>A row for a file, which opens onto nothing.</summary>
    /// <param name="entry">The file the row names.</param>
    /// <returns>The row.</returns>
    private TreeViewItem Leaf(Entry entry)
    {
        // The row is made before its header, since a header is told which step it is: a file has none, because
        // nothing opens under a file.
        var item = new TreeViewItem
        {
            Tag = entry.Path,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };

        _kinds[entry.Path] = entry.Kind;
        item.Header = Header(entry, step: null, collapse: null);

        Guides(item);

        return item;
    }

    /// <summary>How far one step of the tree is indented, which the template's own margin uses too.</summary>
    private const double Indent = 16.0;

    /// <summary>How far the first guide line stands from the pane's left edge.</summary>
    /// <remarks>
    /// Half an indent: the first line is then as far from the edge as one line is from the next, so
    /// the depth reads as one rhythm rather than as a line that happens to start close to the edge.
    /// </remarks>
    private const double Lead = 8.0;

    /// <summary>How much of its colour the row a mark belongs to is washed with.</summary>
    private const double OwnWash = 0.16;

    /// <summary>How much of the colour a row that only inherits it is washed with.</summary>
    private const double InheritedWash = 0.08;

    /// <summary>How strong a guide line is against the row it stands in.</summary>
    private const double Guide = 0.65;

    /// <summary>How strong a guide line in a colour is.</summary>
    /// <remarks>
    /// Only a little above the wash behind the row, because a line is a direction rather than a mark: it has to
    /// be read as the colour's, and it must not become the thing the eye finds first.
    /// </remarks>
    private const double TintedGuide = 0.24;

    /// <summary>How far a name leans towards the colour of the part of the tree it stands in.</summary>
    private const double TintedName = 0.15;

    /// <summary>The name the layer of guide lines carries, which a test reads.</summary>
    private const string GuideName = "TreeGuides";

    /// <summary>
    /// How far a row's badges are moved down to sit on the row's own middle.
    /// </summary>
    /// <remarks>
    /// The theme leaves the last three pixels of a step under its line, so a twenty-pixel mark centred in
    /// what is left sits one pixel high and four low. A whole two pixels puts it back on the row's middle
    /// without moving the name, which is centred against the line rather than against the whole step. It is
    /// a whole number rather than one and a half because the badges are moved with a render transform, and a
    /// half-pixel transform is a layer the compositor resamples, which is what made the pictures in them soft.
    /// </remarks>
    private const double BadgeLift = 2.0;

    /// <summary>
    /// Draws the depth of a row in its indent: one vertical line for every level above it.
    /// </summary>
    /// <remarks>
    /// The lines live in the room the template leaves for the indent, behind the header, so they are not
    /// part of the row's content and do not take a press. A row at level n draws n lines, each at the middle
    /// of an indent step; the rows above and below it draw the same lines, which is what makes them read as
    /// one line down a level rather than as a mark on each row.
    /// </remarks>
    /// <param name="item">The row.</param>
    private static void Guides(TreeViewItem item)
    {
        // The template's first column is sixteen pixels, which is the arrow's picture and one of the
        // two margins around it. The row asks for the wider column after the template is applied, so
        // the six pixels on either side of the arrow are real room rather than an overhang.
        // The row's own shape decides where its chevron is, so it is settled while the template is
        // applied rather than after the row is laid out: a margin settled later moves the control out
        // from under a press already made. The guide layer it adds takes no width, so doing it here
        // cannot push the row it is in.
        item.TemplateApplied += (_, _) => Shape(item);
        item.AttachedToVisualTree += (_, _) => Dispatcher.UIThread.Post(() => Shape(item));

        // The level is worked out from where the row stands in the tree, which can happen before or after
        // the template is applied. Asking again when it changes is what makes the guides appear whichever
        // of the two comes second.
        item.PropertyChanged += (_, change) =>
        {
            if (change.Property == TreeViewItem.LevelProperty)
            {
                Dispatcher.UIThread.Post(() => DrawGuides(item));
            }
        };
    }

    /// <summary>Widens the arrow's column and draws the row's guides once the template is there.</summary>
    /// <param name="item">The row.</param>
    private static void Shape(TreeViewItem item)
    {
        if (
            item.GetVisualDescendants()
                .OfType<Grid>()
                .FirstOrDefault(one => one.Name == "PART_Header") is { } header
            && header.ColumnDefinitions.Count > 0
            && header.ColumnDefinitions[0].Width.Value < 34
        )
        {
            header.ColumnDefinitions = new ColumnDefinitions("34,*");
        }

        if (
            item.GetVisualDescendants()
                .OfType<ToggleButton>()
                .FirstOrDefault(one => one.Name == "PART_ExpandCollapseChevron") is { } chevron
        )
        {
            chevron.Margin = new Thickness(14, 0, 6, 0);
        }

        DrawGuides(item);
    }

    /// <summary>Puts the guide lines in a row's template once it has one.</summary>
    /// <param name="item">The row.</param>
    private static void DrawGuides(TreeViewItem item)
    {
        // The template's own margin reads Level, which is set when the row joins a tree; the ancestors
        // answer the same question even when the event that asks is earlier than that, and the larger of
        // the two is what the row will be drawn at.
        var depth = Math.Max(
            item.Level,
            item.GetLogicalAncestors().OfType<TreeViewItem>().Count()
        );

        if (depth <= 0)
        {
            return;
        }

        var border = item
            .GetVisualDescendants()
            .OfType<Border>()
            .FirstOrDefault(one => one.Name == "SelectionBorder");

        if (border?.Child is not Grid header)
        {
            return;
        }

        var existing = header
            .Children.OfType<Panel>()
            .FirstOrDefault(one => one.Name == GuideName);

        // The colour of the part of the tree the row stands in: every line of the row wears it, whatever its
        // level, so the indent reads as one coloured column rather than as a coloured segment at its foot.
        var tint = Band(item);

        // The line stands in the header, which is shorter than the step: the room below the line is
        // the step's, and a line that stopped at the header would leave a gap at every row. The
        // overhang is read from the two heights rather than written as a number, so a row that grows
        // again takes its lines with it.
        var overhang = Math.Max(0, item.Bounds.Height - border.Bounds.Height);

        // The row's own padding sits above the header, and a line starting at the header would leave a
        // pixel of it between two rows' segments: the layer is pulled up by the header's offset so that
        // the coloured column is one run from the top of a row to the bottom of it.
        var above = Math.Max(0, border.TranslatePoint(new Point(0, 0), item)?.Y ?? 0);
        var margin = new Thickness((-depth * Indent) + Lead, -above, 0, -overhang);

        if (existing is not null)
        {
            existing.Margin = margin;
            WearGuides(existing, depth, tint);

            return;
        }

        var guides = new Panel
        {
            Name = GuideName,

            // No width of its own: the lines are drawn past the panel's edge, and a panel that asked
            // for the indent's width would be a child added to the row after the row was measured,
            // which is the step the eye sees as the whole tree being pushed aside.
            Width = 0,
            Margin = margin,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsHitTestVisible = false,
        };

        for (var level = 1; level <= depth; level++)
        {
            var line = new Border
            {
                Width = 1,
                HorizontalAlignment = HorizontalAlignment.Left,
                VerticalAlignment = VerticalAlignment.Stretch,
                Margin = new Thickness((level * Indent) - (Indent / 2), 0, 0, 0),
                IsHitTestVisible = false,
            };

            WearGuide(line, tint);
            guides.Children.Add(line);
        }

        // The guide layer is a child of the header grid and reaches left over the indent the grid's own
        // margin leaves. It is behind the chevron and the content, so nothing has to be reparented.
        Grid.SetColumn(guides, 0);
        Grid.SetColumnSpan(guides, 2);
        header.Children.Insert(0, guides);
    }

    /// <summary>Puts the look's grey, or the colour of the part a line is in, on a guide layer.</summary>
    /// <param name="guides">The layer.</param>
    /// <param name="depth">How many lines it holds.</param>
    /// <param name="tint">The colour of the part the row stands in, or nothing for the look's own grey.</param>
    private static void WearGuides(Panel guides, int depth, Color? tint)
    {
        for (var level = 1; level <= depth && level <= guides.Children.Count; level++)
        {
            if (guides.Children[level - 1] is Border line)
            {
                WearGuide(line, tint);
            }
        }
    }

    /// <summary>Paints one guide line in the look's grey, or in the colour of the part it stands in.</summary>
    /// <remarks>
    /// A colour is a wash behind the row, and the lines the tree indents by follow it: a reader inside a marked
    /// tree reads its depth in the tree's colour rather than in the look's grey, at every level the row draws.
    /// </remarks>
    /// <param name="line">The line.</param>
    /// <param name="tint">The colour of the part it stands in, or nothing for the look's own grey.</param>
    private static void WearGuide(Border line, Color? tint)
    {
        line.IsVisible = true;

        if (tint is { } colour)
        {
            line.Background = new SolidColorBrush(colour);
            line.Opacity = TintedGuide;

            return;
        }

        line[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);
        line.Opacity = Guide;
    }

    /// <summary>The colour of the part of the tree a row stands in.</summary>
    /// <remarks>
    /// The row itself is asked first when it is a directory, since a marked folder is inside its own colour. A
    /// file is inside the colour of the directory holding it, which is one level up. A row no step above it marks
    /// is in none of it, and answers nothing.
    /// <para>
    /// The colour is the mark's own rather than the darker shade an inherited row is washed with: the lines and
    /// the name wear the colour the folder was given, and the wash is what tells the two apart.
    /// </para>
    /// </remarks>
    /// <param name="item">The row.</param>
    /// <returns>The colour of the part the row stands in, or nothing when it is in none.</returns>
    private static Color? Band(TreeViewItem item) => item.Tag is string path ? BandOf(path) : null;

    /// <summary>The colour of the part of the tree a path stands in.</summary>
    /// <remarks>
    /// The path itself is asked first when it is a directory, since a marked folder is inside its own colour. A
    /// file is inside the colour of the directory holding it, which is one level up. A path no directory above it
    /// marks is in none of it, and answers nothing.
    /// <para>
    /// The colour is the mark's own rather than the darker shade an inherited row is washed with: the lines and
    /// the name wear the colour the folder was given, and the wash is what tells the two apart.
    /// </para>
    /// </remarks>
    /// <param name="path">The path.</param>
    /// <returns>The colour of the part the path stands in, or nothing when it is in none.</returns>
    private static Color? BandOf(string path)
    {
        var directory = Directory.Exists(path) ? path : Path.GetDirectoryName(path);

        while (directory is { Length: > 0 })
        {
            if (FolderColours.Mark(new Entry(directory, EntryKind.Directory)) is { } colour)
            {
                return colour;
            }

            var parent = Path.GetDirectoryName(directory);

            if (parent is not { Length: > 0 } || string.Equals(parent, directory, StringComparison.Ordinal))
            {
                break;
            }

            directory = parent;
        }

        return null;
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
    /// Whether a press begins a drag of the whole choice rather than a new one.
    /// </summary>
    /// <remarks>
    /// A press on a step already in a multiple choice is how a drag of the whole of it begins. A modified press is
    /// the toolkit's selection and nothing else, so it is not a drag of what was chosen but the way a choice is
    /// changed.
    /// </remarks>
    /// <param name="chosen">What is chosen when the press lands.</param>
    /// <param name="path">The step the press landed on.</param>
    /// <param name="modifiers">What is held while it lands.</param>
    /// <returns>Whether the choice is kept for the drag.</returns>
    internal static bool KeepsChoice(
        IReadOnlyList<string> chosen,
        string path,
        KeyModifiers modifiers
    )
    {
        ArgumentNullException.ThrowIfNull(chosen);
        ArgumentNullException.ThrowIfNull(path);

        return chosen.Count > 1
            && chosen.Contains(path, StringComparer.Ordinal)
            && !modifiers.HasFlag(KeyModifiers.Control)
            && !modifiers.HasFlag(KeyModifiers.Shift);
    }

    /// <summary>What a drag from a step carries: the whole choice when the step is in it, and the step alone otherwise.</summary>
    /// <param name="chosen">What is chosen, as entries.</param>
    /// <param name="lead">The step the drag took hold of.</param>
    /// <returns>What the drag carries, the lead among it.</returns>
    internal static IReadOnlyList<Entry> Carrying(IReadOnlyList<Entry> chosen, Entry lead)
    {
        ArgumentNullException.ThrowIfNull(chosen);

        return chosen.Any(entry => string.Equals(entry.Path, lead.Path, StringComparison.Ordinal))
            ? chosen
            : [lead];
    }

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

            // What the catalogue shows of this directory is added after the hiding, for the reason a listing
            // stages it there: it is not on disk, so no rule has anything to say about it. It is read with the
            // real entries so that the two kinds stay in the order a tree reads them.
            var virtualEntries = VirtualEntries.In(path).Select(entry => entry.Entry).ToArray();

            return
            [
                .. directories
                    .Concat(virtualEntries.Where(entry => entry.Kind is EntryKind.Directory))
                    .OrderBy(entry => Path.GetFileName(entry.Path), StringComparer.OrdinalIgnoreCase),
                .. files
                    .Concat(virtualEntries.Where(entry => entry.Kind is EntryKind.File))
                    .OrderBy(entry => Path.GetFileName(entry.Path), StringComparer.OrdinalIgnoreCase),
            ];
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

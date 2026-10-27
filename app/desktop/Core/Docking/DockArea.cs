using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Data;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using MenuItem = Avalonia.Controls.MenuItem;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The dock area: the tree of groups a dock can be placed in, the tabs that choose between the docks of one
/// group, and the dragging that moves a dock from one group to another, or into a window of its own.
/// </summary>
/// <remarks>
/// The area draws the main window's layout tree. Every leaf of it is a <see cref="Region"/> — one group of
/// tabs — and every split is a two-cell grid with a splitter between the cells, recursively, so any group may
/// be split again and the geometry follows the window rather than the other way round.
/// <para>
/// A region keeps every dock it was given and shows one of them at a time, rather than being rebuilt when the
/// set of open docks changes. That is not an optimisation: a control has one parent, and a view moved from one
/// container into another can be caught mid-move — which is what <c>The control … already has a visual
/// parent</c> is. A dock is put in its region once, when it is made; taken out once, when it is closed for
/// good; and shown, hidden, or made the region's own in between. The one deliberate move is a drag, and that
/// move is done by taking the dock out of one region and putting it in another, in that order.
/// </para>
/// <para>
/// What a structural rebuild moves is the <see cref="Region"/> controls and nothing under them: the grids are
/// thrown away and built again, the regions are lifted out of the old grids and put into the new ones, and the
/// dock views never leave the region they are in. That is what keeps a rebuild from re-parenting a view.
/// </para>
/// <para>
/// A header press becomes the toolkit's own drag (<see cref="DockDrag"/>), which is what lets a dock leave the
/// window it was picked up in. The area a drag is carried over answers in two steps: carried anywhere into a
/// group, the group is <em>approached</em> and the five points a dock may be put on are drawn — four isosceles
/// trapezoids around a centre rectangle, filling the group, one for each way it may be split and one for joining
/// its tabs — and the pointer is always on one of them once it is in the group, so the release is what decides.
/// The points are what a drop would do; the release is what does it.
/// </para>
/// </remarks>
internal sealed class DockArea : UserControl
{
    /// <summary>
    /// The class a region's header strip carries, for a theme to address it by.
    /// </summary>
    /// <remarks>
    /// The shell marks the surfaces it owns so that a theme — this host's, or a plugin's — can style them
    /// without knowing what a dock is. The names are fixed, since a plugin's theme has to write them as
    /// literals.
    /// </remarks>
    public const string HeadersClass = "dock-headers";

    /// <summary>The class a dock's own header carries.</summary>
    public const string TitleClass = "dock-title";

    /// <summary>
    /// The class the first header a region is showing carries.
    /// </summary>
    /// <remarks>
    /// The edge a tab strip is drawn with is the one between two tabs, so the first tab has none on its left —
    /// and which that is is the first the region is <em>showing</em>, not the first it was given: a dock whose
    /// header is hidden by being closed is not one a strip starts with. A class rather than a weight, because
    /// what a leading tab looks like is the theme's to decide.
    /// </remarks>
    public const string LeadingClass = "dock-title-leading";

    /// <summary>
    /// The class the last header a region is showing carries.
    /// </summary>
    /// <remarks>
    /// The other end of the same row, and marked for the same reason in reverse: the row of tabs is a surface
    /// of its own, and the edge that ends it is the one between it and the room the strip keeps beside it. What
    /// is between two tabs is one tab's left edge, so the last of them carries nothing on its right unless it
    /// is the last — and a strip of one tab is a row with no edge on its left and this one on its right.
    /// </remarks>
    public const string TrailingClass = "dock-title-trailing";

    /// <summary>The class the header of the dock a region is showing carries.</summary>
    public const string SelectedClass = "selected";

    /// <summary>The class a region's splitter carries.</summary>
    public const string SplitterClass = "dock-splitter";

    /// <summary>The class a splitter that resizes columns carries, in addition to <see cref="SplitterClass"/>.</summary>
    public const string SplitterColumnsClass = "dock-splitter-columns";

    /// <summary>The class a splitter that resizes rows carries, in addition to <see cref="SplitterClass"/>.</summary>
    public const string SplitterRowsClass = "dock-splitter-rows";

    /// <summary>
    /// The class one of the dock points a dragged dock can be put on carries.
    /// </summary>
    /// <remarks>
    /// The answer to a drag is the points themselves and nothing else: they fill the group, so the group around them
    /// is not outlined or washed — a second box over the same rectangle would be a second shape saying the same
    /// thing, and two translucent surfaces over one another are a darker rectangle rather than a clearer answer.
    /// Each point also carries the class of what it would do (<see cref="DockPointLeftClass"/> and the rest), so
    /// that the look can draw the five of them as one cluster and still tell them apart.
    /// </remarks>
    public const string DockPointClass = "dock-point";

    /// <summary>The class the point that would put the dock among a group's tabs carries.</summary>
    public const string DockPointCenterClass = "dock-point-center";

    /// <summary>The class the point that would split a group, the dock on its left, carries.</summary>
    public const string DockPointLeftClass = "dock-point-left";

    /// <summary>The class the point that would split a group, the dock on its right, carries.</summary>
    public const string DockPointRightClass = "dock-point-right";

    /// <summary>The class the point that would split a group, the dock above it, carries.</summary>
    public const string DockPointTopClass = "dock-point-top";

    /// <summary>The class the point that would split a group, the dock below it, carries.</summary>
    public const string DockPointBottomClass = "dock-point-bottom";

    /// <summary>The class the point the pointer is over carries, in addition to <see cref="DockPointClass"/>.</summary>
    public const string DockPointOverClass = "dock-point-over";

    /// <summary>The class the button that closes the dock a region is showing carries.</summary>
    public const string CloseClass = "dock-close";

    /// <summary>The class the area of a window that is one dock carries.</summary>
    /// <remarks>
    /// What a window of one dock is drawn as: the dock's own content under the window's row, with none of the
    /// chrome a region has — no strip, no tabs, no splitter, no commands. The class is how the look knows to
    /// leave the strip's ground and line off, since a surface that is not there is not drawn.
    /// </remarks>
    public const string AloneClass = "dock-area-alone";

    /// <summary>
    /// The height of a region's header strip, which its tabs sit in.
    /// </summary>
    /// <remarks>
    /// The strip, the menu bar and a toolbar are one height, so that the window reads as a stack of bands of
    /// the same weight. A window of one dock has no strip at all — what it draws is the dock itself, under the
    /// window's own row — so its row is the menu bar's height and this one
    /// (<see cref="MainWindow.TitleBarHeight"/>) is not drawn there.
    /// </remarks>
    private const double StripHeight = 36;

    /// <summary>
    /// How thick the band of a group is that the four edge points take, in pixels.
    /// </summary>
    /// <remarks>
    /// The five points fill the group: the four trapezoids are the band along its edges and the centre rectangle is
    /// what is left, so a group bigger than its band is a rectangle inside a ring of four answers. A band rather than
    /// a share because what a pointer has to reach is a distance rather than a proportion — the edge of a large group
    /// is as far from its middle as the edge of any other — and because a centre that keeps the group's own shape is
    /// the "among these tabs" target rather than a place of its own.
    /// <para>
    /// A group too small to give its centre a third of itself shrinks the band with it, so the centre is never lost:
    /// the four edges need most of a small group, and the one answer that says "among these tabs" is the one it must
    /// not lose.
    /// </para>
    /// </remarks>
    private const double DockPointEdge = 120;

    /// <summary>How many pixels wide and tall a contributed mark is drawn in a menu item.</summary>
    /// <remarks>
    /// The size a menu item reserves for a mark of its own, so that an item with a picture and one without keep
    /// their words in the same column rather than stepping sideways when a picture appears.
    /// </remarks>
    private const double MenuMark = 16.0;

    /// <summary>How large a command's picture is in a strip, which is the size of the words beside it.</summary>
    private const double CommandMark = 14.0;

    /// <summary>How wide a splitter is.</summary>
    /// <remarks>
    /// Published, because a theme draws a line through the middle of one: the width it is drawn in is this,
    /// and a line of another width would not be centred in it. It is the room a hand has to take hold of rather
    /// than the room the line takes — the line is one pixel of it, and the rest is the ground either side — so
    /// two docks read as one divider apart and still resize from five. It is the tree's own number, which is
    /// what the least a split may be is measured with.
    /// </remarks>
    public const double SplitterSize = DockTree.SplitterSize;

    /// <summary>The docks and where they are.</summary>
    private readonly DockManager _manager;

    /// <summary>The host's translations, for dock titles and header commands.</summary>
    private readonly I18nService _i18n;

    /// <summary>
    /// The window this area draws.
    /// </summary>
    /// <remarks>
    /// Nothing for the main window, and a key for a window of its own. Every window has an area of its own, and
    /// each draws the docks that are in it: a dock that has been moved into a window of its own is in that
    /// window's area and in nothing else, which is what keeps one view from being asked to be in two places at
    /// once.
    /// </remarks>
    private readonly string? _window;

    /// <summary>Where a dock's own tab menu is registered, and where a move into a window is asked for.</summary>
    private readonly DockWindows _windows;

    /// <summary>
    /// Where the one dock of a window of its own is drawn.
    /// </summary>
    /// <remarks>
    /// A border with nothing of its own rather than the dock's control dropped straight in, because the control
    /// is the plugin's and this is the host's: what a window of one dock owns is the room the view is given, and
    /// the line above that room is the row's.
    /// </remarks>
    private readonly Border _alone = new() { BorderThickness = new Thickness(0) };

    /// <summary>
    /// Whether this window holds one dock, and so has no chrome of its own.
    /// </summary>
    /// <remarks>
    /// The one thing a window of a dock's own is not: a second view beside it. A region is a place a view is
    /// kept with its own strip and splitter because another may be put beside it; a window that is one dock has
    /// nothing to put beside it, so what it draws is the dock's own content.
    /// </remarks>
    public bool Alone => _window is not null;

    /// <summary>
    /// The dock a window of its own is drawing, or nothing while it is drawing none.
    /// </summary>
    /// <remarks>
    /// Read rather than the tree walked, because what a window of one dock holds is one control and the
    /// question about it is whether it is the dock's own: a place that holds a region's surface and a place
    /// that holds a view are told apart by this and not by what is under them.
    /// </remarks>
    internal Control? Sole => _alone.Child;

    /// <summary>The area, with the structure drawn in it and the drop preview over it.</summary>
    private readonly Grid _surface = new();

    /// <summary>The tree drawn: one nested grid per split, one region per group.</summary>
    private readonly Grid _structure = new();

    /// <summary>
    /// What a dragged dock is shown over the area: the group it has come to, and the points it may be put on.
    /// </summary>
    /// <remarks>
    /// A canvas rather than a grid, because the dock points are trapezoids rather than boxes: a shape is drawn
    /// where its own outline says, and where that is changes with the size of the group under the pointer.
    /// </remarks>
    private readonly Canvas _zones = new();

    /// <summary>What the drag in flight is over, or nothing while it is over no group.</summary>
    private Approach? _approaching;

    /// <summary>The points drawn for the group the drag is over, by what each of them would do.</summary>
    private readonly Dictionary<DockSide, Polygon> _points = [];

    /// <summary>The outlines of the points drawn, which is what hit tests the pointer against.</summary>
    private readonly List<(DockSide Side, Point[] Outline)> _shapes = [];

    /// <summary>What each dock that is placed in a region was given, by the dock.</summary>
    private readonly Dictionary<DockInstance, Placed> _placed = [];

    /// <summary>The regions, by the id of the group each draws.</summary>
    private readonly Dictionary<string, Region> _regions = new(StringComparer.Ordinal);

    /// <summary>The splitters the structure currently holds, for deciding which of them the window can honour.</summary>
    private readonly List<SplitPart> _splits = [];

    /// <summary>
    /// The drag the pointer is on, if it is on one.
    /// </summary>
    /// <remarks>
    /// A press is written down here and nothing else: what a press becomes is the toolkit's drag, and the
    /// gesture is this host's only until it has moved far enough to be one. It carries the window the dock was
    /// picked up in, since that is what says whether a drag that lands nowhere tears a window out.
    /// </remarks>
    private DockDrag? _dragging;

    /// <summary>
    /// Whether the boxes are up, which is while a drag that could land here is in flight.
    /// </summary>
    /// <remarks>
    /// Held rather than read off the drag, because the drag may have been picked up in another window: a dock
    /// pulled out of a window of its own and carried over the main one is this area's to land, and nothing of
    /// the gesture itself happened here.
    /// </remarks>
    private bool _zonesUp;

    /// <summary>
    /// Whether this area is following the docks, which is while it is on a tree.
    /// </summary>
    /// <remarks>
    /// Held rather than asked of the toolkit, because attaching and detaching arrive more than once — a dock
    /// dragged to another group is taken off the tree and put back — and a subscription made twice would be a
    /// rebuild run twice for one change.
    /// </remarks>
    private bool _listening;

    /// <summary>Makes the area over the manager it shows.</summary>
    /// <param name="manager">The docks and where they are.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="windows">The host's windows, for the tab menu and for moving a dock into one.</param>
    /// <param name="window">
    /// The window this area is in, or nothing for the main one: it draws the docks that are in that window and
    /// no others. A window of its own is one dock, and draws it without the chrome a region has.
    /// </param>
    public DockArea(DockManager manager, I18nService i18n, DockWindows windows, string? window)
    {
        _manager = manager;
        _i18n = i18n;
        _windows = windows;
        _window = window;

        Build();

        // A header is a button, and a button keeps the pointer while it is pressed; the drag is followed from
        // here, where the events arrive anyway once they have bubbled out of it.
        AddHandler(
            InputElement.PointerMovedEvent,
            OnPointerMoved,
            RoutingStrategies.Bubble,
            handledEventsToo: true
        );
        AddHandler(
            InputElement.PointerReleasedEvent,
            OnPointerReleased,
            RoutingStrategies.Bubble,
            handledEventsToo: true
        );
        AddHandler(InputElement.PointerCaptureLostEvent, (_, _) => Cancel());

        // A dock lands in a group, so a window that has groups takes a drag of one — and a window that is one
        // dock does not: what it draws is the one dock it was made for, and there is no group in it to put
        // another. Nothing is read here beyond where the drag is: what it carries, and whether the dock it
        // carries may land, is asked where it arrives.
        if (!Alone)
        {
            DragDrop.SetAllowDrop(this, true);
            AddHandler(DragDrop.DragEnterEvent, Over);
            AddHandler(DragDrop.DragOverEvent, Over);
            AddHandler(DragDrop.DragLeaveEvent, Off);
            AddHandler(DragDrop.DropEvent, Landed);

            // A splitter is disabled where the window cannot give both of its children their least size, which
            // is only known from the room the layout pass gave it.
            LayoutUpdated += (_, _) => Constrain();
        }
    }

    /// <summary>
    /// Follows the docks again, having been put back on a tree.
    /// </summary>
    /// <param name="e">What was attached.</param>
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        Watch();
    }

    /// <summary>
    /// Lets go of what this area placed, and of the docks it was listening to.
    /// </summary>
    /// <remarks>
    /// A window that is closed takes its area off the tree and keeps nothing: this area is the one that holds a
    /// dock's control — in a region of it, or in the border a window of one dock draws its dock in — and a
    /// control held by a window that is gone is a control that can never be put in another: the toolkit refuses
    /// to give it a second parent, and the dock is stuck in a window nobody can see. That is what an area still
    /// holding a dock for a closed window does the next time that dock is shown anywhere, which is what a dock
    /// coming back to the main window is.
    /// <para>
    /// The listening is given up as well, and for the same reason: an area that is not on the tree is not
    /// drawing anything, and one that went on rebuilding itself for a window that is gone would be doing the
    /// work of a window nobody is looking at.
    /// </para>
    /// </remarks>
    /// <param name="e">What was detached.</param>
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnDetachedFromVisualTree(e);

        if (!_listening)
        {
            return;
        }

        _listening = false;
        _manager.Changed -= Rebuild;
        _windows.DragEnded -= Clear;

        // A window of one dock places nothing: what it draws is the one dock it was made for, put straight into
        // the border that holds it, and the letting go of that is here rather than in the loop below because it
        // is not a placement — there is no group to take it out of.
        if (Alone)
        {
            _alone.Child = null;

            return;
        }

        foreach (var (instance, placed) in _placed.ToArray())
        {
            Detach(placed);
            _placed.Remove(instance);
        }
    }

    /// <summary>Makes the area follow the docks again.</summary>
    private void Watch()
    {
        if (_listening)
        {
            return;
        }

        _listening = true;
        _manager.Changed += Rebuild;

        // A drag that ends by being called off — Esc, or a release nowhere — leaves this area with the points it
        // drew and no event left to take them down, since a cancelled drag has no drop to be told about.
        _windows.DragEnded += Clear;

        // A dock may be in this window, in another one, or in none: what makes an instance this window's is the
        // window it names, which is not a question about the control. So the layer is brought in step as soon
        // as there is one, so that a dock put into an area that is not on a tree yet — the main window while the
        // layout is being restored, a window of a dock's own before it is shown — still lands somewhere, and the
        // work is taken up again from there.
        Rebuild();
    }

    /// <summary>What one dock was given when it was placed.</summary>
    /// <remarks>
    /// The region is the one thing about this that changes: a dock dragged to another group keeps everything
    /// else it was given and is taken out of one region and put in the other.
    /// </remarks>
    private sealed class Placed
    {
        /// <summary>The dock this was made for.</summary>
        public required DockInstance Instance { get; init; }

        /// <summary>The region the dock sits in.</summary>
        public required Region Region { get; set; }

        /// <summary>The dock's control.</summary>
        public required Control View { get; init; }

        /// <summary>The header that selects it, and that a drag is taken from.</summary>
        public required Button Title { get; init; }
    }

    /// <summary>One region: a strip of headers over the docks themselves.</summary>
    /// <remarks>
    /// Every dock the region was given stays in <see cref="Content"/>, and the one being shown is the only one
    /// that is visible. <see cref="Selected"/> is which that is.
    /// <para>
    /// The strip is one row of three: the docks' headers as tabs, then the area the shown dock is dragged by,
    /// then what the shown dock brought in its own header. The commands belong to the region rather than to each
    /// dock because there is one strip and the tabs are as narrow as their words: a title that filled the strip
    /// could not leave room for a tab beside it. What closes a dock is the dock's own tab rather than a button
    /// of the region's, so that it is the tab under the pointer that is closed and not whichever one happens to
    /// be shown.
    /// </para>
    /// </remarks>
    private sealed class Region
    {
        /// <summary>Makes a region, which is named for the group it draws.</summary>
        /// <param name="id">The id of the group it draws.</param>
        public Region(string id)
        {
            Id = id;

            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Star));
            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));

            Grid.SetColumn(Headers, 0);
            Grid.SetColumn(Drag, 1);
            Grid.SetColumn(Commands, 2);

            Bar.Children.Add(Headers);
            Bar.Children.Add(Drag);
            Bar.Children.Add(Commands);

            Strip.Child = Bar;
            DockPanel.SetDock(Strip, Dock.Top);
            Panel.Children.Add(Strip);
            Panel.Children.Add(Content);
        }

        /// <summary>The id of the group this region draws.</summary>
        public string Id { get; }

        /// <summary>The region itself: headers above, docks below.</summary>
        public DockPanel Panel { get; } = new();

        /// <summary>
        /// The strip the headers sit in.
        /// </summary>
        /// <remarks>
        /// A border around the headers rather than a background on them, because a theme's chrome is a surface
        /// with an edge under it, and a <see cref="StackPanel"/> has a background but no border. The padding is
        /// none: the first tab is flush with the edge of the strip it starts, which is what makes a row of tabs
        /// read as the strip rather than as a row laid in it.
        /// </remarks>
        public Border Strip { get; } = new()
        {
            Classes = { HeadersClass },
            Padding = new Thickness(0),
            MinHeight = StripHeight,
        };

        /// <summary>The strip's one row: the tabs, the drag area, and the commands.</summary>
        public Grid Bar { get; } = new();

        /// <summary>The headers, one per dock the region was given, side by side and flush.</summary>
        public StackPanel Headers { get; } =
            new()
            {
                Orientation = Orientation.Horizontal,
                Spacing = 0,
            };

        /// <summary>
        /// The area between the tabs and the commands, which is what the shown dock is dragged by.
        /// </summary>
        /// <remarks>
        /// A drag needs somewhere to start that is not a label, and the tabs are as narrow as their words. What
        /// is left of the strip belongs to what is being shown. Nothing is drawn here, but it must not be
        /// transparent to the pointer, or a drag from it would fall through to whatever is underneath.
        /// </remarks>
        public Border Drag { get; } =
            new()
            {
                Background = Brushes.Transparent,
                Cursor = new Cursor(StandardCursorType.SizeAll),
            };

        /// <summary>The header commands of the dock being shown.</summary>
        /// <remarks>
        /// What the strip keeps to the right of its tabs, which is why the room is a margin here rather than
        /// padding on the strip: the tabs are flush with the edge the strip starts at, and these are not flush
        /// with the one it ends at.
        /// </remarks>
        public StackPanel Commands { get; } =
            new()
            {
                Orientation = Orientation.Horizontal,
                Spacing = 2,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(0, 0, 8, 0),
            };

        /// <summary>The dock whose commands the strip is showing, which is what <see cref="Commands"/> was built for.</summary>
        public DockInstance? Shown { get; set; }

        /// <summary>Every dock the region was given, of which one is visible at a time.</summary>
        public Panel Content { get; } = new();

        /// <summary>Which of its docks is being shown.</summary>
        public DockInstance? Selected { get; set; }
    }

    /// <summary>
    /// One splitter the structure holds, and the two controls whose room it divides.
    /// </summary>
    /// <remarks>
    /// Kept after the structure is built because the least a split may be is a number of pixels and the window
    /// is the only thing that knows how many it has: the splitter has to be disabled when the room under it
    /// cannot give both children their own least, and that is answered by the room it actually got.
    /// </remarks>
    /// <param name="Split">The split the splitter resizes.</param>
    /// <param name="Handle">The grip itself.</param>
    /// <param name="First">The control drawn in the split's first child.</param>
    /// <param name="Second">The control drawn in the split's second child.</param>
    private sealed record SplitPart(LayoutSplit Split, GridSplitter Handle, Control First, Control Second);

    /// <summary>
    /// What a dragged dock has come to: the group under the pointer, or the whole area when there is no group
    /// to be over.
    /// </summary>
    /// <remarks>
    /// The first of the two decisions a drop makes. A group is <em>approached</em> as soon as the pointer is
    /// anywhere in it — the whole of the leaf is the near zone — and what the drag then offers is where in that
    /// group the dock would go. A root holding nothing, or one whose docks are all hidden, is drawn as no group
    /// at all, so the whole area is approached instead: a dock put there makes the group or joins the active
    /// one, since there is nothing to be beside.
    /// </remarks>
    /// <param name="Leaf">The rectangle the group is drawn in, or the whole area when there is no group.</param>
    /// <param name="Group">The group under the pointer, or nothing when there is none to be over.</param>
    private readonly record struct Approach(Rect Leaf, LayoutGroup? Group);

    /// <summary>One point of the cluster: what it would do, and the outline that hit tests the pointer.</summary>
    /// <param name="Side">What a drop on it would do.</param>
    /// <param name="Outline">The outline, in the area's coordinates.</param>
    internal readonly record struct DockPointShape(DockSide Side, IReadOnlyList<Point> Outline);

    /// <summary>Lays the area out once: the structure to draw the tree in, and the boxes over it.</summary>
    private void Build()
    {
        // A window of one dock has no groups to lay out: what it is drawn as is the dock itself, under the
        // window's own row, and everything below this is about putting one view beside another — a strip to
        // choose between them, a splitter to resize one against the other, and boxes to drop a third into. None
        // of that has anything to say in a window that holds one dock, and drawing it would be chrome around a
        // view that nothing can be put beside.
        if (Alone)
        {
            Classes.Add(AloneClass);
            Content = _alone;

            return;
        }

        _zones.IsHitTestVisible = false;
        _zones.IsVisible = false;

        _surface.Children.Add(_structure);
        _surface.Children.Add(_zones);

        Content = _surface;
    }

    /// <summary>Brings the area back in step with what is registered, open, and where.</summary>
    private void Rebuild()
    {
        if (Alone)
        {
            Only();

            return;
        }

        Forget();
        Sync();
        Place();
        Render();
        Order();
        Settle();
        Show();
    }

    /// <summary>
    /// Draws the one dock a window of its own is for, and nothing else.
    /// </summary>
    /// <remarks>
    /// Which dock that is, is the dock that names this window: the window was made for it, and it is the only one
    /// there is. A dock that is closed is drawn too, and hidden — the same as a region does with a dock that is
    /// not the one it is showing — so that opening it again is showing the control that is already there rather
    /// than one that has to be put back.
    /// </remarks>
    private void Only()
    {
        var shown = _manager.Instances.FirstOrDefault(Mine);

        if (!ReferenceEquals(_alone.Child, shown?.View.View))
        {
            _alone.Child = shown?.View.View;
        }

        if (shown is not null)
        {
            shown.View.View.IsVisible = shown.IsOpen;
        }
    }

    /// <summary>Takes away the docks that are no longer open at all.</summary>
    /// <remarks>
    /// A dock that is still there but has moved to another window is let go of here as well, and that is what
    /// makes a move one change: the window it left takes it off its own tree, and the window it went to puts it
    /// on its own. A view that stayed on the tree of the window it left would be one control in two parents,
    /// which no control can be.
    /// </remarks>
    private void Forget()
    {
        foreach (var (instance, placed) in _placed.ToArray())
        {
            if (_manager.Instances.Contains(instance) && Mine(instance))
            {
                continue;
            }

            Detach(placed);
            _placed.Remove(instance);
        }
    }

    /// <summary>
    /// Makes a region for every group the tree holds, and lets go of the ones for groups it no longer holds.
    /// </summary>
    /// <remarks>
    /// The regions are reused for as long as the group is: a group's docks stay in the control that draws them,
    /// and it is the control that moves between grids when the tree is arranged differently. A group that is
    /// gone takes its region with it, and the docks that were in it were detached in <see cref="Forget"/> —
    /// a group is only pruned once it holds nothing.
    /// </remarks>
    private void Sync()
    {
        var groups = Groups();
        var live = groups.Select(group => group.Id).ToHashSet(StringComparer.Ordinal);

        foreach (var (id, region) in _regions.ToArray())
        {
            if (live.Contains(id))
            {
                continue;
            }

            if (region.Panel.Parent is Panel parent)
            {
                parent.Children.Remove(region.Panel);
            }

            _regions.Remove(id);
        }

        foreach (var group in groups)
        {
            if (_regions.ContainsKey(group.Id))
            {
                continue;
            }

            var region = new Region(group.Id);

            region.Panel.Background = Brushes.Transparent;

            // The drag area belongs to whatever the region is showing at the time it is used rather than to a
            // dock, since it outlives every dock that passes through the region.
            region.Drag.PointerPressed += (_, e) => Take(region.Selected, e);
            _regions[group.Id] = region;
        }
    }

    /// <summary>Puts every dock of this window where the tree says it is, moving the ones dragged.</summary>
    private void Place()
    {
        foreach (var instance in _manager.Instances)
        {
            if (!Mine(instance) || !_regions.TryGetValue(instance.GroupId, out var region))
            {
                continue;
            }

            if (!_placed.TryGetValue(instance, out var placed))
            {
                placed = Make(instance, region);
                Attach(placed, region);
                _placed[instance] = placed;

                continue;
            }

            if (ReferenceEquals(placed.Region, region))
            {
                continue;
            }

            // The one move there is: taken out of the region it was in, put in the other, in that order, so it
            // is never in two places nor in none.
            Detach(placed);
            Attach(placed, region);
        }
    }

    /// <summary>
    /// Whether a dock is one of this window's.
    /// </summary>
    /// <remarks>
    /// One question, asked everywhere an instance is looked at, because a window is what a dock belongs to and
    /// two windows that both counted one would both draw it.
    /// </remarks>
    /// <param name="instance">The dock.</param>
    /// <returns>Whether it is in this window.</returns>
    private bool Mine(DockInstance instance) =>
        string.Equals(instance.Window, _window, StringComparison.Ordinal);

    /// <summary>
    /// Makes a dock's view and its header, which it keeps for as long as it lives.
    /// </summary>
    /// <param name="instance">The dock.</param>
    /// <param name="region">The region the dock sits in.</param>
    /// <returns>What the area placed.</returns>
    private Placed Make(DockInstance instance, Region region)
    {
        // The room the tab keeps either side of its name, which is the width the close is given at its end as
        // well: one number, because the close is a strip of the tab's own right side — that room wide and as
        // tall as the tab — and the name keeps as much again on its left so that it stays in the middle of the
        // tab rather than in the middle of what the close left of it. It has to be room enough for the cross and
        // the room the cross keeps from the tab's end both, or the cross is pressed against the name it is drawn
        // beside.
        const double Room = 26.0;

        var name = new TextBlock
        {
            Text = instance.Title,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var close = new Button
        {
            Content = "\u2715",
            Classes = { CloseClass },
            Width = Room,
            // The cross keeps a little room at the tab's own end rather than standing on it: it is a mark
            // inside a strip and not the strip's own edge, and one drawn hard against the edge reads as part
            // of the border rather than as something to press.
            Padding = new Thickness(0, 0, 10, 0),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
        };

        // Stopped here rather than left to bubble: a button's click is a bubbling event, and the tab the close
        // sits in answers one of its own by selecting the dock — so closing a dock would also select it, and the
        // manager would be asked to show what it has just been asked to close.
        close.Click += (_, e) =>
        {
            e.Handled = true;
            _manager.Close(instance);
        };

        // Three columns: the room the close takes, the name, and the close itself. The first holds nothing and
        // is there for the name's sake — a box of two would centre the name in what is left of the close instead
        // of in the tab, which moves the title every time the pointer crosses it.
        var box = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions($"{Room},*,{Room}"),
            RowDefinitions = new RowDefinitions("*"),
        };

        Grid.SetColumn(name, 1);
        Grid.SetColumn(close, 2);

        box.Children.Add(name);
        box.Children.Add(close);

        // The tab's own geometry, where the generic header's is for a command: a tab fills the strip it sits in
        // — no padding, no margin, as tall as the strip — because the tabs *are* the strip. What it holds is
        // stretched rather than centred, so that the close reaches the tab's own ends, and the name is kept in
        // the middle by the column it was put in.
        var title = new Button
        {
            Content = box,
            Padding = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Stretch,
            VerticalContentAlignment = VerticalAlignment.Stretch,
        };

        title.Classes.Add(TitleClass);
        title.Click += (_, _) => Select(instance);

        // The menu a tab opens, made once here rather than per right-click: what a dock's tab offers is about
        // the dock rather than about what the pointer happens to be over, so nothing about it is stale when the
        // menu is opened again.
        title.ContextMenu = TabMenu(instance);

        // Taken on the way down. A button answers for its own press, and marks it handled while doing so, so an
        // ordinary handler here would never be called: the press is over before it reaches one. A tunnelling
        // handler is reached first, which is the only way in — and a press on the close is the close's rather
        // than the tab's, or closing a dock would drag it instead.
        title.AddHandler(
            InputElement.PointerPressedEvent,
            (_, e) =>
            {
                if (!Closing(e.Source))
                {
                    Take(instance, e);
                }
            },
            RoutingStrategies.Tunnel
        );

        var placed = new Placed
        {
            Instance = instance,
            Region = region,
            View = instance.View.View,
            Title = title,
        };

        // A dock that the keyboard comes to is the dock being worked in, which is what says where the next dock
        // opened without a place of its own goes. Told to the manager rather than kept here, because it is part
        // of the arrangement and is written down with it.
        placed.View.GotFocus += (_, _) => _manager.ActivateGroup(placed.Region.Id);

        return placed;
    }

    /// <summary>
    /// The menu a dock's tab opens: what was registered for its place, and what the dock says about itself.
    /// </summary>
    /// <remarks>
    /// The registered items come first, because they are what the host and the plugins have to say about a dock,
    /// and the dock's own header commands follow them under a rule: a command of the dock's is about the thing
    /// being shown rather than about the dock as a container, and the two read as one list only if the seam
    /// between them is drawn.
    /// <para>
    /// A registered item is handed the one contextual shape the program has, with the dock's stable name id
    /// standing where a directory stands for a file menu and no entries at all: a tab is a dock, and stating the
    /// dock in the shape an item already takes is what lets one registration serve both without a second
    /// contract type to keep in step.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock whose tab this is.</param>
    /// <returns>The menu, which a tab always has: a close of the dock is always offered.</returns>
    private ContextMenu TabMenu(DockInstance instance)
    {
        var menu = new ContextMenu();
        var was = new ContextTarget(instance.DockNameId, []);

        foreach (var item in _windows.DockMenuItems(DockMenuTarget.Tab, instance.DockNameId))
        {
            var entry = new MenuItem { Header = _i18n.Get(item.LabelKey) };

            if (item.IconKey is { } key && _windows.Pictures.Find(key) is { } picture)
            {
                entry.Icon = MenuPicture(picture);
            }

            entry.Click += (_, _) => item.Command(was);
            menu.Items.Add(entry);
        }

        // A header drawn as a menu takes the commands, and of every strip what the strip itself offered as its
        // one command: a menu entry is a word and an action, so a control has nothing to put there, and a dock
        // whose strip held the only way to read the work again would have no way to do it in a menu.
        var commands = instance
            .View.Header.Select(item =>
                item switch
                {
                    DockHeaderCommand command => command,
                    DockHeaderView { Command: { } offered } => offered,
                    _ => null,
                }
            )
            .OfType<DockHeaderCommand>()
            .ToArray();

        if (commands.Length > 0 && menu.Items.Count > 0)
        {
            menu.Items.Add(new Separator());
        }

        foreach (var command in commands)
        {
            var entry = new MenuItem { Header = _i18n.Get(command.LabelKey) };

            entry.Click += (_, _) => command.Command();
            menu.Items.Add(entry);
        }

        return menu;
    }

    /// <summary>
    /// A contributed picture, drawn as a menu item's own mark.
    /// </summary>
    /// <remarks>
    /// A border whose fill is the picture rather than the picture itself, so that the mark takes the ink of the
    /// item it sits in: what a menu item is filled with is the look's answer, and a picture that carried its own
    /// colours would be a mark that vanished into the fill the day the look changed.
    /// </remarks>
    /// <param name="picture">The picture to stamp.</param>
    /// <returns>What to put in the item.</returns>
    private static Control MenuPicture(Bitmap picture)
    {
        var mark = new Border
        {
            Width = MenuMark,
            Height = MenuMark,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        // What states the ink is the item, and what is between the two — the presenter an item puts its content
        // in — states nothing of its own, so the nearest item is named rather than guessed at.
        mark.Bind(
            Border.BackgroundProperty,
            new Binding("Foreground")
            {
                RelativeSource = new RelativeSource
                {
                    Mode = RelativeSourceMode.FindAncestor,
                    AncestorType = typeof(MenuItem),
                },
            }
        );

        return mark;
    }

    /// <summary>Whether a press landed on the close a header carries.</summary>
    /// <remarks>
    /// Walked rather than compared, because what the pointer hits is a part of the button's own template rather
    /// than the button — the same reason the tab's own press is taken on the way down.
    /// </remarks>
    /// <param name="pressed">What the press landed on.</param>
    /// <returns>Whether it is the header's close.</returns>
    private static bool Closing(object? pressed)
    {
        for (var at = pressed as StyledElement; at is not null; at = at.Parent)
        {
            if (at is Button button && button.Classes.Contains(CloseClass))
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>Puts a dock into a region.</summary>
    /// <param name="placed">The dock.</param>
    /// <param name="region">The region to put it in.</param>
    private static void Attach(Placed placed, Region region)
    {
        region.Content.Children.Add(placed.View);
        region.Headers.Children.Add(placed.Title);
        placed.Region = region;
    }

    /// <summary>Takes a dock out of its region, leaving the region as if it had not been there.</summary>
    /// <param name="placed">The dock.</param>
    private static void Detach(Placed placed)
    {
        placed.Region.Content.Children.Remove(placed.View);
        placed.Region.Headers.Children.Remove(placed.Title);

        if (ReferenceEquals(placed.Region.Selected, placed.Instance))
        {
            placed.Region.Selected = null;
        }
    }

    /// <summary>
    /// Draws the tree: a nested grid per split, the region of each group where its leaf is.
    /// </summary>
    /// <remarks>
    /// Every region control is taken out of the grid it was in before the old grids are thrown away, because a
    /// control that still believes it has a parent cannot be given another. What moves is the region and nothing
    /// inside it: the dock views stay where they were put, which is the one thing a structural rebuild must not
    /// disturb.
    /// <para>
    /// A group whose docks are all hidden is not drawn at all, and the split it was in gives its room to the
    /// other child. That is the same thing the five fixed regions did with one holding nothing open: a band of
    /// the window kept for a place nobody is reading is a band taken from what is being read.
    /// </para>
    /// </remarks>
    private void Render()
    {
        foreach (var region in _regions.Values)
        {
            if (region.Panel.Parent is Panel parent)
            {
                parent.Children.Remove(region.Panel);
            }
        }

        _structure.Children.Clear();
        _splits.Clear();

        if (Layout.Root is null)
        {
            return;
        }

        if (Draw(Layout.Root) is { } root)
        {
            _structure.Children.Add(root);
        }
    }

    /// <summary>The control a node is drawn as, or nothing when nothing under it is showing.</summary>
    /// <param name="node">The node to draw.</param>
    /// <returns>The control, or nothing.</returns>
    private Control? Draw(LayoutNode node)
    {
        if (node is LayoutGroup group)
        {
            return Drawn(group) && _regions.TryGetValue(group.Id, out var region)
                ? region.Panel
                : null;
        }

        var split = (LayoutSplit)node;
        var first = Draw(split.First);
        var second = Draw(split.Second);

        if (first is null)
        {
            return second;
        }

        if (second is null)
        {
            return first;
        }

        return Join(split, first, second);
    }

    /// <summary>Draws a split as a two-cell grid with a splitter on the seam.</summary>
    /// <param name="split">The split to draw.</param>
    /// <param name="first">The control in its first child.</param>
    /// <param name="second">The control in its second child.</param>
    /// <returns>The grid.</returns>
    private Control Join(LayoutSplit split, Control first, Control second)
    {
        var columns = split.Axis == DockAxis.Columns;
        var grid = new Grid();
        var handle = new GridSplitter();

        Configure(handle, split, first, second);

        // The two content tracks are star-weighted by the split's own share, so that the whole tree follows the
        // window size and no code reacts to a resize to work the sizes out again. The splitter's track takes no
        // room of its own: what separates two docks is the line the grip draws, and the grip hangs over both of
        // them.
        if (columns)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition(new GridLength(split.Ratio, GridUnitType.Star)));
            grid.ColumnDefinitions.Add(new ColumnDefinition(new GridLength(0)));
            grid.ColumnDefinitions.Add(
                new ColumnDefinition(new GridLength(1 - split.Ratio, GridUnitType.Star))
            );

            Grid.SetColumn(first, 0);
            Grid.SetColumn(handle, 1);
            Grid.SetColumn(second, 2);
        }
        else
        {
            grid.RowDefinitions.Add(new RowDefinition(new GridLength(split.Ratio, GridUnitType.Star)));
            grid.RowDefinitions.Add(new RowDefinition(new GridLength(0)));
            grid.RowDefinitions.Add(
                new RowDefinition(new GridLength(1 - split.Ratio, GridUnitType.Star))
            );

            Grid.SetRow(first, 0);
            Grid.SetRow(handle, 1);
            Grid.SetRow(second, 2);
        }

        // The children first and the splitter last: a grip hangs over both of the docks it separates, so it has
        // to be drawn — and hit — above them rather than under the one that follows it in the grid.
        grid.Children.Add(first);
        grid.Children.Add(second);
        grid.Children.Add(handle);

        return grid;
    }

    /// <summary>Sets up one splitter and remembers what it divides.</summary>
    /// <param name="handle">The grip.</param>
    /// <param name="split">The split it resizes.</param>
    /// <param name="first">The control in its first child.</param>
    /// <param name="second">The control in its second child.</param>
    private void Configure(GridSplitter handle, LayoutSplit split, Control first, Control second)
    {
        var columns = split.Axis == DockAxis.Columns;

        handle.ResizeDirection = columns ? GridResizeDirection.Columns : GridResizeDirection.Rows;
        handle.Classes.Add(SplitterClass);
        handle.Classes.Add(columns ? SplitterColumnsClass : SplitterRowsClass);

        // The grip is wider than the room the layout gives it, and hangs over the seam it resizes: what two docks
        // are separated by is the one pixel the theme draws through the middle of this, and the rest of the grip
        // is drawn over the docks themselves rather than as a band of the window's ground between them — a band
        // is what a five-pixel column of nothing looks like, and it reads as a black gap with a line inside it
        // rather than as a join. Half hangs either side, so the line lands on the seam.
        var overhang = (SplitterSize - 1) / 2;

        if (columns)
        {
            handle.Width = SplitterSize;
            handle.Margin = new Thickness(-overhang, 0, 0, 0);
        }
        else
        {
            handle.Height = SplitterSize;
            handle.Margin = new Thickness(0, -overhang, 0, 0);
        }

        handle.DragCompleted += (_, _) => Resized(split, first, second);
        _splits.Add(new SplitPart(split, handle, first, second));
    }

    /// <summary>
    /// Writes back the share a splitter was dragged to.
    /// </summary>
    /// <remarks>
    /// Read from the two measured sides rather than from where the pointer ended: what the user saw while dragging
    /// is the grid's own answer, and a pixel a side is the whole of what the ratio needs. The clamp by the
    /// children's least sizes is what keeps a drag from squeezing one of them out of existence, which the grid
    /// would then clip rather than refuse.
    /// </remarks>
    /// <param name="split">The split that was dragged.</param>
    /// <param name="first">The control in its first child.</param>
    /// <param name="second">The control in its second child.</param>
    private void Resized(LayoutSplit split, Control first, Control second)
    {
        var columns = split.Axis == DockAxis.Columns;
        var before = columns ? first.Bounds.Width : first.Bounds.Height;
        var after = columns ? second.Bounds.Width : second.Bounds.Height;
        var room = before + after;

        if (room <= 0)
        {
            return;
        }

        var ratio = before / room;
        var least = columns ? DockTree.MinimumWidth(split.First) : DockTree.MinimumHeight(split.First);
        var most = 1 - ((columns ? DockTree.MinimumWidth(split.Second) : DockTree.MinimumHeight(split.Second)) / room);

        if (least <= most)
        {
            ratio = Math.Clamp(ratio, least, most);
        }

        _manager.SetRatio(split, ratio);
    }

    /// <summary>
    /// Disables the splitters a window is too small to let both of their children have their least size.
    /// </summary>
    /// <remarks>
    /// A split whose children cannot both be given their minimum cannot be honoured by a drag either: the grid
    /// would squeeze one side past its minimum and the grip would fight the layout for every pixel. Refusing to
    /// move is the honest answer, and it leaves the window clipped rather than oscillating. Minimums are a
    /// runtime constraint only and are never written down.
    /// </remarks>
    private void Constrain()
    {
        foreach (var part in _splits)
        {
            var columns = part.Split.Axis == DockAxis.Columns;
            var room = columns
                ? part.First.Bounds.Width + part.Second.Bounds.Width
                : part.First.Bounds.Height + part.Second.Bounds.Height;
            var needed =
                (columns ? DockTree.MinimumWidth(part.Split) : DockTree.MinimumHeight(part.Split))
                - SplitterSize;

            part.Handle.IsEnabled = room >= needed;
        }
    }

    /// <summary>
    /// Puts each region's tabs and views in the order the tree states.
    /// </summary>
    /// <remarks>
    /// A region draws the docks it was given in the order it was given them, and that order is the order they were
    /// created in — which is not the tab order once a dock has been moved among them. The tree's order is the
    /// truth, so the children are reordered to it rather than the tree being read in creation order. Reordering
    /// moves the controls within the panel they are already in, so no view changes parent.
    /// </remarks>
    private void Order()
    {
        foreach (var group in Groups())
        {
            if (!_regions.TryGetValue(group.Id, out var region))
            {
                continue;
            }

            var placed = group.Docks.Select(PlacedOf).OfType<Placed>().ToArray();

            Reorder(region.Headers, placed.Select(one => one.Title));
            Reorder(region.Content, placed.Select(one => one.View));
        }
    }

    /// <summary>What a recorded dock was placed as, or nothing when it is not placed here.</summary>
    /// <param name="dock">The recorded dock.</param>
    /// <returns>What the area placed, or nothing.</returns>
    private Placed? PlacedOf(LayoutDock dock)
    {
        var instance = _manager.InstanceOf(dock.DockNameId, dock.Ordinal);

        return instance is not null && _placed.TryGetValue(instance, out var placed) ? placed : null;
    }

    /// <summary>Moves some of a panel's children into an order, leaving the ones not named where they are.</summary>
    /// <param name="panel">The panel to reorder.</param>
    /// <param name="wanted">The controls in the order they are to be in.</param>
    private static void Reorder(Panel panel, IEnumerable<Control> wanted)
    {
        var at = 0;

        foreach (var control in wanted)
        {
            var found = panel.Children.IndexOf(control);

            if (found < 0)
            {
                continue;
            }

            if (found != at)
            {
                panel.Children.RemoveAt(found);
                panel.Children.Insert(at, control);
            }

            at++;
        }
    }

    /// <summary>Makes sure each region is showing one of the docks open in it.</summary>
    private void Settle()
    {
        foreach (var group in Groups())
        {
            if (!_regions.TryGetValue(group.Id, out var region))
            {
                continue;
            }

            var docks = Docks(group);

            if (docks.Count == 0)
            {
                region.Selected = null;

                continue;
            }

            var wanted =
                group.Active >= 0 && group.Active < docks.Count && docks[group.Active].IsOpen
                    ? docks[group.Active]
                    : docks.FirstOrDefault(dock => dock.IsOpen);

            region.Selected = wanted;

            if (wanted is null)
            {
                continue;
            }

            var active = docks.IndexOf(wanted);

            if (active >= 0)
            {
                group.Active = Math.Min(active, group.Docks.Count - 1);
            }
        }
    }

    /// <summary>Shows the dock each region settled on, and hides the rest of its docks.</summary>
    private void Show()
    {
        foreach (var (instance, placed) in _placed)
        {
            var shown = instance.IsOpen && ReferenceEquals(placed.Region.Selected, instance);

            placed.View.IsVisible = shown;
            placed.Title.IsVisible = instance.IsOpen;

            // Which dock the region is showing is a class rather than a weight, so that what a selected header
            // looks like is a theme's to decide and not this file's.
            placed.Title.Classes.Set(SelectedClass, shown);
        }

        foreach (var region in _regions.Values)
        {
            Ends(region);
            BindCommands(region);
        }
    }

    /// <summary>
    /// Marks the two ends of a region's strip, which are the headers that draw an edge of their own.
    /// </summary>
    /// <remarks>
    /// The ends of what the region is <em>showing</em> rather than of what it was given: a header hidden by its
    /// dock being closed is not one a strip starts or ends with, and the ones after it take the marks instead.
    /// Both are asked of the same list so that a strip of one header is the first and the last of it at once.
    /// </remarks>
    /// <param name="region">The region whose headers to mark.</param>
    private static void Ends(Region region)
    {
        var headers = region
            .Headers.Children.OfType<Control>()
            .Where(header => header.IsVisible)
            .ToArray();

        for (var at = 0; at < headers.Length; at++)
        {
            headers[at].Classes.Set(LeadingClass, at == 0);
            headers[at].Classes.Set(TrailingClass, at == headers.Length - 1);
        }
    }

    /// <summary>
    /// Makes a region's strip show what the dock it settled on brought in its own header.
    /// </summary>
    /// <remarks>
    /// Rebuilt only when the dock being shown changes: this runs on every change to the set of open docks, and
    /// buttons made again on each of those would lose the pointer resting on them.
    /// </remarks>
    /// <param name="region">The region whose strip to bind.</param>
    private void BindCommands(Region region)
    {
        var shown = region.Selected;

        if (ReferenceEquals(region.Shown, shown))
        {
            return;
        }

        region.Shown = shown;
        region.Commands.Children.Clear();

        if (shown is null)
        {
            return;
        }

        foreach (var command in shown.View.Header.OfType<DockHeaderCommand>())
        {
            var said = _i18n.Get(command.LabelKey);
            var picture = command.IconKey is { Length: > 0 } key ? _windows.Pictures.Find(key) : null;
            var button = Header(said, picture);

            // A command is chrome rather than a thing a surface is for: it draws nothing of its own until the
            // pointer is on it, so that a strip of them does not read as a row of actions.
            button.Classes.Add("ghost");

            // A command drawn as its mark says what it is when the pointer rests on it: that is all that is left
            // of its words, and a strip of marks a reader has to try is a strip of guesses.
            if (picture is not null)
            {
                ToolTip.SetTip(button, said);
            }

            button.Click += (_, _) => command.Command();
            region.Commands.Children.Add(button);
        }

        // A strip a plugin drew is put in as it stands, beside the buttons rather than among them: what is inside
        // it is the plugin's answer, and a host that took it apart would be a second author of it.
        foreach (var view in shown.View.Header.OfType<DockHeaderView>())
        {
            region.Commands.Children.Add(view.View);
        }
    }

    /// <summary>Shows one dock of its region rather than the one that was shown.</summary>
    /// <param name="instance">The dock to show.</param>
    private void Select(DockInstance instance)
    {
        if (!_placed.TryGetValue(instance, out var placed))
        {
            return;
        }

        var group = DockTree.Find(Layout, placed.Region.Id);

        if (group is not null)
        {
            var at = group.Docks.FindIndex(dock =>
                dock.Ordinal == instance.Ordinal
                && string.Equals(dock.DockNameId, instance.DockNameId, StringComparison.Ordinal)
            );

            _manager.SelectTab(group.Id, at >= 0 ? at : group.Active);
        }

        placed.Region.Selected = instance;
        Show();

        // Choosing a dock gives it the keyboard: the keyboard was on the tab that was clicked, and that tab
        // belongs to this strip rather than to the dock — while a dock answers its own keys inside itself, so
        // choosing one and then typing would otherwise say nothing at all.
        instance.View.View.Focus();
    }

    /// <summary>
    /// Remembers that a press may have started a drag.
    /// </summary>
    /// <remarks>
    /// A press is not a drag: what separates the two is how far the pointer goes before it is let go of, which is
    /// why nothing is done here but writing down where it started.
    /// <para>
    /// Which button asked for the gesture is settled when the pointer moves rather than here. What a pointer
    /// event says about its own buttons at the moment of the press is not to be trusted — reading it there is
    /// what kept the drag from starting at all — and the move knows what is held without having to be told.
    /// </para>
    /// <para>
    /// The dock is the one named by the header that was pressed, or — from the strip's drag area — whichever the
    /// region is showing at that moment.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock that was pressed, or nothing.</param>
    /// <param name="e">The press.</param>
    private void Take(DockInstance? instance, PointerPressedEventArgs e)
    {
        if (instance is null)
        {
            _dragging = null;

            return;
        }

        _dragging = new DockDrag(_windows, instance, _window);
        _dragging.Take(e, this);
    }

    /// <summary>
    /// Follows the pointer, and hands a dock it has taken hold of over to the platform.
    /// </summary>
    /// <remarks>
    /// Nothing is drawn here: what a drag looks like over this area is said by the drag itself, which asks the
    /// area it is over where it would land — including, for a dock picked up here, the area it was picked up in.
    /// </remarks>
    /// <param name="sender">The area.</param>
    /// <param name="e">The move.</param>
    private void OnPointerMoved(object? sender, PointerEventArgs e) => _dragging?.Follow(e, this);

    /// <summary>Closes the dock a middle click was let go of on, and forgets a gesture that was not one.</summary>
    /// <param name="sender">The area.</param>
    /// <param name="e">The release.</param>
    private void OnPointerReleased(object? sender, PointerReleasedEventArgs e)
    {
        var instance = _dragging?.Instance;

        Cancel();

        // The middle button is the gesture that closes a dock: a header is where a dock is grabbed, so it is
        // where one is let go of for good. A left release is the toolkit's drag or a click, and both are answered
        // without this: a drag that landed has already been moved, and a click selects.
        if (instance is not null && e.InitialPressMouseButton == MouseButton.Middle)
        {
            _manager.Close(instance);
        }
    }

    /// <summary>Forgets a drag that is over, whatever ended it.</summary>
    private void Cancel() => _dragging = null;

    /// <summary>Shows what a dragged dock could do here, and says whether it may land yet.</summary>
    /// <remarks>
    /// A drag is answered in two steps. Carried anywhere into a group, the drag <em>approaches</em> it: the group is
    /// outlined and the five points a dock could be put on are drawn. The second step is the pointer being over one
    /// of those points, which is the only place a drop can land — so what the area says is "may be dropped" only
    /// when the pointer is on an answer, and merely "still here" while it is only near one.
    /// <para>
    /// A drag this area did not begin is answered here as much as one it did: a dock is carried out of one window
    /// and let go of in another, and the window it arrives in is the one that has to say where it goes. A drag
    /// carrying something else — files, paths, another program's data — is left unanswered, so that what is under
    /// the pointer answers it instead: a listing reads text as paths, and one that is being dragged over is a
    /// listing's to light up.
    /// </para>
    /// <para>
    /// A drag that was called off with Esc is answered with nothing: the platform may still be carrying it, but the
    /// user has said it is over, so no point is offered and a release where it happens to be does nothing.
    /// </para>
    /// </remarks>
    /// <param name="sender">The area.</param>
    /// <param name="e">The drag.</param>
    private void Over(object? sender, DragEventArgs e)
    {
        if (_windows.Cancelled)
        {
            Clear();
            e.DragEffects = DragDropEffects.None;
            e.Handled = true;
            _windows.OverArea = false;

            return;
        }

        if (DockDrag.Carried(_manager, e.DataTransfer) is null)
        {
            return;
        }

        var at = e.GetPosition(this);
        var approach = Approached(at);
        var side = approach is { } here ? SideAt(here, at) : null;

        _zonesUp = true;
        Preview(approach, side);

        // A group takes the drag wherever in it the pointer is: what is being offered is reached by aiming, and a
        // platform that is told "no drop" does not say a drop happened, so the release could not be told apart from
        // one let go of in the open. What the release does is decided by where it is, not by whether the group took
        // the drag.
        e.DragEffects = approach is null ? DragDropEffects.None : DragDropEffects.Move;
        e.Handled = approach is not null;

        // Told to the host's windows rather than kept here, because whether the pointer was over a dock area is one
        // answer for the whole host: the window a drag began in is the one that asks it when the drag ends, and it
        // is not this area. Being anywhere in a group is enough for that: a drag let go of there is a drag called
        // off, not one let go of in the open.
        _windows.OverArea = approach is not null;
    }

    /// <summary>Takes the points down when a drag leaves, and says the pointer is over no place.</summary>
    /// <param name="sender">The area.</param>
    /// <param name="e">The drag.</param>
    private void Off(object? sender, DragEventArgs e)
    {
        Clear();
        _windows.OverArea = false;
    }

    /// <summary>
    /// Lands a dragged dock on the point it was let go of, or does nothing where there is no point.
    /// </summary>
    /// <remarks>
    /// The release is the second half of the two-step gesture: hovering a point only says what would happen, and
    /// this is where the layout is written. A release in a group without a point is a release with no answer — it
    /// does nothing rather than guessing, and it is still a release the area took: what the host does with a drag
    /// whose drop landed nowhere is decided by where the release was, and a drop delivered here was delivered over
    /// this area.
    /// </remarks>
    /// <param name="sender">The area.</param>
    /// <param name="e">The drop.</param>
    private void Landed(object? sender, DragEventArgs e)
    {
        var instance = DockDrag.Carried(_manager, e.DataTransfer);
        var at = e.GetPosition(this);
        var approach = _windows.Cancelled ? null : Approached(at);
        var side = approach is { } here ? SideAt(here, at) : null;
        var target = approach is { } place && side is { } chosen ? TargetOf(place, chosen) : (DockTarget?)null;

        Clear();
        _windows.OverArea = false;

        if (instance is null)
        {
            return;
        }

        // The platform says which target a drop was delivered to by delivering it, so this is the record that the
        // release was over a dock area even where the area had no answer for it: the drag was let go of here, and
        // not in the open.
        _windows.Dropped = true;

        if (target is not { } landed)
        {
            e.DragEffects = DragDropEffects.None;
            e.Handled = true;

            return;
        }

        e.DragEffects = DragDropEffects.Move;
        e.Handled = true;

        // This area's window, which is what says where the dock arrived: one dragged out of a window of its own
        // and let go of over the main window is moved into the main window, and one let go of where it already
        // was is not moved at all. What a window may not show twice is the windows' own answer.
        _windows.Land(instance, landed, _window);
    }

    /// <summary>Takes the points down, whether a drag ended, left, or was called off.</summary>
    private void Clear()
    {
        _zonesUp = false;

        _approaching = null;
        _points.Clear();
        _shapes.Clear();

        _zones.Children.Clear();
        _zones.IsVisible = false;
    }

    /// <summary>
    /// What a point in the area has come to for a dragged dock: the group it is over, or the whole area when there
    /// is no group drawn.
    /// </summary>
    /// <remarks>
    /// The leaves are asked where they are rather than a map of boxes being worked out ahead of time: the tree can
    /// be any shape, and the only thing that knows where a group is drawn is the control drawing it. A point
    /// outside the area has come to nothing, which is what letting go of a drag outside the area means.
    /// <para>
    /// A root that is empty, or whose docks are all hidden, is drawn as no leaf at all: the whole area is what is
    /// approached then, since there is a group to put a dock into but no rectangle of one on screen to be over.
    /// </para>
    /// </remarks>
    /// <param name="at">The point, in the area's coordinates.</param>
    /// <returns>What the point has come to, or nothing when it is outside the area.</returns>
    private Approach? Approached(Point at)
    {
        if (Bounds.Width <= 0 || Bounds.Height <= 0 || !Bounds.Contains(at))
        {
            return null;
        }

        if (Layout.Root is not null)
        {
            foreach (var group in DockTree.Groups(Layout))
            {
                if (
                    _regions.TryGetValue(group.Id, out var region)
                    && Box(region.Panel) is { } leaf
                    && leaf.Contains(at)
                )
                {
                    return new Approach(leaf, group);
                }
            }
        }

        return new Approach(new Rect(0, 0, Bounds.Width, Bounds.Height), null);
    }

    /// <summary>Which of a group's points a point in it is over, or nothing when it is over none.</summary>
    /// <remarks>
    /// The approach of a group is the whole of it, so most of a group is near it without being on an answer: the
    /// cluster of points is the answer, and the rest of the leaf is room to reach it by. An area with no group
    /// drawn is one point over the whole of it, since there is nothing to be beside.
    /// </remarks>
    /// <param name="approach">What the drag has come to.</param>
    /// <param name="at">The point, in the area's coordinates.</param>
    /// <returns>What a drop there would do, or nothing when it is not on a point.</returns>
    private static DockSide? SideAt(Approach approach, Point at)
    {
        if (approach.Group is null)
        {
            return DockSide.Center;
        }

        foreach (var shape in Shapes(approach.Leaf))
        {
            if (Contains(shape.Outline, at))
            {
                return shape.Side;
            }
        }

        return null;
    }

    /// <summary>The place a drop on one of a group's points would put the dock.</summary>
    /// <param name="approach">What the drag has come to.</param>
    /// <param name="side">The point the drop was let go of on.</param>
    /// <returns>What to land.</returns>
    private DockTarget TargetOf(Approach approach, DockSide side)
    {
        if (approach.Group is { } group)
        {
            return new DockTarget(group.Id, side, group.Docks.Count);
        }

        // No group drawn: the dock makes the root group, or — a root whose docks are all hidden — joins the group
        // that would be the active one, which is the same answer opening a dock gives.
        var active =
            Layout.ActiveGroup is { } id && DockTree.Find(Layout, id) is { } named
                ? named
                : DockTree.Groups(Layout).FirstOrDefault();

        return active is null
            ? new DockTarget("", DockSide.Center, 0)
            : new DockTarget(active.Id, DockSide.Center, active.Docks.Count);
    }

    /// <summary>A control's rectangle in the area's own coordinates, or nothing when it is not drawn.</summary>
    /// <param name="control">The control.</param>
    /// <returns>The rectangle, or nothing.</returns>
    private Rect? Box(Control control)
    {
        var origin = control.TranslatePoint(new Point(0, 0), this);

        return origin is null ? null : new Rect(origin.Value, control.Bounds.Size);
    }

    /// <summary>
    /// Draws what the drag has come to: the five points of the group under the pointer, lit by the one it is over.
    /// </summary>
    /// <remarks>
    /// The points are only drawn for the group under the pointer, since a drag is over one group at a time: a second
    /// group's points would be a second answer to a question that has one. What is being aimed at is said by the
    /// point itself — the five fill the group, so a lit point is the whole of where the dock would go and no second
    /// box is drawn over it, which would be one more surface over a shape that already said it.
    /// </remarks>
    /// <param name="approach">What the drag has come to, or nothing.</param>
    /// <param name="side">The point it is over, or nothing.</param>
    private void Preview(Approach? approach, DockSide? side)
    {
        if (!_zonesUp || approach is not { } here)
        {
            return;
        }

        if (
            _approaching is not { } drawn
            || drawn.Leaf != here.Leaf
            || !ReferenceEquals(drawn.Group, here.Group)
        )
        {
            Show(here);
        }

        _approaching = here;
        _zones.IsVisible = true;

        foreach (var (pointed, zone) in _points)
        {
            zone.Classes.Set(DockPointOverClass, pointed == side);
        }
    }

    /// <summary>Builds the overlay for one group: the five points a drop could land on, and nothing else.</summary>
    /// <remarks>
    /// The children are rebuilt when the group under the pointer changes and kept while it does not, because the
    /// pointer spends a drag moving between points of one group and rebuilding five shapes on every move would be
    /// drawing work for a change of class. What moves with the pointer is only which point wears
    /// <see cref="DockPointOverClass"/>.
    /// </remarks>
    /// <param name="approach">What the drag has come to.</param>
    private void Show(Approach approach)
    {
        _zones.Children.Clear();
        _points.Clear();
        _shapes.Clear();

        foreach (var shape in approach.Group is null ? Whole(approach.Leaf) : Shapes(approach.Leaf))
        {
            var polygon = new Polygon { Points = [.. shape.Outline] };

            polygon.Classes.Add(DockPointClass);
            polygon.Classes.Add(SideClass(shape.Side));
            Canvas.SetLeft(polygon, 0);
            Canvas.SetTop(polygon, 0);
            _zones.Children.Add(polygon);
            _points[shape.Side] = polygon;
            _shapes.Add((shape.Side, [.. shape.Outline]));
        }
    }

    /// <summary>
    /// The five points a dock can be put on in a group: four isosceles trapezoids along its edges around a centre
    /// rectangle, filling the group.
    /// </summary>
    /// <remarks>
    /// The group is the target, so the five points are the whole of it: each trapezoid's long side is one of the
    /// group's edges and its short side is the centre rectangle, so the shape itself says which way the dock goes,
    /// and every point of the group is on exactly one of them. A binary split has two children — a column split and
    /// a row split — so left and right are the two ways to split a group into columns and top and bottom are the two
    /// ways to split it into rows.
    /// <para>
    /// The trapezoids are a band of <see cref="DockPointEdge"/> along the edges and the centre is what they leave,
    /// which is why the centre keeps the group's shape rather than being a square: a wide group has a wide centre,
    /// and the middle of a group is the whole of what is not along its edges.
    /// </para>
    /// </remarks>
    /// <param name="leaf">The rectangle of the group, in the area's coordinates.</param>
    /// <returns>The five points, the centre first.</returns>
    internal static IReadOnlyList<DockPointShape> Shapes(Rect leaf)
    {
        var band = Math.Min(DockPointEdge, Math.Min(leaf.Width, leaf.Height) / 3);
        var inner = new Rect(
            leaf.X + band,
            leaf.Y + band,
            leaf.Width - (band * 2),
            leaf.Height - (band * 2)
        );

        return
        [
            new(DockSide.Center, Corners(inner)),
            new(
                DockSide.Left,
                [
                    new Point(leaf.X, leaf.Y),
                    new Point(inner.X, inner.Y),
                    new Point(inner.X, inner.Bottom),
                    new Point(leaf.X, leaf.Bottom),
                ]
            ),
            new(
                DockSide.Right,
                [
                    new Point(inner.Right, inner.Y),
                    new Point(leaf.Right, leaf.Y),
                    new Point(leaf.Right, leaf.Bottom),
                    new Point(inner.Right, inner.Bottom),
                ]
            ),
            new(
                DockSide.Top,
                [
                    new Point(leaf.X, leaf.Y),
                    new Point(leaf.Right, leaf.Y),
                    new Point(inner.Right, inner.Y),
                    new Point(inner.X, inner.Y),
                ]
            ),
            new(
                DockSide.Bottom,
                [
                    new Point(inner.X, inner.Bottom),
                    new Point(inner.Right, inner.Bottom),
                    new Point(leaf.Right, leaf.Bottom),
                    new Point(leaf.X, leaf.Bottom),
                ]
            ),
        ];
    }

    /// <summary>Whether a point is inside a shape, by the crossings of a ray drawn from it.</summary>
    /// <remarks>
    /// The five points are trapezoids and rectangles that meet along their edges, so a bounding box would say a
    /// point in a corner belongs to two of them at once. What a pointer on a seam means is whichever shape comes
    /// first in <see cref="Shapes"/>, which is the centre and then the four sides.
    /// </remarks>
    /// <param name="outline">The shape's corners, in order.</param>
    /// <param name="at">The point to ask about.</param>
    /// <returns>Whether the point is inside the shape.</returns>
    internal static bool Contains(IReadOnlyList<Point> outline, Point at)
    {
        var inside = false;

        for (int corner = 0, previous = outline.Count - 1; corner < outline.Count; previous = corner++)
        {
            var one = outline[corner];
            var other = outline[previous];

            if (
                (one.Y > at.Y) != (other.Y > at.Y)
                && at.X < (((other.X - one.X) * (at.Y - one.Y)) / (other.Y - one.Y)) + one.X
            )
            {
                inside = !inside;
            }
        }

        return inside;
    }

    /// <summary>The class that says what one of a group's points would do.</summary>
    /// <param name="side">What a drop there would do.</param>
    /// <returns>The class the point wears.</returns>
    private static string SideClass(DockSide side) =>
        side switch
        {
            DockSide.Center => DockPointCenterClass,
            DockSide.Left => DockPointLeftClass,
            DockSide.Right => DockPointRightClass,
            DockSide.Top => DockPointTopClass,
            _ => DockPointBottomClass,
        };

    /// <summary>The one point an area with no group drawn offers: the whole of it, as a tab into the group there is.</summary>
    /// <param name="leaf">The area's rectangle.</param>
    /// <returns>The point.</returns>
    private static IReadOnlyList<DockPointShape> Whole(Rect leaf) =>
        [new DockPointShape(DockSide.Center, Corners(leaf))];

    /// <summary>The four corners of a box, in drawing order.</summary>
    /// <param name="box">The box.</param>
    /// <returns>Its corners.</returns>
    private static Point[] Corners(Rect box) =>
        [
            new Point(box.X, box.Y),
            new Point(box.Right, box.Y),
            new Point(box.Right, box.Bottom),
            new Point(box.X, box.Bottom),
        ];

    /// <summary>Every group of the main window's tree.</summary>
    /// <returns>The groups, in document order.</returns>
    private LayoutGroup[] Groups() =>
        Layout.Root is null ? [] : [.. DockTree.Groups(Layout)];

    /// <summary>The docks of a group that this window has placed, in the tree's tab order.</summary>
    /// <param name="group">The group.</param>
    /// <returns>The instances, in tab order.</returns>
    private List<DockInstance> Docks(LayoutGroup group) =>
        [
            .. group
                .Docks.Select(dock => _manager.InstanceOf(dock.DockNameId, dock.Ordinal))
                .OfType<DockInstance>()
                .Where(Mine),
        ];

    /// <summary>Whether anything in a group is shown, and so whether the group is drawn at all.</summary>
    /// <param name="group">The group.</param>
    /// <returns>Whether it has a dock open in it.</returns>
    private bool Drawn(LayoutGroup group) => Docks(group).Exists(dock => dock.IsOpen);

    /// <summary>The main window's tree, which is what this area draws.</summary>
    private DockLayout Layout => _manager.Layout;

    /// <summary>A header: a small button that names, commands, or closes a dock.</summary>
    /// <remarks>
    /// What it names is centred in it. A tab is built where a tab is made rather than here, because what a tab
    /// holds is more than a name and its geometry is the strip's rather than a header's.
    /// </remarks>
    /// <param name="text">What the header names, which is what it says when it wears no picture.</param>
    /// <param name="picture">The picture it wears, or nothing.</param>
    /// <returns>The button.</returns>
    private static Button Header(string text, Bitmap? picture = null) =>
        new()
        {
            Content = picture is { } mark
                ? WindowControls.Glyph(mark, CommandMark)
                : new TextBlock
                {
                    Text = text,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                },
            Padding = picture is null ? new Thickness(7, 1) : new Thickness(6),
            VerticalAlignment = VerticalAlignment.Center,
        };
}

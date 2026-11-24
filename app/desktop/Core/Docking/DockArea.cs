using System.Diagnostics;
using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Data;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Menus;
using RorolalaDesktop.Theming;
using MenuItem = Avalonia.Controls.MenuItem;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The dock area: the tree of groups a dock can be placed in, the tabs that choose between the docks of one
/// group, and the dragging that moves a dock from one group to another, or into a window of its own.
/// </summary>
/// <remarks>
/// The area draws the main window's layout tree. Every leaf of it is a <see cref="Region"/>, one group of
/// tabs, and every split is a two-cell grid with a splitter between the cells, recursively, so any group may
/// be split again and the geometry follows the window rather than the other way round. Where a column seam
/// and a row seam cross, the two lines are drawn as one grip so that a single drag resizes every panel around
/// the crossing: the four of a two-by-two arrangement move together rather than a seam at a time.
/// <para>
/// A region keeps every dock it was given and shows one of them at a time, rather than being rebuilt when the
/// set of open docks changes. That is not an optimisation: a control has one parent, and a view moved from one
/// container into another can be caught mid-move, which is what <c>The control ... already has a visual
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
/// window it was picked up in. What the area answers a drag with depends on how much of its motion the look
/// draws. A level that draws travel answers it with the layout itself: the groups make room for the dock while
/// the pointer is on an answer, and nothing is drawn over them, since a second set of shapes would say the same
/// thing twice. A level that does not answers it with the five points a dock may be put on, four isosceles
/// trapezoids around a centre rectangle, one for each way the group may be split and one for joining its tabs,
/// because a layout that changes at once is a layout nothing was read from. The points are what a drop would do;
/// the release is what does it.
/// </para>
/// </remarks>
internal sealed class DockArea : UserControl
{
    /// <summary>
    /// The class a region's header strip carries, for a theme to address it by.
    /// </summary>
    /// <remarks>
    /// The shell marks the surfaces it owns so that a theme (this host's, or a plugin's) can style them
    /// without knowing what a dock is. The names are fixed, since a plugin's theme has to write them as
    /// literals.
    /// </remarks>
    public const string HeadersClass = "dock-headers";

    /// <summary>
    /// The class a region whose group holds one dock carries.
    /// </summary>
    /// <remarks>
    /// A strip of one tab is not a tab strip. The tab's fill and its divider are what tell a row of tabs
    /// apart, and a row of one has nothing to be told from, so the look draws the header as a title
    /// instead: the same row, the same drag area and the same commands, without the tab chrome that
    /// makes an empty strip look like an empty tab.
    /// </remarks>
    public const string SingleClass = "dock-single";

    /// <summary>The class a dock's own header carries.</summary>
    public const string TitleClass = "dock-title";

    /// <summary>
    /// The class the picture of the dock a tab names carries, where the dock registered one.
    /// </summary>
    /// <remarks>
    /// A mark rather than a word, and marked for a theme to address it by for the same reason a tab is: what a dock
    /// is is drawn in a strip, and what says it is the picture the plugin registered rather than one this file chose.
    /// A dock with no picture registers none, and its tab carries nothing here.
    /// </remarks>
    public const string IconClass = "dock-title-icon";

    /// <summary>
    /// The class the first header a region is showing carries.
    /// </summary>
    /// <remarks>
    /// The edge a tab strip is drawn with is the one between two tabs, so the first tab has none on its left,
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
    /// is the last, and a strip of one tab is a row with no edge on its left and this one on its right.
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

    /// <summary>The class the grip where a column splitter and a row splitter cross carries.</summary>
    /// <remarks>
    /// It is a grip of its own because a drag on it is about two splits at once: what a pointer on it does is
    /// the four panels around the crossing, rather than the two a single seam divides. The grip draws nothing:
    /// the two seams already meet there, and a second cross over them is a mark the pointer has to be told is
    /// a handle.
    /// </remarks>
    public const string CrossClass = "dock-cross";

    /// <summary>
    /// The class one of the dock points a dragged dock can be put on carries.
    /// </summary>
    /// <remarks>
    /// The answer to a drag is the points themselves and nothing else: they fill the group, so the group around them
    /// is not outlined or washed, a second box over the same rectangle would be a second shape saying the same
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

    /// <summary>
    /// The class the room a dragged dock would take carries, while the layout is showing it.
    /// </summary>
    /// <remarks>
    /// The one surface a drag draws while the groups are making room: the gap the dock would land in, washed with
    /// the accent so that what is opened reads as the dock's place rather than as a hole in the arrangement. What
    /// it is drawn on is a shape of the area's own, since the group that will fill it does not exist yet.
    /// </remarks>
    public const string IncomingClass = "dock-incoming";

    /// <summary>How many characters of a dock's name a tab says before its front is left out.</summary>
    /// <remarks>
    /// A dock named by where it is (a browser, whose name is a path) would otherwise make a tab as long as the path
    /// and take the strip from every dock beside it. What is kept is the tail, because a path's last segments are the
    /// place and its first are only the drive it begins on.
    /// </remarks>
    internal const int TabWordLength = 42;

    /// <summary>How wide a tab's word may grow, which is the strip's safeguard where characters and pixels differ.</summary>
    internal const double TabWordWidth = 240;

    /// <summary>The class the area of a window that is one dock carries.</summary>
    /// <remarks>
    /// What a window of one dock is drawn as: the dock's own content under the window's row, with none of the
    /// chrome a region has, no strip, no tabs, no splitter, no commands. The class is how the look knows to
    /// leave the strip's ground and line off, since a surface that is not there is not drawn.
    /// </remarks>
    public const string AloneClass = "dock-area-alone";

    /// <summary>
    /// The height of a region's header strip, which its tabs sit in.
    /// </summary>
    /// <remarks>
    /// The strip, the menu bar and a toolbar are one height, so that the window reads as a stack of bands of
    /// the same weight. A window of one dock has no strip at all, what it draws is the dock itself, under the
    /// window's own row, so its row is the menu bar's height and this one
    /// (<see cref="MainWindow.TitleBarHeight"/>) is not drawn there.
    /// </remarks>
    private const double StripHeight = 32;

    /// <summary>
    /// How thick the band of a group is that the four edge points take, in pixels.
    /// </summary>
    /// <remarks>
    /// The five points fill the group: the four trapezoids are the band along its edges and the centre rectangle is
    /// what is left, so a group bigger than its band is a rectangle inside a ring of four answers. A band rather than
    /// a share because what a pointer has to reach is a distance rather than a proportion, the edge of a large group
    /// is as far from its middle as the edge of any other, and because a centre that keeps the group's own shape is
    /// the "among these tabs" target rather than a place of its own.
    /// <para>
    /// A group too small to give its centre a third of itself shrinks the band with it, so the centre is never lost:
    /// the four edges need most of a small group, and the one answer that says "among these tabs" is the one it must
    /// not lose.
    /// </para>
    /// </remarks>
    private const double DockPointEdge = 120;

    /// <summary>
    /// How far inside a group a drop is aimed, in pixels.
    /// </summary>
    /// <remarks>
    /// A distance rather than a share of the group: what keeps a group's own edge from answering for it is a band of
    /// the pointer's own scale, and the edge of a large group is as near the group beside it as the edge of a small
    /// one. A group too narrow to give that much keeps a quarter of its shorter side instead, so that the aim is
    /// never so small that a drop cannot be aimed at all.
    /// </remarks>
    private const double DockPointPadding = 24;

    /// <summary>How near the pane's own edge a drag has to be to dock against the whole window edge.</summary>
    private const double EdgeReach = 28;

    /// <summary>How large a command's picture is in a strip, which is the size of the words beside it.</summary>
    private const double CommandMark = 14.0;

    /// <summary>How wide a splitter is.</summary>
    /// <remarks>
    /// Published, because a theme draws a line through the middle of one: the width it is drawn in is this,
    /// and a line of another width would not be centred in it. It is the room a hand has to take hold of rather
    /// than the room the line takes (the line is one pixel of it, and the rest is the ground either side) so
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

    /// <summary>
    /// The panel face behind every region, which is what the docks are read on.
    /// </summary>
    /// <remarks>
    /// One face under the whole area rather than a fill on each region. A region that painted its own
    /// background became the element under the pointer, and a drag over a dock then never reached the
    /// area that answers for a drop: a background is a surface to the hit test, and the area is the
    /// element that has to stay the target. The face takes no pointer, so the area keeps it.
    /// </remarks>
    private readonly Border _ground = new() { IsHitTestVisible = false };

    /// <summary>The tree drawn: one nested grid per split, one region per group.</summary>
    private readonly Grid _structure = new();

    /// <summary>
    /// What a dragged dock is shown over the area: the group it has come to, and the points it may be put on.
    /// </summary>
    /// <remarks>
    /// A canvas rather than a grid, because the dock points are trapezoids rather than boxes: a shape is drawn
    /// where its own outline says, and where that is changes with the size of the group under the pointer.
    /// <para>
    /// Drawn only where the look does not draw travel, since a layout that moves says where a drop would land by
    /// moving, and shapes over the rooms it is making would be the same answer twice.
    /// </para>
    /// </remarks>
    private readonly Canvas _zones = new();

    /// <summary>
    /// The layer the regions are taken into while a drag makes room for itself.
    /// </summary>
    /// <remarks>
    /// The tree the layout is drawn as says how big each group is by star sizes, which are a share of the room and
    /// not a place: moving one is a new layout of the tree, and the split a drop opens does not exist in the tree
    /// at all. The regions are therefore taken out of the tree for as long as a drag is showing where it would land
    /// and placed here, where a place and a size are numbers that can be eased.
    /// <para>
    /// It takes the pointer only while it is arranging, and only for the reason it has a face at all then: a drag
    /// is answered by whatever is under it bubbling to this area, so a layer that let the pointer through in the
    /// room between two regions would be a hole in the middle of a drag. The rest of the time it draws nothing and
    /// has no face, since a transparent one would be a layer over every tab that is not this layer's to take.
    /// </para>
    /// </remarks>
    private readonly Canvas _arrange = new();

    /// <summary>The room a dragged dock would take, drawn while the regions are making room for it.</summary>
    private readonly Rectangle _incoming = new() { IsVisible = false, Classes = { IncomingClass } };

    /// <summary>What a region moves with while the layout is arranged, which is the drag itself and nothing else.</summary>
    private readonly Transitions _arrangeTravel = new()
    {
        Motion.Travel(Canvas.LeftProperty, Motion.Panel),
        Motion.Travel(Canvas.TopProperty, Motion.Panel),
        Motion.Travel(Layoutable.WidthProperty, Motion.Panel),
        Motion.Travel(Layoutable.HeightProperty, Motion.Panel),
        Motion.Fade(Visual.OpacityProperty, Motion.State),
    };

    /// <summary>What the area is arranged to show, or nothing while it is drawn as the layout stands.</summary>
    private Arrangement? _arranged;

    /// <summary>
    /// The rectangles the groups stand in while a drag is being answered, taken once and kept.
    /// </summary>
    /// <remarks>
    /// The answers a drag can land on are fixed for as long as the layout is, which is the whole of the gesture: a
    /// room opening under the pointer moves the groups on screen, and it must not move what the pointer is aiming
    /// at, or the answer under it would change as the answer was being shown. They are taken when the drag first
    /// reaches the area and dropped when the layout is drawn again, which is the only thing that may move them.
    /// </remarks>
    private Dictionary<string, Rect>? _aim;

    /// <summary>The way back to the layout as it stands, while the regions are returning to it.</summary>
    private DispatcherTimer? _recede;

    /// <summary>What the drop points arrive with, and nothing they leave with: the answer to a drag is transient.</summary>
    private readonly Transitions _zoneFade = new()
    {
        Motion.Fade(Visual.OpacityProperty, Motion.Feedback),
    };

    /// <summary>The scale a carried dock is drawn at, which is what says it has been lifted off its region.</summary>
    private const double LiftScale = 0.7;

    /// <summary>The dock being carried, drawn from a picture taken when the drag began.</summary>
    private readonly Border _lifted = new() { IsHitTestVisible = false, IsVisible = false };

    /// <summary>The layer the carried dock is drawn in, over the points.</summary>
    private readonly Canvas _liftLayer = new() { IsHitTestVisible = false, IsVisible = false };

    /// <summary>The picture of the dock the lifted frame holds.</summary>
    private readonly Image _carried = new() { Stretch = Stretch.Fill };

    /// <summary>How the carried dock is scaled, from whole to lifted.</summary>
    private readonly ScaleTransform _liftScale = new();

    /// <summary>How the carried dock grows into its lift, which is the pickup the pointer asks for.</summary>
    private readonly Transitions _liftGrow = new()
    {
        Motion.Travel(ScaleTransform.ScaleXProperty, Motion.Feedback),
        Motion.Travel(ScaleTransform.ScaleYProperty, Motion.Feedback),
    };

    /// <summary>The content flight of a dropped dock, while it is running.</summary>
    private DispatcherTimer? _flight;

    /// <summary>
    /// How many frames a landed dock waits for the layout to give its region a room before the flight is given up.
    /// </summary>
    /// <remarks>
    /// One frame is the usual answer, since the layout that places a region runs before the next tick. The rest is
    /// for a window that is not drawing, and the end is a flight given up rather than one aimed at a stale place.
    /// </remarks>
    private const int ArrivalWaits = 6;

    /// <summary>The region whose content is waiting under a flight, while it runs.</summary>
    private Region? _flying;

    /// <summary>
    /// Whether the carried dock has been drawn for the drag in flight, which is what its pickup is played on.
    /// </summary>
    private bool _picked;

    /// <summary>What the drag in flight is over, or nothing while it is over no group.</summary>
    private Approach? _approaching;

    /// <summary>The points drawn for the group the drag is over, by what each of them would do.</summary>
    private readonly Dictionary<DockSide, Polygon> _points = [];

    /// <summary>The outlines of the points drawn, which is what hit tests the pointer against.</summary>
    private readonly List<(DockSide Side, Point[] Outline)> _shapes = [];

    /// <summary>
    /// The dock a window of one dock has already had the pointer and keyboard handlers put on.
    /// </summary>
    /// <remarks>
    /// A window of its own does not put its dock through a region, so the wiring a region's docks get in
    /// <see cref="Make"/> is done for it here, once, since the handlers would otherwise be added again on every
    /// rebuild of a window that draws the same dock.
    /// </remarks>
    private DockInstance? _wired;

    /// <summary>What each dock that is placed in a region was given, by the dock.</summary>
    private readonly Dictionary<DockInstance, Placed> _placed = [];

    /// <summary>The dock this area is keeping its place empty for, while the pointer carries it.</summary>
    private DockInstance? _carriedDock;

    /// <summary>The region that dock was taken out of, which is the one left empty while it is carried.</summary>
    private Region? _sourceRegion;

    /// <summary>The regions, by the id of the group each draws.</summary>
    private readonly Dictionary<string, Region> _regions = new(StringComparer.Ordinal);

    /// <summary>The splitters the structure currently holds, for deciding which of them the window can honour.</summary>
    private readonly List<SplitPart> _splits = [];

    /// <summary>How large the grip where two seams cross is, and how far around the crossing it reaches.</summary>
    private const double CrossSize = 11;

    /// <summary>The layers the crossing grips are drawn in, over the structure and under a dock drag.</summary>
    private readonly Canvas _crossLayer = new();

    /// <summary>The crossing grip the pointer is dragging, if it is on one.</summary>
    private CrossDrag? _crossDrag;

    /// <summary>The crossings last placed, so a layout pass that moved none of them places none again.</summary>
    private IReadOnlyList<CrossSpot> _crossSpots = [];

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
    /// Held rather than asked of the toolkit, because attaching and detaching arrive more than once, a dock
    /// dragged to another group is taken off the tree and put back, and a subscription made twice would be a
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

        // A region is a panel, and a window of one dock is the same panel without a strip. The face is
        // under the whole area rather than on each region, so that the area remains the element a drop
        // reaches. The window's ground is behind the face, which is where it shows at the edge.
        _ground[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);
        _alone[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);

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

        // A dock lands in a group, so a window that has groups takes a drag of one, and a window that is one
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

        // The crossings are where the seams are, which is only known once the layout has given them their
        // room: a grip put where a seam was before a resize would not be on the seam it resizes.
        LayoutUpdated += (_, _) => Crosses();
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
    /// dock's control (in a region of it, or in the border a window of one dock draws its dock in) and a
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

        StopFlight();
        HideLifted();
        Reset();

        if (!_listening)
        {
            return;
        }

        _listening = false;
        _manager.Changed -= Rebuild;
        _manager.Renamed -= Retitled;
        DockDrag.LiftedChanged -= Lifted;
        _windows.DragEnded -= Clear;
        _carriedDock = null;
        _sourceRegion = null;

        // A window of one dock places nothing: what it draws is the one dock it was made for, put straight into
        // the border that holds it, and the letting go of that is here rather than in the loop below because it
        // is not a placement, there is no group to take it out of.
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
        _manager.Renamed += Retitled;
        DockDrag.LiftedChanged += Lifted;

        // A drag that ends by being called off (Esc, or a release nowhere) leaves this area with the points it
        // drew and no event left to take them down, since a cancelled drag has no drop to be told about.
        _windows.DragEnded += Clear;

        // The dock a drag already has in the air is this area's to keep empty, if it was picked up here. The
        // rebuild below places it, and Show asks this field before it draws it.
        _carriedDock = null;
        _sourceRegion = null;

        // A dock may be in this window, in another one, or in none: what makes an instance this window's is the
        // window it names, which is not a question about the control. So the layer is brought in step as soon
        // as there is one, so that a dock put into an area that is not on a tree yet, the main window while the
        // layout is being restored, a window of a dock's own before it is shown, still lands somewhere, and the
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

        /// <summary>The word in the header, which is what a dock's name is drawn as.</summary>
        public required TextBlock Name { get; init; }

        /// <summary>
        /// The heading at the top of the tab's menu, which says which dock the menu is about.
        /// </summary>
        /// <remarks>
        /// One entry and not the menu, because the rest of it does not change with the dock's name: a menu of actions
        /// is about whatever the dock is, and only the line naming it goes stale when the dock is renamed.
        /// </remarks>
        public required MenuItem Heading { get; init; }
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

            Content.Transitions = Fade;
        }

        /// <summary>The id of the group this region draws.</summary>
        public string Id { get; }

        /// <summary>What the content arrives with when the tab it shows changes.</summary>
        public Transitions Fade { get; } =
            new() { Motion.Fade(Visual.OpacityProperty, Motion.State) };

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

        /// <summary>
        /// Gives up the controls the strip was holding, so that the dock that drew them can carry them elsewhere.
        /// </summary>
        /// <remarks>
        /// What a strip holds is a plugin's own control, and one control has one parent. A region that kept it
        /// after the dock left would hold the only place it can be, and the region the dock went to would be
        /// refused it: the buttons would be gone with the dock, and asking for them again would throw.
        /// </remarks>
        public void Unbind()
        {
            Shown = null;
            Commands.Children.Clear();
        }

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

    /// <summary>One place two seams cross, and the grips a drag there resizes.</summary>
    /// <param name="Center">Where the crossing stands, in the area's coordinates.</param>
    /// <param name="Splits">The splitters whose seams meet there, one per axis.</param>
    private sealed record CrossSpot(Point Center, IReadOnlyList<SplitPart> Splits);

    /// <summary>One splitter a crossing drag is moving, with the share it had when the drag began.</summary>
    /// <param name="Split">The splitter.</param>
    /// <param name="Ratio">The share it had, which the drag is measured from.</param>
    /// <param name="Room">The room its two children shared, which is what a pixel of the drag is a share of.</param>
    private sealed class CrossMoved(SplitPart split, double ratio, double room)
    {
        /// <summary>The splitter.</summary>
        public SplitPart Split { get; } = split;

        /// <summary>The share it had when the drag began, which every move is measured from.</summary>
        public double Start { get; } = ratio;

        /// <summary>The share it stands at now.</summary>
        public double Ratio { get; set; } = ratio;

        /// <summary>The room its two children share.</summary>
        public double Room { get; } = room;
    }

    /// <summary>A drag on a crossing grip: what it is moving, and where it holds the grip.</summary>
    /// <param name="Handle">The grip the pointer went down on.</param>
    /// <param name="From">Where the pointer went down.</param>
    /// <param name="Splits">The splitters the drag moves, from the shares the drag began at.</param>
    /// <param name="Left">Where the grip stood, in the layer's coordinates.</param>
    /// <param name="Top">Where the grip stood, in the layer's coordinates.</param>
    private sealed record CrossDrag(
        Panel Handle,
        Point From,
        IReadOnlyList<CrossMoved> Splits,
        double Left,
        double Top
    );

    /// <summary>
    /// What a dragged dock has come to: the group under the pointer, or the whole area when there is no group
    /// to be over.
    /// </summary>
    /// <remarks>
    /// The first of the two decisions a drop makes. A group is <em>approached</em> as soon as the pointer is
    /// anywhere in it, the whole of the leaf is the near zone, and what the drag then offers is where in that
    /// group the dock would go. A root holding nothing, or one whose docks are all hidden, is drawn as no group
    /// at all, so the whole area is approached instead: a dock put there makes the group or joins the active
    /// one, since there is nothing to be beside.
    /// </remarks>
    /// <param name="Leaf">The rectangle the group is drawn in, or the whole area when there is no group.</param>
    /// <param name="Group">The group under the pointer, or nothing when there is none to be over.</param>
    private readonly record struct Approach(Rect Leaf, LayoutGroup? Group, DockSide? Edge = null);

    /// <summary>One point of the cluster: what it would do, and the outline that hit tests the pointer.</summary>
    /// <param name="Side">What a drop on it would do.</param>
    /// <param name="Outline">The outline, in the area's coordinates.</param>
    internal readonly record struct DockPointShape(DockSide Side, IReadOnlyList<Point> Outline);

    /// <summary>
    /// What a drag would make of the layout: the tree the drop would leave, and the room the dock would take.
    /// </summary>
    /// <remarks>
    /// The tree is the live layout with the dock taken out of where it is and put where the drop names (see
    /// <see cref="DockManager.Ahead"/>), and the room is the group that stands for the dock in it. What is kept
    /// besides is the split that has to open to make the room, and the share it has while the room is closed: a
    /// preview of a split that is already at its share would jump to it rather than open into it.
    /// </remarks>
    /// <param name="Root">The tree, as the drop would leave it.</param>
    /// <param name="Incoming">The group in that tree that stands for the dock being carried.</param>
    /// <param name="Opening">The split that opens to make room for it.</param>
    /// <param name="From">The share <paramref name="Opening"/> has while the room is closed.</param>
    /// <param name="Target">The group the drop is on, or nothing for the edge of the whole area.</param>
    /// <param name="Side">The side the drop is on.</param>
    private sealed record Arrangement(
        LayoutNode Root,
        string Incoming,
        LayoutSplit Opening,
        double From,
        string? Target,
        DockSide Side
    );

    /// <summary>Lays the area out once: the structure to draw the tree in, and the boxes over it.</summary>
    private void Build()
    {
        // A window of one dock has no groups to lay out: what it is drawn as is the dock itself, under the
        // window's own row, and everything below this is about putting one view beside another, a strip to
        // choose between them, a splitter to resize one against the other, and boxes to drop a third into. None
        // of that has anything to say in a window that holds one dock, and drawing it would be chrome around a
        // view that nothing can be put beside.
        if (Alone)
        {
            Classes.Add(AloneClass);

            // A window of one dock has nothing else to draw: what it draws is the dock itself, in the room
            // the window gave it.
            Content = new Grid { Children = { _ground, _alone } };

            return;
        }

        _zones.IsHitTestVisible = false;
        _zones.IsVisible = false;
        _zones.Transitions = _zoneFade;

        _surface.Children.Add(_structure);

        // The arrangement stands over the tree the regions were taken out of, which stays under it: the seams the
        // splitters sit on, and the crossings read from them, are the tree's and not the arrangement's, and the
        // tree keeps its shape while the regions are out of it.
        _arrange.Children.Add(_incoming);
        _surface.Children.Add(_arrange);

        // The crossing grips stand over the docks, under the points a dragged dock may be put on: a drag that
        // is looking for a place to land is about the docks, and the seams are not something to catch.
        _surface.Children.Add(_crossLayer);
        _surface.Children.Add(_zones);

        // The carried dock stands over the points rather than under them: it is what the hand is holding, and the
        // points are what it could be put on. The picture is a snapshot taken when the drag began, so the frame
        // is a picture of the dock and not a second live copy of it.
        _lifted.Child = _carried;
        _lifted.RenderTransform = _liftScale;
        _lifted[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);
        _lifted[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);
        _lifted[!Border.BoxShadowProperty] = new DynamicResourceExtension(PaletteKeys.Shadow);
        _lifted.BorderThickness = new Thickness(1);
        _lifted.CornerRadius = new CornerRadius(8);
        _lifted.ClipToBounds = true;
        _liftLayer.Children.Add(_lifted);
        _surface.Children.Add(_liftLayer);

        Content = new Grid { Children = { _ground, _surface } };
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
    /// there is. A dock that is closed is drawn too, and hidden, the same as a region does with a dock that is
    /// not the one it is showing, so that opening it again is showing the control that is already there rather
    /// than one that has to be put back.
    /// </remarks>
    private void Only()
    {
        var shown = _manager.Instances.FirstOrDefault(Mine);

        if (!ReferenceEquals(_alone.Child, shown?.View.View))
        {
            _alone.Child = shown?.View.View;
            _wired = null;
        }

        if (shown is not null)
        {
            shown.View.View.IsVisible = shown.IsOpen && !ReferenceEquals(shown, _carriedDock);

            // The one dock of a window of its own is worked in on the same two moments as any dock, a press
            // landing in it and the keyboard reaching it, and there is no region here to have done it (see
            // `Make`). Wired once: a rebuild of the same window draws the same dock, and a handler added again
            // would say the same thing twice for the life of the window.
            if (!ReferenceEquals(_wired, shown))
            {
                _wired = shown;

                shown.View.View.GotFocus += (_, _) => _manager.WorkedIn(shown);
                shown.View.View.AddHandler(
                    InputElement.PointerPressedEvent,
                    (_, _) => _manager.WorkedIn(shown),
                    RoutingStrategies.Tunnel
                );
            }
        }
    }

    /// <summary>
    /// Says a dock's name again, in its tab and at the top of the tab's menu.
    /// </summary>
    /// <remarks>
    /// What a dock that names itself by where it is asks for when it goes somewhere else. Nothing is rebuilt: the dock
    /// is the same dock, and what changed is a word, a tip and the line a menu opens with.
    /// </remarks>
    /// <param name="instance">The dock whose name changed.</param>
    private void Retitled(DockInstance instance)
    {
        if (!_placed.TryGetValue(instance, out var placed))
        {
            return;
        }

        placed.Name.Text = TabTitle(instance.Title);
        ToolTip.SetTip(placed.Title, instance.Detail);
        placed.Heading.Header = instance.Detail;
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
    /// gone takes its region with it, and the docks that were in it were detached in <see cref="Forget"/>,
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

            // A region that is gone is never bound again, so the strip it is holding has to be given up here or
            // the control stays parented to a region nothing can reach.
            region.Unbind();

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
        // The room the tab keeps either side of its name: one number, because the close is a strip of the tab's own
        // right side, that room wide and as tall as the tab, and the name keeps as much again on its right so that it
        // stays in the middle of the tab rather than in the middle of what the close left of it. It has to be room
        // enough for the cross and the room the cross keeps from the tab's end both, or the cross is pressed against
        // the name it is drawn beside.
        const double Room = 26.0;

        // What the left of a tab is made of where it carries the dock's picture: the room between the tab's edge and the
        // picture, the picture, and the room between the picture and the word. Only the first is the room widened for the
        // picture, the word keeps as much from the picture as it kept from the tab's edge before there was one, and the
        // whole is kept whether or not the dock has a picture so that two tabs side by side begin their words in the same
        // place.
        const double PictureRoom = 18.0;
        const double PictureGap = 6.0;
        const double LeadingRoom = PictureRoom + Marks.TabSize + PictureGap;

        var name = new TextBlock
        {
            Text = TabTitle(instance.Title),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.NoWrap,
            // The front is what gives way, for the reason `TabTitle` gives: a name that is a path ends at the place.
            TextTrimming = TextTrimming.LeadingCharacterEllipsis,
            MaxWidth = TabWordWidth,
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
        // sits in answers one of its own by selecting the dock, so closing a dock would also select it, and the
        // manager would be asked to show what it has just been asked to close.
        close.Click += (_, e) =>
        {
            e.Handled = true;
            _manager.Close(instance);
        };

        // The dock's own picture, where the dock registered one: read from the library the Window menu reads its marks
        // from, and stamped in the ink of the tab it sits in rather than in a colour of its own.
        var picture = instance.Registration.IconKey is { Length: > 0 } key
            ? _windows.Pictures.Find(key)
            : null;

        // Three columns: the room on the left, the name, and the close itself. The first holds the dock's picture where
        // it has one and is empty where it has not, a box of two would centre the name in what is left of the close
        // instead of in the tab, which moves the title every time the pointer crosses it, and the picture stands at the
        // tab's own edge in that room's first part, with only a little between it and the word that follows.
        var box = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions($"{LeadingRoom},*,{Room}"),
            RowDefinitions = new RowDefinitions("*"),
        };

        Grid.SetColumn(name, 1);
        Grid.SetColumn(close, 2);

        if (picture is not null)
        {
            var mark = Marks.Tab(picture, IconClass);
            mark.HorizontalAlignment = HorizontalAlignment.Left;
            mark.Margin = new Thickness(PictureRoom, 0, 0, 0);

            box.Children.Add(mark);
        }

        box.Children.Add(name);
        box.Children.Add(close);

        // The tab's own geometry, where the generic header's is for a command: a tab fills the strip it sits in
        // (no padding, no margin, as tall as the strip) because the tabs *are* the strip. What it holds is
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

        // The whole of what the tab is showing where the pointer rests on it: a tab's word is shortened to keep the
        // strip, and what it was shortened from is the one thing a reader who has stopped at it wants.
        ToolTip.SetTip(title, instance.Detail);

        // The menu a tab opens, made once here rather than per right-click: what a dock's tab offers is about
        // the dock rather than about what the pointer happens to be over, so nothing about it is stale when the
        // menu is opened again.
        var (tabMenu, heading) = TabMenu(instance);
        title.ContextMenu = tabMenu;

        // Taken on the way down. A button answers for its own press, and marks it handled while doing so, so an
        // ordinary handler here would never be called: the press is over before it reaches one. A tunnelling
        // handler is reached first, which is the only way in, and a press on the close is the close's rather
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
            Name = name,
            Heading = heading,
        };

        // A dock that the keyboard comes to is the dock being worked in, which is what says where the next dock
        // opened without a place of its own goes. Told to the manager rather than kept here, because it is part
        // of the arrangement and is written down with it.
        placed.View.GotFocus += (_, _) =>
        {
            _manager.ActivateGroup(placed.Region.Id);
            _manager.WorkedIn(placed.Instance);
        };

        // A press anywhere in the dock is the user working in it, whether or not what was pressed is something that
        // takes the keyboard: a view of a plugin's that is not focusable is still a dock being read.
        placed.View.AddHandler(
            InputElement.PointerPressedEvent,
            (_, _) => _manager.WorkedIn(placed.Instance),
            RoutingStrategies.Tunnel
        );

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
    /// Above them all is the dock's name, which is what says which dock the rest of the menu is about: a tab's word
    /// is shortened to keep the strip, and a menu is where the whole of where the dock is can be read. It is a word
    /// rather than a command, so it is drawn unable to be pressed, and it is returned so that a dock renamed while its
    /// menu is made can have it said again.
    /// </para>
    /// <para>
    /// A registered item is handed the one contextual shape the program has, with the dock's stable name id
    /// standing where a directory stands for a file menu and no entries at all: a tab is a dock, and stating the
    /// dock in the shape an item already takes is what lets one registration serve both without a second
    /// contract type to keep in step.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock whose tab this is.</param>
    /// <returns>The menu, which a tab always has, and the line in it that names the dock.</returns>
    private (ContextMenu Menu, MenuItem Heading) TabMenu(DockInstance instance)
    {
        var menu = new ContextMenu();
        var heading = new MenuItem { Header = instance.Detail, IsEnabled = false };
        var was = new ContextTarget(instance.DockNameId, []);

        menu.Items.Add(heading);
        menu.Items.Add(new Separator());

        var renderer = new MenuRenderer(_i18n.Get, MenuIcon);
        var leaves = _windows
            .DockMenuItems(DockMenuTarget.Tab, instance.DockNameId)
            .Select(item => new MenuLeaf(
                item.Keys,
                item.Order,
                () => item.Command(was),
                item.IconKey,
                item.Force,
                item.Applies is null ? null : () => item.Applies(was),
                item.Enabled is null ? null : () => item.Enabled(was)
            ))
            .ToArray();

        foreach (var entry in renderer.Build(leaves, force: false))
        {
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

        return (menu, heading);
    }

    /// <summary>What an icon key is drawn as in a dock's own menu, or nothing when the library holds none.</summary>
    /// <param name="key">The key naming the picture.</param>
    /// <returns>The picture, or nothing.</returns>
    private Control? MenuIcon(string key) =>
        _windows.Pictures.Find(key) is { } picture ? Marks.Menu(picture) : null;

    /// <summary>Whether a press landed on the close a header carries.</summary>
    /// <remarks>
    /// Walked rather than compared, because what the pointer hits is a part of the button's own template rather
    /// than the button, the same reason the tab's own press is taken on the way down.
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

        // The strip is the shown dock's, so it goes with it: the control a plugin draws is one instance, and a
        // region that kept it would hold the one parent it may have when the dock is placed in another.
        if (ReferenceEquals(placed.Region.Shown, placed.Instance))
        {
            placed.Region.Unbind();
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
        // A room a drag has opened outlives the tree being drawn again: a layout that changes while the pointer is
        // still holding a dock is the docks moving and not the answer, so what is arranged is put back where it was
        // without being played again.
        var arranged = _arranged;

        Reset();

        // The answers the pointer aims at are taken again from the tree that is drawn, which is the one thing that
        // may move them: a room opening under the pointer is not part of the layout.
        _aim = null;

        foreach (var region in _regions.Values)
        {
            if (region.Panel.Parent is Panel parent)
            {
                parent.Children.Remove(region.Panel);
            }
        }

        _structure.Children.Clear();
        _splits.Clear();
        _crossLayer.Children.Clear();
        _crossSpots = [];

        if (Layout.Root is null)
        {
            return;
        }

        if (Draw(Layout.Root) is { } root)
        {
            _structure.Children.Add(root);
        }

        if (arranged is not null)
        {
            _arranged = arranged;
            Restage();
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
        // to be drawn, and hit, above them rather than under the one that follows it in the grid.
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
        // is drawn over the docks themselves rather than as a band of the window's ground between them, a band
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
    /// Places a grip wherever a column seam and a row seam meet, so one drag resizes the panels around it.
    /// </summary>
    /// <remarks>
    /// Every crossing is a column split and a row split whose grips overlap once the layout has given them their
    /// room: a grip is wider than its seam and hangs over both sides, so a point where the two are over one
    /// another is a place the pointer can be told is the four panels rather than one seam. A layout where two
    /// row splitters meet one column splitter a few pixels apart, a two-by-two grid, where the two sides may not
    /// stand at the same ratio, is one crossing rather than two, since the grips overlap and the drag is about
    /// the same four panels.
    /// </remarks>
    private void Crosses()
    {
        if (_crossDrag is not null)
        {
            // While a grip is being dragged the seams move under it, and the controls must not be thrown
            // away under the pointer that captured one.
            return;
        }

        var spots = new List<(Point Center, List<SplitPart> Splits)>();
        var columns = _splits.Where(part => part.Split.Axis == DockAxis.Columns).ToList();
        var rows = _splits.Where(part => part.Split.Axis == DockAxis.Rows).ToList();

        foreach (var column in columns)
        {
            var down = BoundsOf(column.Handle);

            foreach (var row in rows)
            {
                var at = down.Intersect(BoundsOf(row.Handle));

                if (at.Width <= 0.5 || at.Height <= 0.5)
                {
                    continue;
                }

                var center = at.Center;
                var near = spots.FindIndex(spot =>
                    Math.Abs(spot.Center.X - center.X) <= CrossSize
                    && Math.Abs(spot.Center.Y - center.Y) <= CrossSize
                );

                if (near < 0)
                {
                    spots.Add((center, [column, row]));

                    continue;
                }

                var (known, splits) = spots[near];

                foreach (var split in new[] { column, row })
                {
                    if (!splits.Contains(split))
                    {
                        splits.Add(split);
                    }
                }

                spots[near] = (known, splits);
            }
        }

        // What is placed is rebuilt when the crossings move, which is what a splitter drag, a resize or a
        // rearrangement makes them do. A layout pass that moved none of them is the common case and must not
        // touch the controls: adding them back would be another layout pass, and that one another.
        if (SameCrossings(spots))
        {
            return;
        }

        _crossSpots = [.. spots.Select(spot => new CrossSpot(spot.Center, spot.Splits))];
        _crossLayer.Children.Clear();

        foreach (var (center, splits) in spots)
        {
            var handle = Cross(center, splits);
            Canvas.SetLeft(handle, center.X - (CrossSize / 2));
            Canvas.SetTop(handle, center.Y - (CrossSize / 2));
            _crossLayer.Children.Add(handle);
        }
    }

    /// <summary>Whether the crossings placed are the ones the layout has now, at the places it has them.</summary>
    /// <param name="spots">The crossings the layout has now.</param>
    /// <returns>Whether they are the ones already drawn.</returns>
    private bool SameCrossings(List<(Point Center, List<SplitPart> Splits)> spots)
    {
        if (spots.Count != _crossSpots.Count)
        {
            return false;
        }

        for (var at = 0; at < spots.Count; at++)
        {
            var (center, splits) = spots[at];
            var drawn = _crossSpots[at];

            if (
                Math.Abs(center.X - drawn.Center.X) > 0.25
                || Math.Abs(center.Y - drawn.Center.Y) > 0.25
                || !splits.SequenceEqual(drawn.Splits)
            )
            {
                return false;
            }
        }

        return true;
    }

    /// <summary>The grip where two seams cross, which carries the drag for both of them.</summary>
    /// <param name="center">Where the crossing stands.</param>
    /// <param name="splits">The splitters whose seams meet there.</param>
    /// <returns>The grip.</returns>
    private Panel Cross(Point center, IReadOnlyList<SplitPart> splits)
    {
        var grip = new Panel
        {
            Width = CrossSize,
            Height = CrossSize,
            Background = Brushes.Transparent,
            Cursor = new Cursor(StandardCursorType.SizeAll),
        };

        grip.Classes.Add(CrossClass);

        grip.PointerPressed += (_, e) => BeginCross(center, splits, grip, e);
        grip.PointerMoved += MovedCross;
        grip.PointerReleased += EndedCross;

        return grip;
    }

    /// <summary>Starts a drag on a crossing, from the shares the splitters stand at.</summary>
    /// <param name="center">Where the crossing stands.</param>
    /// <param name="splits">The splitters whose seams meet there.</param>
    /// <param name="grip">The grip the pointer went down on.</param>
    /// <param name="e">What the pointer did.</param>
    private void BeginCross(
        Point center,
        IReadOnlyList<SplitPart> splits,
        Panel grip,
        PointerPressedEventArgs e
    )
    {
        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            return;
        }

        var moved = new List<CrossMoved>(splits.Count);

        foreach (var split in splits)
        {
            var columns = split.Split.Axis == DockAxis.Columns;
            var first = columns ? split.First.Bounds.Width : split.First.Bounds.Height;
            var second = columns ? split.Second.Bounds.Width : split.Second.Bounds.Height;
            var room = first + second;

            if (room <= 0)
            {
                continue;
            }

            moved.Add(new CrossMoved(split, first / room, room));
        }

        if (moved.Count == 0)
        {
            return;
        }

        _crossDrag = new CrossDrag(
            grip,
            e.GetPosition(this),
            moved,
            center.X - (CrossSize / 2),
            center.Y - (CrossSize / 2)
        );

        e.Pointer.Capture(grip);
        e.Handled = true;
    }

    /// <summary>Moves every splitter a crossing drag holds by the share the pointer has moved.</summary>
    /// <param name="sender">The grip the pointer is on.</param>
    /// <param name="e">What the pointer did.</param>
    private void MovedCross(object? sender, PointerEventArgs e)
    {
        if (_crossDrag is not { } drag)
        {
            return;
        }

        var at = e.GetPosition(this);
        var movedX = at.X - drag.From.X;
        var movedY = at.Y - drag.From.Y;

        foreach (var moved in drag.Splits)
        {
            var columns = moved.Split.Split.Axis == DockAxis.Columns;
            var least =
                (columns
                    ? DockTree.MinimumWidth(moved.Split.Split.First)
                    : DockTree.MinimumHeight(moved.Split.Split.First))
                / moved.Room;
            var most =
                1
                - (
                    (columns
                        ? DockTree.MinimumWidth(moved.Split.Split.Second)
                        : DockTree.MinimumHeight(moved.Split.Split.Second))
                    / moved.Room
                );
            var ratio = moved.Start + ((columns ? movedX : movedY) / moved.Room);
            moved.Ratio = least <= most ? Math.Clamp(ratio, least, most) : ratio;
            ApplyRatio(moved.Split, moved.Ratio);
        }

        Canvas.SetLeft(drag.Handle, drag.Left + movedX);
        Canvas.SetTop(drag.Handle, drag.Top + movedY);
        e.Handled = true;
    }

    /// <summary>Writes what a crossing drag left every splitter at, as one change.</summary>
    /// <param name="sender">The grip the pointer is on.</param>
    /// <param name="e">What the pointer did.</param>
    private void EndedCross(object? sender, PointerReleasedEventArgs e)
    {
        if (_crossDrag is not { } drag)
        {
            return;
        }

        _crossDrag = null;
        _manager.SetRatios([.. drag.Splits.Select(moved => (moved.Split.Split, moved.Ratio))]);

        e.Pointer.Capture(null);
        e.Handled = true;
    }

    /// <summary>Draws a split's two star weights where its seam now stands.</summary>
    /// <param name="part">The splitter.</param>
    /// <param name="ratio">The share its first child is to have.</param>
    private static void ApplyRatio(SplitPart part, double ratio)
    {
        if (part.Handle.Parent is not Grid grid)
        {
            return;
        }

        var first = new GridLength(ratio, GridUnitType.Star);
        var second = new GridLength(1 - ratio, GridUnitType.Star);

        if (part.Split.Axis == DockAxis.Columns && grid.ColumnDefinitions.Count >= 3)
        {
            grid.ColumnDefinitions[0].Width = first;
            grid.ColumnDefinitions[2].Width = second;

            return;
        }

        if (grid.RowDefinitions.Count >= 3)
        {
            grid.RowDefinitions[0].Height = first;
            grid.RowDefinitions[2].Height = second;
        }
    }

    /// <summary>Where a control stands, in the area's own coordinates.</summary>
    /// <param name="control">The control.</param>
    /// <returns>Its rectangle, or an empty one when it is not laid out yet.</returns>
    private Rect BoundsOf(Control control) =>
        control.TranslatePoint(default, this) is { } origin
        && control.Bounds.Size is { Width: > 0, Height: > 0 } size
            ? new Rect(origin, size)
            : default;

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
    /// created in, which is not the tab order once a dock has been moved among them. The tree's order is the
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
            var lifted =
                ReferenceEquals(instance, _carriedDock)
                && _sourceRegion is not null
                && ReferenceEquals(placed.Region, _sourceRegion);

            placed.View.IsVisible = shown && !lifted;
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

        region.Strip.Classes.Set(SingleClass, headers.Length == 1);
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

        var changed = !ReferenceEquals(placed.Region.Selected, instance);

        placed.Region.Selected = instance;
        Show();

        if (changed)
        {
            Reveal(placed.Region);
        }

        // Choosing a tab is working in that dock. Told here as well because the tab may have been chosen from
        // a menu or a test rather than by a press the dock itself heard.
        _manager.WorkedIn(instance);

        // Choosing a dock gives it the keyboard: the keyboard was on the tab that was clicked, and that tab
        // belongs to this strip rather than to the dock, while a dock answers its own keys inside itself, so
        // choosing one and then typing would otherwise say nothing at all.
        instance.View.View.Focus();
    }

    /// <summary>Brings a region's content in with a fade after another dock became the one it shows.</summary>
    /// <param name="region">The region whose content changed.</param>
    /// <remarks>
    /// What changes is the panel rather than a dock's own control: a plugin's view is not this file's to animate,
    /// and the panel holds only the one view that is visible. The opacity is put to nothing without a transition
    /// and then brought up with one, so the frame that hides the dock that was shown is not itself a movement.
    /// With motion off the two states are the same duration, and the switch is left as a cut.
    /// </remarks>
    private static void Reveal(Region region)
    {
        if (Motion.FadeTime(Motion.State) <= TimeSpan.Zero)
        {
            return;
        }

        region.Content.Transitions = null;
        region.Content.Opacity = 0;
        region.Content.Transitions = region.Fade;

        Dispatcher.UIThread.Post(() => region.Content.Opacity = 1);
    }

    /// <summary>
    /// Remembers that a press may have started a drag.
    /// </summary>
    /// <remarks>
    /// A press is not a drag: what separates the two is how far the pointer goes before it is let go of, which is
    /// why nothing is done here but writing down where it started.
    /// <para>
    /// Which button asked for the gesture is settled when the pointer moves rather than here. What a pointer
    /// event says about its own buttons at the moment of the press is not to be trusted, reading it there is
    /// what kept the drag from starting at all, and the move knows what is held without having to be told.
    /// </para>
    /// <para>
    /// The dock is the one named by the header that was pressed, or, from the strip's drag area, whichever the
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
    /// area it is over where it would land, including, for a dock picked up here, the area it was picked up in.
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
    /// A drag is answered in two steps. Carried anywhere into a group, the drag <em>approaches</em> it, which says
    /// how far it has come rather than where it would land. The second step is the pointer being over an answer:
    /// a side of the group, or its middle, which is what a drop can land on. So what the area says is "may be
    /// dropped" only when the pointer is on an answer, and merely "still here" while it is only near one.
    /// <para>
    /// What an answer looks like is the level of motion's to decide (see <see cref="ShowsRoom"/>): the groups make
    /// room for the dock, or the five points are drawn over the group.
    /// </para>
    /// <para>
    /// A drag this area did not begin is answered here as much as one it did: a dock is carried out of one window
    /// and let go of in another, and the window it arrives in is the one that has to say where it goes. A drag
    /// carrying something else (files, paths, another program's data) is left unanswered, so that what is under
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

        if (DockDrag.Carried(_manager, e.DataTransfer) is not { } carried)
        {
            return;
        }

        var at = e.GetPosition(this);
        var approach = Approached(at);
        var side = approach is { } here ? SideAt(here, at) : null;

        ShowLifted(at);

        _zonesUp = true;
        Preview(approach, side, carried);

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

    /// <summary>Takes the answers down when a drag leaves, and says the pointer is over no place.</summary>
    /// <remarks>
    /// A leave is raised for the control the drag left and bubbles, so a drag that has moved from one group of this
    /// area to another arrives here as well: what is asked is where the pointer is, and the area has lost the drag
    /// only when the pointer is outside it. Reading a child's leave as the area's would tear the drag down and put
    /// it up again on every change of what is under the pointer, which is the carried dock jumping back to the size
    /// it had and the room opening a second time.
    /// </remarks>
    /// <param name="sender">The area.</param>
    /// <param name="e">The drag.</param>
    private void Off(object? sender, DragEventArgs e)
    {
        if (new Rect(Bounds.Size).Contains(e.GetPosition(this)))
        {
            return;
        }

        Clear();
        _windows.OverArea = false;
    }

    /// <summary>
    /// Lands a dragged dock on the answer it was let go of, or does nothing where there is no answer.
    /// </summary>
    /// <remarks>
    /// The release is the second half of the two-step gesture: hovering only says what would happen, and this is
    /// where the layout is written. What was drawn while the drag was over the area is not carried over: the layout
    /// is asked for the same answer again, so a drag that is let go of changes the tree once and in one way. A
    /// release in a group without an answer is a release with no answer, it does nothing rather than guessing, and
    /// it is still a release the area took: what the host does with a drag whose drop landed nowhere is decided by
    /// where the release was, and a drop delivered here was delivered over this area.
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

        // The points go, the carried picture stays: a drop is where it flies from. The room a drag was making goes
        // as well, since what lands is the layout itself rather than a preview of it.
        ClearZones();
        Reset();
        _windows.OverArea = false;

        if (instance is null)
        {
            HideLifted();

            return;
        }

        // The platform says which target a drop was delivered to by delivering it, so this is the record that the
        // release was over a dock area even where the area had no answer for it: the drag was let go of here, and
        // not in the open.
        _windows.Dropped = true;

        if (target is not { } landed)
        {
            HideLifted();
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

        // The place the dock landed in is what the carried picture flies into. What waits under the picture is the
        // dock's own content, at the place it now belongs, so the flight is one image rather than a live tree. A
        // dock that landed where it already was is flown too: its view is hidden because it is the one being
        // carried, and the flight is what puts it back.
        if (_placed.TryGetValue(instance, out var arrived))
        {
            Fly(arrived.Region);
        }
        else
        {
            HideLifted();
        }
    }

    /// <summary>Empties the place a dock was picked up from, so a drag reads as the dock being taken away.</summary>
    /// <remarks>
    /// A region keeps its room while its dock is in the air: what goes is the view, not the column. The dock's
    /// tab stays, because the strip belongs to the region and the dock is still one of its tabs, and the content
    /// below is what is left blank until the drag lands or is called off.
    /// </remarks>
    /// <param name="sender">The drag.</param>
    /// <param name="e">Not read.</param>
    private void Lifted(object? sender, EventArgs e)
    {
        var source = DockDrag.Source;

        _carriedDock = source is { } given && Owns(given) ? given : null;
        _sourceRegion =
            _carriedDock is { } carried && _placed.TryGetValue(carried, out var placed)
                ? placed.Region
                : null;

        // A drag that has ended is drawn back into the layout by the return that is already running, or by the one
        // its end begins: the room it was making closes into the place the dock stands in again, and the tree is
        // drawn once it has. What changes meanwhile is which dock is shown, which is said here rather than by
        // drawing the tree again.
        if (_recede is not null || _arranged is not null)
        {
            Show();
        }
        else
        {
            Rebuild();
        }
    }

    /// <summary>Whether this area draws one dock, which is what makes it the place a drag was picked up from.</summary>
    /// <param name="instance">The dock being carried.</param>
    /// <returns>Whether it is one of this area's.</returns>
    private bool Owns(DockInstance instance) =>
        _placed.ContainsKey(instance)
        || (Alone && ReferenceEquals(_alone.Child, instance.View.View));

    /// <summary>Takes the answers down, whether a drag ended, left, or was called off.</summary>
    private void Clear()
    {
        Release();
        ClearZones();

        // A flight is drawing the carried picture towards the place it landed: the drag is over, but the picture
        // is not, and taking it down here would leave the region blank until the flight's timer ends.
        if (_flight is null)
        {
            HideLifted();
        }
    }

    /// <summary>Takes the points down but leaves the carried dock up, which is what a drop does.</summary>
    private void ClearZones()
    {
        _zonesUp = false;
        _aim = null;
        _picked = false;

        HidePoints();
    }

    /// <summary>Takes the points down but leaves the drag on, which is what the layout moving answers instead.</summary>
    private void HidePoints()
    {
        if (!_zones.IsVisible)
        {
            return;
        }

        _approaching = null;
        _points.Clear();
        _shapes.Clear();

        _zones.Children.Clear();
        _zones.IsVisible = false;
    }

    /// <summary>Draws the carried dock at the pointer, shrunk as though it had been lifted off its region.</summary>
    /// <remarks>
    /// The pickup is played once per drag and not once per frame the picture is put back on: a drag whose answers
    /// are re-asked is the same dock being carried, and a pickup played again would size the picture up for a frame
    /// before it shrank, which reads as the dock being picked up a second time.
    /// </remarks>
    /// <param name="at">Where the pointer is, in the area's coordinates.</param>
    private void ShowLifted(Point at)
    {
        if (DockDrag.Picture is not { } picture || DockDrag.Lifted is not { Width: > 0, Height: > 0 } size)
        {
            return;
        }

        var growing = !_picked;
        var first = !_lifted.IsVisible;

        _picked = true;

        if (first)
        {
            _carried.Source = picture;
            _lifted.Width = size.Width;
            _lifted.Height = size.Height;
            _lifted.RenderTransformOrigin = new RelativePoint(
                DockDrag.Grip.X,
                DockDrag.Grip.Y,
                RelativeUnit.Absolute
            );

            _liftScale.Transitions = null;
            _liftScale.ScaleX = 1;
            _liftScale.ScaleY = 1;
            _liftScale.Transitions = _liftGrow;

            _lifted.IsVisible = true;
            _liftLayer.IsVisible = true;
        }

        Canvas.SetLeft(_lifted, at.X - DockDrag.Grip.X);
        Canvas.SetTop(_lifted, at.Y - DockDrag.Grip.Y);

        if (!first)
        {
            return;
        }

        if (growing && Motion.TravelTime(Motion.Feedback) > TimeSpan.Zero)
        {
            Dispatcher.UIThread.Post(() =>
            {
                _liftScale.ScaleX = LiftScale;
                _liftScale.ScaleY = LiftScale;
            });
        }
        else
        {
            _liftScale.ScaleX = LiftScale;
            _liftScale.ScaleY = LiftScale;
        }
    }

    /// <summary>Takes the carried dock off, which is what ends a drag whatever became of it.</summary>
    private void HideLifted()
    {
        if (!_lifted.IsVisible)
        {
            return;
        }

        _lifted.IsVisible = false;
        _liftLayer.IsVisible = false;
        _carried.Source = null;

        _liftScale.Transitions = null;
        _liftScale.ScaleX = 1;
        _liftScale.ScaleY = 1;
        _liftScale.Transitions = _liftGrow;
    }

    /// <summary>Flies the carried picture into the place the dropped dock now stands.</summary>
    /// <remarks>
    /// What moves is the picture taken when the dock was picked up, not the dock itself. A live dock is a tree of
    /// controls, and scaling that tree from frame to frame makes its listing and its drawing do their work at
    /// every scale; the picture is drawn once, so the flight costs one image. The content waits underneath, at the
    /// place it now belongs, and is shown when the picture arrives.
    /// <para>
    /// Where that place is is read on a frame after the layout that put it there, because a region just moved into
    /// a tree still has the bounds it had in the one it left: a flight aimed at those is a flight to where the dock
    /// used to be, which is another group's place, or to a canvas that is no longer drawn. The picture waits where
    /// the pointer left it for those frames, and a place that never arrives ends the flight rather than flying the
    /// dock nowhere.
    /// </para>
    /// </remarks>
    /// <param name="region">The region whose content arrived.</param>
    private void Fly(Region region)
    {
        StopFlight();

        var left = Canvas.GetLeft(_lifted);
        var top = Canvas.GetTop(_lifted);

        if (
            Motion.TravelTime(Motion.Panel) <= TimeSpan.Zero
            || !_lifted.IsVisible
            || _lifted.Width < 1
            || _lifted.Height < 1
            || double.IsNaN(left)
            || double.IsNaN(top)
        )
        {
            HideLifted();
            Reveal(region);

            return;
        }

        var grip = DockDrag.Grip;
        var scaleX = _liftScale.ScaleX;
        var scaleY = _liftScale.ScaleY;
        var endScaleX = scaleX;
        var endScaleY = scaleY;
        var endLeft = left;
        var endTop = top;
        var resting = false;
        var waited = 0;

        // The pickup transition is stopped where it stands, so the flight begins from the size the picture has
        // reached rather than from the size it was aiming for.
        _liftScale.Transitions = null;
        _liftScale.ScaleX = scaleX;
        _liftScale.ScaleY = scaleY;

        region.Content.Transitions = null;
        region.Content.Opacity = 0;
        region.Content.IsHitTestVisible = false;

        var started = new Stopwatch();
        var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(15) };

        timer.Tick += (_, _) =>
        {
            if (!resting)
            {
                if (RestingPlace(region) is not { } to)
                {
                    if (++waited > ArrivalWaits)
                    {
                        timer.Stop();
                        _flight = null;
                        _flying = null;
                        Arrived(region);
                    }

                    return;
                }

                endScaleX = to.Width / _lifted.Width;
                endScaleY = to.Height / _lifted.Height;
                endLeft = to.X - (grip.X * (1 - endScaleX));
                endTop = to.Y - (grip.Y * (1 - endScaleY));

                resting = true;
                started.Start();
            }

            var t = Math.Min(1, started.Elapsed.TotalMilliseconds / Motion.Panel.TotalMilliseconds);
            var eased = Motion.Expo.Ease(t);

            _liftScale.ScaleX = scaleX + ((endScaleX - scaleX) * eased);
            _liftScale.ScaleY = scaleY + ((endScaleY - scaleY) * eased);
            Canvas.SetLeft(_lifted, left + ((endLeft - left) * eased));
            Canvas.SetTop(_lifted, top + ((endTop - top) * eased));

            if (t < 1)
            {
                return;
            }

            timer.Stop();
            _flight = null;
            _flying = null;
            Arrived(region);
        };

        _flight = timer;
        _flying = region;
        timer.Start();
    }

    /// <summary>
    /// Where a dropped dock's content now stands, or nothing while the layout has not given it its room yet.
    /// </summary>
    /// <remarks>
    /// The rectangle is read off the control and checked against the room the tree gives its group, which is worked
    /// out from the tree rather than from the controls: a region that has just been moved into a tree keeps the
    /// bounds it had, and those are either another group's place or a place in the arrangement that is no more.
    /// </remarks>
    /// <param name="region">The region the dock landed in.</param>
    /// <returns>Where the content stands, or nothing.</returns>
    private Rect? RestingPlace(Region region)
    {
        var found = BoundsOf(region.Content);

        if (found.Width < 1 || found.Height < 1)
        {
            return null;
        }

        // The middle of the content rather than the whole of it, because a room is worked out in fractions and the
        // controls are placed on whole pixels: a pixel of rounding must not read as a place the dock has left.
        return Rooms(Layout.Root, null, null, 0).TryGetValue(region.Id, out var box)
            && box.Contains(found.Center)
                ? found
                : null;
    }

    /// <summary>Puts a region's own content back, and takes the carried picture down with it.</summary>
    /// <param name="region">The region the dock landed in.</param>
    private void Arrived(Region region)
    {
        region.Content.Opacity = 1;
        region.Content.IsHitTestVisible = true;
        region.Content.Transitions = region.Fade;

        HideLifted();
    }

    /// <summary>Ends a flight in flight, putting its region back as it stands.</summary>
    private void StopFlight()
    {
        _flight?.Stop();
        _flight = null;

        if (_flying is { } region)
        {
            region.Content.Opacity = 1;
            region.Content.IsHitTestVisible = true;
            region.Content.Transitions = region.Fade;
        }

        _flying = null;
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
        var room = new Rect(Bounds.Size);

        if (room.Width <= 0 || room.Height <= 0 || !room.Contains(at))
        {
            return null;
        }

        // The pane's own edge means the outermost side of the arrangement, not the side of the group
        // under the pointer: a drop there wraps the whole tree, which is what makes a dock reach the
        // edge of the window however the tree happens to be arranged.
        if (EdgeAt(at) is { } edge)
        {
            return new Approach(room, null, edge);
        }

        if (Layout.Root is not null)
        {
            // The answers as they stood when the drag reached the area: a room opened under the pointer moves the
            // groups on screen, and what the pointer is aimed at has to stay where it was seen.
            var rooms = _aim ??= Rooms(Layout.Root, null, null, 0);

            foreach (var group in DockTree.Groups(Layout))
            {
                if (rooms.TryGetValue(group.Id, out var leaf) && leaf.Contains(at))
                {
                    return new Approach(leaf, group);
                }
            }
        }

        return new Approach(room, null);
    }

    /// <summary>The pane edge a point is near, or nothing when it is not near one.</summary>
    /// <param name="at">The point, in the area's coordinates.</param>
    /// <returns>The edge, or nothing.</returns>
    private DockSide? EdgeAt(Point at)
    {
        if (at.X <= EdgeReach)
        {
            return DockSide.Left;
        }

        if (at.X >= Bounds.Width - EdgeReach)
        {
            return DockSide.Right;
        }

        if (at.Y <= EdgeReach)
        {
            return DockSide.Top;
        }

        return at.Y >= Bounds.Height - EdgeReach ? DockSide.Bottom : null;
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
        if (approach.Edge is { } edge)
        {
            return edge;
        }

        if (approach.Group is null)
        {
            return DockSide.Center;
        }

        foreach (var shape in Aim(approach.Leaf))
        {
            if (Contains(shape.Outline, at))
            {
                return shape.Side;
            }
        }

        return null;
    }

    /// <summary>The points a drop can land on in a group, as the level in force aims them.</summary>
    /// <remarks>
    /// The cluster is inset from the group's own edges, because what is said is where a drop would land and not
    /// which group it is nearest: the very edge of a group is as likely to be aimed at the group beside it, and
    /// the room the layout makes while the pointer is on an answer is made by the answer rather than by the group.
    /// A group too small to spare the share keeps its answers whole, since the room is measured in pixels and a
    /// share of nothing is nothing.
    /// </remarks>
    /// <param name="leaf">The rectangle of the group.</param>
    /// <returns>The aim, the centre first.</returns>
    internal static IReadOnlyList<DockPointShape> Aim(Rect leaf) => Shapes(leaf, ShowsRoom);

    /// <summary>The place a drop on one of a group's points would put the dock.</summary>
    /// <param name="approach">What the drag has come to.</param>
    /// <param name="side">The point the drop was let go of on.</param>
    /// <returns>What to land.</returns>
    private DockTarget TargetOf(Approach approach, DockSide side)
    {
        if (approach.Edge is not null)
        {
            return new DockTarget("", side, 0, Whole: true);
        }

        if (approach.Group is { } group)
        {
            return new DockTarget(group.Id, side, group.Docks.Count);
        }

        // No group drawn: the dock makes the root group, or, a root whose docks are all hidden, joins the group
        // that would be the active one, which is the same answer opening a dock gives.
        var active =
            Layout.ActiveGroup is { } id && DockTree.Find(Layout, id) is { } named
                ? named
                : DockTree.Groups(Layout).FirstOrDefault();

        return active is null
            ? new DockTarget("", DockSide.Center, 0)
            : new DockTarget(active.Id, DockSide.Center, active.Docks.Count);
    }

    /// <summary>
    /// Says what the drag has come to: the room it would take, or the points it could be put on.
    /// </summary>
    /// <remarks>
    /// The two answers are for the two kinds of look. Where travel is drawn, the answer is the layout itself: the
    /// groups make room for the dock while the pointer is on an answer, and no shape is drawn, because a shape
    /// over the room that was just made would be the same answer twice. Where travel is not drawn, the layout
    /// changes only when the drop is let go of, so the answer is the five points, lit by the one under the pointer.
    /// <para>
    /// A group takes the drag wherever in it the pointer is, but a drop lands only on an answer, so what the area
    /// says is "may be dropped" only when the pointer is on one, and merely "still here" while it is only near
    /// one. Joining a group's tabs is an answer that moves nothing, and the release lands it all the same.
    /// </para>
    /// </remarks>
    /// <param name="approach">What the drag has come to, or nothing.</param>
    /// <param name="side">The point it is over, or nothing.</param>
    /// <param name="carried">The dock the drag is carrying.</param>
    private void Preview(Approach? approach, DockSide? side, DockInstance carried)
    {
        if (!_zonesUp)
        {
            return;
        }

        // The answers are drawn where the layout does not move to say them, and, for a debugging run, over the
        // room it makes as well: what is shown then is what a drop would land on, in the shape it is hit in.
        if (!ShowsRoom || DockDebug.Aim)
        {
            ShowPoints(approach, side);
        }
        else
        {
            HidePoints();
        }

        if (!ShowsRoom)
        {
            return;
        }

        if (approach is { } here && side is { } chosen && chosen != DockSide.Center)
        {
            Arrange(here, chosen, carried);

            return;
        }

        Release();
    }

    /// <summary>
    /// Whether the area says where a drop would land by moving the layout rather than by drawing the points.
    /// </summary>
    /// <remarks>
    /// Asked of the level in force rather than kept as a setting of the area's: a level that draws travel can show
    /// a room being made, and a level that does not has to draw the answer instead, because a layout that changes
    /// at once is a layout nothing was read from.
    /// </remarks>
    private static bool ShowsRoom => Motion.TravelTime(Motion.Panel) > TimeSpan.Zero;

    /// <summary>
    /// Draws what the drag has come to: the five points of the group under the pointer, lit by the one it is over.
    /// </summary>
    /// <remarks>
    /// The points are only drawn for the group under the pointer, since a drag is over one group at a time: a
    /// second group's points would be a second answer to a question that has one. What is being aimed at is said
    /// by the point itself, the five of them fill what a drop may land on, so a lit point is the whole of where the
    /// dock would go and no second box is drawn over it, which would be one more surface over a shape that already
    /// said it.
    /// </remarks>
    /// <param name="approach">What the drag has come to, or nothing.</param>
    /// <param name="side">The point it is over, or nothing.</param>
    private void ShowPoints(Approach? approach, DockSide? side)
    {
        if (approach is not { } here)
        {
            return;
        }

        if (
            _approaching is not { } drawn
            || drawn.Leaf != here.Leaf
            || drawn.Edge != here.Edge
            || !ReferenceEquals(drawn.Group, here.Group)
        )
        {
            Show(here);
        }

        _approaching = here;

        if (!_zones.IsVisible)
        {
            _zones.IsVisible = true;

            // The cluster is put in before it is shown, so what fades is the whole answer rather than each
            // shape on its own. A move between groups keeps it up and only the lit point changes, which is
            // why the fade is on the one becoming visible rather than on every Preview.
            if (Motion.FadeTime(Motion.Feedback) > TimeSpan.Zero)
            {
                _zones.Transitions = null;
                _zones.Opacity = 0;
                _zones.Transitions = _zoneFade;

                Dispatcher.UIThread.Post(() => _zones.Opacity = 1);
            }
            else
            {
                _zones.Opacity = 1;
            }
        }

        foreach (var (pointed, zone) in _points)
        {
            zone.Classes.Set(DockPointOverClass, pointed == side);
        }
    }

    /// <summary>Makes room for a dragged dock in the layout, or gives up the room it was making.</summary>
    /// <remarks>
    /// The room is opened once and then moved: a pointer going from one answer to another is a second tree with the
    /// same regions in it, so what changes is where each region stands and not what the area draws at all. A drop
    /// that would change nothing closes the room and leaves the layout as it stands.
    /// </remarks>
    /// <param name="approach">The group the drop is on.</param>
    /// <param name="side">The side of it.</param>
    /// <param name="carried">The dock being dragged.</param>
    private void Arrange(Approach approach, DockSide side, DockInstance carried)
    {
        // A room that was closing is opened again from where it had got to rather than started over: the hand has
        // come back to an answer it left, and a second arrival from nowhere would be a jump.
        var returning = _recede is not null;

        _recede?.Stop();
        _recede = null;

        var standing = _arranged;

        if (
            !returning
            && standing is not null
            && standing.Side == side
            && string.Equals(standing.Target, approach.Group?.Id, StringComparison.Ordinal)
        )
        {
            return;
        }

        if (Room(approach, side, carried) is not { } made)
        {
            Release();

            return;
        }

        if (standing is not null && Same(standing, made))
        {
            Move();

            return;
        }

        _arranged = made;

        if (standing is not null)
        {
            Move();

            return;
        }

        if (!Adopt())
        {
            Reset();

            return;
        }

        // The room opens from the layout as it stands, which is only true once the layout pass has placed the
        // regions where they were put: moving them in the same pass would be a jump to the answer.
        Dispatcher.UIThread.Post(() =>
        {
            Wear();
            Move();
        });
    }

    /// <summary>Puts an open room back after the tree was drawn again, without playing it again.</summary>
    /// <remarks>
    /// What a layout that changed under a drag changes is the tree, not the answer: the room is where it was, and
    /// placing it again rather than opening it again is what keeps a dock arriving or leaving in the background
    /// from flickering the room the pointer is on.
    /// </remarks>
    private void Restage()
    {
        if (!Adopt())
        {
            Reset();

            return;
        }

        Move();
        Wear();
    }

    /// <summary>What a drop on a side of a group would make of the layout.</summary>
    /// <param name="approach">The group the drop is on.</param>
    /// <param name="side">The side of it.</param>
    /// <param name="carried">The dock being dragged.</param>
    /// <returns>The room it would take, or nothing when the drop would change nothing.</returns>
    private Arrangement? Room(Approach approach, DockSide side, DockInstance carried)
    {
        if (_manager.Ahead(carried, TargetOf(approach, side)) is not { Root: { } root } ahead)
        {
            return null;
        }

        if (DockTree.GroupOf(ahead, DockManager.AheadNameId, 0) is not { } incoming)
        {
            return null;
        }

        if (Opening(root, incoming.Id) is not { } opening)
        {
            // The tree holds nothing else: the room is the whole layout, and there is nothing to make it out of.
            return null;
        }

        return new Arrangement(
            root,
            incoming.Id,
            opening,
            IsGroup(opening.First, incoming.Id) ? 0 : 1,
            approach.Group?.Id,
            side
        );
    }

    /// <summary>Whether two arrangements show the same answer, which is the same tree with the regions in it.</summary>
    /// <param name="one">An arrangement.</param>
    /// <param name="other">The other.</param>
    /// <returns>Whether they say the same thing.</returns>
    private static bool Same(Arrangement one, Arrangement other) =>
        one.Side == other.Side && string.Equals(one.Target, other.Target, StringComparison.Ordinal);

    /// <summary>The split that opens to make room for a group, or nothing when the tree is that group alone.</summary>
    /// <param name="node">The tree to walk.</param>
    /// <param name="id">The group standing for the dock.</param>
    /// <returns>The split, or nothing.</returns>
    private static LayoutSplit? Opening(LayoutNode? node, string id) =>
        node is not LayoutSplit split
            ? null
            : IsGroup(split.First, id) || IsGroup(split.Second, id)
                ? split
                : Opening(split.First, id) ?? Opening(split.Second, id);

    /// <summary>Whether a node is one group.</summary>
    /// <param name="node">The node.</param>
    /// <param name="id">The group id.</param>
    /// <returns>Whether it is the group with that id.</returns>
    private static bool IsGroup(LayoutNode? node, string id) =>
        node is LayoutGroup group && string.Equals(group.Id, id, StringComparison.Ordinal);

    /// <summary>
    /// Takes the regions out of the tree and puts them where they stand, which is what lets them move.
    /// </summary>
    /// <remarks>
    /// The tree places a region by its share of the room, and a share is the grid's own number rather than a
    /// distance: it cannot be eased from one value to another. The regions are therefore placed as rectangles for
    /// as long as a drag is showing where it would land, at the rectangles the tree gives them, which is where
    /// they already are, and the move to the room the drop makes is made from there.
    /// </remarks>
    /// <returns>Whether the area has a room to arrange them in.</returns>
    private bool Adopt()
    {
        var room = new Rect(_surface.Bounds.Size);

        if (room.Width < 1 || room.Height < 1)
        {
            return false;
        }

        var standing = Rooms(Layout.Root, null, null, 0);

        if (standing.Count == 0)
        {
            return false;
        }

        _arrange.Background = Brushes.Transparent;

        // The seams of the layout as it stands are not the seams of the one a drop would make, and they hang over
        // the docks: left up, they would be drawn through the room that is opening and behind the groups moving.
        foreach (var part in _splits)
        {
            part.Handle.IsVisible = false;
        }

        foreach (var region in _regions.Values)
        {
            if (region.Panel.Parent is Panel parent)
            {
                parent.Children.Remove(region.Panel);
            }
        }

        foreach (var (id, box) in standing)
        {
            if (!_regions.TryGetValue(id, out var region))
            {
                continue;
            }

            var panel = region.Panel;

            panel.Transitions = null;
            panel.Opacity = 1;

            // A group that is moving is a surface rather than a hole in the ground: two groups pass over one
            // another while the room opens, and a face that is the ground's own colour is what makes the one in
            // front the one that is read. At rest the face is the area's, which is why the region has none then.
            panel[!Panel.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);

            Place(panel, box);

            if (!_arrange.Children.Contains(panel))
            {
                _arrange.Children.Add(panel);
            }
        }

        _incoming.Transitions = null;
        _incoming.Opacity = 1;
        _incoming.IsVisible = true;
        Place(_incoming, Closed());

        return true;
    }

    /// <summary>Turns the transitions on for the groups that are arranged, which is after they have been placed.</summary>
    private void Wear()
    {
        foreach (var region in _regions.Values)
        {
            if (ReferenceEquals(region.Panel.Parent, _arrange))
            {
                region.Panel.Transitions = _arrangeTravel;
            }
        }

        _incoming.Transitions = _arrangeTravel;
    }

    /// <summary>Places every region where the arrangement puts it, which is what moves them there.</summary>
    private void Move()
    {
        if (_arranged is not { } standing)
        {
            return;
        }

        var rooms = Rooms(standing.Root, standing.Incoming, null, 0);

        foreach (var (id, region) in _regions)
        {
            var panel = region.Panel;

            if (!ReferenceEquals(panel.Parent, _arrange))
            {
                continue;
            }

            if (rooms.TryGetValue(id, out var box))
            {
                Place(panel, box);
                panel.Opacity = 1;

                continue;
            }

            // A group whose only dock was taken out is in no tree at all: the strip it is holding is left where
            // it stands and faded, since a region that is nowhere is not one to be moved anywhere.
            panel.Opacity = 0;
        }

        if (rooms.TryGetValue(standing.Incoming, out var room))
        {
            Place(_incoming, room);
        }
    }

    /// <summary>Gives up the room a drag was making, showing the layout as it stands again.</summary>
    /// <remarks>
    /// The room is closed the way it was opened: the regions return to where they stand and the room shuts, and the
    /// tree is drawn again once they have arrived. A level that draws no travel has nothing to show, so the area is
    /// put back at once. A room closed because the layout itself changed is given up by the drawing of the new
    /// layout, which <see cref="Reset"/> is a part of.
    /// </remarks>
    private void Release()
    {
        if (_arranged is not { } standing || _recede is not null)
        {
            return;
        }

        if (!ShowsRoom)
        {
            Reset();
            Render();

            return;
        }

        var rooms = Rooms(Layout.Root, null, null, 0);

        foreach (var (id, region) in _regions)
        {
            var panel = region.Panel;

            if (!ReferenceEquals(panel.Parent, _arrange))
            {
                continue;
            }

            if (rooms.TryGetValue(id, out var box))
            {
                Place(panel, box);
            }

            panel.Opacity = 1;
        }

        if (Rooms(standing.Root, standing.Incoming, null, 0).TryGetValue(standing.Incoming, out var room))
        {
            Place(_incoming, Shrink(room, standing.Opening.Axis, standing.From));
        }

        // The dock a drag was carrying is shown in its region again while the room closes, which is what a drag that
        // has ended means and what a drag still in flight does not: the views are said again from where the drag
        // stands rather than moved, and a region whose dock is still in the air keeps it hidden.
        if (_sourceRegion is not null && _carriedDock is null)
        {
            Show();
        }

        var timer = new DispatcherTimer { Interval = Motion.TravelTime(Motion.Panel) };

        timer.Tick += (_, _) =>
        {
            timer.Stop();
            _recede = null;

            // The room is given up before the tree is drawn: a drawing puts an open room back where it was, and
            // this one is closed.
            _arranged = null;

            // The tree is drawn again rather than put back by hand, because what the drag left behind may have
            // changed what the area holds: a carried dock is shown in its region again once it has been let go of.
            Rebuild();
        };

        _recede = timer;
        timer.Start();
    }

    /// <summary>Puts the regions and the room back as the layout stands, without drawing the tree again.</summary>
    private void Reset()
    {
        _recede?.Stop();
        _recede = null;
        _arranged = null;

        _incoming.IsVisible = false;
        _arrange.Background = null;

        foreach (var part in _splits)
        {
            part.Handle.IsVisible = true;
        }

        foreach (var region in _regions.Values)
        {
            region.Panel.Transitions = null;
            region.Panel.Background = Brushes.Transparent;
            region.Panel.Opacity = 1;
            region.Panel.Width = double.NaN;
            region.Panel.Height = double.NaN;
        }
    }

    /// <summary>Where the room stands before it is opened: the seam it opens from, of no size.</summary>
    /// <returns>The rectangle, in the arrangement's coordinates.</returns>
    private Rect Closed()
    {
        var standing = _arranged;

        return standing is not null
            && Rooms(standing.Root, standing.Incoming, null, 0).TryGetValue(standing.Incoming, out var room)
            ? Shrink(room, standing.Opening.Axis, standing.From)
            : default;
    }

    /// <summary>Puts a control at a rectangle, which a canvas and a size are enough for.</summary>
    /// <param name="control">The control to place.</param>
    /// <param name="box">Where.</param>
    private static void Place(Control control, Rect box)
    {
        Canvas.SetLeft(control, box.X);
        Canvas.SetTop(control, box.Y);
        control.Width = Math.Max(0, box.Width);
        control.Height = Math.Max(0, box.Height);
    }

    /// <summary>The rectangle a room closes into: the seam it opens from, of no size.</summary>
    /// <param name="box">Where the room is when it is open.</param>
    /// <param name="axis">The axis the split it opens in divides.</param>
    /// <param name="from">The share the room's side has while it is closed.</param>
    /// <returns>The rectangle it stands at closed.</returns>
    private static Rect Shrink(Rect box, DockAxis axis, double from) =>
        axis == DockAxis.Columns
            ? new Rect(from < 0.5 ? box.X : box.Right, box.Y, 0, box.Height)
            : new Rect(box.X, from < 0.5 ? box.Y : box.Bottom, box.Width, 0);

    /// <summary>
    /// The rectangle of every group a tree draws, in the area's own coordinates.
    /// </summary>
    /// <remarks>
    /// The same arithmetic the tree is drawn with: a split divides its room by its share, and a group that draws
    /// nothing gives its room to the group beside it. It is worked out rather than read off the controls because a
    /// drag is answered while the regions may be somewhere else, the arrangement having taken them out of the
    /// tree, and because what a hit test needs is the layout rather than what happens to be drawn.
    /// </remarks>
    /// <param name="root">The tree to lay out.</param>
    /// <param name="incoming">The group standing for the dock being carried, which draws with no docks in it.</param>
    /// <param name="opening">The split whose share is to be overridden, or nothing.</param>
    /// <param name="share">The share <paramref name="opening"/> is read as.</param>
    /// <returns>The rectangle of each group drawn, by the group's id.</returns>
    private Dictionary<string, Rect> Rooms(
        LayoutNode? root,
        string? incoming,
        LayoutSplit? opening,
        double share
    )
    {
        var rooms = new Dictionary<string, Rect>(StringComparer.Ordinal);

        Walk(root, new Rect(_surface.Bounds.Size));

        return rooms;

        void Walk(LayoutNode? node, Rect room)
        {
            switch (node)
            {
                case null:
                    return;

                case LayoutGroup group:
                    if (Draws(group))
                    {
                        rooms[group.Id] = room;
                    }

                    return;

                default:
                    var split = (LayoutSplit)node;

                    // A split with one side drawing nothing is no split at all: the control that is there takes the
                    // room of both, which is what the tree builds when it draws itself.
                    if (!Draws(split.First))
                    {
                        Walk(split.Second, room);

                        return;
                    }

                    if (!Draws(split.Second))
                    {
                        Walk(split.First, room);

                        return;
                    }

                    var ratio = ReferenceEquals(split, opening) ? share : split.Ratio;

                    if (split.Axis == DockAxis.Columns)
                    {
                        Walk(split.First, new Rect(room.X, room.Y, room.Width * ratio, room.Height));
                        Walk(
                            split.Second,
                            new Rect(
                                room.X + (room.Width * ratio),
                                room.Y,
                                room.Width * (1 - ratio),
                                room.Height
                            )
                        );
                    }
                    else
                    {
                        Walk(split.First, new Rect(room.X, room.Y, room.Width, room.Height * ratio));
                        Walk(
                            split.Second,
                            new Rect(
                                room.X,
                                room.Y + (room.Height * ratio),
                                room.Width,
                                room.Height * (1 - ratio)
                            )
                        );
                    }

                    return;
            }
        }

        bool Draws(LayoutNode? node) =>
            node switch
            {
                null => false,
                LayoutGroup group =>
                    (incoming is { } carried && string.Equals(group.Id, carried, StringComparison.Ordinal))
                    || Drawn(group),
                LayoutSplit split => Draws(split.First) || Draws(split.Second),
                _ => false,
            };
    }

    /// <summary>Builds the overlay for one group: the answers a drop could land on, and nothing else.</summary>
    /// <remarks>
    /// The children are rebuilt when the group under the pointer changes and kept while it does not, because the
    /// pointer spends a drag moving between points of one group and rebuilding five shapes on every move would be
    /// drawing work for a change of class. What moves with the pointer is only which point wears
    /// <see cref="DockPointOverClass"/>.
    /// <para>
    /// What is drawn is what is hit (see <see cref="Aim"/>), so a debugging run watches the same shapes a drop is
    /// read against rather than a larger set drawn for the eye.
    /// </para>
    /// </remarks>
    /// <param name="approach">What the drag has come to.</param>
    private void Show(Approach approach)
    {
        _zones.Children.Clear();
        _points.Clear();
        _shapes.Clear();

        IReadOnlyList<DockPointShape> shapes = approach.Edge is { } edge
            ? [.. Aim(approach.Leaf).Where(shape => shape.Side == edge)]
            : approach.Group is null
                ? Whole(approach.Leaf)
                : Aim(approach.Leaf);

        foreach (var shape in shapes)
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
    /// rectangle, filling the group or the room a drag is aimed in.
    /// </summary>
    /// <remarks>
    /// The points are the target, so the five of them fill it: each trapezoid's long side is one of its edges and
    /// its short side is the centre rectangle, so the shape itself says which way the dock goes, and every point
    /// of the target is on exactly one of them. A binary split has two children, a column split and a row split,
    /// so left and right are the two ways to split a group into columns and top and bottom are the two ways to
    /// split it into rows.
    /// <para>
    /// The trapezoids are a band of <see cref="DockPointEdge"/> along the edges and the centre is what they leave,
    /// which is why the centre keeps the target's shape rather than being a square: a wide target has a wide
    /// centre, and the middle of one is the whole of what is not along its edges.
    /// </para>
    /// <para>
    /// An inset cluster is the same five points on the group brought in by <see cref="DockPointPadding"/> on each
    /// side, which is what a drag is aimed in where the layout moves: the group's own edge is where a drop would
    /// be aimed at the group beside it, and a room the layout makes is made by an answer and not by a group.
    /// </para>
    /// </remarks>
    /// <param name="leaf">The rectangle of the group, in the area's coordinates.</param>
    /// <param name="inset">Whether to draw the cluster inside the group rather than over the whole of it.</param>
    /// <returns>The five points, the centre first.</returns>
    internal static IReadOnlyList<DockPointShape> Shapes(Rect leaf, bool inset = false)
    {
        var padding = Math.Min(DockPointPadding, Math.Min(leaf.Width, leaf.Height) / 4);

        var box = inset
            ? new Rect(
                leaf.X + padding,
                leaf.Y + padding,
                leaf.Width - (padding * 2),
                leaf.Height - (padding * 2)
            )
            : leaf;

        var band = Math.Min(DockPointEdge, Math.Min(box.Width, box.Height) / 3);
        var inner = new Rect(
            box.X + band,
            box.Y + band,
            box.Width - (band * 2),
            box.Height - (band * 2)
        );

        return
        [
            new(DockSide.Center, Corners(inner)),
            new(
                DockSide.Left,
                [
                    new Point(box.X, box.Y),
                    new Point(inner.X, inner.Y),
                    new Point(inner.X, inner.Bottom),
                    new Point(box.X, box.Bottom),
                ]
            ),
            new(
                DockSide.Right,
                [
                    new Point(inner.Right, inner.Y),
                    new Point(box.Right, box.Y),
                    new Point(box.Right, box.Bottom),
                    new Point(inner.Right, inner.Bottom),
                ]
            ),
            new(
                DockSide.Top,
                [
                    new Point(box.X, box.Y),
                    new Point(box.Right, box.Y),
                    new Point(inner.Right, inner.Y),
                    new Point(inner.X, inner.Y),
                ]
            ),
            new(
                DockSide.Bottom,
                [
                    new Point(inner.X, inner.Bottom),
                    new Point(inner.Right, inner.Bottom),
                    new Point(box.Right, box.Bottom),
                    new Point(box.X, box.Bottom),
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

    /// <summary>
    /// The word a tab says for a dock's title: all of it while it fits, and only the end of it when it does not.
    /// </summary>
    /// <remarks>
    /// The front is what goes, because where a dock is is what its tail says: a browser's name is a path, and the last
    /// segments are the place rather than the drive it begins on. What was left out is on the tab's tip and at the top
    /// of its menu, so shortening the word loses nothing.
    /// </remarks>
    /// <param name="title">The dock's title.</param>
    /// <returns>The word.</returns>
    internal static string TabTitle(string title)
    {
        ArgumentNullException.ThrowIfNull(title);

        if (title.Length <= TabWordLength)
        {
            return title;
        }

        return string.Concat("...".AsSpan(), title.AsSpan(title.Length - (TabWordLength - 3)));
    }

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

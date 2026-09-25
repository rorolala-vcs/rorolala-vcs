using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The dock area: the regions a dock can be placed in, the headers that choose between them, and
/// the dragging that moves one from a region to another.
/// </summary>
/// <remarks>
/// A region keeps every dock it was given and shows one of them at a time, rather than rebuilding
/// itself when the set of open docks changes. That is not an optimisation: a control has one
/// parent, and a view moved from one container into another can be caught mid-move — which is what
/// <c>The control … already has a visual parent</c> is. Nothing here is re-parented as a side
/// effect of something else. A dock is put in its region once, when it is made; taken out once,
/// when it is closed for good; and shown, hidden, or made the region's own in between. The one
/// deliberate move is a drag, and that move is done by taking the dock out of one region and
/// putting it in another, in that order.
/// <para>
/// Dragging a header shows where the dock would land before it is let go of: the area divides into
/// the regions a drop could mean, and the one the pointer is over is lit. The four zones are shown
/// whether or not anything is in them, so a region that is empty and taking no space can still be
/// what a drag is aimed at.
/// </para>
/// <para>
/// A region with nothing open in it takes no space, and its splitter goes with it, so an empty left
/// or right strip never sits there eating clicks.
/// </para>
/// </remarks>
internal sealed class DockArea : UserControl
{
    /// <summary>
    /// The class a region's header strip carries, for a theme to address it by.
    /// </summary>
    /// <remarks>
    /// The shell marks the surfaces it owns so that a theme — this host's, or a plugin's — can style
    /// them without knowing what a dock is. The names are part of Section 10, since a plugin's theme
    /// has to write them as literals.
    /// </remarks>
    public const string HeadersClass = "dock-headers";

    /// <summary>The class a dock's own header carries.</summary>
    public const string TitleClass = "dock-title";

    /// <summary>The class the header of the dock a region is showing carries.</summary>
    public const string SelectedClass = "selected";

    /// <summary>The class a region's splitter carries.</summary>
    public const string SplitterClass = "dock-splitter";

    /// <summary>The class a splitter that resizes columns carries, in addition to <see cref="SplitterClass"/>.</summary>
    public const string SplitterColumnsClass = "dock-splitter-columns";

    /// <summary>The class a splitter that resizes rows carries, in addition to <see cref="SplitterClass"/>.</summary>
    public const string SplitterRowsClass = "dock-splitter-rows";

    /// <summary>The class the zone a dragged dock would land in carries.</summary>
    public const string DropZoneClass = "dock-drop-zone";

    /// <summary>The class the button that closes the dock a region is showing carries.</summary>
    public const string CloseClass = "dock-close";

    /// <summary>The narrowest a column region is allowed to become.</summary>
    private const double MinColumn = 120;

    /// <summary>The shortest a row region is allowed to become.</summary>
    private const double MinRow = 56;

    /// <summary>How wide a splitter is.</summary>
    /// <remarks>
    /// Published, because a theme draws a line through the middle of one: the width it is drawn in is
    /// this, and a line of another width would not be centred in it.
    /// </remarks>
    public const double SplitterSize = 4;

    /// <summary>How far a pointer moves before a press becomes a drag rather than a click.</summary>
    private const double DragThreshold = 4;

    /// <summary>How much of a side is that side, as a fraction of the whole.</summary>
    private const double ZoneEdge = 0.25;

    /// <summary>How large a floating dock's window opens.</summary>
    private static readonly Size FloatSize = new(560, 400);

    /// <summary>
    /// The base theme's accent, which is what a drop zone is drawn in.
    /// </summary>
    /// <remarks>
    /// Asked for by name rather than named here: which colour the accent is belongs to the theme, and
    /// a theme that says nothing about drop zones — the base one alone — still has an accent for this
    /// to take. A colour written here instead would be one no theme could change.
    /// <para>
    /// The name is the base theme's, so the base theme is named here as surely as if its type were:
    /// swapping the base for one with different resource names means changing these two lines, and
    /// nothing would say so but a drop zone that quietly lost its edge.
    /// </para>
    /// </remarks>
    private const string AccentKey = "ThemeAccentBrush";

    /// <summary>The base theme's accent at its faintest, and the little the drop zone is filled with.</summary>
    private const string WashKey = "ThemeAccentBrush4";

    /// <summary>The docks and where they are.</summary>
    private readonly DockManager _manager;

    /// <summary>The host's translations, for dock titles and header commands.</summary>
    private readonly I18nService _i18n;

    /// <summary>The area, with the drop zones drawn over it.</summary>
    private readonly Grid _surface = new();

    /// <summary>The whole area: the three columns above, and the bottom region below them.</summary>
    private readonly Grid _root = new();

    /// <summary>The three columns: left, centre, right.</summary>
    private readonly Grid _middle = new();

    /// <summary>The region along the top.</summary>
    private readonly Region _top = new("top");

    /// <summary>The region to the left.</summary>
    private readonly Region _left = new("left");

    /// <summary>The central region.</summary>
    private readonly Region _center = new("center");

    /// <summary>The region to the right.</summary>
    private readonly Region _right = new("right");

    /// <summary>The region along the bottom.</summary>
    private readonly Region _bottom = new("bottom");

    /// <summary>The splitter under the top region.</summary>
    private readonly GridSplitter _topSplitter = new();

    /// <summary>The splitter between the left region and the centre.</summary>
    private readonly GridSplitter _leftSplitter = new();

    /// <summary>The splitter between the centre and the right region.</summary>
    private readonly GridSplitter _rightSplitter = new();

    /// <summary>The splitter above the bottom region.</summary>
    private readonly GridSplitter _bottomSplitter = new();

    /// <summary>The four zones a dragged dock could land in, shown only while one is dragged.</summary>
    private readonly Grid _zones = new();

    /// <summary>What each dock that is placed in a region was given, by the dock.</summary>
    private readonly Dictionary<DockInstance, Placed> _placed = [];

    /// <summary>The window each floating dock lives in, by the dock.</summary>
    private readonly Dictionary<DockInstance, Window> _floats = [];

    /// <summary>The zone each placement is drawn by.</summary>
    private readonly Dictionary<DockPlacement, Border> _lit = [];

    /// <summary>The dock being dragged, if one is.</summary>
    private DockInstance? _dragging;

    /// <summary>Where the pointer was when the drag began.</summary>
    private Point _from;

    /// <summary>Whether the pointer has moved far enough to be dragging rather than clicking.</summary>
    private bool _moved;

    /// <summary>Whether a float is being closed by this area rather than by the user.</summary>
    private bool _closingFloat;

    /// <summary>Makes the area over the manager it shows.</summary>
    /// <param name="manager">The docks and where they are.</param>
    /// <param name="i18n">The host's translations.</param>
    public DockArea(DockManager manager, I18nService i18n)
    {
        _manager = manager;
        _i18n = i18n;

        Build();

        // A header is a button, and a button keeps the pointer while it is pressed; the drag is
        // followed from here, where the events arrive anyway once they have bubbled out of it.
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

        _manager.Changed += Rebuild;
        Rebuild();
    }

    /// <summary>What one dock was given when it was placed.</summary>
    /// <remarks>
    /// The region is the one thing about this that changes: a dock dragged to another region keeps
    /// everything else it was given and is taken out of one region and put in the other.
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

    /// <summary>
    /// One region: a strip of headers over the docks themselves.
    /// </summary>
    /// <remarks>
    /// Every dock the region was given stays in <see cref="Content"/>, and the one being shown is
    /// the only one that is visible. <see cref="Selected"/> is which that is.
    /// <para>
    /// The strip is one row of four: the docks' headers as tabs, then the area the shown dock is
    /// dragged by, then what the shown dock brought in its own header, then the button that closes it.
    /// The commands and the close belong to the region rather than to each dock because there is one
    /// strip and the tabs are as narrow as their words: a title that filled the strip could not leave
    /// room for a tab beside it, and one close per dock would sit in the middle of the row.
    /// </para>
    /// </remarks>
    private sealed class Region
    {
        /// <summary>Makes a region, which is named for the size that is remembered for it.</summary>
        /// <param name="name">The name its size is remembered under.</param>
        public Region(string name)
        {
            Name = name;

            Close = new Button { Classes = { CloseClass }, Content = "\u2715" };

            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Star));
            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
            Bar.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));

            Grid.SetColumn(Headers, 0);
            Grid.SetColumn(Drag, 1);
            Grid.SetColumn(Commands, 2);
            Grid.SetColumn(Close, 3);

            Bar.Children.Add(Headers);
            Bar.Children.Add(Drag);
            Bar.Children.Add(Commands);
            Bar.Children.Add(Close);

            Strip.Child = Bar;
            DockPanel.SetDock(Strip, Dock.Top);
            Panel.Children.Add(Strip);
            Panel.Children.Add(Content);
        }

        /// <summary>The name this region's size is remembered under.</summary>
        public string Name { get; }

        /// <summary>The region itself: headers above, docks below.</summary>
        public DockPanel Panel { get; } = new();

        /// <summary>
        /// The strip the headers sit in.
        /// </summary>
        /// <remarks>
        /// A border around the headers rather than a background on them, because a theme's chrome is a
        /// surface with an edge under it, and a <see cref="StackPanel"/> has a background but no
        /// border. Nothing is padded here: the headers bring their own margins.
        /// </remarks>
        public Border Strip { get; } = new() { Classes = { HeadersClass } };

        /// <summary>The strip's one row: the tabs, the drag area, the commands, and the close.</summary>
        public Grid Bar { get; } = new();

        /// <summary>The headers, one per dock the region was given.</summary>
        public StackPanel Headers { get; } =
            new()
            {
                Orientation = Orientation.Horizontal,
                Spacing = 2,

                // No bottom margin: a header's own edge is meant to land on the header strip's, so
                // that the one being shown reads as an underline along the bottom of the strip.
                Margin = new Thickness(4, 2, 4, 0),
            };

        /// <summary>
        /// The area between the tabs and the commands, which is what the shown dock is dragged by.
        /// </summary>
        /// <remarks>
        /// A drag needs somewhere to start that is not a label, and the tabs are as narrow as their
        /// words. What is left of the strip belongs to what is being shown. Nothing is drawn here,
        /// but it must not be transparent to the pointer, or a drag from it would fall through to
        /// whatever is underneath.
        /// </remarks>
        public Border Drag { get; } =
            new()
            {
                Background = Brushes.Transparent,
                Cursor = new Cursor(StandardCursorType.SizeAll),
            };

        /// <summary>The header commands of the dock being shown.</summary>
        public StackPanel Commands { get; } =
            new()
            {
                Orientation = Orientation.Horizontal,
                Spacing = 2,
                Margin = new Thickness(0, 2, 0, 0),
            };

        /// <summary>The button that closes the dock being shown.</summary>
        public Button Close { get; }

        /// <summary>The dock whose commands the strip is showing, which is what <see cref="Commands"/> was built for.</summary>
        public DockInstance? Shown { get; set; }

        /// <summary>Every dock the region was given, of which one is visible at a time.</summary>
        public Panel Content { get; } = new();

        /// <summary>Which of its docks is being shown.</summary>
        public DockInstance? Selected { get; set; }
    }

    /// <summary>Lays the regions, the splitters, and the drop zones out once.</summary>
    private void Build()
    {
        _middle.ColumnDefinitions.Add(new ColumnDefinition(ColumnLength(_manager.Layout.LeftWidth)));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Star));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(ColumnLength(_manager.Layout.RightWidth)));

        Grid.SetColumn(_left.Panel, 0);
        Grid.SetColumn(_leftSplitter, 1);
        Grid.SetColumn(_center.Panel, 2);
        Grid.SetColumn(_rightSplitter, 3);
        Grid.SetColumn(_right.Panel, 4);

        ConfigureSplitter(_topSplitter, GridResizeDirection.Rows, _top);
        ConfigureSplitter(_leftSplitter, GridResizeDirection.Columns, _left);
        ConfigureSplitter(_rightSplitter, GridResizeDirection.Columns, _right);
        ConfigureSplitter(_bottomSplitter, GridResizeDirection.Rows, _bottom);

        _middle.Children.Add(_left.Panel);
        _middle.Children.Add(_leftSplitter);
        _middle.Children.Add(_center.Panel);
        _middle.Children.Add(_rightSplitter);
        _middle.Children.Add(_right.Panel);

        _root.RowDefinitions.Add(new RowDefinition(RowLength(_manager.Layout.TopHeight)));
        _root.RowDefinitions.Add(new RowDefinition(GridLength.Auto));
        _root.RowDefinitions.Add(new RowDefinition(GridLength.Star));
        _root.RowDefinitions.Add(new RowDefinition(GridLength.Auto));
        _root.RowDefinitions.Add(new RowDefinition(RowLength(_manager.Layout.BottomHeight)));

        Grid.SetRow(_top.Panel, 0);
        Grid.SetRow(_topSplitter, 1);
        Grid.SetRow(_middle, 2);
        Grid.SetRow(_bottomSplitter, 3);
        Grid.SetRow(_bottom.Panel, 4);

        _root.Children.Add(_top.Panel);
        _root.Children.Add(_topSplitter);
        _root.Children.Add(_middle);
        _root.Children.Add(_bottomSplitter);
        _root.Children.Add(_bottom.Panel);

        foreach (var region in Regions())
        {
            region.Panel.Background = Brushes.Transparent;

            // The drag area and the close belong to whatever the region is showing at the time they
            // are used rather than to a dock, since the two outlive every dock that passes through
            // the region.
            region.Drag.PointerPressed += (_, e) => Take(region.Selected, e);
            region.Close.Click += (_, _) => CloseShown(region);
        }

        Zones();

        _surface.Children.Add(_root);
        _surface.Children.Add(_zones);

        Content = _surface;
    }

    /// <summary>Lays out the five zones a dragged dock could land in.</summary>
    private void Zones()
    {
        // Columns and rows in the same fractions the zones are read in, so what is lit up is where
        // the pointer would have to be: a quarter at each edge of each axis, the middle left over.
        _zones.ColumnDefinitions.Add(new ColumnDefinition(1, GridUnitType.Star));
        _zones.ColumnDefinitions.Add(new ColumnDefinition(2, GridUnitType.Star));
        _zones.ColumnDefinitions.Add(new ColumnDefinition(1, GridUnitType.Star));
        _zones.RowDefinitions.Add(new RowDefinition(1, GridUnitType.Star));
        _zones.RowDefinitions.Add(new RowDefinition(2, GridUnitType.Star));
        _zones.RowDefinitions.Add(new RowDefinition(1, GridUnitType.Star));

        Add(DockPlacement.Top, 0, 0, 3);
        Add(DockPlacement.Left, 0, 1, 1);
        Add(DockPlacement.Center, 1, 1, 1);
        Add(DockPlacement.Right, 2, 1, 1);
        Add(DockPlacement.Bottom, 0, 2, 3);

        _zones.IsHitTestVisible = false;

        return;

        void Add(DockPlacement placement, int column, int row, int columns)
        {
            var zone = new Border
            {
                Classes = { DropZoneClass },
                Margin = new Thickness(2),
                Opacity = 0,
            };

            zone.Bind(Border.BorderBrushProperty, new DynamicResourceExtension(AccentKey));
            zone.Bind(Border.BackgroundProperty, new DynamicResourceExtension(WashKey));

            Grid.SetColumn(zone, column);
            Grid.SetColumnSpan(zone, columns);
            Grid.SetRow(zone, row);

            _zones.Children.Add(zone);
            _lit[placement] = zone;
        }
    }

    /// <summary>Sets up one splitter, remembering the region's size when it is let go.</summary>
    private void ConfigureSplitter(GridSplitter splitter, GridResizeDirection direction, Region region)
    {
        splitter.ResizeDirection = direction;
        splitter.Classes.Add(SplitterClass);
        splitter.Classes.Add(
            direction == GridResizeDirection.Columns ? SplitterColumnsClass : SplitterRowsClass
        );

        if (direction == GridResizeDirection.Columns)
        {
            splitter.Width = SplitterSize;
        }
        else
        {
            splitter.Height = SplitterSize;
        }

        splitter.DragCompleted += (_, _) => Remember(region);
    }

    /// <summary>Records a region's size after a splitter moved it.</summary>
    private void Remember(Region region)
    {
        switch (region.Name)
        {
            case "top":
                _manager.Layout.TopHeight = _root.RowDefinitions[0].ActualHeight;
                break;

            case "left":
                _manager.Layout.LeftWidth = _middle.ColumnDefinitions[0].ActualWidth;
                break;

            case "right":
                _manager.Layout.RightWidth = _middle.ColumnDefinitions[4].ActualWidth;
                break;

            case "bottom":
                _manager.Layout.BottomHeight = _root.RowDefinitions[4].ActualHeight;
                break;

            default:
                break;
        }
    }

    /// <summary>Brings the area back in step with what is registered, open, and where.</summary>
    private void Rebuild()
    {
        Forget();
        Place();
        Settle();
        Show();
        Arrange();
        Floats();
    }

    /// <summary>Takes away the docks that are no longer open at all.</summary>
    private void Forget()
    {
        foreach (var (instance, placed) in _placed.ToArray())
        {
            if (_manager.Instances.Contains(instance))
            {
                continue;
            }

            Detach(placed);
            _placed.Remove(instance);
        }
    }

    /// <summary>Puts every dock where it says it is, moving the ones that have been dragged.</summary>
    private void Place()
    {
        foreach (var instance in _manager.Instances)
        {
            if (instance.Placement == DockPlacement.Float)
            {
                continue;
            }

            var region = RegionFor(instance.Placement);

            if (!_placed.TryGetValue(instance, out var placed))
            {
                placed = Make(instance);
                Attach(placed, region);
                _placed[instance] = placed;

                continue;
            }

            if (ReferenceEquals(placed.Region, region))
            {
                continue;
            }

            // The one move there is: taken out of the region it was in, put in the other, in that
            // order, so it is never in two places nor in none.
            Detach(placed);
            Attach(placed, region);
        }
    }

    /// <summary>Makes a dock's view and its header, which it keeps for as long as it lives.</summary>
    private Placed Make(DockInstance instance)
    {
        var title = Header(instance.Title);

        title.Classes.Add(TitleClass);
        title.Click += (_, _) => Select(instance);

        // Taken on the way down. A button answers for its own press, and marks it handled while
        // doing so, so an ordinary handler here would never be called: the press is over before it
        // reaches one. A tunnelling handler is reached first, which is the only way in.
        title.AddHandler(
            InputElement.PointerPressedEvent,
            (_, e) => Take(instance, e),
            RoutingStrategies.Tunnel
        );

        return new Placed
        {
            Instance = instance,
            Region = RegionFor(instance.Placement),
            View = instance.View.View,
            Title = title,
        };
    }

    /// <summary>Puts a dock into a region, and makes it the one the region is showing.</summary>
    private static void Attach(Placed placed, Region region)
    {
        region.Content.Children.Add(placed.View);
        region.Headers.Children.Add(placed.Title);
        placed.Region = region;
        region.Selected = placed.Instance;
    }

    /// <summary>Takes a dock out of its region, leaving the region as if it had not been there.</summary>
    private static void Detach(Placed placed)
    {
        placed.Region.Content.Children.Remove(placed.View);
        placed.Region.Headers.Children.Remove(placed.Title);

        if (ReferenceEquals(placed.Region.Selected, placed.Instance))
        {
            placed.Region.Selected = null;
        }
    }

    /// <summary>Makes sure each region is showing one of the docks open in it.</summary>
    private void Settle()
    {
        foreach (var region in Regions())
        {
            var open = Open(region);

            if (region.Selected is null || !open.Contains(region.Selected))
            {
                region.Selected = open.FirstOrDefault();
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

            // Which dock the region is showing is a class rather than a weight, so that what a
            // selected header looks like is a theme's to decide and not this file's.
            placed.Title.Classes.Set(SelectedClass, shown);
        }

        foreach (var region in Regions())
        {
            BindCommands(region);
        }
    }

    /// <summary>
    /// Makes a region's strip show what the dock it settled on brought in its own header.
    /// </summary>
    /// <remarks>
    /// Rebuilt only when the dock being shown changes: this runs on every change to the set of open
    /// docks, and buttons made again on each of those would lose the pointer resting on them.
    /// </remarks>
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

        foreach (var command in shown.View.HeaderCommands)
        {
            var button = Header(_i18n.Get(command.LabelKey));
            button.Click += (_, _) => command.Command();
            region.Commands.Children.Add(button);
        }
    }

    /// <summary>Closes the dock a region is showing, which is what its close button does.</summary>
    private void CloseShown(Region region)
    {
        if (region.Selected is { } shown)
        {
            _manager.Close(shown);
        }
    }

    /// <summary>Collapses the regions that hold nothing, and sizes the ones that do not.</summary>
    private void Arrange()
    {
        var top = Open(_top).Count > 0;
        var left = Open(_left).Count > 0;
        var right = Open(_right).Count > 0;
        var bottom = Open(_bottom).Count > 0;

        _top.Panel.IsVisible = top;
        _topSplitter.IsVisible = top;
        _root.RowDefinitions[0].Height = top
            ? RowLength(_manager.Layout.TopHeight)
            : new GridLength(0);

        _left.Panel.IsVisible = left;
        _leftSplitter.IsVisible = left;
        _middle.ColumnDefinitions[0].Width = left
            ? ColumnLength(_manager.Layout.LeftWidth)
            : new GridLength(0);

        _right.Panel.IsVisible = right;
        _rightSplitter.IsVisible = right;
        _middle.ColumnDefinitions[4].Width = right
            ? ColumnLength(_manager.Layout.RightWidth)
            : new GridLength(0);

        _bottom.Panel.IsVisible = bottom;
        _bottomSplitter.IsVisible = bottom;
        _root.RowDefinitions[4].Height = bottom
            ? RowLength(_manager.Layout.BottomHeight)
            : new GridLength(0);
    }

    /// <summary>Opens a window for each floating dock, and shows or hides it with the dock.</summary>
    private void Floats()
    {
        foreach (var instance in _manager.Instances)
        {
            if (instance.Placement != DockPlacement.Float)
            {
                continue;
            }

            if (!_floats.TryGetValue(instance, out var window))
            {
                window = Float(instance);
                _floats[instance] = window;
                window.Show();

                continue;
            }

            if (instance.IsOpen && !window.IsVisible)
            {
                window.Show();
            }
            else if (!instance.IsOpen && window.IsVisible)
            {
                window.Hide();
            }
        }

        foreach (var (instance, window) in _floats.ToArray())
        {
            if (_manager.Instances.Contains(instance))
            {
                continue;
            }

            _closingFloat = true;
            window.Close();
            _closingFloat = false;
            _floats.Remove(instance);
        }
    }

    /// <summary>Shows one dock in a window of its own.</summary>
    private Window Float(DockInstance instance)
    {
        var window = new Window
        {
            Title = instance.Title,
            Width = FloatSize.Width,
            Height = FloatSize.Height,
            Content = instance.View.View,
        };

        window.Closed += (_, _) =>
        {
            if (_closingFloat)
            {
                return;
            }

            _manager.Close(instance);
        };

        return window;
    }

    /// <summary>Shows one dock of its region rather than the one that was shown.</summary>
    private void Select(DockInstance instance)
    {
        RegionFor(instance.Placement).Selected = instance;
        Show();
    }

    /// <summary>
    /// Remembers that a drag may have started on a header.
    /// </summary>
    /// <remarks>
    /// A press is not a drag: what separates the two is how far the pointer goes before it is let
    /// go of, which is why nothing is done here but writing down where it started.
    /// <para>
    /// Which button asked for the gesture is settled at the release rather than here. What a
    /// pointer event says about its own buttons at the moment of the press is not to be trusted —
    /// reading it there is what kept the drag from starting at all — and the release knows what
    /// began the gesture without having to be told.
    /// </para>
    /// <para>
    /// The dock is the one named by the header that was pressed, or — from the strip's drag area —
    /// whichever the region is showing at that moment.
    /// </para>
    /// </remarks>
    private void Take(DockInstance? instance, PointerPressedEventArgs e)
    {
        _dragging = instance;
        _from = e.GetPosition(this);
        _moved = false;
    }

    /// <summary>Lights the zone a dragged dock would land in.</summary>
    private void OnPointerMoved(object? sender, PointerEventArgs e)
    {
        if (_dragging is null)
        {
            return;
        }

        var at = e.GetPosition(this);

        if (!_moved)
        {
            if (Distance(at, _from) < DragThreshold)
            {
                return;
            }

            _moved = true;
        }

        Preview(ZoneFor(at));
    }

    /// <summary>Lands a dragged dock in the region it was let go of over, or closes it on a middle click.</summary>
    private void OnPointerReleased(object? sender, PointerReleasedEventArgs e)
    {
        var instance = _dragging;
        var moved = _moved;
        var at = e.GetPosition(this);
        var button = e.InitialPressMouseButton;

        Cancel();

        if (instance is null)
        {
            return;
        }

        // The middle button is the gesture that closes a dock: a header is where a dock is grabbed,
        // so it is where one is let go of for good.
        if (button == MouseButton.Middle)
        {
            _manager.Close(instance);

            return;
        }

        // A press that never moved is a click, and selecting the dock is what a click means; a
        // release belonging to another button ends the gesture without moving anything.
        if (button == MouseButton.Left && moved && ZoneFor(at) is { } placement)
        {
            _manager.Move(instance, placement);
        }
    }

    /// <summary>Forgets a drag that is over, whatever ended it.</summary>
    private void Cancel()
    {
        _dragging = null;
        _moved = false;
        Preview(null);
    }

    /// <summary>
    /// Which region a point in the area would land a dock in.
    /// </summary>
    /// <remarks>
    /// Every edge is the outer quarter of the area on its axis, and the middle is what is left over,
    /// so a drag that stays in the middle goes to the middle. A point outside the area lands
    /// nowhere, which is what letting go of a drag outside the area means.
    /// </remarks>
    private DockPlacement? ZoneFor(Point at)
    {
        var bounds = Bounds;

        if (
            bounds.Width <= 0
            || bounds.Height <= 0
            || at.X < 0
            || at.Y < 0
            || at.X > bounds.Width
            || at.Y > bounds.Height
        )
        {
            return null;
        }

        if (at.Y < bounds.Height * ZoneEdge)
        {
            return DockPlacement.Top;
        }

        if (at.Y > bounds.Height * (1 - ZoneEdge))
        {
            return DockPlacement.Bottom;
        }

        if (at.X < bounds.Width * ZoneEdge)
        {
            return DockPlacement.Left;
        }

        return at.X > bounds.Width * (1 - ZoneEdge)
            ? DockPlacement.Right
            : DockPlacement.Center;
    }

    /// <summary>Shows the zone a dock would land in, and takes the others away.</summary>
    /// <remarks>
    /// Faded rather than shown and hidden, so that a drag reads as the zone lighting up instead of
    /// appearing; the fade itself belongs to a theme, so a theme with none still lights it, at once.
    /// </remarks>
    private void Preview(DockPlacement? placement)
    {
        foreach (var (landing, zone) in _lit)
        {
            zone.Opacity = placement is { } at && at == landing ? 1 : 0;
        }
    }

    /// <summary>The docks open in one region, in the order they were made.</summary>
    private List<DockInstance> Open(Region region) =>
        _manager
            .Instances.Where(instance =>
                instance.IsOpen
                && instance.Placement != DockPlacement.Float
                && ReferenceEquals(RegionFor(instance.Placement), region)
            )
            .ToList();

    /// <summary>Every region of the area.</summary>
    private Region[] Regions() => [_top, _left, _center, _right, _bottom];

    /// <summary>The region a placement names.</summary>
    private Region RegionFor(DockPlacement placement) =>
        placement switch
        {
            DockPlacement.Top => _top,
            DockPlacement.Left => _left,
            DockPlacement.Right => _right,
            DockPlacement.Bottom => _bottom,
            _ => _center,
        };

    /// <summary>How far apart two points are.</summary>
    private static double Distance(Point left, Point right)
    {
        var x = left.X - right.X;
        var y = left.Y - right.Y;

        return Math.Sqrt((x * x) + (y * y));
    }

    /// <summary>A header: a small button that names, commands, or closes a dock.</summary>
    private static Button Header(string text) =>
        new()
        {
            Content = text,
            Padding = new Thickness(7, 1),
            VerticalAlignment = VerticalAlignment.Center,
        };

    /// <summary>A column's size, never narrower than the least it may be.</summary>
    private static GridLength ColumnLength(double size) => new(Math.Max(MinColumn, size));

    /// <summary>A row's size, never shorter than the least it may be.</summary>
    private static GridLength RowLength(double size) => new(Math.Max(MinRow, size));
}

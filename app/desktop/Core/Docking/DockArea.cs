using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The dock area: the regions a dock can be placed in, and the headers that choose between them.
/// </summary>
/// <remarks>
/// A region keeps every dock it was given and shows one of them at a time, rather than rebuilding
/// itself when the set of open docks changes. That is not an optimisation: a control has one
/// parent, and a view moved from one container into another can be caught mid-move — which is what
/// <c>The control … already has a visual parent</c> is. Nothing here is ever re-parented. A dock is
/// put in its region once, when it is made; taken out once, when it is closed for good; and shown,
/// hidden, or made the region's own in between, which is all that opening and closing it means.
/// <para>
/// A region with nothing open in it takes no space, and its splitter goes with it, so an empty left
/// or right strip never sits there eating clicks.
/// </para>
/// </remarks>
internal sealed class DockArea : UserControl
{
    /// <summary>The smallest a region is allowed to become.</summary>
    private const double MinRegion = 120;

    /// <summary>How wide a splitter is.</summary>
    private const double SplitterSize = 4;

    /// <summary>How large a floating dock's window opens.</summary>
    private static readonly Size FloatSize = new(560, 400);

    /// <summary>The docks and where they are.</summary>
    private readonly DockManager _manager;

    /// <summary>The host's translations, for dock titles and header commands.</summary>
    private readonly I18nService _i18n;

    /// <summary>The whole area: the three columns above, and the bottom region below them.</summary>
    private readonly Grid _root = new();

    /// <summary>The three columns: left, centre, right.</summary>
    private readonly Grid _middle = new();

    /// <summary>The region to the left.</summary>
    private readonly Region _left = new("left");

    /// <summary>The central region.</summary>
    private readonly Region _center = new("center");

    /// <summary>The region to the right.</summary>
    private readonly Region _right = new("right");

    /// <summary>The region along the bottom.</summary>
    private readonly Region _bottom = new("bottom");

    /// <summary>The splitter between the left region and the centre.</summary>
    private readonly GridSplitter _leftSplitter = new();

    /// <summary>The splitter between the centre and the right region.</summary>
    private readonly GridSplitter _rightSplitter = new();

    /// <summary>The splitter above the bottom region.</summary>
    private readonly GridSplitter _bottomSplitter = new();

    /// <summary>What each dock that is placed in a region was given, by the dock.</summary>
    private readonly Dictionary<DockInstance, Placed> _placed = [];

    /// <summary>The window each floating dock lives in, by the dock.</summary>
    private readonly Dictionary<DockInstance, Window> _floats = [];

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
        _manager.Changed += Rebuild;
        Rebuild();
    }

    /// <summary>What one dock was given when it was placed.</summary>
    /// <param name="Region">The region it sits in.</param>
    /// <param name="View">Its control, which is a child of the region's content panel.</param>
    /// <param name="Title">The header that selects it.</param>
    /// <param name="Commands">The header commands it brought with it.</param>
    /// <param name="Close">The header that closes it.</param>
    private sealed record Placed(
        Region Region,
        Control View,
        Button Title,
        IReadOnlyList<Button> Commands,
        Button Close
    );

    /// <summary>One dock and the window showing it.</summary>
    /// <param name="Instance">The dock.</param>
    /// <param name="Window">The window it is shown in.</param>
    private sealed record Floating(DockInstance Instance, Window Window);

    /// <summary>
    /// One region: a strip of headers over the docks themselves.
    /// </summary>
    /// <remarks>
    /// Every dock the region was given stays in <see cref="Content"/>, and the one being shown is
    /// the only one that is visible. <see cref="Selected"/> is which that is.
    /// </remarks>
    private sealed class Region
    {
        /// <summary>Makes a region, which is named for the size that is remembered for it.</summary>
        /// <param name="name">The name its size is remembered under.</param>
        public Region(string name)
        {
            Name = name;

            DockPanel.SetDock(Headers, Dock.Top);
            Panel.Children.Add(Headers);
            Panel.Children.Add(Content);
        }

        /// <summary>The name this region's size is remembered under.</summary>
        public string Name { get; }

        /// <summary>The region: headers above, docks below.</summary>
        public DockPanel Panel { get; } = new();

        /// <summary>The headers, one per dock the region was given.</summary>
        public StackPanel Headers { get; } =
            new()
            {
                Orientation = Orientation.Horizontal,
                Spacing = 2,
                Margin = new Thickness(4, 2),
            };

        /// <summary>Every dock the region was given, of which one is visible at a time.</summary>
        public Panel Content { get; } = new();

        /// <summary>Which of them is being shown.</summary>
        public DockInstance? Selected { get; set; }
    }

    /// <summary>Lays the regions and the splitters out once.</summary>
    private void Build()
    {
        _middle.ColumnDefinitions.Add(new ColumnDefinition(RegionLength(_manager.Layout.LeftWidth)));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Star));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(RegionLength(_manager.Layout.RightWidth)));

        Grid.SetColumn(_left.Panel, 0);
        Grid.SetColumn(_leftSplitter, 1);
        Grid.SetColumn(_center.Panel, 2);
        Grid.SetColumn(_rightSplitter, 3);
        Grid.SetColumn(_right.Panel, 4);

        ConfigureSplitter(_leftSplitter, GridResizeDirection.Columns, _left);
        ConfigureSplitter(_rightSplitter, GridResizeDirection.Columns, _right);
        ConfigureSplitter(_bottomSplitter, GridResizeDirection.Rows, _bottom);

        _middle.Children.Add(_left.Panel);
        _middle.Children.Add(_leftSplitter);
        _middle.Children.Add(_center.Panel);
        _middle.Children.Add(_rightSplitter);
        _middle.Children.Add(_right.Panel);

        _root.RowDefinitions.Add(new RowDefinition(GridLength.Star));
        _root.RowDefinitions.Add(new RowDefinition(GridLength.Auto));
        _root.RowDefinitions.Add(new RowDefinition(RegionLength(_manager.Layout.BottomHeight)));

        Grid.SetRow(_middle, 0);
        Grid.SetRow(_bottomSplitter, 1);
        Grid.SetRow(_bottom.Panel, 2);

        _root.Children.Add(_middle);
        _root.Children.Add(_bottomSplitter);
        _root.Children.Add(_bottom.Panel);

        foreach (var region in new[] { _left, _center, _right, _bottom })
        {
            region.Panel.Background = Brushes.Transparent;
        }

        Content = _root;
    }

    /// <summary>Sets up one splitter, remembering the region's size when it is let go.</summary>
    private void ConfigureSplitter(GridSplitter splitter, GridResizeDirection direction, Region region)
    {
        splitter.ResizeDirection = direction;
        splitter.Background = Brushes.Transparent;

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
            case "left":
                _manager.Layout.LeftWidth = _middle.ColumnDefinitions[0].ActualWidth;
                break;

            case "right":
                _manager.Layout.RightWidth = _middle.ColumnDefinitions[4].ActualWidth;
                break;

            case "bottom":
                _manager.Layout.BottomHeight = _root.RowDefinitions[2].ActualHeight;
                break;

            default:
                break;
        }
    }

    /// <summary>Brings the area back in step with what is registered, open and shown.</summary>
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

            placed.Region.Content.Children.Remove(placed.View);
            placed.Region.Headers.Children.Remove(placed.Title);

            foreach (var command in placed.Commands)
            {
                placed.Region.Headers.Children.Remove(command);
            }

            placed.Region.Headers.Children.Remove(placed.Close);

            if (ReferenceEquals(placed.Region.Selected, instance))
            {
                placed.Region.Selected = null;
            }

            _placed.Remove(instance);
        }
    }

    /// <summary>Gives the docks that are new to the area their place in it.</summary>
    private void Place()
    {
        foreach (var instance in _manager.Instances)
        {
            if (_placed.ContainsKey(instance) || instance.Placement == DockPlacement.Float)
            {
                continue;
            }

            var region = RegionFor(instance.Placement);
            var title = Header(instance.Title);
            var commands = new List<Button>();

            title.Click += (_, _) => Select(region, instance);

            foreach (var command in instance.View.HeaderCommands)
            {
                var button = Header(_i18n.Get(command.LabelKey));
                button.Click += (_, _) => command.Command();
                commands.Add(button);
            }

            var close = Header("\u2715");
            close.Click += (_, _) => _manager.Close(instance);

            region.Content.Children.Add(instance.View.View);
            region.Headers.Children.Add(title);

            foreach (var command in commands)
            {
                region.Headers.Children.Add(command);
            }

            region.Headers.Children.Add(close);

            _placed[instance] = new Placed(region, instance.View.View, title, commands, close);
        }
    }

    /// <summary>Makes sure each region is showing one of the docks open in it.</summary>
    private void Settle()
    {
        foreach (var region in new[] { _left, _center, _right, _bottom })
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
            placed.Title.FontWeight = shown ? FontWeight.SemiBold : FontWeight.Normal;

            foreach (var command in placed.Commands)
            {
                command.IsVisible = instance.IsOpen;
            }

            placed.Close.IsVisible = instance.IsOpen;
        }
    }

    /// <summary>Collapses the regions that hold nothing, and sizes the ones that do not.</summary>
    private void Arrange()
    {
        var left = Open(_left).Count > 0;
        var right = Open(_right).Count > 0;
        var bottom = Open(_bottom).Count > 0;

        _left.Panel.IsVisible = left;
        _leftSplitter.IsVisible = left;
        _middle.ColumnDefinitions[0].Width = left
            ? RegionLength(_manager.Layout.LeftWidth)
            : new GridLength(0);

        _right.Panel.IsVisible = right;
        _rightSplitter.IsVisible = right;
        _middle.ColumnDefinitions[4].Width = right
            ? RegionLength(_manager.Layout.RightWidth)
            : new GridLength(0);

        _bottom.Panel.IsVisible = bottom;
        _bottomSplitter.IsVisible = bottom;
        _root.RowDefinitions[2].Height = bottom
            ? RegionLength(_manager.Layout.BottomHeight)
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

    /// <summary>Shows one dock of a region rather than the one that was shown.</summary>
    private void Select(Region region, DockInstance instance)
    {
        region.Selected = instance;
        Show();
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

    /// <summary>The region a placement names.</summary>
    private Region RegionFor(DockPlacement placement) =>
        placement switch
        {
            DockPlacement.Left => _left,
            DockPlacement.Right => _right,
            DockPlacement.Bottom => _bottom,
            _ => _center,
        };

    /// <summary>A header: a small button that names, commands, or closes a dock.</summary>
    private static Button Header(string text) =>
        new()
        {
            Content = text,
            Padding = new Thickness(7, 1),
            VerticalAlignment = VerticalAlignment.Center,
        };

    /// <summary>A region's size, never smaller than the least it may be.</summary>
    private static GridLength RegionLength(double size) => new(Math.Max(MinRegion, size));
}

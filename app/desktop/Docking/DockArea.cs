using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The dock area: the regions docks are placed in, and the tabs showing them.
/// </summary>
/// <remarks>
/// Each region is a tab control, so several docks can share a region and a created dock gets a tab
/// of its own. A region with nothing in it takes no space, and its splitter goes with it, so an
/// empty left or right strip never sits there eating clicks.
/// <para>
/// The area is rebuilt from the manager whenever the set of open docks changes. Rebuilding rather
/// than patching is deliberate for now: the set changes rarely, and one path that always yields what
/// the manager holds is easier to keep right than several that each move one tab.
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
    private readonly TabControl _left = new();

    /// <summary>The central region.</summary>
    private readonly TabControl _center = new();

    /// <summary>The region to the right.</summary>
    private readonly TabControl _right = new();

    /// <summary>The region along the bottom.</summary>
    private readonly TabControl _bottom = new();

    /// <summary>The splitter between the left region and the centre.</summary>
    private readonly GridSplitter _leftSplitter = new();

    /// <summary>The splitter between the centre and the right region.</summary>
    private readonly GridSplitter _rightSplitter = new();

    /// <summary>The splitter above the bottom region.</summary>
    private readonly GridSplitter _bottomSplitter = new();

    /// <summary>The floating docks and the windows showing them.</summary>
    private readonly List<Floating> _floats = [];

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

    /// <summary>A floating dock and the window showing it.</summary>
    /// <param name="Instance">The dock.</param>
    /// <param name="Window">The window it is shown in.</param>
    private sealed record Floating(DockInstance Instance, Window Window);

    /// <summary>Lays the regions and the splitters out once.</summary>
    private void Build()
    {
        _middle.ColumnDefinitions.Add(new ColumnDefinition(Region(_manager.Layout.LeftWidth)));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Star));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        _middle.ColumnDefinitions.Add(new ColumnDefinition(Region(_manager.Layout.RightWidth)));

        Grid.SetColumn(_left, 0);
        Grid.SetColumn(_leftSplitter, 1);
        Grid.SetColumn(_center, 2);
        Grid.SetColumn(_rightSplitter, 3);
        Grid.SetColumn(_right, 4);

        ConfigureSplitter(_leftSplitter, GridResizeDirection.Columns, "left");
        ConfigureSplitter(_rightSplitter, GridResizeDirection.Columns, "right");
        ConfigureSplitter(_bottomSplitter, GridResizeDirection.Rows, "bottom");

        _middle.Children.Add(_left);
        _middle.Children.Add(_leftSplitter);
        _middle.Children.Add(_center);
        _middle.Children.Add(_rightSplitter);
        _middle.Children.Add(_right);

        _root.RowDefinitions.Add(new RowDefinition(GridLength.Star));
        _root.RowDefinitions.Add(new RowDefinition(GridLength.Auto));
        _root.RowDefinitions.Add(new RowDefinition(Region(_manager.Layout.BottomHeight)));

        Grid.SetRow(_middle, 0);
        Grid.SetRow(_bottomSplitter, 1);
        Grid.SetRow(_bottom, 2);

        _root.Children.Add(_middle);
        _root.Children.Add(_bottomSplitter);
        _root.Children.Add(_bottom);

        foreach (var region in new[] { _left, _center, _right, _bottom })
        {
            region.Background = Brushes.Transparent;
        }

        Content = _root;
    }

    /// <summary>Sets up one splitter, remembering the region's size when it is let go.</summary>
    private void ConfigureSplitter(GridSplitter splitter, GridResizeDirection direction, string region)
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
    private void Remember(string region)
    {
        switch (region)
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
        }
    }

    /// <summary>Shows the docks that are open, and hides the regions that hold none.</summary>
    private void Rebuild()
    {
        Fill(_left, DockPlacement.Left);
        Fill(_center, DockPlacement.Center);
        Fill(_right, DockPlacement.Right);
        Fill(_bottom, DockPlacement.Bottom);

        Arrange();
        SyncFloats();
    }

    /// <summary>Puts every open dock in one region into that region's tabs.</summary>
    private void Fill(TabControl region, DockPlacement placement)
    {
        region.Items.Clear();

        foreach (
            var instance in _manager
                .Instances.Where(instance => instance.IsOpen && instance.Placement == placement)
                .OrderBy(instance => instance.Ordinal)
        )
        {
            region.Items.Add(Tab(instance));
        }
    }

    /// <summary>Collapses the regions that hold nothing, and shows the splitters that do something.</summary>
    private void Arrange()
    {
        var left = _left.Items.Count > 0;
        var right = _right.Items.Count > 0;
        var bottom = _bottom.Items.Count > 0;

        _left.IsVisible = left;
        _leftSplitter.IsVisible = left;
        _middle.ColumnDefinitions[0].Width = left
            ? Region(_manager.Layout.LeftWidth)
            : new GridLength(0);

        _right.IsVisible = right;
        _rightSplitter.IsVisible = right;
        _middle.ColumnDefinitions[4].Width = right
            ? Region(_manager.Layout.RightWidth)
            : new GridLength(0);

        _bottom.IsVisible = bottom;
        _bottomSplitter.IsVisible = bottom;
        _root.RowDefinitions[2].Height = bottom
            ? Region(_manager.Layout.BottomHeight)
            : new GridLength(0);
    }

    /// <summary>Makes one dock's tab: its title, its header commands, and a way to close it.</summary>
    private TabItem Tab(DockInstance instance)
    {
        var header = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };

        header.Children.Add(
            new TextBlock
            {
                Text = instance.Title,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        foreach (var command in instance.View.HeaderCommands)
        {
            var label = _i18n.Get(command.LabelKey);
            var button = new Button
            {
                Content = label,
                Padding = new Thickness(6, 1),
                VerticalAlignment = VerticalAlignment.Center,
            };

            button.Click += (_, _) => command.Command();
            header.Children.Add(button);
        }

        var close = new Button
        {
            Content = "\u2715",
            Padding = new Thickness(6, 1),
            VerticalAlignment = VerticalAlignment.Center,
        };

        close.Click += (_, _) => _manager.Close(instance);
        header.Children.Add(close);

        return new TabItem { Header = header, Content = instance.View.View };
    }

    /// <summary>Opens a window for each open dock placed as a float, and closes the rest.</summary>
    private void SyncFloats()
    {
        foreach (
            var instance in _manager
                .Instances.Where(instance =>
                    instance.IsOpen && instance.Placement == DockPlacement.Float
                )
                .OrderBy(instance => instance.Ordinal)
        )
        {
            if (_floats.Any(found => found.Instance == instance))
            {
                continue;
            }

            OpenFloat(instance);
        }

        for (var index = _floats.Count - 1; index >= 0; index -= 1)
        {
            var floating = _floats[index];

            if (
                floating.Instance.IsOpen
                && floating.Instance.Placement == DockPlacement.Float
                && _manager.Instances.Contains(floating.Instance)
            )
            {
                continue;
            }

            _closingFloat = true;
            floating.Window.Close();
            _closingFloat = false;
            _floats.RemoveAt(index);
        }
    }

    /// <summary>Shows one dock in a window of its own.</summary>
    private void OpenFloat(DockInstance instance)
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

        _floats.Add(new Floating(instance, window));
        window.Show();
    }

    /// <summary>A region's size, never smaller than the least it may be.</summary>
    private static GridLength Region(double size) => new(Math.Max(MinRegion, size));
}

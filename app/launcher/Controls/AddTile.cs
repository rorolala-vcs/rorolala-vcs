using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Theming;
using ShapePath = Avalonia.Controls.Shapes.Path;
using ShapeRectangle = Avalonia.Controls.Shapes.Rectangle;

namespace RorolalaLauncher.Controls;

/// <summary>
/// The dashed button under the racks that adds an address.
/// </summary>
/// <remarks>
/// It is a shape rather than a button because the edge is dashed: the toolkit's border draws a solid line,
/// and a dashed edge is what says this is a place to add something rather than one more entry. What is
/// pressed is the whole tile, and the hover fills it with the selection wash, as an ordinary button does.
/// </remarks>
internal sealed class AddTile : Panel
{
    /// <summary>What the tile is filled with when it is neither hovered nor pressed.</summary>
    private readonly Border _bed;

    /// <summary>The text beside the plus.</summary>
    private readonly TextBlock _label;

    /// <summary>The plus.</summary>
    private readonly ShapePath _plus;

    /// <summary>The dashed edge.</summary>
    private readonly ShapeRectangle _outline;

    /// <summary>The colours in force.</summary>
    private Skin _skin;

    /// <summary>Whether the pointer is over the tile.</summary>
    private bool _over;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>Makes the tile.</summary>
    /// <param name="label">What it says.</param>
    /// <param name="skin">The colours in force.</param>
    public AddTile(string label, Skin skin)
    {
        ArgumentException.ThrowIfNullOrEmpty(label);
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;

        Height = 38;
        Margin = new Thickness(0, 10, 0, 0);
        Cursor = new Cursor(StandardCursorType.Hand);
        Focusable = true;

        _bed = new Border
        {
            CornerRadius = new CornerRadius(7),
            Background = Brushes.Transparent,
        };

        _plus = Marks.Plus(skin.Muted, 14);
        _label = new TextBlock
        {
            Text = label,
            FontSize = 12.5,
            Foreground = skin.Muted,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var face = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 7,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        face.Children.Add(_plus);
        face.Children.Add(_label);

        _outline = new ShapeRectangle
        {
            StrokeThickness = 1,
            StrokeDashArray = new Avalonia.Collections.AvaloniaList<double> { 3, 2 },
            RadiusX = 7,
            RadiusY = 7,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsHitTestVisible = false,
            Margin = new Thickness(0.5),
        };

        Children.Add(_bed);
        Children.Add(face);
        Children.Add(_outline);

        PointerEntered += (_, _) =>
        {
            _over = true;
            Paint();
        };

        PointerExited += (_, _) =>
        {
            _over = false;
            _pressed = false;
            Paint();
        };

        PointerPressed += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
            {
                return;
            }

            _pressed = true;
            Paint();
            e.Handled = true;
        };

        PointerReleased += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            var pressed = _pressed && e.InitialPressMouseButton == MouseButton.Left;

            _pressed = false;
            Paint();

            if (pressed)
            {
                e.Handled = true;
                Clicked?.Invoke(this, EventArgs.Empty);
            }
        };

        KeyDown += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (e.Key is Key.Enter or Key.Space)
            {
                e.Handled = true;
                Clicked?.Invoke(this, EventArgs.Empty);
            }
        };

        Repaint(skin);
    }

    /// <summary>Raised when it is pressed and released on.</summary>
    public event EventHandler? Clicked;

    /// <summary>Draws the tile in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;
        Paint();
    }

    /// <summary>Draws the tile as it stands.</summary>
    private void Paint()
    {
        var live = _over || _pressed;

        _bed.Background = live ? _skin.Selection : Brushes.Transparent;
        _outline.Stroke = live
            ? new SolidColorBrush(_skin.Palette.Primary, 0.62)
            : new SolidColorBrush(Skin.Mix(_skin.Palette.Splitter, _skin.Palette.Front, 0.18));
        _plus.Stroke = live ? _skin.Front : _skin.Muted;
        _label.Foreground = live ? _skin.Front : _skin.Muted;
    }
}

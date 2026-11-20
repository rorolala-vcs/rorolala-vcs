using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// One of the choices in a segmented control: a word on a small surface, filled when it is in force.
/// </summary>
internal sealed class SegmentButton : Border
{
    /// <summary>What the segment says.</summary>
    private readonly TextBlock _label;

    /// <summary>Whether this is the choice in force.</summary>
    private readonly bool _chosen;

    /// <summary>The colours in force.</summary>
    private readonly Skin _skin;

    /// <summary>Whether the pointer is over it.</summary>
    private bool _over;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>Makes one segment.</summary>
    /// <param name="label">What it says.</param>
    /// <param name="chosen">Whether it is the choice in force.</param>
    /// <param name="skin">The colours in force.</param>
    public SegmentButton(string label, bool chosen, Skin skin)
    {
        ArgumentException.ThrowIfNullOrEmpty(label);
        ArgumentNullException.ThrowIfNull(skin);

        _chosen = chosen;
        _skin = skin;

        Height = 26;
        Padding = new Thickness(9, 0, 9, 0);
        CornerRadius = new CornerRadius(4);
        Cursor = new Cursor(StandardCursorType.Hand);

        _label = new TextBlock
        {
            Text = label,
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        Child = _label;

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

        Paint();
    }

    /// <summary>Raised when it is pressed and released on.</summary>
    public event EventHandler? Clicked;

    /// <summary>Draws the segment.</summary>
    private void Paint()
    {
        Background = _chosen
            ? _skin.Bright
            : _over
                ? _skin.Hover
                : Brushes.Transparent;
        _label.Foreground = _chosen || _over ? _skin.Front : _skin.Muted;
        _label.FontWeight = _chosen ? FontWeight.SemiBold : FontWeight.Normal;
    }
}

/// <summary>
/// A row of segments with one of them in force, for a choice of two or three settings.
/// </summary>
/// <remarks>
/// The control is one border with a sunken ground and a row of segments on it, which is what makes the
/// choice read as one control rather than as three buttons beside one another. It does not keep the
/// choice: what is chosen is written by the window that owns the setting, and the control is drawn again
/// from what that window says.
/// </remarks>
internal sealed class Segmented : Border
{
    /// <summary>Makes a segmented control.</summary>
    /// <param name="options">The values and the words they are read as.</param>
    /// <param name="current">The value in force.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="chosen">What choosing a value does.</param>
    public Segmented(
        IReadOnlyList<(string Value, string Label)> options,
        string current,
        Skin skin,
        Action<string> chosen
    )
    {
        ArgumentNullException.ThrowIfNull(options);
        ArgumentNullException.ThrowIfNull(skin);
        ArgumentNullException.ThrowIfNull(chosen);

        Background = skin.Dark;
        BorderBrush = skin.Splitter;
        BorderThickness = new Thickness(1);
        CornerRadius = new CornerRadius(6);
        Padding = new Thickness(2);

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
        };

        foreach (var (value, label) in options)
        {
            var segment = new SegmentButton(
                label,
                string.Equals(value, current, StringComparison.Ordinal),
                skin
            );

            segment.Clicked += (_, _) => chosen(value);

            row.Children.Add(segment);
        }

        Child = row;
    }
}

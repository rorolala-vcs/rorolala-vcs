using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>How a button stands out from what it stands on.</summary>
internal enum ButtonKind
{
    /// <summary>No surface of its own: an ink on the ground, for what is beside the thing it is about.</summary>
    Quiet,

    /// <summary>A surface of its own with a line around it, for an ordinary choice.</summary>
    Surface,

    /// <summary>Filled with the primary, for the choice the window is asking for.</summary>
    Primary,

    /// <summary>In the colour a thing that went wrong is written in, for taking something away.</summary>
    Danger,
}

/// <summary>
/// A button drawn as this program draws one: a rectangle, an ink and a hover, and nothing else.
/// </summary>
/// <remarks>
/// The toolkit's own button carries the base theme's look — its own corner, its own colours, its own idea of
/// what a pressed button is — and a program whose every surface is drawn from one palette cannot leave one
/// control drawn from another. So a button is a border that knows when it is pressed, which is small enough
/// to be worth saying here rather than fought with a style per control.
/// </remarks>
internal sealed class FlatButton : Border
{
    /// <summary>What the button carries: a string is written, a control is drawn.</summary>
    private readonly Control _content;

    /// <summary>What it looks like when it is neither hovered nor pressed.</summary>
    private IBrush _idle = Brushes.Transparent;

    /// <summary>What it looks like under a pointer.</summary>
    private IBrush _hover = Brushes.Transparent;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>Whether it may be pressed at all.</summary>
    private bool _enabled = true;

    /// <summary>Whether the pointer is over it.</summary>
    private bool _over;

    /// <summary>Makes a button.</summary>
    /// <param name="content">What it carries.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="kind">How it stands out.</param>
    public FlatButton(Control content, Skin skin, ButtonKind kind = ButtonKind.Quiet)
    {
        ArgumentNullException.ThrowIfNull(content);
        ArgumentNullException.ThrowIfNull(skin);

        _content = content;
        Kind = kind;

        CornerRadius = new CornerRadius(6);
        Height = 34;
        Padding = new Thickness(14, 0, 14, 0);
        VerticalAlignment = VerticalAlignment.Center;
        Child = content;
        Cursor = new Cursor(StandardCursorType.Hand);

        Repaint(skin);
    }

    /// <summary>Raised when it is pressed and released on.</summary>
    public event EventHandler? Clicked;

    /// <inheritdoc />
    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        ArgumentNullException.ThrowIfNull(change);

        base.OnPropertyChanged(change);

        // What a button carries sits in the middle of it: the text of a label is drawn at the top of the room
        // a stretched text block gives it, and a button is taller than its line, so a label left to itself
        // reads as having been written against the top edge rather than in the button. It is done here rather
        // than once in the constructor because a button that says what it is doing is given a new line to
        // carry, and a line that arrived that way would otherwise be drawn against the top of the button.
        if (change.Property == ChildProperty && change.NewValue is Control content)
        {
            content.VerticalAlignment = VerticalAlignment.Center;
        }
    }

    /// <summary>How it stands out.</summary>
    public ButtonKind Kind { get; }

    /// <summary>Whether it may be pressed.</summary>
    public bool Enabled
    {
        get => _enabled;

        set
        {
            _enabled = value;
            Opacity = value ? 1 : 0.4;
            Cursor = new Cursor(value ? StandardCursorType.Hand : StandardCursorType.Arrow);
        }
    }

    /// <summary>Draws the button in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        var (idle, hover) = Look(skin, Kind);

        _idle = idle;
        _hover = hover;

        Background = _over ? _hover : _idle;
        BorderThickness = Kind is ButtonKind.Surface or ButtonKind.Primary or ButtonKind.Danger
            ? new Thickness(1)
            : default;
        BorderBrush = Kind switch
        {
            ButtonKind.Primary => new SolidColorBrush(skin.Palette.Primary),
            ButtonKind.Danger => skin.Alarm,
            _ => skin.Splitter,
        };

        Ink(skin);
    }

    /// <summary>The two surfaces a kind is drawn on: when nothing is over it, and when something is.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="kind">How the button stands out.</param>
    /// <returns>What it is filled with, and what it is filled with under a pointer.</returns>
    private static (IBrush Idle, IBrush Hover) Look(Skin skin, ButtonKind kind) =>
        kind switch
        {
            ButtonKind.Primary => (
                new SolidColorBrush(skin.Palette.Primary),
                new SolidColorBrush(skin.Palette.Primary, 0.86)
            ),
            ButtonKind.Surface => (
                new SolidColorBrush(skin.Palette.Bright),
                new SolidColorBrush(skin.Palette.Dark)
            ),
            ButtonKind.Danger => (
                new SolidColorBrush(skin.Palette.Bright),
                new SolidColorBrush(skin.Palette.Front, 0.1)
            ),
            _ => (Brushes.Transparent, new SolidColorBrush(skin.Palette.Front, 0.09)),
        };

    /// <inheritdoc />
    protected override void OnPointerEntered(PointerEventArgs e)
    {
        _over = true;
        Background = _hover;

        base.OnPointerEntered(e);
    }

    /// <inheritdoc />
    protected override void OnPointerExited(PointerEventArgs e)
    {
        _over = false;
        _pressed = false;
        Background = _idle;

        base.OnPointerExited(e);
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (!_enabled || !e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            base.OnPointerPressed(e);

            return;
        }

        _pressed = true;
        Background = _idle;
        e.Handled = true;
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        var pressed = _pressed && _enabled;

        _pressed = false;
        Background = _over ? _hover : _idle;

        if (pressed && e.InitialPressMouseButton == MouseButton.Left)
        {
            e.Handled = true;
            Clicked?.Invoke(this, EventArgs.Empty);
        }

        base.OnPointerReleased(e);
    }

    /// <summary>Draws what the button carries in the ink its kind is read in.</summary>
    /// <param name="skin">The colours in force.</param>
    private void Ink(Skin skin)
    {
        var ink = Kind switch
        {
            ButtonKind.Primary => skin.OnPrimary,
            ButtonKind.Danger => skin.Alarm,
            _ => skin.Front,
        };

        switch (_content)
        {
            case TextBlock text:
                text.Foreground = ink;
                break;

            case Avalonia.Controls.Shapes.Path path:
                path.Stroke = ink;
                break;

            case Panel panel:
                foreach (var child in panel.Children)
                {
                    Ink(child, ink);
                }

                break;

            default:
                break;
        }
    }

    /// <summary>Draws a control inside the button, which may hold drawings of its own.</summary>
    /// <param name="control">What is drawn.</param>
    /// <param name="ink">What it is drawn in.</param>
    private static void Ink(Control control, IBrush ink)
    {
        switch (control)
        {
            case TextBlock text:
                text.Foreground = ink;
                break;

            case Avalonia.Controls.Shapes.Path path:
                path.Stroke = ink;
                break;
        }
    }
}

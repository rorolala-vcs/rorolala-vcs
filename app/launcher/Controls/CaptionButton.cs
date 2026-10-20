using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Theming;
using ShapePath = Avalonia.Controls.Shapes.Path;

namespace RorolalaLauncher.Controls;

/// <summary>What one of the marks on a window's own strip does.</summary>
internal enum CaptionKind
{
    /// <summary>Puts the window away.</summary>
    Minimize,

    /// <summary>Closes the window.</summary>
    Close,

    /// <summary>Opens what the program may be told, which is this window's own and not a window control.</summary>
    Settings,
}

/// <summary>
/// One mark on the strip at the corner of the window, drawn the way the Desktop program draws the same mark.
/// </summary>
/// <remarks>
/// The two frames the product has are one frame in two spellings, so a mark here is the mark there: the
/// Windows frame draws its buttons as flat rectangles that fill with the wash of the pointer, wearing the
/// Desktop program's own two pictures in the ink the button has; the macOS frame draws the lights with their
/// own colours and a mark that appears only while the pointer is over them. What the program adds is one
/// button of its own — the settings — drawn beside them in the same family rather than as a different kind
/// of thing.
/// <para>
/// A picture is stamped as an opacity mask over the ink rather than drawn as an image, which is how the
/// Desktop program stamps the same two files: a button under the pointer, and the one that closes the
/// window, take the colour the look gives them instead of one baked into the file.
/// </para>
/// </remarks>
internal sealed class CaptionButton : Border
{
    /// <summary>How wide a window button's picture is on a frame of the design's.</summary>
    private const double MarkExtent = 12.0;

    /// <summary>How wide it is on a light, where a light is smaller than a button.</summary>
    private const double LightExtent = 8.0;

    /// <summary>The ink a mark is drawn in while the pointer is elsewhere, which is the look's muted ink.</summary>
    private static readonly Color Resting = Color.FromArgb(0xBE, 0xFF, 0xFF, 0xFF);

    /// <summary>The ink a mark is drawn in while the pointer is over it, which is the look's own ink.</summary>
    private static readonly Color Aroused = Colors.White;

    /// <summary>What a press that is held fills a mark with, as the look fills a held caption button.</summary>
    private static readonly Color Held = Color.FromArgb(0x44, 0xFF, 0xFF, 0xFF);

    /// <summary>What a mark that is neither hovered nor held is filled with, which is nothing.</summary>
    private static readonly IBrush Nothing = Brushes.Transparent;

    /// <summary>The colour of the light that closes the window, which is the one the Desktop program draws.</summary>
    private static readonly Color CloseLight = Color.FromRgb(0xFF, 0x5F, 0x57);

    /// <summary>The colour of the light that puts the window away, which is the one the Desktop program draws.</summary>
    private static readonly Color MinimizeLight = Color.FromRgb(0xFE, 0xBC, 0x2E);

    /// <summary>The ink the mark on a light is drawn in, which is the one the Desktop program draws it in.</summary>
    private static readonly Color LightInk = Color.FromArgb(0x66, 0x00, 0x00, 0x00);

    /// <summary>What the button does.</summary>
    private readonly CaptionKind _kind;

    /// <summary>How the frame is drawn, which decides which of the two spellings this mark is.</summary>
    private readonly WindowStyle _style;

    /// <summary>The colours in force, which the close mark's wash is taken from.</summary>
    private readonly Skin _skin;

    /// <summary>Where the mark is drawn over a light, which is only drawn while it is reached for.</summary>
    private readonly bool _light;

    /// <summary>The picture the button carries, where it carries one of the product's own.</summary>
    private readonly Border? _picture;

    /// <summary>The mark drawn as a line, where it is not one of the product's pictures.</summary>
    private readonly ShapePath? _mark;

    /// <summary>Whether the pointer is over the mark.</summary>
    private bool _over;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>Makes one mark of a window's own strip.</summary>
    /// <param name="style">How the frame is drawn.</param>
    /// <param name="kind">What the mark does.</param>
    /// <param name="skin">The colours in force.</param>
    public CaptionButton(WindowStyle style, CaptionKind kind, Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _style = style;
        _kind = kind;
        _skin = skin;

        var mac = style == WindowStyle.Mac;

        _light = mac && kind != CaptionKind.Settings;

        Width = mac ? 20 : 44;
        Height = mac ? 20 : 29;
        CornerRadius = mac ? new CornerRadius(10) : new CornerRadius(0);
        BorderThickness = default;
        Padding = default;
        Background = Nothing;
        Cursor = new Cursor(StandardCursorType.Hand);
        HorizontalAlignment = HorizontalAlignment.Center;
        VerticalAlignment = VerticalAlignment.Center;

        var face = new Panel
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        if (_light)
        {
            face.Children.Add(
                new Border
                {
                    Width = 12,
                    Height = 12,
                    CornerRadius = new CornerRadius(6),
                    Background = new SolidColorBrush(
                        kind == CaptionKind.Close ? CloseLight : MinimizeLight
                    ),
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                }
            );

            _picture = Picture(kind, LightExtent);
            _picture.Opacity = 0;
            face.Children.Add(_picture);
        }
        else if (kind == CaptionKind.Settings)
        {
            // The settings is this program's own button and the set holds no picture for it, so it is the
            // one mark on the strip drawn as a line rather than stamped from a file.
            _mark = Marks.Cog(new SolidColorBrush(Resting), 14);
            face.Children.Add(_mark);
        }
        else
        {
            _picture = Picture(kind, MarkExtent);
            face.Children.Add(_picture);
        }

        Child = face;

        Repaint();
    }

    /// <summary>Raised when the mark is pressed and released on.</summary>
    public event EventHandler? Pressed;

    /// <summary>The same colour at another opacity, which is how a wash is made from an ink.</summary>
    /// <param name="colour">The colour.</param>
    /// <param name="alpha">How opaque the wash is.</param>
    /// <returns>The wash.</returns>
    private static Color Wash(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);

    /// <summary>
    /// One of the Desktop program's two pictures, stamped in whatever ink the button is drawn in.
    /// </summary>
    /// <param name="kind">Which picture.</param>
    /// <param name="extent">How wide it is drawn.</param>
    /// <returns>The picture, which is its own ink's to colour.</returns>
    private static Border Picture(CaptionKind kind, double extent) =>
        LauncherIcons.Glyph(
            kind == CaptionKind.Close ? LauncherIcons.Close : LauncherIcons.Minimize,
            extent,
            Brushes.Transparent
        );

    /// <summary>Draws the mark as it stands: the frame's own states, and which light it is.</summary>
    private void Repaint()
    {
        if (_light)
        {
            // The light itself is the state: its colour is constant and its mark is what a reach for it
            // reveals, which is the one thing the frame does for these buttons.
            // UNWRAP: a light is made with a picture, which is what says it is one.
            _picture!.Background = new SolidColorBrush(LightInk);
            _picture.Opacity = _over ? 1 : 0;

            return;
        }

        var closing = _kind == CaptionKind.Close && _style != WindowStyle.Mac;

        var ink = closing && _over ? _skin.AlarmColour
            : _over ? Aroused
            : Resting;

        if (_picture is not null)
        {
            _picture.Background = new SolidColorBrush(ink);
        }

        if (_mark is not null)
        {
            _mark.Stroke = new SolidColorBrush(ink);
        }

        Background = _pressed
            ? (closing
                ? new SolidColorBrush(Wash(_skin.AlarmColour, 0x66))
                : new SolidColorBrush(Held))
            : _over
                ? (closing
                    ? new SolidColorBrush(Wash(_skin.AlarmColour, 0x3C))
                    : new SolidColorBrush(Wash(Colors.White, 0x1F)))
                : Nothing;
    }

    /// <inheritdoc />
    protected override void OnPointerEntered(PointerEventArgs e)
    {
        _over = true;
        Repaint();

        base.OnPointerEntered(e);
    }

    /// <inheritdoc />
    protected override void OnPointerExited(PointerEventArgs e)
    {
        _over = false;
        _pressed = false;
        Repaint();

        base.OnPointerExited(e);
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            base.OnPointerPressed(e);

            return;
        }

        _pressed = true;
        Repaint();
        e.Handled = true;
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        var pressed = _pressed;

        _pressed = false;
        Repaint();

        if (pressed && e.InitialPressMouseButton == MouseButton.Left)
        {
            e.Handled = true;
            Pressed?.Invoke(this, EventArgs.Empty);
        }

        base.OnPointerReleased(e);
    }
}

using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// One of the three day-or-night choices, drawn as the window it would open rather than as its name.
/// </summary>
/// <remarks>
/// A picture of the program in the look it chooses says what the choice is faster than a word for it does,
/// and the three together say what they are a choice between without a sentence to explain them. What is
/// left to say is which of the three is in force, which is the frame: the one chosen is drawn in the
/// primary, and the two beside it in the line that tells one surface from another.
/// </remarks>
internal sealed class ThemeButton : Border
{
    /// <summary>The colours in force, which the frame is drawn in.</summary>
    private readonly Skin _skin;

    /// <summary>Whether this is the choice in force.</summary>
    private readonly bool _current;

    /// <summary>The choice it makes.</summary>
    private readonly ColorMode _mode;

    /// <summary>Makes one of the three.</summary>
    /// <param name="mode">The choice it makes.</param>
    /// <param name="picture">The window as the choice would draw it.</param>
    /// <param name="current">Whether it is the choice in force.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="choose">What choosing it does.</param>
    /// <param name="scale">How many times the artwork's own size it is drawn at.</param>
    public ThemeButton(
        ColorMode mode,
        Bitmap picture,
        bool current,
        Skin skin,
        Action<ColorMode> choose,
        double scale = 1
    )
    {
        ArgumentNullException.ThrowIfNull(picture);
        ArgumentNullException.ThrowIfNull(skin);
        ArgumentNullException.ThrowIfNull(choose);

        _mode = mode;
        _current = current;
        _skin = skin;

        var side = picture.PixelSize.Width * scale;

        Width = side + 4;
        Height = side + 4;
        CornerRadius = new CornerRadius(0);
        BorderThickness = new Thickness(current ? 2 : 1);
        BorderBrush = current ? skin.Primary : skin.Splitter;
        Cursor = new Cursor(StandardCursorType.Hand);
        HorizontalAlignment = HorizontalAlignment.Center;
        VerticalAlignment = VerticalAlignment.Center;

        var drawn = new Image
        {
            Source = picture,
            Width = side,
            Height = side,
            Stretch = Stretch.Uniform,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The artwork is pixel art: it is drawn at whole multiples of its own size, and a filter over it would
        // be the one thing that could make a pixel not a square.
        RenderOptions.SetBitmapInterpolationMode(drawn, BitmapInterpolationMode.None);

        Child = drawn;

        // A literal call per choice rather than a key held in a variable: what is stated about a picture is
        // checked by the translation check only when the call site names it.
        var named = mode switch
        {
            ColorMode.Light => RolaI18N.Get("launcher.settings.light"),
            ColorMode.Dark => RolaI18N.Get("launcher.settings.dark"),
            _ => RolaI18N.Get("launcher.settings.system"),
        };

        ToolTip.SetTip(this, named);
        AutomationProperties.SetName(this, named);

        PointerReleased += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (e.InitialPressMouseButton != MouseButton.Left)
            {
                return;
            }

            choose(_mode);
        };
    }

    /// <inheritdoc />
    protected override void OnPointerEntered(PointerEventArgs e)
    {
        // What a reach for it says: the frame of the one in force stays the primary, and another's comes
        // forward to the ink it would be read in.
        if (!_current)
        {
            BorderBrush = _skin.Front;
        }

        base.OnPointerEntered(e);
    }

    /// <inheritdoc />
    protected override void OnPointerExited(PointerEventArgs e)
    {
        if (!_current)
        {
            BorderBrush = _skin.Splitter;
        }

        base.OnPointerExited(e);
    }
}

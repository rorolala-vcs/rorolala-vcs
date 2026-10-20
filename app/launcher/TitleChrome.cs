using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher;

/// <summary>
/// The window's own buttons, on one strip at the corner of the banner.
/// </summary>
/// <remarks>
/// The strip is one block of a light black with square corners, and the buttons sit on it: a picture is the
/// frame here, and a mark drawn straight onto a picture is a mark that disappears into it wherever the art
/// happens to be the colour of the ink. One block behind all of them rather than one behind each is also
/// what makes them read as a row of controls rather than as three things in three boxes.
/// <para>
/// What is drawn on it is the Desktop program's own two frames. The Windows frame puts its marks at the
/// corner in the order they read — the settings, then the one that puts the window away, then the one that
/// closes it — each as wide as a caption button there, filling only under the pointer. The macOS frame puts
/// its lights at the other corner, in its own order, with the mark each one carries hidden until the pointer
/// reaches it; the settings is drawn beside them as the one control that is this program's rather than the
/// frame's. A tiling window manager is the frame already, so nothing that belongs to a frame is drawn there
/// and only the settings is left.
/// </para>
/// </remarks>
internal sealed class TitleChrome : Panel
{
    /// <summary>What the strip is filled with: a black light enough to read a picture through.</summary>
    private static readonly Color Bed = Color.FromArgb(0x66, 0x00, 0x00, 0x00);

    /// <summary>The window the buttons act on.</summary>
    private readonly Window _window;

    /// <summary>The colours in force.</summary>
    private Skin _skin;

    /// <summary>The style the buttons were last arranged in.</summary>
    private WindowStyle _style;

    /// <summary>Makes the buttons, drawn the way a Windows frame is drawn until told otherwise.</summary>
    /// <param name="window">The window they act on.</param>
    /// <param name="skin">The colours in force.</param>
    public TitleChrome(Window window, Skin skin)
    {
        ArgumentNullException.ThrowIfNull(window);

        _window = window;
        _skin = skin;
        _style = WindowStyle.Windows;

        HorizontalAlignment = HorizontalAlignment.Stretch;
        VerticalAlignment = VerticalAlignment.Stretch;

        Arrange(skin, _style);
    }

    /// <summary>Raised when what the program may be told was asked for.</summary>
    public event EventHandler? SettingsAsked;

    /// <summary>
    /// Puts the strip where a style puts it, with the buttons that style has.
    /// </summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="style">How this window's frame is drawn.</param>
    public void Arrange(Skin skin, WindowStyle style)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;
        _style = style;

        Children.Clear();

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Center,

            // The lights stand off one another, where the marks are a row of one control each: room left
            // between two of them is room the frame has between its own buttons.
            Spacing = style == WindowStyle.Mac ? 8 : 0,
        };

        if (style == WindowStyle.Mac)
        {
            row.Children.Add(Button(CaptionKind.Close, style));
            row.Children.Add(Button(CaptionKind.Minimize, style));
            row.Children.Add(Button(CaptionKind.Settings, style));
        }
        else
        {
            row.Children.Add(Button(CaptionKind.Settings, style));

            // A tiling window manager is the frame already: a window button drawn here would be a button
            // inside a frame the manager has.
            if (style != WindowStyle.TillingWm)
            {
                row.Children.Add(Button(CaptionKind.Minimize, style));
                row.Children.Add(Button(CaptionKind.Close, style));
            }
        }

        Children.Add(
            new Border
            {
                Background = new SolidColorBrush(Bed),
                CornerRadius = new CornerRadius(0),
                Padding = style == WindowStyle.Mac ? new Thickness(12, 5, 12, 5) : default,
                Child = row,
                HorizontalAlignment = style == WindowStyle.Mac
                    ? HorizontalAlignment.Left
                    : HorizontalAlignment.Right,
                VerticalAlignment = VerticalAlignment.Top,
            }
        );
    }

    /// <summary>One of the buttons on the strip.</summary>
    /// <param name="kind">What it does.</param>
    /// <param name="style">How the frame is drawn.</param>
    /// <returns>The button.</returns>
    private CaptionButton Button(CaptionKind kind, WindowStyle style)
    {
        var button = new CaptionButton(style, kind, _skin);

        button.Pressed += (_, _) =>
        {
            switch (kind)
            {
                case CaptionKind.Close:
                    _window.Close();

                    break;

                case CaptionKind.Minimize:
                    _window.WindowState = WindowState.Minimized;

                    break;

                default:
                    SettingsAsked?.Invoke(this, EventArgs.Empty);

                    break;
            }
        };

        return button;
    }
}

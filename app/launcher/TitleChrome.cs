using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher;

/// <summary>
/// The row at the top of the window: what the program is called, and the buttons its frame carries.
/// </summary>
/// <remarks>
/// The row is a real title bar rather than a strip drawn over a picture. The Desktop program's three frames
/// are the same three here: Windows puts the settings, the minimize and the close at the right in the order
/// they are read, macOS puts its three lights at the left and hides the marks until the pointer reaches them,
/// and a tiling window manager is the frame already, so only the settings is drawn.
/// <para>
/// The background is the elevated ground and the line beneath it is the border, which is what makes the row
/// read as part of the frame rather than as something laid over the content.
/// </para>
/// </remarks>
internal sealed class TitleChrome : Border
{
    /// <summary>How tall the row is, which is the window's title bar.</summary>
    private const double HeightOfRow = 34;

    /// <summary>The window the buttons act on.</summary>
    private readonly Window _window;

    /// <summary>The colours in force.</summary>
    private Skin _skin;

    /// <summary>Makes the row, drawn the way a Windows frame is drawn until told otherwise.</summary>
    /// <param name="window">The window they act on.</param>
    /// <param name="skin">The colours in force.</param>
    public TitleChrome(Window window, Skin skin)
    {
        ArgumentNullException.ThrowIfNull(window);

        _window = window;
        _skin = skin;

        Height = HeightOfRow;
        Background = skin.Bright;
        BorderBrush = skin.Splitter;
        BorderThickness = new Thickness(0, 0, 0, 1);

        PointerPressed += (_, e) => _window.BeginMoveDrag(e);

        Arrange(skin, WindowStyle.Windows);
    }

    /// <summary>Raised when what the program may be told was asked for.</summary>
    public event EventHandler? SettingsAsked;

    /// <summary>
    /// Puts the title and the buttons where a style puts them.
    /// </summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="style">How this window's frame is drawn.</param>
    public void Arrange(Skin skin, WindowStyle style)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;

        Background = skin.Bright;
        BorderBrush = skin.Splitter;

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,Auto,*,Auto"),
        };

        var brand = new TextBlock
        {
            Text = "ROLA Launcher",
            FontSize = 13,
            FontWeight = FontWeight.Bold,
            Foreground = skin.Front,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(12, 0, 0, 0),
        };

        if (style == WindowStyle.Mac)
        {
            var lights = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(12, 0, 0, 0),
            };

            lights.Children.Add(Button(CaptionKind.Close, style));
            lights.Children.Add(Button(CaptionKind.Minimize, style));
            lights.Children.Add(Button(CaptionKind.Settings, style));

            row.Children.Add(lights);
            Grid.SetColumn(brand, 1);
            row.Children.Add(brand);

            Child = row;

            return;
        }

        row.Children.Add(brand);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            VerticalAlignment = VerticalAlignment.Stretch,
            HorizontalAlignment = HorizontalAlignment.Right,
        };

        buttons.Children.Add(Button(CaptionKind.Settings, style));

        // A tiling window manager is the frame already: a window button drawn here would be a button
        // inside a frame the manager has.
        if (style != WindowStyle.TilingWm)
        {
            buttons.Children.Add(Button(CaptionKind.Minimize, style));
            buttons.Children.Add(Button(CaptionKind.Close, style));
        }

        Grid.SetColumn(buttons, 3);
        row.Children.Add(buttons);

        Child = row;
    }

    /// <summary>One of the buttons on the row.</summary>
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

using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// A window this program puts over itself: its own frame, its own title row, and a body a caller fills.
/// </summary>
/// <remarks>
/// The launcher draws its own frame, so a window it opens does the same: a title bar borrowed from the
/// desktop would be a second way a window of this program looks, and on a tiling window manager it would
/// be a second frame around one. It is modal over the launcher, which is what makes a tiling window manager
/// keep it where it was put instead of tiling it with everything else.
/// </remarks>
internal abstract class DialogWindow : Window
{
    /// <summary>The row the window is moved by, and the title it states.</summary>
    private readonly Grid _bar;

    /// <summary>The title, which is drawn in the ink in force.</summary>
    private readonly TextBlock _heading;

    /// <summary>The button that puts the window away.</summary>
    private readonly FlatButton _close;

    /// <summary>Makes a window with a title and an empty body.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="heading">What the window is about.</param>
    /// <param name="width">How wide it is.</param>
    /// <param name="height">How tall it is.</param>
    protected DialogWindow(Skin skin, string heading, double width, double height)
    {
        ArgumentNullException.ThrowIfNull(skin);

        Width = width;
        Height = height;
        Title = heading;
        CanResize = false;
        ShowInTaskbar = false;
        WindowDecorations = Avalonia.Controls.WindowDecorations.None;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = skin.Mid;

        _heading = new TextBlock
        {
            Text = heading,
            FontSize = 13,
            FontWeight = FontWeight.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(20, 0, 0, 0),
        };

        _close = new FlatButton(LauncherIcons.Glyph(LauncherIcons.Close, 10, skin.Front), skin)
        {
            Width = 34,
            Height = 26,
            Padding = default,
            Margin = new Thickness(0, 0, 10, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };

        _close.Clicked += (_, _) => Close();

        _bar = new Grid
        {
            Height = 44,
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
        };

        _bar.Children.Add(_heading);
        Grid.SetColumn(_close, 1);
        _bar.Children.Add(_close);

        _bar.PointerPressed += (_, e) => BeginMoveDrag(e);

        Body = new StackPanel { Spacing = 14, Margin = new Thickness(24, 6, 24, 24) };

        var root = new Grid { RowDefinitions = new RowDefinitions("Auto,*") };

        root.Children.Add(_bar);
        Grid.SetRow(Body, 1);
        root.Children.Add(Body);

        Content = new Border
        {
            BorderBrush = skin.Splitter,
            BorderThickness = new Thickness(1),
            Child = root,
            Background = skin.Mid,
        };
    }

    /// <summary>Where a caller puts what the window is for.</summary>
    protected StackPanel Body { get; }

    /// <summary>
    /// Changes what the window says it is about.
    /// </summary>
    /// <remarks>
    /// The account window is two windows at once — a greeting and a question — and the strip at its top is the
    /// one thing on it that is not drawn again when the other is: what it is about is stated once, by the
    /// window, and stated again here when the window becomes the other of the two.
    /// </remarks>
    /// <param name="heading">What it is about now.</param>
    protected void Retitle(string heading)
    {
        Title = heading;
        _heading.Text = heading;
    }

    /// <summary>Draws the window in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    protected void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        Background = skin.Mid;
        _heading.Foreground = skin.Front;

        if (Content is Border frame)
        {
            frame.Background = skin.Mid;
            frame.BorderBrush = skin.Splitter;
        }

        // The mark is stamped in the ink it stands on, so a variant that changed is a mark made again
        // rather than one left in the colour of the variant it was made in.
        _close.Child = LauncherIcons.Glyph(LauncherIcons.Close, 10, skin.Front);
        _close.Repaint(skin);
    }

    /// <summary>A line of text as a dialog draws one.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="text">What it says.</param>
    /// <param name="size">How large it is.</param>
    /// <param name="muted">Whether it is the ink beside the thing rather than the thing.</param>
    /// <returns>The text.</returns>
    protected static TextBlock Line(Skin skin, string text, double size = 13, bool muted = false) =>
        new()
        {
            Text = text,
            FontSize = size,
            Foreground = muted ? skin.Muted : skin.Front,
            TextWrapping = TextWrapping.Wrap,
        };

    /// <summary>A text field as a dialog draws one.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="watermark">What it says before anything is typed.</param>
    /// <returns>The field.</returns>
    protected static TextBox Field(Skin skin, string watermark) =>
        new()
        {
            PlaceholderText = watermark,
            FontSize = 13,
            Height = 36,
            Padding = new Thickness(10, 0, 10, 0),
            VerticalContentAlignment = VerticalAlignment.Center,
            Background = skin.Dark,
            Foreground = skin.Front,
            BorderBrush = skin.Splitter,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(6),
        };

    /// <summary>A row of buttons, laid out at the end of what it is in.</summary>
    /// <param name="buttons">The buttons, in the order they are drawn.</param>
    /// <returns>The row.</returns>
    protected static StackPanel Row(params Control[] buttons)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
        };

        foreach (var button in buttons)
        {
            row.Children.Add(button);
        }

        return row;
    }
}

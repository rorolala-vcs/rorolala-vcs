using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// One Workspace on the list: its mark, what it is called, where it is, and what state it is in.
/// </summary>
/// <remarks>
/// What a card shows is read from the file system each time it is drawn rather than kept beside the path:
/// the list outlives the directories it names, and a Workspace that was moved or taken away says so the
/// next time the list is seen. A card is a place rather than a button — the whole of it is pressed, and the
/// window is opened where the card says; the one thing on it that is not the place is the mark at its end,
/// which takes the card off the list and is the only part of it that must not open anything.
/// </remarks>
internal sealed class WorkspaceCard : Border
{
    /// <summary>What the card is about.</summary>
    private readonly string _path;

    /// <summary>What opened it.</summary>
    private readonly Action<string> _opened;

    /// <summary>What takes it off the list.</summary>
    private readonly Action<string> _removed;

    /// <summary>What the card says it is.</summary>
    private readonly TextBlock _state;

    /// <summary>What the card is called.</summary>
    private readonly TextBlock _name;

    /// <summary>Where the card is.</summary>
    private readonly TextBlock _place;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>The colours the card was last drawn in.</summary>
    private Skin _skin = null!;

    /// <summary>Makes a card for one directory.</summary>
    /// <param name="path">The directory.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="opened">What to do when it is pressed.</param>
    /// <param name="removed">What to do when the mark at its end is pressed.</param>
    public WorkspaceCard(string path, Skin skin, Action<string> opened, Action<string> removed)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(skin);
        ArgumentNullException.ThrowIfNull(opened);
        ArgumentNullException.ThrowIfNull(removed);

        _path = path;
        _opened = opened;
        _removed = removed;
        State = Workspaces.Of(path);

        _name = new TextBlock
        {
            Text = Workspaces.Named(path),
            FontSize = 15,
            FontWeight = FontWeight.SemiBold,
        };

        _place = new TextBlock
        {
            Text = path,
            FontSize = 11,
            Margin = new Thickness(0, 3, 0, 0),
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        _state = new TextBlock
        {
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(12, 0, 8, 0),
        };

        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(_name);
        text.Children.Add(_place);

        var close = new FlatButton(LauncherIcons.Glyph(LauncherIcons.Close, 12, skin.Muted), skin)
        {
            Width = 28,
            Height = 28,
            Padding = default,
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(close, RolaI18N.Get("launcher.workspace.remove"));

        close.Clicked += (_, _) => _removed(_path);

        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto,Auto") };

        row.Children.Add(new WorkspaceBadge(path));

        Grid.SetColumn(text, 1);
        row.Children.Add(text);

        Grid.SetColumn(_state, 2);
        row.Children.Add(_state);

        Grid.SetColumn(close, 3);
        row.Children.Add(close);

        Child = row;
        Padding = new Thickness(12, 10, 12, 10);
        CornerRadius = new CornerRadius(10);
        BorderThickness = new Thickness(1);
        Cursor = new Cursor(StandardCursorType.Hand);

        Repaint(skin);
    }

    /// <summary>What the directory was, as the card was drawn.</summary>
    public WorkspaceState State { get; }

    /// <summary>The directory the card is about.</summary>
    public string Path => _path;

    /// <summary>Draws the card in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;

        Background = new SolidColorBrush(skin.Palette.Bright);
        BorderBrush = skin.Splitter;

        _name.Foreground = skin.Front;
        _place.Foreground = skin.Muted;

        _state.Text = State switch
        {
            WorkspaceState.Missing => RolaI18N.Get("launcher.workspace.missing"),
            WorkspaceState.NotWorkspace => RolaI18N.Get("launcher.workspace.not_workspace"),
            _ => string.Empty,
        };

        _state.Foreground = State == WorkspaceState.Workspace ? skin.Muted : skin.Alarm;
    }

    /// <inheritdoc />
    protected override void OnPointerEntered(PointerEventArgs e)
    {
        if (_skin is not null)
        {
            Background = new SolidColorBrush(_skin.Palette.Dark);
        }

        base.OnPointerEntered(e);
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        _pressed = e.GetCurrentPoint(this).Properties.IsLeftButtonPressed;
        e.Handled = _pressed;

        base.OnPointerPressed(e);
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        var pressed = _pressed && e.InitialPressMouseButton == MouseButton.Left;

        _pressed = false;

        if (pressed)
        {
            _opened(_path);
        }

        base.OnPointerReleased(e);
    }

    /// <inheritdoc />
    protected override void OnPointerExited(PointerEventArgs e)
    {
        _pressed = false;

        if (_skin is not null)
        {
            Background = new SolidColorBrush(_skin.Palette.Bright);
        }

        base.OnPointerExited(e);
    }
}

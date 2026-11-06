using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// One Workspace on the list: its mark, what it is called, where it is, and what state it is in.
/// </summary>
/// <remarks>
/// What a card shows is read from the file system each time it is drawn rather than kept beside the path:
/// the list outlives the directories it names, and a Workspace that was moved or taken away says so the
/// next time the list is seen. A card is a place rather than a button, the whole of it is pressed, and the
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

    /// <summary>The mark that turns while the window it is about is being opened.</summary>
    private readonly TurningMark _busyMark;

    /// <summary>What takes the card off the list, which is not offered while the window is being opened.</summary>
    private readonly FlatButton _close;

    /// <summary>What the card is called.</summary>
    private readonly TextBlock _name;

    /// <summary>Where the card is.</summary>
    private readonly TextBlock _place;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>Whether the window this card is about is being opened, which is not a card to press again.</summary>
    private bool _busy;

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
        };

        _busyMark = new TurningMark(new SolidColorBrush(skin.Palette.Primary), 14)
        {
            IsVisible = false,
        };

        AutomationProperties.SetName(_busyMark, RolaI18N.Get("launcher.workspace.opening"));

        // The state and the mark share one cell: a card is either saying what its directory is or saying that its
        // window is being opened, and the two are never true at once.
        var state = new Panel
        {
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(12, 0, 8, 0),
        };

        state.Children.Add(_state);
        state.Children.Add(_busyMark);

        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(_name);
        text.Children.Add(_place);

        _close = new FlatButton(LauncherIcons.Glyph(LauncherIcons.Close, 12, skin.Muted), skin)
        {
            Width = 28,
            Height = 28,
            Padding = default,
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(_close, RolaI18N.Get("launcher.workspace.remove"));

        _close.Clicked += (_, _) => _removed(_path);

        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto,Auto") };

        row.Children.Add(new WorkspaceBadge(path));

        Grid.SetColumn(text, 1);
        row.Children.Add(text);

        Grid.SetColumn(state, 2);
        row.Children.Add(state);

        Grid.SetColumn(_close, 3);
        row.Children.Add(_close);

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

    /// <summary>
    /// Says that the window this card is about is being opened, or that it is not any more.
    /// </summary>
    /// <remarks>
    /// Only the card whose window is being opened turns the mark; the rest are put out of reach by the window
    /// rather than by themselves, which is why they are disabled there and this is not asked of them. What this
    /// card does with it is stop being a place to press: the one thing it is for is already happening.
    /// </remarks>
    /// <param name="busy">Whether this is the card whose window is being opened.</param>
    public void SetBusy(bool busy)
    {
        _busy = busy;

        _busyMark.IsVisible = busy;
        _state.IsVisible = !busy;
        _close.IsVisible = !busy;

        Cursor = new Cursor(busy ? StandardCursorType.Arrow : StandardCursorType.Hand);
    }

    /// <inheritdoc />
    protected override void OnPointerEntered(PointerEventArgs e)
    {
        if (_skin is not null && !_busy)
        {
            Background = new SolidColorBrush(_skin.Palette.Dark);
        }

        base.OnPointerEntered(e);
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (_busy)
        {
            return;
        }

        _pressed = e.GetCurrentPoint(this).Properties.IsLeftButtonPressed;
        e.Handled = _pressed;

        base.OnPointerPressed(e);
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        var pressed = !_busy && _pressed && e.InitialPressMouseButton == MouseButton.Left;

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

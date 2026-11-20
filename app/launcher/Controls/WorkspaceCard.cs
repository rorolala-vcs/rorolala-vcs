using Avalonia;
using Avalonia.Automation;
using Avalonia.Collections;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Theming;
using ShapeRectangle = Avalonia.Controls.Shapes.Rectangle;

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
/// <para>
/// A Workspace that is missing, or that is a directory holding no Workspace, is drawn in the error state:
/// a dashed edge, a wash of the error colour, and every piece of it in that colour. The edge is dashed
/// because a card that is present but under the pointer is a card with a solid edge, and the two should
/// not be told apart by the wash alone.
/// </para>
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

    /// <summary>The line beside the mark that says what is happening.</summary>
    private readonly TextBlock _busyText;

    /// <summary>The row that says the window is being opened.</summary>
    private readonly StackPanel _busyRow;

    /// <summary>What takes the card off the list, which is not offered while the window is being opened.</summary>
    private readonly FlatButton _close;

    /// <summary>The picture the close button carries, which is drawn in the state's colour.</summary>
    private readonly Border _closeGlyph;

    /// <summary>What the card is called.</summary>
    private readonly TextBlock _name;

    /// <summary>Where the card is.</summary>
    private readonly TextBlock _place;

    /// <summary>The mark the Workspace wears.</summary>
    private readonly WorkspaceBadge _badge;

    /// <summary>The Git logomark beside the name of a git-rola Workspace.</summary>
    private readonly Border _git;

    /// <summary>Whether the directory is a git-rola project, which is shown even where it is also a Workspace.</summary>
    private readonly bool _isGit;

    /// <summary>The row the name and the Git mark share, which is what keeps the mark against the name.</summary>
    private readonly Grid _nameLine;

    /// <summary>The dashed edge an invalid Workspace is drawn with.</summary>
    private readonly ShapeRectangle _outline;

    /// <summary>Whether a press is being made.</summary>
    private bool _pressed;

    /// <summary>Whether the window this card is about is being opened, which is not a card to press again.</summary>
    private bool _busy;

    /// <summary>Whether the pointer is over the card.</summary>
    private bool _over;

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
        _isGit = Workspaces.IsGit(path);

        _name = new TextBlock
        {
            Text = Workspaces.Named(path),
            FontSize = 15,
            FontWeight = FontWeight.SemiBold,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
        };

        _place = new TextBlock
        {
            Text = path,
            FontFamily = skin.Mono,
            FontSize = 11.5,
            Margin = new Thickness(0, 3, 0, 0),
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        _state = new TextBlock
        {
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
        };

        _busyMark = new TurningMark(new SolidColorBrush(skin.Palette.Primary), 13)
        {
            IsVisible = false,
        };

        _busyText = new TextBlock
        {
            Text = RolaI18N.Get("launcher.workspace.opening"),
            FontSize = 12,
            VerticalAlignment = VerticalAlignment.Center,
        };

        _busyRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 7,
            VerticalAlignment = VerticalAlignment.Center,
            IsVisible = false,
        };

        _busyRow.Children.Add(_busyMark);
        _busyRow.Children.Add(_busyText);

        var state = new Panel
        {
            VerticalAlignment = VerticalAlignment.Center,
        };

        state.Children.Add(_state);
        state.Children.Add(_busyRow);

        _git = LauncherIcons.Glyph(LauncherIcons.Git, 14, skin.Muted);
        _git.VerticalAlignment = VerticalAlignment.Center;
        _git.IsHitTestVisible = true;
        ToolTip.SetTip(_git, RolaI18N.Get("launcher.workspace.git_workspace"));
        AutomationProperties.SetName(_git, RolaI18N.Get("launcher.workspace.git_workspace"));

        _nameLine = new Grid
        {
            // The name takes only the room it needs, the Git mark follows it, and the rest of the row is
            // empty. A star on the name would push the mark to the far end of the card, which reads as an
            // action on the card rather than as part of what the Workspace is.
            ColumnDefinitions = new ColumnDefinitions("Auto,Auto,*"),
            ColumnSpacing = 6,
        };

        _nameLine.Children.Add(_name);

        Grid.SetColumn(_git, 1);
        _nameLine.Children.Add(_git);

        _nameLine.SizeChanged += (_, _) => FitName();

        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };

        text.Children.Add(_nameLine);
        text.Children.Add(_place);

        _closeGlyph = LauncherIcons.Glyph(LauncherIcons.Close, 12, skin.Muted);

        _close = new FlatButton(_closeGlyph, skin, ButtonKind.QuietDanger)
        {
            Width = 28,
            Height = 28,
            Padding = default,
            CornerRadius = new CornerRadius(5),
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(_close, RolaI18N.Get("launcher.workspace.remove"));

        _close.Clicked += (_, _) => _removed(_path);

        _badge = new WorkspaceBadge(path);

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("40,*,Auto,28"),
            ColumnSpacing = 12,
            Margin = new Thickness(10, 12, 10, 12),
        };

        row.Children.Add(_badge);

        Grid.SetColumn(text, 1);
        row.Children.Add(text);

        Grid.SetColumn(state, 2);
        row.Children.Add(state);

        Grid.SetColumn(_close, 3);
        row.Children.Add(_close);

        _outline = new ShapeRectangle
        {
            StrokeThickness = 1,
            StrokeDashArray = new AvaloniaList<double> { 4, 3 },
            RadiusX = 10,
            RadiusY = 10,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsHitTestVisible = false,
            IsVisible = false,
            Margin = new Thickness(0.5),
        };

        var root = new Panel();

        root.Children.Add(row);
        root.Children.Add(_outline);

        Child = root;
        Padding = default;
        CornerRadius = new CornerRadius(10);
        BorderThickness = new Thickness(1);
        ClipToBounds = true;
        Cursor = new Cursor(StandardCursorType.Hand);

        Repaint(skin);
    }

    /// <summary>What the directory was, as the card was drawn.</summary>
    public WorkspaceState State { get; }

    /// <summary>The directory the card is about.</summary>
    public string Path => _path;

    /// <summary>Whether the Workspace is one the error state is drawn for.</summary>
    private bool Invalid => State is WorkspaceState.Missing or WorkspaceState.NotWorkspace;

    /// <summary>Draws the card in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;

        var invalid = Invalid;

        _name.Foreground = invalid ? skin.Alarm : skin.Front;
        _place.Foreground = invalid ? skin.Alarm : skin.Muted;
        _closeGlyph.Background = invalid ? skin.Alarm : skin.Muted;
        _git.Background = _over ? skin.Front : skin.Muted;
        _git.IsVisible = !invalid && _isGit;
        _close.IsVisible = !_busy;

        FitName();

        _state.Text = State switch
        {
            WorkspaceState.Missing => RolaI18N.Get("launcher.workspace.missing"),
            WorkspaceState.NotWorkspace => RolaI18N.Get("launcher.workspace.not_workspace"),
            _ => string.Empty,
        };

        _state.Foreground = skin.Alarm;
        _state.IsVisible = !_busy && !string.IsNullOrEmpty(_state.Text);
        _busyText.Foreground = skin.Muted;

        _badge.Apply(invalid, skin);

        _outline.IsVisible = invalid;
        _outline.Stroke = skin.Alarm;

        BorderThickness = invalid ? default : new Thickness(1);
        BorderBrush = skin.Splitter;
        Background = invalid
            ? new SolidColorBrush(Skin.Mix(skin.Palette.Bright, skin.AlarmColour, _over ? 0.11 : 0.07))
            : new SolidColorBrush(_over ? skin.Palette.Dark : skin.Palette.Bright);
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

        _busyRow.IsVisible = busy;
        _busyMark.IsVisible = busy;
        _state.IsVisible = !busy && !string.IsNullOrEmpty(_state.Text);
        _close.IsVisible = !busy;

        Cursor = new Cursor(busy ? StandardCursorType.Arrow : StandardCursorType.Hand);
    }

    /// <summary>
    /// Gives the name the room left beside the Git mark, so a long name ends in an ellipsis before it
    /// reaches the mark rather than under it.
    /// </summary>
    private void FitName()
    {
        var room = _nameLine.Bounds.Width - (_git.IsVisible ? 20 : 0);

        _name.MaxWidth = Math.Max(0, room);
    }

    /// <inheritdoc />
    protected override void OnPointerEntered(PointerEventArgs e)
    {
        _over = true;

        if (_skin is not null && !_busy)
        {
            Repaint(_skin);
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
        _over = false;

        if (_skin is not null)
        {
            Repaint(_skin);
        }

        base.OnPointerExited(e);
    }
}

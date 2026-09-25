using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>What an entry is called on screen.</summary>
internal static class Names
{
    /// <summary>
    /// The name of a path: its last part, or the whole of it when there is no last part.
    /// </summary>
    /// <remarks>
    /// The computer is the one place that has no name of its own, having no path: what it is called is
    /// a word rather than a piece of one. The way up is the other, and is named before it is looked at —
    /// the name it would be read as is <c>..</c>, which is a lucky accident of the path rather than what
    /// it is called.
    /// </remarks>
    /// <param name="entry">The entry to name.</param>
    /// <returns>The name to show.</returns>
    public static string Show(Entry entry) =>
        Browser.IsUp(entry.Path)
            ? Browser.UpName
            : Browser.IsComputer(entry.Path)
                ? RolaI18N.Get("rorolala_file_system.computer")
                : Path.GetFileName(entry.Path) is { Length: > 0 } name
                    ? name
                    : entry.Path;
}

/// <summary>The entries as a table, one to a line.</summary>
internal sealed class ListBrowser : UserControl
{
    /// <summary>How wide the column of icons is: enough for the word over it.</summary>
    private const double IconColumn = 30;

    /// <summary>
    /// How wide the grab between two columns is, and the marks it wears.
    /// </summary>
    /// <remarks>
    /// The marks are Section 10's, written as the literals that section publishes, because a plugin has
    /// nowhere else to read them from: the shell's own constants live in the host, which a plugin may not
    /// reference. The width is the width those rules draw a line through the middle of, so a band of another
    /// width would be a grab wearing a line that is not centred in it.
    /// </remarks>
    private const double GrabColumn = 4;

    /// <summary>What a grab between two of anything is marked with.</summary>
    private const string GrabMark = "dock-splitter";

    /// <summary>What a grab that resizes columns is marked with as well.</summary>
    private const string GrabColumnMark = "dock-splitter-columns";

    /// <summary>How narrow a column may be dragged.</summary>
    private const double MinColumn = 56;

    /// <summary>How wide a column of values is before anybody drags it.</summary>
    private const double PermissionsColumn = 92;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double ModifiedColumn = 132;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double SizeColumn = 84;

    /// <summary>
    /// Which column the name is in.
    /// </summary>
    /// <remarks>
    /// The name is the column that is as wide as what is left over, so it is the one that takes a move for
    /// free and the one that keeps the table as wide as the dock.
    /// </remarks>
    private const int NameColumn = 1;

    /// <summary>What an entry does when it is opened or right-clicked.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The columns after the name, which every row of the table has the same of.</summary>
    private readonly Column[] _values = Values();

    /// <summary>How wide every column is, the grabs between them included.</summary>
    private readonly GridLength[] _widths;

    /// <summary>The row of column names, which every row is kept in step with.</summary>
    private readonly Grid _header = new();

    /// <summary>The list the rows are in.</summary>
    private readonly ListBox _list;

    /// <summary>Every row that is on screen, so that a column moved reaches all of them.</summary>
    private readonly List<Grid> _rows = [];

    /// <summary>Where a grab was taken, and how wide the two columns each side of it were then.</summary>
    private (int Left, int Right, double At, double LeftWas, double RightWas)? _grabbed;

    /// <summary>Makes the table over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or right-clicked.</param>
    public ListBrowser(Browser browser, BrowserActions actions)
    {
        _actions = actions;
        _widths = Widths();

        _list = new ListBox
        {
            ItemsSource = browser.Shown,
            ItemTemplate = new FuncDataTemplate<Entry>((entry, _) => Row(entry), true),
            ContextMenu = actions.Empty(this),
        };

        // Selecting precedes the second tap, so what was activated is what is selected.
        _list.DoubleTapped += (_, _) =>
        {
            if (_list.SelectedItem is Entry entry)
            {
                actions.Activate(entry);
            }
        };

        var header = Header();

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(header, Dock.Top);
        panel.Children.Add(header);
        panel.Children.Add(_list);

        Content = panel;
    }

    /// <summary>One column of values, after the icon and the name.</summary>
    /// <param name="Header">The translation key the column is headed by.</param>
    /// <param name="Width">How wide the column starts.</param>
    /// <param name="Value">What the column says about one entry.</param>
    /// <param name="Align">Where the text sits in the column.</param>
    private sealed record Column(
        string Header,
        double Width,
        Func<Facts, string> Value,
        TextAlignment Align = TextAlignment.Left
    );

    /// <summary>
    /// The columns after the name, in the order they are shown.
    /// </summary>
    /// <remarks>
    /// Permissions are not a column on Windows, where the system does not carry what those nine letters
    /// say. A column blank in every row would be this table saying "not here" in a column of its own, and
    /// a table is better off with one column fewer.
    /// </remarks>
    private static Column[] Values() =>
        OperatingSystem.IsWindows()
            ? [Modified(), Size()]
            : [
                new("rorolala_file_system.column_permissions", PermissionsColumn, facts => facts.Permissions),
                Modified(),
                Size(),
            ];

    /// <summary>The column that says when an entry was last written.</summary>
    private static Column Modified() =>
        new("rorolala_file_system.column_modified", ModifiedColumn, facts => facts.Modified);

    /// <summary>The column that says how large an entry is, which is a number and so stands on the right.</summary>
    private static Column Size() =>
        new("rorolala_file_system.column_size", SizeColumn, facts => facts.Size, TextAlignment.Right);

    /// <summary>How wide every column starts, in the order they are laid out.</summary>
    private GridLength[] Widths()
    {
        var widths = new List<GridLength> { new(IconColumn), new(1, GridUnitType.Star) };

        foreach (var column in _values)
        {
            widths.Add(new GridLength(GrabColumn));
            widths.Add(new GridLength(column.Width));
        }

        return [.. widths];
    }

    /// <summary>The columns of the table, as the widths stand.</summary>
    private ColumnDefinitions Columns()
    {
        var columns = new ColumnDefinitions();

        foreach (var width in _widths)
        {
            columns.Add(new ColumnDefinition(width));
        }

        return columns;
    }

    /// <summary>
    /// The row of column names, with a grab between each pair of columns.
    /// </summary>
    /// <remarks>
    /// Held in a list item, which is what gives it the same inset as a row of entries: what a row is inset
    /// by is the theme's business, and a header inset by a number of this file's own would put every column
    /// somewhere other than its data. It is not a row, though, so it is not focusable and the hover its
    /// template draws for an item is taken off it: what answers the pointer in this strip is a grab, and a
    /// strip that lit up wherever the pointer went would be saying otherwise.
    /// </remarks>
    private Control Header()
    {
        _header.ColumnDefinitions = Columns();

        var cells = new List<Control?>
        {
            Cell(RolaI18N.Get("rorolala_file_system.column_icon")),
            Cell(RolaI18N.Get("rorolala_file_system.column_name")),
        };

        // A grab is between two columns, so the pair it takes in is its own place in the layout plus or
        // minus one.
        var left = NameColumn;

        foreach (var column in _values)
        {
            cells.Add(Grab(left, left + 2));
            cells.Add(Cell(RolaI18N.Get(column.Header)));

            left += 2;
        }

        Place(_header, cells);

        var item = new ListBoxItem { Content = _header, Focusable = false };

        item.Styles.Add(
            new Style(selector => selector.OfType<Border>().Name("SelectionBorder").Class(":pointerover"))
            {
                Setters = { new Setter(Border.BackgroundProperty, Brushes.Transparent) },
            }
        );

        return item;
    }

    /// <summary>One entry as a row: its icon, its name, and what each column of values says.</summary>
    private Control Row(Entry entry)
    {
        var facts = Stats.Of(entry);
        var name = Names.Show(entry);
        var icon = Icons.For(entry);
        icon.HorizontalAlignment = HorizontalAlignment.Left;

        var cells = new List<Control?> { icon, Cell(name.Length > 0 ? name : entry.Path) };

        foreach (var column in _values)
        {
            // The cell over the grab is nothing: a row has no grab, only the header does.
            cells.Add(null);
            cells.Add(Cell(column.Value(facts), column.Align));
        }

        var row = new Grid { ColumnDefinitions = Columns() };
        Place(row, cells);

        row.ContextMenu = _actions.Menu(this, entry);

        _rows.Add(row);
        row.DetachedFromVisualTree += (_, _) => _rows.Remove(row);

        return row;
    }

    /// <summary>
    /// The grab between two columns of the table.
    /// </summary>
    /// <remarks>
    /// A <see cref="GridSplitter"/>, because that is the control the look is written for: the shell marks
    /// the grab between two regions with these classes and the look draws one, and a grab between two
    /// columns is that same grab.
    /// <para>
    /// It is put in a panel rather than in the header's own grid, and that is load-bearing rather than tidy.
    /// A splitter moves the columns of the grid it is in, and the table's rows are one grid each, so a
    /// splitter in the header would move the header's columns alone; put in a panel it moves nothing at all,
    /// which is the behaviour this leans on, and the columns are moved in the handlers below instead. A grid
    /// of its own would not do: asking a grid for its columns is what a splitter does when it is pressed,
    /// and a grid asked for its columns outside a layout pass is a grid whose next arrange dereferences
    /// something only a measure creates.
    /// </para>
    /// </remarks>
    /// <param name="left">The column on its left.</param>
    /// <param name="right">The column on its right.</param>
    private Control Grab(int left, int right)
    {
        var grab = new GridSplitter
        {
            ResizeDirection = GridResizeDirection.Columns,
            Width = GrabColumn,
            HorizontalAlignment = HorizontalAlignment.Center,
            Classes = { GrabMark, GrabColumnMark },
        };

        // Handled events too, because the splitter takes the pointer itself and a handler that skipped what
        // it had handled would never hear a drag.
        grab.AddHandler(PointerPressedEvent, (_, e) => Taken(left, right, e), RoutingStrategies.Bubble, true);
        grab.AddHandler(PointerMovedEvent, Moved, RoutingStrategies.Bubble, true);
        grab.AddHandler(PointerReleasedEvent, (_, _) => _grabbed = null, RoutingStrategies.Bubble, true);
        grab.AddHandler(PointerCaptureLostEvent, (_, _) => _grabbed = null, RoutingStrategies.Bubble, true);

        return new Panel { Children = { grab } };
    }

    /// <summary>Takes a grab, remembering where and how wide the two columns each side of it were.</summary>
    /// <param name="left">The column on its left.</param>
    /// <param name="right">The column on its right.</param>
    /// <param name="e">The press.</param>
    private void Taken(int left, int right, PointerPressedEventArgs e) =>
        _grabbed = (left, right, e.GetPosition(this).X, Pixels(left), Pixels(right));

    /// <summary>
    /// Moves the grab: the two columns it is between take the move between them.
    /// </summary>
    /// <remarks>
    /// The pair taking the move is what makes the border land where the pointer is and leaves every other
    /// border in the table where it was. Where one of the pair is the name — the column that is as wide as
    /// what is left over — that one is skipped rather than moved: it takes the move for free, which is also
    /// what keeps the table as wide as the dock.
    /// <para>
    /// Neither column is dragged away entirely: the move is taken as far as the narrower of the two allows,
    /// and the name counts as a column here, which is why its width is asked of the layout.
    /// </para>
    /// </remarks>
    /// <param name="sender">The grab.</param>
    /// <param name="e">The move.</param>
    private void Moved(object? sender, PointerEventArgs e)
    {
        if (_grabbed is not { } grab)
        {
            return;
        }

        var left = _widths[grab.Left].IsStar;
        var right = _widths[grab.Right].IsStar;

        if (left && right)
        {
            return;
        }

        var least = MinColumn;
        var low = least - (left ? Room() : grab.LeftWas);
        var high = (right ? Room() : grab.RightWas) - least;

        // Nothing to give: a column already at its least on both sides is a grab that cannot move.
        if (low > high)
        {
            return;
        }

        var change = Math.Clamp(e.GetPosition(this).X - grab.At, low, high);

        if (!left)
        {
            _widths[grab.Left] = new GridLength(grab.LeftWas + change);
        }

        if (!right)
        {
            _widths[grab.Right] = new GridLength(grab.RightWas - change);
        }

        Apply();
    }

    /// <summary>How wide a column is in pixels, or nothing where it is the one that takes what is left.</summary>
    /// <param name="at">The column to ask about.</param>
    private double Pixels(int at) => _widths[at].IsStar ? 0 : _widths[at].Value;

    /// <summary>How wide the column that takes what is left is, as the layout has it.</summary>
    private double Room() =>
        _header.ColumnDefinitions.Count > NameColumn
            ? _header.ColumnDefinitions[NameColumn].ActualWidth
            : 0;

    /// <summary>Puts the widths into the header and into every row that is on screen.</summary>
    private void Apply()
    {
        Into(_header);

        foreach (var row in _rows)
        {
            Into(row);
        }

        return;

        void Into(Grid grid)
        {
            for (var at = 0; at < _widths.Length; at++)
            {
                grid.ColumnDefinitions[at].Width = _widths[at];
            }
        }
    }

    /// <summary>Puts each cell under its column, the ones that are nothing standing for a grab.</summary>
    /// <param name="grid">The row to fill.</param>
    /// <param name="cells">What each column holds, or nothing.</param>
    private static void Place(Grid grid, IReadOnlyList<Control?> cells)
    {
        for (var at = 0; at < cells.Count; at++)
        {
            if (cells[at] is { } cell)
            {
                Grid.SetColumn(cell, at);
                grid.Children.Add(cell);
            }
        }
    }

    /// <summary>One cell's text: one line, cut off rather than wrapped.</summary>
    /// <param name="text">What the cell says.</param>
    /// <param name="align">Where the text sits in it.</param>
    private static TextBlock Cell(string text, TextAlignment align = TextAlignment.Left) =>
        new()
        {
            Text = text,
            TextAlignment = align,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
        };
}

/// <summary>The entries as tiles, wrapping across the width.</summary>
internal sealed class GridBrowser : UserControl
{
    /// <summary>How much room is left around a tile's icon and name.</summary>
    private const int Around = 10;

    /// <summary>
    /// What a tile is filled with while the pointer is over it, where the theme names no tint of its own.
    /// </summary>
    /// <remarks>
    /// A grey rather than a shade of the accent: what a theme says a hover is is its own business, and this
    /// is only what is drawn when there is no theme to say.
    /// </remarks>
    private static readonly IBrush Neutral = new SolidColorBrush(Color.Parse("#1F808080"));

    /// <summary>
    /// The theme's own hover tint.
    /// </summary>
    /// <remarks>
    /// Named here rather than read off a control because a plugin has no other way to ask for the theme's
    /// palette; a program wearing no theme answers with nothing and the tile falls back to grey.
    /// </remarks>
    private const string Tint = "rorolala.theme.tint.deeper";

    /// <summary>What an entry does when it is opened or right-clicked.</summary>
    private readonly BrowserActions _actions;

    /// <summary>How many pixels wide and tall a tile's icon is, and so how wide its name is too.</summary>
    private readonly int _icon;

    /// <summary>Makes the grid layout over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or right-clicked.</param>
    /// <param name="icon">How large an icon is at the zoom this dock is at.</param>
    public GridBrowser(Browser browser, BrowserActions actions, int icon)
    {
        _actions = actions;
        _icon = icon;

        var tiles = new WrapPanel { Orientation = Orientation.Horizontal };

        foreach (var entry in browser.Shown)
        {
            tiles.Children.Add(Tile(entry));
        }

        Content = new ScrollViewer { Content = tiles, ContextMenu = actions.Empty(this) };
    }

    /// <summary>One entry as a tile: its icon above its name, on one line and cut off when it is too long.</summary>
    /// <remarks>
    /// The name is as wide as the icon and not one pixel wider, so that the two read as one column rather
    /// than as a picture with a caption under it that happens to start somewhere else. What does not fit is
    /// taken off the end rather than wrapped, because a tile of two lines is a tile of another height, and a
    /// row of tiles that are not the same height is not a row.
    /// </remarks>
    private Control Tile(Entry entry)
    {
        var tile = new Border
        {
            Padding = new Thickness(Around),
            Margin = new Thickness(4),
            ContextMenu = _actions.Menu(this, entry),
            Child = new StackPanel
            {
                Spacing = 6,
                Children =
                {
                    Icons.For(entry, _icon),
                    new TextBlock
                    {
                        Text = Names.Show(entry),
                        Width = _icon,
                        TextAlignment = TextAlignment.Center,
                        TextWrapping = TextWrapping.NoWrap,
                        TextTrimming = TextTrimming.CharacterEllipsis,
                    },
                },
            },
        };

        // The whole tile answers the pointer rather than the picture or the word alone, so that the target
        // under it is the thing that is about to be opened.
        tile.PointerEntered += (_, _) => tile.Background = Hover(tile);
        tile.PointerExited += (_, _) => tile.Background = null;
        tile.DoubleTapped += (_, _) => _actions.Activate(entry);

        return tile;
    }

    /// <summary>
    /// What a tile is washed with while the pointer is over it.
    /// </summary>
    /// <remarks>
    /// Asked of the theme while the pointer is over the tile rather than worked out when the tile is made,
    /// because a control is built before it is anywhere a theme reaches.
    /// </remarks>
    private static IBrush Hover(Control tile) =>
        tile.TryGetResource(Tint, null, out var found) && found is IBrush brush ? brush : Neutral;
}

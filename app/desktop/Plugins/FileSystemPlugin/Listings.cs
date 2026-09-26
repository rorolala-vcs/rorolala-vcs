using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Selection;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.VisualTree;
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

/// <summary>
/// What both ways of reading a directory have in common: the choice, the keyboard, and the menus.
/// </summary>
/// <remarks>
/// One directory is read two ways — as a table of rows and as tiles — and the two are one browser seen
/// twice rather than two browsers. Everything a user does to an entry, short of how it is laid out, is
/// therefore written once here: what a click means, what the arrows and the typed letters mean, and what
/// the menu on a choice offers. A view supplies the arrangement and a face for one entry, and inherits
/// the rest.
/// <para>
/// The choice is the toolkit's own list selection, because it is exactly the one asked for — a click
/// chooses, <c>Ctrl</c> adds or removes, <c>Shift</c> takes a range — and because the look already fills
/// a chosen row with the accent (§10). What is added here is only what the toolkit does not have: the
/// arrows within a wrapped grid, the typed letters, and a menu that knows about a set.
/// </para>
/// </remarks>
internal abstract class EntryView : UserControl
{
    /// <summary>How faded an entry is while it is cut, so that it reads as on its way out.</summary>
    private const double CutOpacity = 0.45;

    /// <summary>How long a run of typed letters stays one run.</summary>
    private static readonly TimeSpan Typing = TimeSpan.FromSeconds(1);

    /// <summary>How tall a row is where nothing on screen says, which is only ever a first guess.</summary>
    private const double RowGuess = 26.0;

    /// <summary>Every row on screen, so that a faded cut and a moved column reach all of them.</summary>
    private readonly List<(Entry Entry, Control Row)> _rows = [];

    /// <summary>Where the pointer or the keyboard last landed, which is what a range and an open begin at.</summary>
    private int _lead = -1;

    /// <summary>The letters typed since the last pause, and when the last of them was typed.</summary>
    private string _typed = string.Empty;
    private DateTime _typedAt = DateTime.MinValue;

    /// <summary>The modifiers the last key was pressed with, which text input does not carry itself.</summary>
    private KeyModifiers _modifiers;

    /// <summary>Sets up the shared behaviour over one list.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">The location the entries belong to.</param>
    /// <param name="actions">What the entries do when opened or given a menu.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    protected EntryView(IPluginHost host, Browser browser, BrowserActions actions, Clip clip)
    {
        Host = host;
        Browser = browser;
        Actions = actions;
        Clipboard = clip;

        List = new ListBox
        {
            ItemsSource = browser.Shown,
            SelectionMode = SelectionMode.Multiple,
            Background = Brushes.Transparent,
            ItemTemplate = new FuncDataTemplate<Entry>((entry, _) => Item(entry), true),
        };

        // The menu on the space around the entries. An entry carries one of its own, and the toolkit
        // takes the nearest, so this one is the empty space by construction.
        List.ContextMenu = actions.Empty(this);

        // All three are taken on the way down, before the list reads them itself: the arrows are replaced
        // by ones that know about a wrapped grid, and Enter and the typed letters are not the toolkit's
        // at all. A handler that let the list act first would be correcting an action already taken.
        List.AddHandler(KeyDownEvent, Keyed, RoutingStrategies.Tunnel);
        List.AddHandler(PointerPressedEvent, Pressed, RoutingStrategies.Tunnel);
        List.AddHandler(TextInputEvent, Typed, RoutingStrategies.Tunnel);

        List.DoubleTapped += Opened;

        // Repainting only while on screen, like the browser itself: a dock that was closed is not a view
        // of anything to keep in step.
        AttachedToVisualTree += (_, _) => Clipboard.Changed += Repaint;
        DetachedFromVisualTree += (_, _) => Clipboard.Changed -= Repaint;
    }

    /// <summary>The host, for what cannot be done.</summary>
    protected IPluginHost Host { get; }

    /// <summary>The location the entries belong to.</summary>
    protected Browser Browser { get; }

    /// <summary>What the entries do when opened or given a menu.</summary>
    protected BrowserActions Actions { get; }

    /// <summary>What a copy or a cut has put within reach of a paste.</summary>
    protected Clip Clipboard { get; }

    /// <summary>The list every entry is a row or a tile of.</summary>
    protected ListBox List { get; }

    /// <summary>Every row on screen, so that a column move can reach the ones drawn as a table.</summary>
    protected IReadOnlyList<(Entry Entry, Control Row)> Rows => _rows;

    /// <summary>The entry the pointer or the keyboard last landed on, or nothing before either has.</summary>
    protected Entry? Lead => _lead >= 0 && _lead < Browser.Shown.Count ? Browser.Shown[_lead] : null;

    /// <summary>
    /// One entry as a row or a tile, which is the one thing a view supplies.
    /// </summary>
    /// <remarks>
    /// Called by the toolkit when it realises an item, and by no view directly, so a view may build it out
    /// of anything the selection and the theme do not reach.
    /// </remarks>
    /// <param name="entry">The entry to draw.</param>
    /// <returns>What stands for the entry.</returns>
    protected abstract Control Item(Entry entry);

    /// <summary>
    /// How many entries stand side by side, which is what an up or down arrow steps by.
    /// </summary>
    /// <remarks>
    /// One for anything that stacks, since a step up or down one row is a step of one entry; a wrapped
    /// grid answers with the number the layout put on a line, read from the layout rather than worked out,
    /// because how wide a tile is is the view's own business.
    /// </remarks>
    protected virtual int Columns() => 1;

    /// <summary>
    /// Finishes a row or a tile: the menu on it, how faded it is, and the record of it on screen.
    /// </summary>
    /// <remarks>
    /// The menu is filled when it opens rather than when it is made, because a view's rows are made once
    /// and the toolkit reuses them: what the menu is about is the choice as it stands at the moment of
    /// opening, which is later.
    /// </remarks>
    /// <param name="entry">What the row stands for.</param>
    /// <param name="row">The row to finish.</param>
    /// <returns>The row.</returns>
    protected Control Prepared(Entry entry, Control row)
    {
        var menu = new ContextMenu();
        menu.Opening += (_, _) => Actions.Fill(menu, this, Chosen(entry));
        row.ContextMenu = menu;

        Apply(row, entry);

        _rows.Add((entry, row));
        row.DetachedFromVisualTree += (_, _) =>
        {
            for (var at = _rows.Count - 1; at >= 0; at--)
            {
                if (ReferenceEquals(_rows[at].Row, row))
                {
                    _rows.RemoveAt(at);
                }
            }
        };

        return row;
    }

    /// <summary>What a menu opened on an entry is about: the whole choice when that entry is in it.</summary>
    /// <param name="entry">The entry the menu was opened on.</param>
    protected IReadOnlyList<Entry> Chosen(Entry entry)
    {
        var chosen = Chosen();

        return chosen.Any(item => string.Equals(item.Path, entry.Path, StringComparison.Ordinal))
            ? chosen
            : [entry];
    }

    /// <summary>The chosen entries, in the order the listing shows.</summary>
    protected IReadOnlyList<Entry> Chosen()
    {
        var shown = Browser.Shown;
        var chosen = new List<Entry>();

        foreach (var at in List.Selection.SelectedIndexes)
        {
            if (at >= 0 && at < shown.Count)
            {
                chosen.Add(shown[at]);
            }
        }

        return chosen;
    }

    /// <summary>How faded a row is, which is its entry being cut and nothing else.</summary>
    /// <param name="row">The row to fade.</param>
    /// <param name="entry">What it stands for.</param>
    protected virtual void Apply(Control row, Entry entry) =>
        row.Opacity = Clipboard.IsCut(entry.Path) ? CutOpacity : 1.0;

    /// <summary>Draws the cut state again on every row that is on screen.</summary>
    private void Repaint()
    {
        foreach (var (entry, row) in _rows)
        {
            Apply(row, entry);
        }
    }

    /// <summary>
    /// Reads a key the list would otherwise read, because it means something more here.
    /// </summary>
    /// <remarks>
    /// The clipboard is taken with <c>Ctrl</c> held, the arrows step, <c>Home</c>, <c>End</c>,
    /// <c>PageUp</c> and <c>PageDown</c> go further, <c>Shift</c> extends from where the last step landed,
    /// and <c>Enter</c> opens. Everything else — the toolkit's own select-all among it — is left to the
    /// list.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        _modifiers = e.KeyModifiers;

        var shift = e.KeyModifiers.HasFlag(KeyModifiers.Shift);
        var control = e.KeyModifiers.HasFlag(KeyModifiers.Control) || e.KeyModifiers.HasFlag(KeyModifiers.Meta);

        if (control)
        {
            switch (e.Key)
            {
                case Key.C:
                    Actions.Copy(this, Chosen());
                    e.Handled = true;
                    break;
                case Key.X:
                    Actions.Cut(this, Chosen());
                    e.Handled = true;
                    break;
                case Key.V:
                    Actions.Paste(this, Browser.Current);
                    e.Handled = true;
                    break;
            }

            return;
        }

        switch (e.Key)
        {
            case Key.Enter:
                if (Lead is { } lead)
                {
                    Actions.Open(lead);
                }

                e.Handled = true;
                break;
            case Key.Up:
                Step(-Columns(), shift);
                e.Handled = true;
                break;
            case Key.Down:
                Step(Columns(), shift);
                e.Handled = true;
                break;

            // Across is a step of one only where entries stand side by side; in a column of rows it is
            // left to the toolkit, which does nothing with it here.
            case Key.Left when Columns() > 1:
                Step(-1, shift);
                e.Handled = true;
                break;
            case Key.Right when Columns() > 1:
                Step(1, shift);
                e.Handled = true;
                break;
            case Key.Home:
                To(0, shift);
                e.Handled = true;
                break;
            case Key.End:
                To(Browser.Shown.Count - 1, shift);
                e.Handled = true;
                break;
            case Key.PageUp:
                Step(-Page(), shift);
                e.Handled = true;
                break;
            case Key.PageDown:
                Step(Page(), shift);
                e.Handled = true;
                break;
        }
    }

    /// <summary>
    /// Takes a run of typed letters as a move to the next entry that begins with them.
    /// </summary>
    /// <remarks>
    /// The run is forgotten a second after the last letter, so the same letter can begin a new run — which
    /// is what makes typing one letter a second time go to the next entry rather than wait for a name that
    /// starts with two. Searching starts after where the last move landed, so a run walks the entries that
    /// begin with it, and only wraps when it runs off the end.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The text typed.</param>
    private void Typed(object? sender, TextInputEventArgs e)
    {
        var text = e.Text;

        if (_modifiers.HasFlag(KeyModifiers.Control) ||
            _modifiers.HasFlag(KeyModifiers.Alt) ||
            string.IsNullOrEmpty(text))
        {
            return;
        }

        var now = DateTime.UtcNow;
        _typed = now - _typedAt > Typing ? text : _typed + text;
        _typedAt = now;

        var at = Seek(_typed, _lead + 1);

        if (at < 0)
        {
            at = Seek(_typed, 0);
        }

        if (at < 0)
        {
            return;
        }

        To(at, false);
        e.Handled = true;
    }

    /// <summary>The first entry from a place on that begins with what was typed, or nothing.</summary>
    /// <param name="typed">What was typed.</param>
    /// <param name="from">Where to start looking.</param>
    private int Seek(string typed, int from)
    {
        var shown = Browser.Shown;

        for (var step = 0; step < shown.Count; step++)
        {
            var at = (((from + step) % shown.Count) + shown.Count) % shown.Count;

            if (Names.Show(shown[at]).StartsWith(typed, StringComparison.OrdinalIgnoreCase))
            {
                return at;
            }
        }

        return -1;
    }

    /// <summary>Steps from where the last step or click landed, or to an end where none has.</summary>
    /// <param name="by">How far to step, in entries.</param>
    /// <param name="shift">Whether the step extends the choice rather than replacing it.</param>
    private void Step(int by, bool shift)
    {
        if (_lead < 0)
        {
            To(by < 0 ? Browser.Shown.Count - 1 : 0, false);

            return;
        }

        To(_lead + by, shift);
    }

    /// <summary>
    /// Moves to an entry, choosing it alone or taking the range from where the last one landed.
    /// </summary>
    /// <remarks>
    /// The range is taken from the last entry landed on rather than from the toolkit's own anchor, so that
    /// a click and a step agree about where a range grows from: both leave that entry here. Replacing the
    /// choice with the range is what makes a second <c>Shift</c> and arrow grow the same range rather than
    /// adding a second one.
    /// </remarks>
    /// <param name="target">The entry to move to, clamped to the listing.</param>
    /// <param name="shift">Whether to take the range rather than the one entry.</param>
    private void To(int target, bool shift)
    {
        var count = Browser.Shown.Count;

        if (count == 0)
        {
            return;
        }

        target = Math.Clamp(target, 0, count - 1);

        var selection = List.Selection;

        using (selection.BatchUpdate())
        {
            selection.Clear();

            if (shift && _lead >= 0 && _lead != target)
            {
                selection.SelectRange(_lead, target);
            }
            else
            {
                selection.Select(target);
            }
        }

        _lead = target;
        List.ScrollIntoView(target);
    }

    /// <summary>
    /// How many entries a screenful is, which is a line of them times the lines that fit.
    /// </summary>
    /// <remarks>
    /// Read from what is on screen rather than assumed, so that a dock resized tall pages further and a
    /// zoomed grid pages by its own rows.
    /// </remarks>
    private int Page()
    {
        var tall = List.ContainerFromIndex(0)?.Bounds.Height ?? 0;

        if (tall <= 0)
        {
            tall = RowGuess;
        }

        var lines = Math.Max(1, (int)(List.Bounds.Height / tall));

        return Math.Max(1, Columns() * lines);
    }

    /// <summary>Remembers where the pointer went down, since a menu and a step begin there.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The press.</param>
    private void Pressed(object? sender, PointerPressedEventArgs e)
    {
        if (IndexAt(e.Source) is var at && at >= 0)
        {
            _lead = at;
        }
    }

    /// <summary>Opens the entry a double-click landed on.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The tap.</param>
    private void Opened(object? sender, TappedEventArgs e)
    {
        if (IndexAt(e.Source) is var at && at >= 0 && at < Browser.Shown.Count)
        {
            Actions.Open(Browser.Shown[at]);
        }
    }

    /// <summary>
    /// The entry an event landed on, or nothing where it landed on the space around them.
    /// </summary>
    /// <remarks>
    /// Walked up to the item rather than asked of the selection, because a double-click on the space beside
    /// the entries has no entry and must open nothing, and because the selection may hold entries the event
    /// did not touch.
    /// </remarks>
    /// <param name="source">Where the event came from.</param>
    private int IndexAt(object? source)
    {
        for (var visual = source as Visual; visual is not null; visual = visual.GetVisualParent())
        {
            if (visual is ListBoxItem item && List.IndexFromContainer(item) is var at && at >= 0)
            {
                return at;
            }
        }

        return -1;
    }
}

/// <summary>The entries as a table, one to a line.</summary>
internal sealed class ListBrowser : EntryView
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

    /// <summary>The columns after the name, which every row of the table has the same of.</summary>
    private readonly Column[] _values = Values();

    /// <summary>How wide every column is, the grabs between them included.</summary>
    private readonly GridLength[] _widths;

    /// <summary>The row of column names, which every row is kept in step with.</summary>
    private readonly Grid _header = new();

    /// <summary>Where a grab was taken, and how wide the two columns each side of it were then.</summary>
    private (int Left, int Right, double At, double LeftWas, double RightWas)? _grabbed;

    /// <summary>Makes the table over what the browser holds.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or given a menu.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public ListBrowser(IPluginHost host, Browser browser, BrowserActions actions, Clip clip)
        : base(host, browser, actions, clip)
    {
        _widths = Widths();

        var header = Header();

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(header, Dock.Top);
        panel.Children.Add(header);
        panel.Children.Add(List);

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

    /// <summary>One entry as a row: its icon, its name, and what each column of values says.</summary>
    /// <param name="entry">The entry to draw.</param>
    protected override Control Item(Entry entry) => Prepared(entry, Row(entry));

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
    private ColumnDefinitions Definitions()
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
        _header.ColumnDefinitions = Definitions();

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
    /// <param name="entry">The entry to draw.</param>
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

        var row = new Grid { ColumnDefinitions = Definitions() };
        Place(row, cells);

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

        foreach (var (_, row) in Rows)
        {
            if (row is Grid grid)
            {
                Into(grid);
            }
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
internal sealed class GridBrowser : EntryView
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

    /// <summary>How many pixels wide and tall a tile's icon is, and so how wide its name is too.</summary>
    private readonly int _icon;

    /// <summary>Makes the grid layout over what the browser holds.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or given a menu.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    /// <param name="icon">How large an icon is at the zoom this dock is at.</param>
    public GridBrowser(IPluginHost host, Browser browser, BrowserActions actions, Clip clip, int icon)
        : base(host, browser, actions, clip)
    {
        _icon = icon;

        List.ItemsPanel = new FuncTemplate<Panel?>(() => new WrapPanel { Orientation = Orientation.Horizontal });

        // A tile is its own target, so the list item's inset is taken off: an inset here would space the
        // tiles by a number the table chose, and a wrapped grid states its own.
        List.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MinHeightProperty, 0.0),
                },
            }
        );

        Content = List;
    }

    /// <summary>One entry as a tile: its icon above its name, on one line and cut off when it is too long.</summary>
    /// <remarks>
    /// The name is as wide as the icon and not one pixel wider, so that the two read as one column rather
    /// than as a picture with a caption under it that happens to start somewhere else. What does not fit is
    /// taken off the end rather than wrapped, because a tile of two lines is a tile of another height, and a
    /// row of tiles that are not the same height is not a row.
    /// </remarks>
    /// <param name="entry">The entry to draw.</param>
    protected override Control Item(Entry entry) => Prepared(entry, Tile(entry));

    /// <summary>
    /// How many tiles the layout put on a line.
    /// </summary>
    /// <remarks>
    /// Read from where the tiles actually landed rather than worked out from a width, because how wide a
    /// tile is is this view's own business and the layout has already decided. The first line is on screen
    /// whenever anything is, so the break is found by walking until one tile stands lower than the first.
    /// </remarks>
    protected override int Columns()
    {
        var count = Browser.Shown.Count;

        if (count <= 1 || List.ContainerFromIndex(0) is not { } first)
        {
            return 1;
        }

        var top = first.Bounds.Y;

        for (var at = 1; at < count; at++)
        {
            if (List.ContainerFromIndex(at) is not { } next)
            {
                return at;
            }

            if (next.Bounds.Y > top + 0.5)
            {
                return at;
            }
        }

        return count;
    }

    /// <summary>One entry as a tile: its icon above its name.</summary>
    /// <param name="entry">The entry to draw.</param>
    private Control Tile(Entry entry)
    {
        var tile = new Border
        {
            Padding = new Thickness(Around),
            Margin = new Thickness(4),
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

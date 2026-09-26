using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Selection;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using Avalonia.Styling;
using Avalonia.Threading;
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

    /// <summary>How far the pointer moves with the button down before it drags or frames rather than clicks.</summary>
    private const double Frame = 4;

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

    /// <summary>The pointer that holds the capture, while a frame is being drawn.</summary>
    private IPointer? _pointer;

    /// <summary>Where a press on the space around the entries was, while it may still become a frame.</summary>
    private Point _framedAt;
    private bool _mayFrame;

    /// <summary>Whether a frame is being drawn, and the band it is drawn with.</summary>
    private bool _framing;
    private readonly Border _band = new()
    {
        IsVisible = false,
        IsHitTestVisible = false,
        BorderThickness = new Thickness(1),
    };

    /// <summary>What the band is drawn over, filling the view and taking no pointer of its own.</summary>
    private readonly Canvas _over = new() { IsHitTestVisible = false };

    /// <summary>Where a press landed and on which entry, while it may still become a drag.</summary>
    private Point _slip;
    private int _slippedAt = -1;

    /// <summary>The choice as it stood when the pointer went down, before the toolkit collapsed it to one.</summary>
    private IReadOnlyList<Entry> _atPress = [];

    /// <summary>The press a drag would begin from, while it may still become one.</summary>
    private PointerPressedEventArgs? _from;

    /// <summary>What the drag in flight carries, for the taking away that a move out of the program owes.</summary>
    private IReadOnlyList<Entry> _carried = [];

    /// <summary>The row wearing the drop mark, so that it can be taken off again.</summary>
    private Control? _marked;

    /// <summary>
    /// Whether this program's own drop handler answered the drag in flight.
    /// </summary>
    /// <remarks>
    /// It is what tells a moved-out drag from a moved-in one: a drop answered here has already moved the files,
    /// so the source must not take the originals away as well.
    /// </remarks>
    private static bool _answered;

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
        List.AddHandler(PointerMovedEvent, Moved, RoutingStrategies.Tunnel);
        List.AddHandler(PointerReleasedEvent, Released, RoutingStrategies.Tunnel);
        List.AddHandler(TextInputEvent, Typed, RoutingStrategies.Tunnel);

        List.DoubleTapped += Opened;
        List.PointerCaptureLost += Lost;

        // A drop from another program arrives as a routed drag event, which a control only hears where it has
        // said it will take one.
        DragDrop.SetAllowDrop(List, true);
        List.AddHandler(DragDrop.DragOverEvent, DraggedOver);
        List.AddHandler(DragDrop.DragLeaveEvent, DraggedOff);
        List.AddHandler(DragDrop.DropEvent, Dropped);

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
    /// Puts what the view is made of on screen, with the frame's band drawn over it.
    /// </summary>
    /// <remarks>
    /// The band is drawn over the view rather than into it, so that a frame can go where a row is: drawn
    /// among the entries it would be laid out as one of them, or clipped to the one it sat in.
    /// </remarks>
    /// <param name="content">The list, or the list with a header over it.</param>
    protected void Present(Control content)
    {
        _over.Children.Add(_band);

        var grid = new Grid();
        grid.Children.Add(content);
        grid.Children.Add(_over);

        Content = grid;
    }

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

    /// <summary>Copies what is chosen to the clipboard.</summary>
    public void Copy() => Actions.Copy(this, Offered());

    /// <summary>Cuts what is chosen to the clipboard, which a paste then moves.</summary>
    public void Cut() => Actions.Cut(this, Offered());

    /// <summary>Pastes what is on the clipboard into the directory being looked at.</summary>
    public void Paste() => Actions.Paste(this, Browser.Current);

    /// <summary>
    /// What a shortcut acts on.
    /// </summary>
    /// <remarks>
    /// The choice, and where nothing is chosen, where the last click landed: a shortcut is reached with the
    /// keyboard anywhere in the dock, so it cannot assume a choice was made, and acting on the entry the
    /// pointer last touched is what a user who has clicked one and not another means by "this".
    /// </remarks>
    private IReadOnlyList<Entry> Offered()
    {
        var chosen = Chosen();

        return chosen.Count > 0 || Lead is not { } lead ? chosen : [lead];
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
    /// The arrows step, <c>Home</c>, <c>End</c>, <c>PageUp</c> and <c>PageDown</c> go further,
    /// <c>Shift</c> extends from where the last step landed, and <c>Enter</c> opens. Everything else — the
    /// toolkit's own select-all among it, and the clipboard, which is the dock's and handled there — is
    /// left to the list.
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
                    Copy();
                    e.Handled = true;
                    break;
                case Key.X:
                    Cut();
                    e.Handled = true;
                    break;
                case Key.V:
                    Paste();
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
    /// Takes a run of typed characters as a move to the next entry that begins with them.
    /// </summary>
    /// <remarks>
    /// A run of one character written over and over is that character asked for again rather than a name that
    /// begins with two of them: the search is for the one character and starts after where the last move
    /// landed, so pressing a letter repeatedly walks the entries that begin with it. That is what makes a
    /// directory full of similarly named things reachable from the keyboard, and it is the behaviour of the
    /// file managers this is read like.
    /// <para>
    /// Any run of characters is searched for as a whole — so typing quickly spells a longer name — and the
    /// search always starts after the last move and wraps. The run is forgotten a second after the last
    /// character, which is how a new run begins.
    /// </para>
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

        var sought = Repeated(_typed) ? _typed[..1] : _typed;
        var at = Seek(sought, _lead + 1);

        if (at < 0)
        {
            at = Seek(sought, 0);
        }

        if (at < 0)
        {
            return;
        }

        To(at, false);
        e.Handled = true;
    }

    /// <summary>Whether a run of typed characters is one character written over and over.</summary>
    /// <param name="typed">What was typed.</param>
    private static bool Repeated(string typed) =>
        typed.Length > 1 && typed.All(character => character == typed[0]);

    /// <summary>The first entry from a place on that begins with what was typed, or nothing.</summary>
    /// <remarks>
    /// The way up is left out: it is not a name but a place, and it is reached by its own means rather than by
    /// being spelled — a run that matches it would be a run that answers with something the reader did not
    /// name.
    /// </remarks>
    /// <param name="typed">What was typed.</param>
    /// <param name="from">Where to start looking.</param>
    private int Seek(string typed, int from)
    {
        var shown = Browser.Shown;

        for (var step = 0; step < shown.Count; step++)
        {
            var at = (((from + step) % shown.Count) + shown.Count) % shown.Count;
            var entry = shown[at];

            if (!Browser.IsUp(entry.Path) &&
                Names.Show(entry).StartsWith(typed, StringComparison.OrdinalIgnoreCase))
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

    /// <summary>
    /// Remembers where the pointer went down, since a menu, a step and a frame all begin there.
    /// </summary>
    /// <remarks>
    /// A press on an entry is a step; a press on the space around the entries is how a choice is dropped and
    /// maybe the start of a frame. Which of the two it is is settled here, because only the press knows whether
    /// it landed on something.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The press.</param>
    private void Pressed(object? sender, PointerPressedEventArgs e)
    {
        var at = IndexAt(e.Source);

        _mayFrame = false;

        if (at >= 0)
        {
            _lead = at;

            if (e.GetCurrentPoint(List).Properties.IsLeftButtonPressed)
            {
                _slip = e.GetPosition(List);
                _slippedAt = at;

                // Read now rather than when the drag starts, because pressing an entry that is part of a choice
                // collapses that choice to the one entry before a drag could be noticed — the toolkit's own
                // behaviour, and the wrong one to carry a set by.
                _atPress = Chosen();
                _from = e;
            }

            return;
        }

        // The space around the entries: a click there drops the choice, and a drag there frames a new one. The
        // choice goes at once, so that a click that never becomes a frame has done what it looked like it would.
        if (e.GetCurrentPoint(List).Properties.IsLeftButtonPressed)
        {
            _lead = -1;
            _mayFrame = true;
            _framedAt = e.GetPosition(this);
            List.Selection.Clear();
            List.Focus();
        }
    }

    /// <summary>
    /// Turns a press into a drag or a frame, and follows whichever is on.
    /// </summary>
    /// <remarks>
    /// A drag is begun through the toolkit, whose X11 backend carries XDND in Avalonia 12, so that a drag can
    /// leave the program and another program's drag can arrive (Section 19.6). A frame is this plugin's own,
    /// because it is a selection gesture the toolkit has none of.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The move.</param>
    private void Moved(object? sender, PointerEventArgs e)
    {
        if (!e.GetCurrentPoint(List).Properties.IsLeftButtonPressed)
        {
            return;
        }

        if (_slippedAt >= 0)
        {
            var slid = e.GetPosition(List) - _slip;

            if (Math.Abs(slid.X) >= Frame || Math.Abs(slid.Y) >= Frame)
            {
                Started();
            }

            return;
        }

        if (!_mayFrame)
        {
            return;
        }

        var away = e.GetPosition(this) - _framedAt;

        if (!_framing)
        {
            if (Math.Abs(away.X) < Frame && Math.Abs(away.Y) < Frame)
            {
                return;
            }

            _framing = true;
            _pointer = e.Pointer;
            _pointer.Capture(List);
        }

        Stretch(e);
    }

    /// <summary>
    /// Draws the frame from where the press was to where the pointer is, and chooses what it covers.
    /// </summary>
    /// <remarks>
    /// The frame is drawn in this view's own coordinates, and a row is asked where it is in the same space, so
    /// the two meet however the list is scrolled. Only the rows the toolkit has made count: one never drawn is
    /// one the user cannot see, and a frame is a gesture on what is on screen.
    /// </remarks>
    /// <param name="e">The move.</param>
    private void Stretch(PointerEventArgs e)
    {
        var frame = new Rect(_framedAt, e.GetPosition(this)).Normalize();

        _band.BorderBrush = Resource("ThemeAccentBrush", Brushes.DodgerBlue);
        _band.Background = Resource("ThemeAccentBrush4", Brushes.Transparent);
        Canvas.SetLeft(_band, frame.X);
        Canvas.SetTop(_band, frame.Y);
        _band.Width = frame.Width;
        _band.Height = frame.Height;
        _band.IsVisible = true;

        var chosen = new List<int>();

        for (var at = 0; at < Browser.Shown.Count; at++)
        {
            if (List.ContainerFromIndex(at) is not { } row ||
                row.TranslatePoint(new Point(0, 0), this) is not { } origin)
            {
                continue;
            }

            if (frame.Intersects(new Rect(origin, row.Bounds.Size)))
            {
                chosen.Add(at);
            }
        }

        var selection = List.Selection;

        using (selection.BatchUpdate())
        {
            selection.Clear();

            foreach (var at in chosen)
            {
                selection.Select(at);
            }
        }

        _lead = chosen.Count > 0 ? chosen[^1] : -1;
    }

    /// <summary>Ends a frame, leaving what it chose chosen.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The release.</param>
    private void Released(object? sender, PointerReleasedEventArgs e)
    {
        if (_mayFrame)
        {
            Stop();
        }
    }

    /// <summary>Ends a frame whose capture was taken away, which leaves what it chose chosen.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The lost capture.</param>
    private void Lost(object? sender, PointerCaptureLostEventArgs e)
    {
        if (_mayFrame)
        {
            Stop();
        }
    }

    /// <summary>Clears the frame: no capture, no band, and nothing left waiting to become one.</summary>
    private void Stop()
    {
        _mayFrame = false;
        _framing = false;
        _band.IsVisible = false;
        _pointer?.Capture(null);
        _pointer = null;
    }

    /// <summary>
    /// The wash a row is framed with, and a stand-in where the look names no tint of its own.
    /// </summary>
    /// <remarks>
    /// Asked of the theme through this view, because a plugin has no other way to read the palette (Section
    /// 10); a program wearing no theme answers with nothing and the stand-in is used.
    /// </remarks>
    /// <param name="key">The resource key.</param>
    /// <param name="fallback">What to draw with where the look defines nothing under the key.</param>
    private IBrush Resource(string key, IBrush fallback) =>
        this.TryGetResource(key, null, out var found) && found is IBrush brush ? brush : fallback;

    /// <summary>
    /// Starts a drag of the choice as it stood when the pointer went down.
    /// </summary>
    /// <remarks>
    /// The files are offered themselves as well as their paths written out, so that a file manager receives
    /// them as files and a text field as text. What the platform does with the drag is its own business: this
    /// hands over the data and waits to be told what became of it (Section 19.6).
    /// </remarks>
    private async void Started()
    {
        var from = _from;
        var pressed = _slippedAt;
        var chosen = _atPress;

        _slippedAt = -1;
        _from = null;
        _atPress = [];

        if (from is null ||
            pressed < 0 ||
            pressed >= Browser.Shown.Count ||
            TopLevel.GetTopLevel(this)?.StorageProvider is not { } storage)
        {
            return;
        }

        var dragged = Browser.Shown[pressed];
        IReadOnlyList<Entry> carrying = chosen.Any(item => string.Equals(item.Path, dragged.Path, StringComparison.Ordinal))
            ? chosen
            : [dragged];

        // Places rather than things: nothing about the way up or the computer is a file to hand over.
        carrying = [.. carrying.Where(entry => !Browser.IsUp(entry.Path) && !Browser.IsComputer(entry.Path))];

        if (carrying.Count == 0)
        {
            return;
        }

        var transfer = new DataTransfer();

        foreach (var entry in carrying)
        {
            var item = entry.Kind == EntryKind.Directory
                ? (IStorageItem?)await storage.TryGetFolderFromPathAsync(entry.Path)
                : await storage.TryGetFileFromPathAsync(entry.Path);

            if (item is not null)
            {
                transfer.Add(DataTransferItem.CreateFile(item));
            }
        }

        transfer.Add(DataTransferItem.CreateText(string.Join(Environment.NewLine, carrying.Select(entry => entry.Path))));

        _carried = carrying;
        _answered = false;

        try
        {
            var effect = await DragDrop.DoDragDropAsync(from, transfer, DragDropEffects.Move | DragDropEffects.Copy);

            // A move another program made is a move this program has to finish: the other program copied the
            // files it was handed, and the originals are the source's to take away. A move this program
            // answered itself already moved them, which is what _answered records — taking them away again
            // would delete what was just carried.
            if (effect == DragDropEffects.Move && !_answered)
            {
                await FileOps.Remove(_carried, Host.Log.Error);
                Later();
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Host.Log.Error(error.Message);
        }

        _carried = [];
    }

    /// <summary>Says whether a drag may land here, and lights the directory it would land in.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOver(object? sender, DragEventArgs e)
    {
        if (Landing(e) is not { } land)
        {
            e.DragEffects = DragDropEffects.None;
            Mark(null);

            return;
        }

        e.DragEffects = Copying(e) ? DragDropEffects.Copy : DragDropEffects.Move;
        Mark(land.Row);
        e.Handled = true;
    }

    /// <summary>Takes the drop mark off when a drag leaves.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOff(object? sender, DragEventArgs e) => Mark(null);

    /// <summary>
    /// Moves or copies what a drag brought into the directory it was let go over.
    /// </summary>
    /// <remarks>
    /// A drag this program started is answered here too, so a drag between two of its docks never leaves it,
    /// and the source is told by <see cref="_answered"/> not to remove the originals again.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drop.</param>
    private async void Dropped(object? sender, DragEventArgs e)
    {
        Mark(null);

        if (Landing(e) is not { } land)
        {
            return;
        }

        var into = land.Into;

        // Told before the work is waited for: what the platform is owed an answer to cannot wait for a question
        // the agent may have to put to a person.
        _answered = true;
        e.Handled = true;

        var copy = Copying(e) || e.DragEffects == DragDropEffects.Copy;

        // Into the directory it is already in is a move that would only rename it, or a copy that makes a
        // second one. Neither is what letting go inside one directory means, so neither is offered.
        var paths = Paths(e.DataTransfer)
            .Where(path => !string.Equals(ParentOf(path), into, StringComparison.Ordinal))
            .ToArray();

        if (paths.Length == 0)
        {
            return;
        }

        if (copy)
        {
            await FileOps.Copy(paths, into, Host.Log.Error);
        }
        else
        {
            await FileOps.Move(paths, into, Host.Log.Error);
        }

        Later();
    }

    /// <summary>
    /// Reads the directory again, but not before this event is over.
    /// </summary>
    /// <remarks>
    /// A drop is answered inside the drag's own event, and reading the directory again rebuilds the view that
    /// is answering it — a view taken apart while it is still handling the event that took it apart. Deferring
    /// the read is what keeps the two apart, and it costs the time it takes to get back to the loop.
    /// </remarks>
    private void Later() => Dispatcher.UIThread.Post(Browser.Refresh);

    /// <summary>
    /// <summary>Where a drag would land, or nothing where it may not land here at all.</summary>
    /// </summary>
    /// <remarks>
    /// A directory entry is a place to let go into, and so is the way up — which is not a directory of its own
    /// but the one holding the listing, so what it lands in is asked of the browser. A file and the computer
    /// are not, and neither is a drag carrying no file. The space around the entries is the directory being
    /// looked at.
    /// </remarks>
    /// <param name="e">The drag.</param>
    private Land? Landing(DragEventArgs e)
    {
        if (!e.DataTransfer.Contains(DataFormat.File))
        {
            return null;
        }

        var at = e.GetPosition(this);

        for (var visual = this.GetVisualAt(at) as Visual; visual is not null; visual = visual.GetVisualParent())
        {
            if (visual is ListBoxItem item && List.IndexFromContainer(item) is var index && index >= 0)
            {
                var entry = Browser.Shown[index];

                if (Browser.IsUp(entry.Path))
                {
                    return Browser.Parent is { Length: > 0 } up ? new Land(up, entry.Path) : null;
                }

                return entry.Kind == EntryKind.Directory && !Browser.IsComputer(entry.Path)
                    ? new Land(entry.Path, entry.Path)
                    : null;
            }
        }

        return Browser.IsComputer(Browser.Current) ? null : new Land(Browser.Current, string.Empty);
    }

    /// <summary>Where a drag lands: the directory it goes into, and the row that stands for it.</summary>
    /// <param name="Into">The directory the dragged entries go into.</param>
    /// <param name="Row">The path of the row to light, or nothing where no row stands for the place.</param>
    private readonly record struct Land(string Into, string Row);

    /// <summary>Whether the drag is one to copy rather than to move, which is what holding a control asks for.</summary>
    /// <param name="e">The drag.</param>
    private static bool Copying(DragEventArgs e) => e.KeyModifiers.HasFlag(KeyModifiers.Control);

    /// <summary>The paths a drag carries, whether it brought them as files or as text.</summary>
    /// <param name="data">What the drag carries.</param>
    private static IReadOnlyList<string> Paths(IDataTransfer data)
    {
        var files = data.TryGetFiles();
        var fromFiles = files?
            .Select(file => file.TryGetLocalPath())
            .Where(path => path is { Length: > 0 })
            .Select(path => FileOps.Bare(path!))
            .ToArray() ?? [];

        if (fromFiles.Length > 0)
        {
            return fromFiles;
        }

        var text = data.TryGetText();

        return text is null
            ? []
            : text.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Select(FileOps.Bare)
                .Where(path => File.Exists(path) || System.IO.Directory.Exists(path))
                .ToArray();
    }

    /// <summary>Paints the row for the directory a drag is over, or takes the paint off.</summary>
    /// <param name="directory">The directory being pointed at, or nothing.</param>
    private void Mark(string? directory)
    {
        Unmark();

        if (directory is null)
        {
            return;
        }

        foreach (var (entry, row) in _rows)
        {
            if (!string.Equals(entry.Path, directory, StringComparison.Ordinal))
            {
                continue;
            }

            Paint(row, Resource("ThemeAccentBrush4", new SolidColorBrush(Color.FromArgb(0x33, 0x80, 0x80, 0x80))));
            _marked = row;

            return;
        }
    }

    /// <summary>Takes the drop mark off whatever was wearing it.</summary>
    private void Unmark()
    {
        if (_marked is { } row)
        {
            Paint(row, null);
            _marked = null;
        }
    }

    /// <summary>Sets or clears a row's fill, whichever kind of row it is.</summary>
    /// <param name="row">The row.</param>
    /// <param name="brush">What to fill it with, or nothing to empty it.</param>
    private static void Paint(Control row, IBrush? brush)
    {
        switch (row)
        {
            case Panel panel:
                panel.Background = brush;
                break;
            case Border border:
                border.Background = brush;
                break;
        }
    }

    /// <summary>The directory an entry sits in, for telling a move that would go nowhere.</summary>
    /// <param name="path">The path to ask about.</param>
    private static string ParentOf(string path) => Path.GetDirectoryName(FileOps.Bare(path)) ?? string.Empty;

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

    /// <summary>
    /// How much room is left above and below each row, and below the last of them.
    /// </summary>
    /// <remarks>
    /// The room is not only looks: a frame (§7.7) is begun where no entry is, so a table whose rows met edge
    /// to edge would be a table a frame could never start in — every point would be on a row. It is left on
    /// the **item** rather than inside it, for the reason the grid leaves it there (§7.5): an item's own area
    /// is what answers the pointer, so room left inside it is room it still covers. It is left vertically
    /// only, since room at the sides would carry the columns away from the headings standing over them.
    /// </remarks>
    private const int Gap = 3;

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

        // The header is not in the list, so the list's own inset would push every row's columns one way and
        // leave the header's where they were — a table whose headings stand a few pixels off their data.
        // Both are inset by their list item alone, which is what puts them on the same line. What is left is
        // room below the last row for a frame to begin in.
        List.Padding = new Thickness(0, 0, 0, Gap);

        // Room between the rows, on the item rather than inside it, and vertical only.
        List.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters = { new Setter(Layoutable.MarginProperty, new Thickness(0, Gap, 0, Gap)) },
            }
        );

        var header = Header();

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(header, Dock.Top);
        panel.Children.Add(header);
        panel.Children.Add(List);

        Present(panel);
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
    /// How much room is left around each tile, and so between one and the next.
    /// </summary>
    /// <remarks>
    /// The room is not only looks: a frame (§7.7) is begun where no entry is, so a grid whose tiles met
    /// edge to edge would be a grid a frame could never start in — every point would be on a tile. It is left
    /// on the **item** rather than on the tile drawn inside it, because it is the item's own area that answers
    /// the pointer: room left on what the item holds is room the item still covers, and a frame begun there
    /// would be a frame begun on the entry.
    /// </remarks>
    private const int Gap = 6;

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

        // A wrapping grid needs a width to wrap against, and the base theme gives a list a horizontal
        // scrollbar instead: with one, the panel is measured at an unbounded width, lays every tile on one
        // line, and the dock scrolls sideways rather than putting the next tile on the next row. So the
        // horizontal scroll is taken off here, which is what makes the panel wrap.
        ScrollViewer.SetHorizontalScrollBarVisibility(List, ScrollBarVisibility.Disabled);

        // Tiles state their own spacing, so the list's own inset becomes that spacing rather than the
        // table's: it is the table that wants the inset's alignment with its header, not this. What is left is
        // a band around the whole grid for a frame to begin in.
        List.Padding = new Thickness(Gap);

        // A tile is its own target, so the list item's inset is taken off; and the room between tiles is
        // left here, on the item, rather than on the tile it holds.
        List.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MarginProperty, new Thickness(Gap)),
                    new Setter(Layoutable.MinHeightProperty, 0.0),
                },
            }
        );

        Present(List);
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

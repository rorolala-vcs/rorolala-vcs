using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Selection;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using MenuItem = Avalonia.Controls.MenuItem;

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

    /// <summary>
    /// How much of a name a rename chooses to begin with.
    /// </summary>
    /// <remarks>
    /// The extension of a file is left out of what is chosen, since what is being renamed is the name:
    /// typing over the whole of <c>hero.psd</c> to call it <c>villain</c> would take the extension with it,
    /// which is a way to lose what a file is by accident. Where the extension begins is the last dot, so
    /// <c>archive.tar.gz</c> renames to <c>archive</c> and keeps <c>.tar.gz</c> — what a program that names a
    /// file by its extension calls the extension.
    /// <para>
    /// A directory has no extension to leave out, and neither has a dot-file: the dot that begins
    /// <c>.gitignore</c> is part of the name rather than the start of one, and a name with no dot at all has
    /// none.
    /// </para>
    /// </remarks>
    /// <param name="kind">What is being renamed.</param>
    /// <param name="name">The name it has.</param>
    /// <returns>Where the chosen part of the name ends, which is the whole name when there is none to leave.</returns>
    public static int Chosen(EntryKind kind, string name)
    {
        if (kind is EntryKind.Directory)
        {
            return name.Length;
        }

        var dot = name.LastIndexOf('.');

        return dot > 0 ? dot : name.Length;
    }

    /// <summary>
    /// A name with a number put before its extension, which is what a second of the same name is called.
    /// </summary>
    /// <remarks>
    /// It is the choice above, used the other way round: the boundary <see cref="Chosen"/> leaves out of what
    /// a rename selects is exactly where a number belongs, because it is the boundary between the name and
    /// what says what kind of file it is. So <c>源文件.txt</c> becomes <c>源文件 (2).txt</c> rather than
    /// <c>源文件.txt (2)</c> — a name that ends in the number has no extension at all as far as anything
    /// that opens a file by one is concerned, and the file stops being the kind of file it is. Everything
    /// before the last dot is the name, so a stack of them keeps all of it but the last:
    /// <c>源文件.pkg.tar.zst</c> is <c>源文件.pkg.tar (2).zst</c>, since <c>.zst</c> is what a program that
    /// reads an extension reads.
    /// <para>
    /// A directory and a dot-file have no extension, so the number goes at the end: <c>.gitignore</c> is
    /// <c>.gitignore (2)</c> rather than a name the number was pushed in front of.
    /// </para>
    /// </remarks>
    /// <param name="name">The name that is taken.</param>
    /// <param name="number">The number to put in it.</param>
    /// <returns>The name to try.</returns>
    public static string Numbered(string name, int number)
    {
        var dot = Chosen(EntryKind.File, name);

        return dot == name.Length
            ? $"{name} ({number})"
            : $"{name[..dot]} ({number}){name[dot..]}";
    }
}

/// <summary>
/// What a gesture on a listing leaves chosen: the choice it was made from, the entries it was made on, and the
/// modifiers it was made with.
/// </summary>
/// <remarks>
/// Clicking and framing are two gestures and do not mean the same thing, so what a modifier means is read by the
/// gesture that was made and not by the modifier alone: a frame over a run of entries reads it as a rule over the
/// whole run, and a click reads it as a rule over the one entry under the pointer.
/// <para>
/// Both are read from the choice as it stood when the pointer went down rather than from the choice on screen, so
/// that a frame dragged back and forth settles on the answer the rule gives rather than on its own last preview —
/// which is also what lets a gesture begin without dropping the choice it was made beside.
/// </para>
/// </remarks>
internal static class Choice
{
    /// <summary>
    /// The choice a frame leaves: a rule read over every entry the frame covers.
    /// </summary>
    /// <remarks>
    /// With nothing held the frame is a choice of what it covers and of nothing else, which is what makes a frame
    /// over nothing a gesture that chooses nothing. <c>Shift</c> toggles each entry it covers, <c>Ctrl</c> takes
    /// each away, and the two together put each back.
    /// </remarks>
    /// <param name="from">The choice as it stood when the pointer went down.</param>
    /// <param name="acted">The entries the frame covers.</param>
    /// <param name="modifiers">What was held while it was drawn.</param>
    /// <returns>The places to choose, in the order the listing shows them.</returns>
    public static IReadOnlyList<int> Framed(
        IReadOnlyCollection<int> from,
        IReadOnlyCollection<int> acted,
        KeyModifiers modifiers
    )
    {
        var shift = modifiers.HasFlag(KeyModifiers.Shift);
        var control = modifiers.HasFlag(KeyModifiers.Control);

        if (!shift && !control)
        {
            return [.. acted.Order()];
        }

        var chosen = new HashSet<int>(from);

        foreach (var at in acted)
        {
            if (shift && control)
            {
                chosen.Add(at);
            }
            else if (control)
            {
                chosen.Remove(at);
            }
            else if (!chosen.Remove(at))
            {
                // Shift alone toggles, and what was not there to be taken away is put there instead.
                chosen.Add(at);
            }
        }

        return [.. chosen.Order()];
    }

    /// <summary>
    /// The choice a click leaves: a rule read over the one entry the pointer landed on.
    /// </summary>
    /// <remarks>
    /// Nothing held chooses that entry and lets go of the rest; <c>Ctrl</c> toggles the one entry. <c>Shift</c>
    /// is the range, and it is read against the two ends the choice has where it is one unbroken run: the end the
    /// click is nearer to is moved to the click and the end further off stays, so that a click beyond the run
    /// grows it and a click inside it takes the run back on the side it was made from — a run made too long is
    /// shortened by shift clicking the entry it is to end at, which adding a range could never do. A choice that
    /// is empty or has a gap in it has no such pair of ends, so <c>Shift</c> over it is what it always was: the
    /// run from where the last click or step landed, added to what is chosen.
    /// <para>
    /// There is no <c>Ctrl</c> with <c>Shift</c> here: a click is on one entry and has no run for the two to
    /// divide, so <c>Ctrl</c> is read and <c>Shift</c> beside it is not.
    /// </para>
    /// </remarks>
    /// <param name="from">The choice as it stood when the pointer went down.</param>
    /// <param name="at">The entry the click landed on, or nothing where it landed on the space around them.</param>
    /// <param name="anchor">Where the last click or step landed, which a run is added from; nothing where none has.</param>
    /// <param name="modifiers">What was held while it was made.</param>
    /// <returns>The places to choose, in the order the listing shows them.</returns>
    public static IReadOnlyList<int> Clicked(
        IReadOnlyCollection<int> from,
        int at,
        int anchor,
        KeyModifiers modifiers
    )
    {
        var shift = modifiers.HasFlag(KeyModifiers.Shift);
        var control = modifiers.HasFlag(KeyModifiers.Control);

        // The space around the entries is a click on nothing: with nothing held that is the choice let go of, and
        // a modifier over nothing has nothing to toggle, to add or to take away.
        if (at < 0)
        {
            return !shift && !control ? [] : [.. from.Order()];
        }

        if (control)
        {
            var toggled = new HashSet<int>(from);

            if (!toggled.Remove(at))
            {
                toggled.Add(at);
            }

            return [.. toggled.Order()];
        }

        if (shift && Run(from) is { } run)
        {
            // The end the click is nearer to becomes the click; the other one stays. A click exactly between the
            // two is the same distance from both and is read as the back end's, so that a run is never shortened
            // on a side the user gave no sign of meaning.
            var far = at <= run.First
                ? run.Last
                : at >= run.Last
                    ? run.First
                    : at - run.First < run.Last - at
                        ? run.Last
                        : run.First;

            var moved = new List<int>();

            for (var place = Math.Min(far, at); place <= Math.Max(far, at); place++)
            {
                moved.Add(place);
            }

            return moved;
        }

        if (shift && anchor >= 0 && anchor != at)
        {
            var added = new HashSet<int>(from);

            for (var place = Math.Min(anchor, at); place <= Math.Max(anchor, at); place++)
            {
                added.Add(place);
            }

            return [.. added.Order()];
        }

        return [at];
    }

    /// <summary>
    /// The ends of a choice that is one unbroken run, or nothing where it is empty or has a gap in it.
    /// </summary>
    /// <remarks>
    /// A run is what a click can take hold of an end of: a choice with a gap in it has no one end at either side
    /// for a click to move, which is what leaves the fallback in <see cref="Clicked"/> to say what a
    /// <c>Shift</c> click over one means.
    /// </remarks>
    /// <param name="places">The places chosen.</param>
    /// <returns>The first and last place of the run, or nothing where the choice is not one.</returns>
    private static (int First, int Last)? Run(IReadOnlyCollection<int> places)
    {
        if (places.Count == 0)
        {
            return null;
        }

        var first = places.Min();
        var last = places.Max();

        return last - first + 1 == places.Count ? (first, last) : null;
    }
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
/// The choice is the toolkit's own list selection, because the look already fills a chosen row with the accent
/// — but not the toolkit's own rules for what a press means. Here a click chooses the one entry it landed
/// on, <c>Shift</c> with it moves the nearer end of the run already chosen to the entry it landed on, and
/// <c>Ctrl</c> with it toggles that one entry; a frame over a run of entries reads its modifiers as rules over
/// the whole run instead, which is a different rule set for a different gesture (see <see cref="Choice"/>). The
/// press is therefore read on the way down (<see cref="Pressed"/>) and the toolkit is never left to decide a
/// choice of its own: its <c>Shift</c> takes a range from a fixed anchor and replaces the choice with it, where
/// this one moves an end of the choice and has no frame at all. What else is added here is what the toolkit does
/// not have: the frame, the arrows within a wrapped grid, the typed letters, and a menu that knows about a set.
/// </para>
/// </remarks>
internal abstract class EntryView : UserControl
{
    /// <summary>How faded an entry the platform hides is drawn, while hidden entries are shown.</summary>
    /// <remarks>
    /// Fainter rather than gone: what is hidden is still an entry of the directory, and it is dimmed so that
    /// the eye passes over it rather than being left out of the listing. It is the plugin's one answer for
    /// how a hidden row is drawn, which the tree reads as well — a second number there would be one look
    /// drawn two ways.
    /// </remarks>
    internal const double HiddenOpacity = 0.55;

    /// <summary>How long a run of typed letters stays one run.</summary>
    private static readonly TimeSpan Typing = TimeSpan.FromSeconds(1);

    /// <summary>How tall a row is where nothing on screen says, which is only ever a first guess.</summary>
    private const double RowGuess = 28.0;

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

    /// <summary>Whether a frame is being drawn.</summary>
    private bool _framing;

    /// <summary>Whether a column edge is being dragged, which the view drives rather than a control.</summary>
    private bool _resizing;

    /// <summary>The band a frame is drawn with.</summary>
    private readonly Border _band = Band();

    /// <summary>
    /// The band a frame is drawn with: a border of the primary and a wash of it inside.
    /// </summary>
    /// <remarks>
    /// Both are bound rather than read, because a resource read where the view is built is read before
    /// the view is anywhere a theme reaches — and a band that fell back to a colour of its own would be a
    /// second answer to what the primary is.
    /// </remarks>
    private static Border Band()
    {
        var band = new Border
        {
            IsVisible = false,
            IsHitTestVisible = false,
            BorderThickness = new Thickness(1),
        };

        band[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Primary);
        band[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Selection);

        return band;
    }

    /// <summary>What the band is drawn over, filling the view and taking no pointer of its own.</summary>
    private readonly Canvas _over = new() { IsHitTestVisible = false };

    /// <summary>The card that follows a drag, drawn over this view while the pointer is in it.</summary>
    private readonly Ghost _ghost;

    /// <summary>How a drag is taken here: where it would land, and what is lit while it is over.</summary>
    private readonly Drops _drops;

    /// <summary>Where a press landed and on which entry, while it may still become a drag.</summary>
    private Point _slip;
    private int _slippedAt = -1;

    /// <summary>
    /// The choice as it stood when the pointer went down, as the entries it held.
    /// </summary>
    /// <remarks>
    /// As entries and not as places, because this is what a drag carries: what is handed over is what was
    /// chosen when the hand went down, and not whatever a rebuilt listing would number those places as.
    /// </remarks>
    private IReadOnlyList<Entry> _atPress = [];

    /// <summary>
    /// The same choice as places in the listing, which is what a click and a frame are both read against.
    /// </summary>
    /// <remarks>
    /// Places, because what a frame covers is read in places — it asks each row where it is — and the rule the
    /// gesture means is then read over the two sets by the listing's own numbering. The choice read from is the
    /// one that stood when the pointer went down and not the one on screen while a frame is drawn: a frame that
    /// grew and shrank would otherwise read its rule against its own last preview, and a range toggled back and
    /// forth would never come back to what it started as (see <see cref="Choice"/>).
    /// </remarks>
    private IReadOnlyList<int> _base = [];

    /// <summary>Where the press landed, as a place in the listing, or nothing where it landed on the space around them.</summary>
    private int _pressedAt = -1;

    /// <summary>The modifiers the press was made with, which say what a click and a frame both mean.</summary>
    private KeyModifiers _pressedWith = KeyModifiers.None;

    /// <summary>
    /// Where the last click or step landed before this press, which a <c>Shift</c> click takes a range from.
    /// </summary>
    /// <remarks>
    /// Kept apart from <see cref="_lead"/> because the press moves that one to where it landed: a range is taken
    /// from where the gesture before this one landed, and reading the lead after the press would make every such
    /// range a range of the one entry it was taken to.
    /// </remarks>
    private int _anchor = -1;

    /// <summary>The press a drag would begin from, while it may still become one.</summary>
    private PointerPressedEventArgs? _from;

    /// <summary>The row wearing the drop mark, so that it can be taken off again.</summary>
    private Control? _marked;

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

        _ghost = new Ghost(_over);
        _drops = new Drops(host, browser, Landing, Mark);

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

        // The keyboard is the list's, taken on the way down before the list reads it itself: the arrows are
        // replaced by ones that know about a wrapped grid, and Enter and the typed letters are not the
        // toolkit's at all. A handler that let the list act first would be correcting an action already taken.
        //
        // Text is not a key: only the element with the keyboard raises it, and it is raised on the way up, so
        // the letters are taken on both legs — the down leg reaches nothing while no field has the keyboard,
        // and what a field does raise is left to the field (see `Typed`).
        List.AddHandler(KeyDownEvent, Keyed, RoutingStrategies.Tunnel);
        List.AddHandler(
            TextInputEvent,
            Typed,
            RoutingStrategies.Tunnel | RoutingStrategies.Bubble
        );

        // The pointer is taken for the whole view rather than for the list alone, so that a frame can begin in
        // the room left between the rows as much as on the list's own edges: the list is hit nowhere in the
        // room between two rows, and a press there reaching nothing would be a frame that cannot be started
        // where the room to start one is. Taken on the way down like the keys, since what a press begins
        // — a step, a drag or a frame — is settled before the list acts on it.
        AddHandler(PointerPressedEvent, Pressed, RoutingStrategies.Tunnel);
        AddHandler(PointerMovedEvent, Moved, RoutingStrategies.Tunnel);
        AddHandler(PointerReleasedEvent, Released, RoutingStrategies.Tunnel);

        // A resize captures the pointer to the view, so losing it is this control's to hear rather than the
        // list's.
        AddHandler(PointerCaptureLostEvent, Uncaptured, RoutingStrategies.Tunnel);

        // The wheel is left alone, unlike the keys and the pointer above: it reaches the list's own scroller,
        // which is the toolkit's to answer, so a turn moves the listing the way every other list in the
        // program moves and with nothing of this plugin's animating in between.

        List.DoubleTapped += Opened;
        List.PointerCaptureLost += Lost;

        // A drop from another program arrives as a routed drag event, which a control only hears where it has
        // said it will take one.
        DragDrop.SetAllowDrop(List, true);

        // Enter as well as over: the first position a drag is told of is an enter, so it is the one that draws the
        // mark and the card first — and a drag that stopped moving the moment it arrived would be the one drawn
        // never at all.
        List.AddHandler(DragDrop.DragEnterEvent, DraggedOver);
        List.AddHandler(DragDrop.DragOverEvent, DraggedOver);
        List.AddHandler(DragDrop.DragLeaveEvent, DraggedOff);
        List.AddHandler(DragDrop.DropEvent, Dropped);

        // Repainting only while on screen, like the browser itself: a dock that was closed is not a view
        // of anything to keep in step.
        AttachedToVisualTree += (_, _) =>
        {
            Clipboard.Changed += Repaint;
            actions.Naming += Named;
        };
        DetachedFromVisualTree += (_, _) =>
        {
            Clipboard.Changed -= Repaint;
            actions.Naming -= Named;
        };
    }

    /// <summary>Puts a box in the row of an entry that has just been made, if this view draws it.</summary>
    /// <remarks>
    /// Heard rather than read from the actions because the view showing the directory the folder went into is
    /// not always this one — a dock beside this one may be looking at it — and a hand-off read where it is
    /// kept would be taken by whichever view was made first. A directory this view is not showing is not this
    /// view's to name: a box put in the row of the wrong listing is a name typed where the user is not looking.
    /// </remarks>
    /// <param name="entry">The entry whose name is to be typed.</param>
    private void Named(Entry entry)
    {
        if (!string.Equals(Path.GetDirectoryName(Path.TrimEndingDirectorySeparator(entry.Path)), Browser.Current, StringComparison.Ordinal))
        {
            return;
        }

        // A moment later rather than now, because the row is drawn by the reading that is about to happen: the
        // view is not laid out yet, and a box asked for at a row that does not exist is a box that never is.
        Dispatcher.UIThread.Post(() => Rename(entry), DispatcherPriority.Background);
    }

    /// <summary>The host, for what cannot be done.</summary>
    protected IPluginHost Host { get; }

    /// <summary>The location the entries belong to.</summary>
    protected Browser Browser { get; }

    /// <summary>How many entries this view is showing, which is what a dock reports it drew.</summary>
    public int Shown => Browser.Shown.Count;

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
    /// Whether a press begins a resize of something the view draws, which the view then drives.
    /// </summary>
    /// <remarks>
    /// Asked before the choice and the frame, and answered by taking the press: the pointer is captured to
    /// the view itself, so a drag that leaves the grip keeps resizing, and a press near a column edge
    /// resizes rather than dropping the choice or beginning a frame. A frame begins wherever no entry is,
    /// and a column edge is exactly such a place, so the two would otherwise be the same gesture.
    /// </remarks>
    /// <param name="e">The press.</param>
    /// <returns>Whether the press is the view's to resize with.</returns>
    protected virtual bool StartResize(PointerPressedEventArgs e) => false;

    /// <summary>Follows a resize the view took.</summary>
    /// <param name="e">The move.</param>
    protected virtual void FollowResize(PointerEventArgs e) { }

    /// <summary>Ends a resize the view took.</summary>
    protected virtual void EndResize() { }

    /// <summary>
    /// Lights whatever the pointer is over, on a move that is not part of a drag or a frame.
    /// </summary>
    /// <remarks>
    /// A resize is reached through a band wider than the grip drawn in it, and the band has nothing to
    /// draw: this is where the view says what the pointer is over, so that the band is not invisible.
    /// </remarks>
    /// <param name="at">Where the pointer is, in the view's own coordinates.</param>
    protected virtual void Hovered(Point at) { }

    /// <summary>
    /// Puts what the view is made of on screen, on nothing but the view itself and with the frame's band over it.
    /// </summary>
    /// <remarks>
    /// The band is drawn over the view rather than into it, so that a frame can go where a row is: drawn
    /// among the entries it would be laid out as one of them, or clipped to the one it sat in. It is laid
    /// over the whole view and not over the content, because the frame is measured in this control's own
    /// coordinates and a band inset by a container would not land where the pointer is.
    /// <para>
    /// There is no container around the content, and that is the point rather than tidy: what a listing is
    /// fills the dock, so the room a frame is begun in has to be the listing's own — the gaps between its rows
    /// or its tiles — rather than a margin belonging to something the listing sits in.
    /// </para>
    /// </remarks>
    /// <param name="content">The card, or the tiles, the list at the heart of it.</param>
    protected void Present(Control content)
    {
        _over.Children.Add(_band);

        // A ground of nothing rather than none at all, so that a press on the room between the entries reaches
        // this view at all: what a frame is begun from is a press that landed on no entry.
        var grid = new Grid { Background = Brushes.Transparent };
        grid.Children.Add(content);

        // The word is drawn over the empty space rather than in place of the list, so that the menu and the
        // frame the space still answers keep answering.
        if (Barren())
        {
            grid.Children.Add(Notice());
        }

        grid.Children.Add(_over);

        Content = grid;
    }

    /// <summary>
    /// Whether the listing holds nothing but the way up.
    /// </summary>
    /// <remarks>
    /// The way up stands before every listing that is not the base's own and is not an entry of the
    /// directory, so a directory with nothing in it still shows one row; the word for an empty listing is
    /// due where there is nothing but that row.
    /// </remarks>
    private bool Barren() => !Browser.Shown.Any(entry => !Browser.IsUp(entry.Path));

    /// <summary>
    /// What a listing with nothing in it says.
    /// </summary>
    /// <remarks>
    /// It takes no pointer of its own: the space it sits over is still the list's, and a word that ate the
    /// right-click there would take the empty-space menu and the start of a frame with it.
    /// </remarks>
    private static Control Notice() =>
        new StackPanel
        {
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Children =
            {
                new TextBlock
                {
                    Text = "\U0001F5C2",
                    FontSize = 34,
                    Classes = { "faint" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
                new TextBlock
                {
                    Text = RolaI18N.Get("rorolala_file_system.empty"),
                    Classes = { "muted" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
            },
        };

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
        menu.Opening += (_, _) => Actions.Fill(menu, this, Chosen(entry), force: Held);
        row.ContextMenu = menu;

        // What the pointer held is read when it is pressed, because the menu that opens next is laid out before
        // anything can be asked of it, and Shift+right-click means the forced actions are wanted from the start.
        row.PointerPressed += (_, pressed) => Held = (pressed.KeyModifiers & KeyModifiers.Shift) != 0;

        // And while the menu is up, the key itself decides: Shift unfolds what it forces, and letting it go
        // folds it away again. Tunnelled, because the item under the keyboard reads the key first.
        menu.AddHandler(KeyDownEvent, (_, key) => Revealed(menu, key), RoutingStrategies.Tunnel);
        menu.AddHandler(KeyUpEvent, (_, key) => Revealed(menu, key), RoutingStrategies.Tunnel);

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

    /// <summary>Whether Shift was held when the pointer was last pressed, which a menu opens with.</summary>
    protected bool Held { get; private set; }

    /// <summary>Shows the forced actions while Shift is held, and hides them when it is let go.</summary>
    /// <param name="menu">The menu being read.</param>
    /// <param name="key">The key, which says what is being held.</param>
    private static void Revealed(ContextMenu menu, KeyEventArgs key)
    {
        var forced = (key.KeyModifiers & KeyModifiers.Shift) != 0;

        foreach (
            var item in menu
                .Items.OfType<MenuItem>()
                .Where(item => item.Classes.Contains(BrowserActions.Force))
        )
        {
            item.IsVisible = forced;
        }
    }

    /// <summary>
    /// What a menu opened on an entry is about: the whole choice when that entry is in it.
    /// </summary>
    /// <remarks>
    /// The entry the menu was opened on comes first, because a menu is about what was pointed at before it is
    /// about the rest of the choice: it is the one whose kind decides which menu this is, and the one whose
    /// words a contributed action is named by.
    /// </remarks>
    /// <param name="entry">The entry the menu was opened on.</param>
    protected IReadOnlyList<Entry> Chosen(Entry entry)
    {
        var chosen = Chosen();

        if (!chosen.Any(item => string.Equals(item.Path, entry.Path, StringComparison.Ordinal)))
        {
            return [entry];
        }

        return chosen[0].Path == entry.Path
            ? chosen
            : [entry, .. chosen.Where(item => item.Path != entry.Path)];
    }

    /// <summary>Copies what is chosen to the clipboard.</summary>
    public void Copy() => Actions.Copy(this, Offered());

    /// <summary>Cuts what is chosen to the clipboard, which a paste then moves.</summary>
    public void Cut() => Actions.Cut(this, Offered());

    /// <summary>Pastes what is on the clipboard into the directory being looked at.</summary>
    public void Paste() => Actions.Paste(this, Browser.Current);

    /// <summary>Removes what is chosen, asking first unless told not to.</summary>
    /// <param name="ask">Whether to put the question to the user first.</param>
    public void Delete(bool ask) => Actions.Remove(Offered(), ask);

    /// <summary>The box a name is being edited in, and the entry it is about, while one is.</summary>
    /// <remarks>
    /// The name is edited in the row itself rather than asked for in a window: what is being renamed is a
    /// name in a listing, and the listing is where it can be seen against the names around it.
    /// </remarks>
    private TextBox? _editor;
    private Entry? _editing;

    /// <summary>Renames what is chosen, or the entry the pointer last landed on.</summary>
    public void Rename()
    {
        if (Offered() is [{ } only])
        {
            Rename(only);
        }
    }

    /// <summary>Puts the name of `entry` into a box where the listing draws it.</summary>
    /// <param name="entry">What is being renamed.</param>
    public void Rename(Entry entry)
    {
        // What is edited is a row, and a row that is not on screen has no container to hold a cell: bringing
        // it into view is what makes a name reachable in a directory too long to show at once.
        List.ScrollIntoView(entry);

        if (Beginning(entry))
        {
            return;
        }

        // The container is made by the layout the scroll asks for, so it is a moment behind: one more turn
        // of the loop is what puts the box in a row rather than nowhere.
        Dispatcher.UIThread.Post(() => Beginning(entry), DispatcherPriority.Background);
    }

    /// <summary>Puts the name of `entry` into a box, where its row and the cell in it are already drawn.</summary>
    /// <param name="entry">What is being renamed.</param>
    /// <returns>Whether the box was made.</returns>
    private bool Beginning(Entry entry)
    {
        if (_editor is not null)
        {
            return true;
        }

        var label = _rows
            .Where(row => string.Equals(row.Entry.Path, entry.Path, StringComparison.Ordinal))
            .Select(row => row.Row)
            .SelectMany(row => row.GetVisualDescendants())
            .OfType<TextBlock>()
            .FirstOrDefault(text => text.Classes.Contains("entry-name"));

        if (label?.Parent is not Panel parent)
        {
            return false;
        }

        var at = parent.Children.IndexOf(label);
        var name = Path.GetFileName(Path.TrimEndingDirectorySeparator(entry.Path)) ?? string.Empty;
        var box = new TextBox
        {
            Text = name,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The box takes the place the name had, in the cell that name sat in: a grid's cell is the child's,
        // so it is carried over rather than the row being built again around it.
        Grid.SetColumn(box, Grid.GetColumn(label));
        Grid.SetRow(box, Grid.GetRow(label));
        Grid.SetColumnSpan(box, Grid.GetColumnSpan(label));

        parent.Children[at] = box;

        _editor = box;
        _editing = entry;

        // Taken on the way down, so that Enter and Escape are read before the box's own reading of them: a
        // field that took Enter for itself would be a name nobody could finish.
        box.AddHandler(KeyDownEvent, (_, args) => Edited(args), RoutingStrategies.Tunnel);
        box.LostFocus += (_, _) => Committed();

        box.Focus();

        // The name is chosen rather than taken whole, so that what is typed over is the name and not the
        // extension: see `Names.Chosen` for what is left out and why.
        box.SelectionStart = 0;
        box.SelectionEnd = Names.Chosen(entry.Kind, name);

        return true;
    }

    /// <summary>Makes a directory in the directory being looked at, which is then named where it is drawn.</summary>
    public void NewFolder() => Actions.NewFolder(Browser.Current);

    /// <summary>Reads a key while a name is being edited: Enter takes it, Escape leaves the name alone.</summary>
    /// <param name="e">The key.</param>
    private void Edited(KeyEventArgs e)
    {
        switch (e.Key)
        {
            case Key.Enter:
                e.Handled = true;
                Committed();
                break;
            case Key.Escape:
                e.Handled = true;
                Cancelled();
                break;
            default:
                break;
        }
    }

    /// <summary>Takes what the box says as the name the entry is to have, and lets go of the box.</summary>
    private void Committed()
    {
        var box = _editor;
        var entry = _editing;

        if (box is null || entry is null)
        {
            return;
        }

        _editor = null;
        _editing = null;

        Actions.Rename(entry, box.Text ?? string.Empty);

        // The box belongs to a row, and the reading is what puts the name back: a rename that had nothing to
        // do would otherwise leave the field standing. The keyboard goes back to the listing with it.
        Browser.Touch();
        Listen();
    }

    /// <summary>Leaves the name as it was, and reads the directory again so the box goes away.</summary>
    private void Cancelled()
    {
        if (_editor is null)
        {
            return;
        }

        _editor = null;
        _editing = null;

        Browser.Touch();
        Listen();
    }

    /// <summary>
    /// Puts the keyboard in the listing, so that the dock's own keys reach it again.
    /// </summary>
    /// <remarks>
    /// The listing is where a dock answers its keys from: they are taken at the top of the dock and passed to the
    /// view being read, so a keyboard that is nowhere in the dock is a dock whose keys do nothing. That is what
    /// happens when the address — which hides itself when an edit ends — was where the keyboard was.
    /// </remarks>
    public void Listen() => List.Focus();

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

    /// <summary>
    /// What this view is showing.
    /// </summary>
    /// <remarks>
    /// The listing the view was built with rather than the browser's latest: a read of the directory replaces what
    /// the browser shows before anything is told of it, so a view asked what was chosen just before it is built
    /// again has to answer for the rows it is still showing, or the choice would be read against files that moved
    /// under it.
    /// </remarks>
    private IReadOnlyList<Entry> ShownEntries => List.ItemsSource as IReadOnlyList<Entry> ?? Browser.Shown;

    /// <summary>The chosen entries, in the order the listing shows.</summary>
    public IReadOnlyList<Entry> Chosen()
    {
        var shown = ShownEntries;
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

    /// <summary>
    /// Puts the choice back on what it was on before the listing was built again.
    /// </summary>
    /// <remarks>
    /// A listing is built again whenever its directory is read again — a window come back to, a key asking for a
    /// refresh, a change of zoom — and the choice is what the user was working with. Losing it to a read that
    /// found the same files is losing work rather than changing the subject, and the tree keeps its own choice
    /// across the same rebuild for the same reason. An entry that is gone is simply not chosen again: a listing
    /// whose files have changed is not the place to report which of them went.
    /// </remarks>
    /// <param name="chosen">What was chosen, in the order it was.</param>
    public void Choose(IReadOnlyList<Entry> chosen)
    {
        if (chosen.Count == 0)
        {
            return;
        }

        var shown = ShownEntries;
        var last = -1;

        using (List.Selection.BatchUpdate())
        {
            List.Selection.Clear();

            foreach (var entry in chosen)
            {
                var at = Index(shown, entry);

                if (at < 0)
                {
                    continue;
                }

                List.Selection.Select(at);
                last = at;
            }
        }

        if (last >= 0)
        {
            _lead = last;
        }
    }

    /// <summary>Where an entry stands in a listing, or nothing when it is not there.</summary>
    /// <param name="shown">The listing.</param>
    /// <param name="entry">The entry to find.</param>
    /// <returns>The place it stands in, or nothing.</returns>
    private static int Index(IReadOnlyList<Entry> shown, Entry entry)
    {
        for (var at = 0; at < shown.Count; at++)
        {
            if (shown[at] == entry)
            {
                return at;
            }
        }

        return -1;
    }

    /// <summary>
    /// How faded a row is: one a hidden provider hides reads as secondary, and one that is cut as on its way
    /// out.
    /// </summary>
    /// <remarks>
    /// Both are said here rather than in two passes over the rows, because a row has one opacity and the second
    /// pass would be the one that decided it. Cut wins, because being on its way out is what the user just did.
    /// <para>
    /// What is hidden is asked of the providers rather than carried on the entry, so a row drawn while a
    /// provider was switched off is faded again the moment it is switched on.
    /// </para>
    /// </remarks>
    /// <param name="row">The row to fade.</param>
    /// <param name="entry">What it stands for.</param>
    protected virtual void Apply(Control row, Entry entry) =>
        row.Opacity = Clipboard.IsCut(entry.Path)
            ? Clipboard.Faded
            : Hides(entry)
                ? HiddenOpacity
                : 1.0;

    /// <summary>
    /// Whether a hidden provider hides an entry, which is what the location a row belongs to answers.
    /// </summary>
    /// <remarks>
    /// The location is what a view holds, and a row staged for one location has to be faded by the same
    /// answer that left it out of that location's listing.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether it is hidden.</returns>
    protected bool Hides(Entry entry) => Browser.Hides(entry);

    /// <summary>
    /// Draws every row again with what the listing is worth now: how faded, and whether it is cut.
    /// </summary>
    /// <remarks>
    /// For a change that leaves the entries exactly as they are — the switch that shows what is hidden
    /// changes how the entries are drawn and not which ones there are, so the rows are already in the tree and
    /// need their opacity said again rather than the view being built a second time.
    /// </remarks>
    public void Repaint()
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
    /// toolkit's own select-all among it — is left to the list, and the clipboard is the dock's and answered
    /// there.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        // A name being edited is the box's own: the arrows move its caret and Enter takes what it says, so
        // the keys below are not read while a row is a field. The listing is not what has the keyboard then.
        if (_editor is not null || e.Source is TextBox)
        {
            return;
        }

        _modifiers = e.KeyModifiers;

        var shift = e.KeyModifiers.HasFlag(KeyModifiers.Shift);

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
        // The letters are the box's while a name is being edited, and the field's wherever else a field is
        // what has the keyboard: what is typed there is a name, not a run to seek entries by.
        if (_editor is not null || e.Source is TextBox)
        {
            return;
        }

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
    /// Remembers where the pointer went down, and what that press may become.
    /// </summary>
    /// <remarks>
    /// A press on an entry of the choice — or on any entry at all where nothing is chosen — may become a drag of
    /// what is chosen. Anywhere else it may become a frame, and that is what keeps a frame begun beside a choice
    /// from carrying the entries it was drawn to add to. A press with <c>Shift</c> or <c>Ctrl</c>
    /// held is a hand that is choosing rather than moving, so it may become a frame wherever it lands and never a
    /// drag; whether it became a click or a frame is what says what the modifier meant, since the two gestures
    /// have their own rules (see <see cref="Choice"/>).
    /// <para>
    /// The toolkit is not left to decide what a left press means, because its rules are not these: it takes a
    /// range with <c>Shift</c> and replaces the choice with it, where this adds it, and it has no frame at all.
    /// The press is therefore marked handled, and only where it is read here: a right press is still the toolkit's
    /// and still opens the menu it did. A press in a name being edited is not this view's either, since in a field
    /// it moves a caret.
    /// </para>
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The press.</param>
    private void Pressed(object? sender, PointerPressedEventArgs e)
    {
        // A name being edited is the box's own: a press in it moves the caret, and one outside it is what ends
        // the edit by taking the keyboard away. Either way the press is not this view's to read.
        if (_editor is not null || e.Source is TextBox)
        {
            return;
        }

        var at = IndexAt(e.Source);

        _mayFrame = false;

        // A column edge is taken before the choice and the frame, and taken away from whatever is under
        // it: a press there resizes the column, and marked handled here it never reaches the grip the
        // look draws, whose own drag would then be a second answer to the same press.
        if (
            e.GetCurrentPoint(this).Properties.IsLeftButtonPressed
            && StartResize(e)
        )
        {
            _resizing = true;
            e.Pointer.Capture(this);
            e.Handled = true;

            return;
        }

        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            if (at >= 0)
            {
                _lead = at;
            }

            return;
        }

        var shift = e.KeyModifiers.HasFlag(KeyModifiers.Shift);
        var control = e.KeyModifiers.HasFlag(KeyModifiers.Control);

        // Whatever a press left waiting is done with, so that a gesture whose release was never seen — one whose
        // capture another control took — cannot be finished by the next press instead.
        _slippedAt = -1;
        _from = null;

        // Read before the choice can move, and read against the listing this view was handed: what a click and a
        // frame both mean is a change to the choice as it stood, not to whatever the preview last showed. The
        // anchor is read first of all, since the press is about to move the lead it is taken from.
        _base = [.. List.Selection.SelectedIndexes];
        _atPress = Chosen();
        _anchor = _lead;
        _lead = at;
        _pressedAt = at;
        _pressedWith = e.KeyModifiers;

        e.Handled = true;

        // The keyboard goes to the row the press landed on, which is what the toolkit would have done with the
        // press, or to the listing itself where the press was not on a row. A dock answers its keys at the top of
        // itself and passes them down, so either way they reach the listing.
        (at >= 0 ? List.ContainerFromIndex(at) : List)?.Focus();

        if (at >= 0 && !shift && !control && (_base.Count == 0 || _base.Contains(at)))
        {
            // Where nothing is chosen the entry is chosen at once, so that what a drag takes hold of is what the
            // pointer is on. A choice already standing is left whole, and the pointer coming up without a drag is
            // what narrows it to the one entry (see `Released`).
            if (_base.Count == 0)
            {
                Select([at]);
            }

            _slip = e.GetPosition(List);
            _slippedAt = at;
            _from = e;

            return;
        }

        // Anything else may become a frame, and a press that never moves far enough is read as the click it was
        // when the pointer comes up.
        _mayFrame = true;
        _framedAt = e.GetPosition(this);
    }

    /// <summary>
    /// Turns a press into a drag or a frame, and follows whichever is on.
    /// </summary>
    /// <remarks>
    /// A drag is begun through the toolkit, whose X11 backend carries XDND in Avalonia 12, so that a drag can
    /// leave the program and another program's drag can arrive. A frame is this plugin's own,
    /// because it is a selection gesture the toolkit has none of.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The move.</param>
    private void Moved(object? sender, PointerEventArgs e)
    {
        if (_resizing)
        {
            FollowResize(e);

            return;
        }

        Hovered(e.GetPosition(this));

        if (!e.GetCurrentPoint(List).Properties.IsLeftButtonPressed)
        {
            return;
        }

        if (_slippedAt >= 0)
        {
            var slid = e.GetPosition(List) - _slip;

            if (Math.Abs(slid.X) >= Drag.Slip || Math.Abs(slid.Y) >= Drag.Slip)
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
            if (Math.Abs(away.X) < Drag.Slip && Math.Abs(away.Y) < Drag.Slip)
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
    /// Draws the frame from where the press was to where the pointer is, and previews what it comes to.
    /// </summary>
    /// <remarks>
    /// The frame is drawn in this view's own coordinates, and a row is asked where it is in the same space, so
    /// the two meet however the list is scrolled. Only the rows the toolkit has made count: one never drawn is
    /// one the user cannot see, and a frame is a gesture on what is on screen.
    /// <para>
    /// What the frame comes to is written into the listing as it is drawn rather than kept to one side, because
    /// that is what the user has to see: the accent on the entries the pointer has taken is the whole of the
    /// feedback a frame has. It is read from the choice the press was made from and not from what the last frame
    /// drew, so that a frame dragged back and forth arrives at the answer it would have had (see
    /// <see cref="Choice"/>), which is also why a frame left over nothing does not undo the choice it was drawn
    /// beside until the pointer comes up to say that is what it meant.
    /// </para>
    /// </remarks>
    /// <param name="e">The move.</param>
    private void Stretch(PointerEventArgs e)
    {
        var frame = new Rect(_framedAt, e.GetPosition(this)).Normalize();

        _band.IsVisible = true;
        Canvas.SetLeft(_band, frame.X);
        Canvas.SetTop(_band, frame.Y);
        _band.Width = frame.Width;
        _band.Height = frame.Height;

        var covered = new List<int>();

        for (var at = 0; at < Browser.Shown.Count; at++)
        {
            if (List.ContainerFromIndex(at) is not { } row ||
                row.TranslatePoint(new Point(0, 0), this) is not { } origin)
            {
                continue;
            }

            if (frame.Intersects(new Rect(origin, row.Bounds.Size)))
            {
                covered.Add(at);
            }
        }

        Select(Choice.Framed(_base, covered, _pressedWith));

        _lead = covered.Count > 0 ? covered[^1] : -1;
    }

    /// <summary>
    /// Settles what a press turned out to be, once the pointer is up.
    /// </summary>
    /// <remarks>
    /// A frame settles itself as it is drawn, since its preview is the choice it leaves; what is left for the
    /// pointer coming up is a press that never became one. A press that never slipped far enough to mean a drag
    /// is the click it was, and a press that could only ever have become a frame and never moved is a click that
    /// happened to begin in the space around the entries. Nothing is decided while the pointer is down, so that a
    /// press on an entry of the choice may still become the drag of that whole choice.
    /// </remarks>
    /// <param name="sender">The list.</param>
    /// <param name="e">The release.</param>
    private void Released(object? sender, PointerReleasedEventArgs e)
    {
        if (_resizing)
        {
            _resizing = false;
            EndResize();
            e.Pointer.Capture(null);

            return;
        }

        if (_slippedAt >= 0)
        {
            _slippedAt = -1;
            _from = null;
            _atPress = [];

            Clicked();

            return;
        }

        if (!_mayFrame)
        {
            return;
        }

        var framed = _framing;

        Stop();

        if (!framed)
        {
            Clicked();
        }
    }

    /// <summary>
    /// Reads a press that never became a drag or a frame as the click it was.
    /// </summary>
    /// <remarks>
    /// A click is read over the one entry the pointer landed on, which is not what a frame over a run of the same
    /// entries means: the two gestures have their own rules (see <see cref="Choice"/>). The space around the
    /// entries is the click on nothing, which with nothing held clears the choice and with a modifier leaves it
    /// standing.
    /// </remarks>
    private void Clicked()
    {
        Select(Choice.Clicked(_base, _pressedAt, _anchor, _pressedWith));

        _lead = _pressedAt;
    }

    /// <summary>Puts a choice into the listing, which is what draws it.</summary>
    /// <param name="chosen">The places to choose, in the order the listing shows them.</param>
    private void Select(IReadOnlyList<int> chosen)
    {
        var selection = List.Selection;

        using (selection.BatchUpdate())
        {
            selection.Clear();

            foreach (var at in chosen)
            {
                selection.Select(at);
            }
        }
    }

    /// <summary>Ends a resize whose capture was taken away, which leaves the columns as they stand.</summary>
    /// <param name="sender">The view.</param>
    /// <param name="e">The lost capture.</param>
    private void Uncaptured(object? sender, PointerCaptureLostEventArgs e)
    {
        if (_resizing)
        {
            _resizing = false;
            EndResize();
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
    /// Asked of the theme through this view, because a plugin has no other way to read the palette; a
    /// program wearing no theme answers with nothing and the stand-in is used.
    /// </remarks>
    /// <param name="key">The resource key.</param>
    /// <param name="fallback">What to draw with where the look defines nothing under the key.</param>
    private IBrush Resource(string key, IBrush fallback) =>
        TopLevel.GetTopLevel(this)?.TryFindResource(key, null, out var found) == true && found is IBrush brush
            ? brush
            : fallback;

    /// <summary>
    /// Starts a drag of the choice as it stood when the pointer went down.
    /// </summary>
    /// <remarks>
    /// What is carried is the whole choice, or — where there was no choice to carry — the one entry the press
    /// landed on. Anywhere else the press begins a frame rather than a drag, which is what keeps a frame drawn
    /// beside a choice from carrying the entries it was drawn to add to (see <see cref="Pressed"/>). Everything
    /// after that is the same for every view that can be dragged from (<see cref="Drag.Away"/>).
    /// </remarks>
    private void Started()
    {
        var from = _from;
        var pressed = _slippedAt;
        var chosen = _atPress;

        _slippedAt = -1;
        _from = null;
        _atPress = [];

        if (from is null || pressed < 0 || pressed >= Browser.Shown.Count)
        {
            return;
        }

        var dragged = Browser.Shown[pressed];
        IReadOnlyList<Entry> carrying = chosen.Count > 0 ? chosen : [dragged];

        // Places rather than things: nothing about the way up or the computer is a file to hand over.
        carrying = [.. carrying.Where(entry => !Browser.IsUp(entry.Path) && !Browser.IsComputer(entry.Path))];

        if (carrying.Count == 0)
        {
            return;
        }

        Drag.Away(Browser, from, carrying, dragged, Host.Log.Error);
    }

    /// <summary>Says whether a drag may land here, and lights the directory it would land in.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOver(object? sender, DragEventArgs e)
    {
        // The card is put first and from the position, since it is what says where the pointer is rather than
        // what would be done there.
        _ghost.Following(e.GetPosition(this));

        e.DragEffects = _drops.Over(e);
        e.Handled = e.DragEffects != DragDropEffects.None;
    }

    /// <summary>Takes the drop mark off when a drag leaves, and the card with it.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOff(object? sender, DragEventArgs e)
    {
        _drops.Off();
        _ghost.Unghost();
    }

    /// <summary>Lets a drag go where it was let go, and takes the card off.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drop.</param>
    private void Dropped(object? sender, DragEventArgs e)
    {
        _ghost.Unghost();
        _drops.Dropped(e);
    }

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

            Paint(row, Resource(PaletteKeys.Selection, new SolidColorBrush(Color.FromArgb(0x33, 0x80, 0x80, 0x80))));
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
    /// <summary>How wide the column of icons is, which is the design's own measure.</summary>
    private const double IconColumn = 22;

    /// <summary>How large a dot is: a mark read at a glance rather than a thing to point at.</summary>
    private const double DotSize = 8;

    /// <summary>
    /// How wide the grab between two columns is, and the marks it wears.
    /// </summary>
    /// <remarks>
    /// The marks are the theme's, written as the literals it publishes, because a plugin has
    /// nowhere else to read them from: the shell's own constants live in the host, which a plugin may not
    /// reference. The width is the width those rules draw a line through the middle of, so a band of another
    /// width would be a grab wearing a line that is not centred in it.
    /// </remarks>
    private const double GrabColumn = 4;

    /// <summary>
    /// How wide the band a column edge can be taken in is.
    /// </summary>
    /// <remarks>
    /// Wider than the grip drawn in it, and deliberately: four pixels is what the line is centred in, not
    /// what a pointer can be asked to find. The grip is still drawn at four, and the band around it is the
    /// room to press — the two are different questions and this is the one the hand answers.
    /// </remarks>
    private const double GrabZone = 12;

    /// <summary>What a grab between two of anything is marked with.</summary>
    private const string GrabMark = "dock-splitter";

    /// <summary>What a grab that resizes columns is marked with as well.</summary>
    private const string GrabColumnMark = "dock-splitter-columns";

    /// <summary>The pointer a column edge is taken with, made once rather than per move.</summary>
    private static readonly Cursor ResizeCursor = new(StandardCursorType.SizeWestEast);

    /// <summary>How narrow a column may be dragged.</summary>
    private const double MinColumn = 56;

    /// <summary>How wide a column of values is before anybody drags it.</summary>
    private const double PermissionsColumn = 110;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double ModifiedColumn = 150;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double SizeColumn = 88;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double OwnershipColumn = 96;

    /// <summary>What a cell says when its provider has nothing to say about the entry.</summary>
    private const string Placeholder = "-";

    /// <summary>
    /// Which column the name is in.
    /// </summary>
    /// <remarks>
    /// The name is the column that is as wide as what is left over, so it is the one that takes a move for
    /// free and the one that keeps the table as wide as the dock.
    /// </remarks>
    private const int NameColumn = 1;

    /// <summary>The columns after the name, which every row of the table has the same of.</summary>
    private readonly Column[] _values;

    /// <summary>The lock provider that speaks for this directory, or nothing when none does.</summary>
    private readonly IEntryLockProvider? _locks;

    /// <summary>How wide every column is, the grabs between them included.</summary>
    private readonly GridLength[] _widths;

    /// <summary>The row of column names, which every row is kept in step with.</summary>
    private readonly Grid _header = new();

    /// <summary>
    /// The band the column names sit in, which is also where a column edge may be taken.
    /// </summary>
    /// <remarks>
    /// The edge is the header's to resize because that is where a column is named; the rows below are the
    /// entries, and a press there is a choice.
    /// </remarks>
    private Border? _heading;

    /// <summary>Where a grab was taken, and how wide the two columns each side of it were then.</summary>
    private (int Left, int Right, double At, double LeftWas, double RightWas)? _grabbed;

    /// <summary>The contributed columns that stand before everything, the picture included.</summary>
    private readonly Column[] _leads;

    /// <summary>Makes the table over what the browser holds.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or given a menu.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public ListBrowser(IPluginHost host, Browser browser, BrowserActions actions, Clip clip)
        : base(host, browser, actions, clip)
    {
        // A contributed column is placed where its own contribution asks to be, and is only there when a
        // provider speaks for this directory: a column of placeholders would be a column of the listing
        // saying what it does not know. The Ownership column comes last of all, since a lock is the one
        // answer that is a state of the entry rather than a value of it.
        var contributed = EntryColumns.For(Browser.Current);
        var values = new List<Column>();

        _leads =
        [
            .. contributed
                .Where(provider => provider.Placement == ColumnPlacement.Leftmost)
                .Select(Contributed),
        ];

        values.AddRange(
            contributed
                .Where(provider => provider.Placement == ColumnPlacement.BesideName)
                .Select(Contributed)
        );
        values.AddRange(Values());
        values.AddRange(
            contributed
                .Where(provider => provider.Placement == ColumnPlacement.AfterValues)
                .Select(Contributed)
        );

        _locks = EntryLockProviders.For(Browser.Current);

        if (_locks is { } locks)
        {
            values.Add(Lock(locks));
        }

        _values = [.. values];
        _widths = Widths();

        // The list's own border and inset are taken off: what draws the table is the view, and a border and a
        // padding of the list's own would be a second edge and a second inset inside it.
        List.Padding = new Thickness(0);
        List.BorderThickness = new Thickness(0);

        // The item keeps the design's row height, and its own inset is taken off so that the header and the
        // rows share one edge. The four pixels left between two rows are the line's room and the frame's: the
        // rows are told apart by the line under each, and the room around it is the only place a press can
        // land that is not on an entry — which is where a frame is begun from.
        List.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MarginProperty, new Thickness(0, 2, 0, 2)),
                    new Setter(Layoutable.MinHeightProperty, 30.0),
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                },
            }
        );

        var header = Header();

        // The band a column edge is taken in is wider than the line drawn in it, so the pointer is the only
        // sign that the edge is within reach; leaving the view takes that sign away.
        PointerExited += (_, _) => Cursor = null;

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(header, Dock.Top);
        panel.Children.Add(header);
        panel.Children.Add(List);

        // The table is the dock's own surface rather than a card on it: it used to be a bordered, rounded,
        // raised surface with the room around it left by the view, and the entries were read in less of the
        // dock than there was because of it.
        Present(panel);
    }

    /// <summary>One column of values, after the icon and the name.</summary>
    /// <param name="Header">The translation key the column is headed by.</param>
    /// <param name="Width">How wide the column starts.</param>
    /// <param name="Value">What the column says about one entry.</param>
    /// <param name="Align">Where the text sits in the column.</param>
    /// <param name="Draw">
    /// How the column draws one entry, where a value of its own is not enough — a contributed column
    /// draws its own ink. Answered instead of <see cref="Value"/> when it is there.
    /// </param>
    private sealed record Column(
        string Header,
        double Width,
        Func<Facts, string> Value,
        TextAlignment Align = TextAlignment.Left,
        Func<Entry, Control>? Draw = null
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

    /// <summary>
    /// The Ownership column one lock provider contributes.
    /// </summary>
    /// <remarks>
    /// The value in it is the provider's to write and is not read from the entry's metadata, which is
    /// why the column draws its own cell: what a lock looks like — its words, and the ink they are
    /// written in — is what the provider answered, and none of it is in <see cref="Facts"/>.
    /// <para>
    /// What it says stands on the right, which is where a column of short answers is read from: the holder's
    /// name is the last thing on the row, and the edge it is read against is the one the listing ends in.
    /// </para>
    /// </remarks>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <returns>The column to add after the table's own.</returns>
    private static Column Lock(IEntryLockProvider provider) =>
        new(
            provider.ColumnKey,
            OwnershipColumn,
            _ => Placeholder,
            TextAlignment.Right,
            Draw: entry => Locked(provider, entry)
        );

    /// <summary>What one entry's lock is written as, in the words and ink the provider asked for.</summary>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <param name="entry">The entry to write about.</param>
    /// <returns>The cell.</returns>
    private static TextBlock Locked(IEntryLockProvider provider, Entry entry)
    {
        var mark = provider.Mark(entry);
        var cell = Cell(mark.Said() ?? Placeholder, TextAlignment.Right);
        cell[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(Icons.Ink(mark.TextInk));

        return cell;
    }

    /// <summary>
    /// The column one column provider contributes.
    /// </summary>
    /// <remarks>
    /// The value in it is the provider's to write and is not read from the entry's metadata, which is why
    /// the column draws its own cell: what it says goes through <see cref="EntryColumnMark.Said"/>, which
    /// is a key translated or a literal as the contribution asked, and neither is in <see cref="Facts"/>.
    /// </remarks>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <returns>The column to add where the contribution asked.</returns>
    private static Column Contributed(IEntryColumnProvider provider) =>
        new(
            provider.ColumnKey,
            provider.Width,
            _ => Placeholder,
            Draw: entry => Contributed(provider, entry)
        );

    /// <summary>What one entry's cell in a contributed column is drawn as.</summary>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <param name="entry">The entry to write about.</param>
    /// <returns>The cell, which draws nothing at all where a column of marks has no mark for the entry.</returns>
    internal static Control Contributed(IEntryColumnProvider provider, Entry entry)
    {
        var mark = provider.Value(entry);

        if (mark.Dot)
        {
            return Dot(mark);
        }

        // A column of marks says nothing by having no mark: the dash the listing says what it does not know
        // with would be a mark of its own, and a cell with no mark has nothing else to say.
        return provider.Marks ? new Panel() : Written(mark);
    }

    /// <summary>What one entry's cell in a contributed column that is words is written as.</summary>
    /// <remarks>
    /// The ink is the contribution's to name and this plugin's to resolve: a column drawn in a colour the
    /// contribution picked for itself would be a colour the look does not hold, which is a column that
    /// vanishes somewhere the theme is another — unless the contribution has a palette of its own, which is
    /// what a colour of its own says, and then it is drawn in that. The Ownership column writes its words the
    /// same way.
    /// </remarks>
    /// <param name="mark">What the contribution answered.</param>
    /// <returns>The cell.</returns>
    private static TextBlock Written(EntryColumnMark mark)
    {
        var cell = Cell(mark.Said() ?? Placeholder);

        if (mark.Colour is { } colour)
        {
            cell.Foreground = new SolidColorBrush(colour);
        }
        else
        {
            cell[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(Icons.Ink(mark.Ink));
        }

        return cell;
    }

    /// <summary>
    /// What one entry's cell in a contributed column that is a mark is drawn as: a round dot in the ink.
    /// </summary>
    /// <remarks>
    /// A dot rather than a word, because what it stands for is one of a handful of states and the colour is
    /// read at a glance: a column of the same few words repeated is a column of text where a mark would do,
    /// and the words are still there — said when the pointer rests on the mark, which is where a reader who
    /// wants them is already looking.
    /// </remarks>
    /// <param name="mark">What the contribution answered.</param>
    /// <returns>The cell.</returns>
    private static Control Dot(EntryColumnMark mark)
    {
        var dot = new Border
        {
            Width = DotSize,
            Height = DotSize,
            CornerRadius = new CornerRadius(DotSize / 2),
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The colour is the contribution's where it named one, and the role it named otherwise: a mark drawn in
        // the role and then painted over would be drawn twice, and the role's own brush would win.
        if (mark.Colour is { } colour)
        {
            dot.Background = new SolidColorBrush(colour);
        }
        else
        {
            dot[!Border.BackgroundProperty] = new DynamicResourceExtension(Icons.Ink(mark.Ink));
        }

        if (mark.Edge is { } edge)
        {
            // The line is the contribution's own rather than the design's, since whatever named the colour named
            // what reads around it: a hairline of the listing's would be a mark drawn differently from the one
            // the contribution asked for.
            dot.BorderThickness = new Thickness(1);
            dot.BorderBrush = new SolidColorBrush(edge);
        }

        if (mark.Hint is { Length: > 0 } hint)
        {
            ToolTip.SetTip(dot, hint);

            // At once rather than after the wait a tooltip usually takes: the mark is a colour, and a reader
            // who has to stop and wait for what a colour means is a reader who has to stop. Nothing is said by
            // the wait that is not said by the mark.
            ToolTip.SetShowDelay(dot, 0);
        }

        return dot;
    }

    /// <summary>How wide every column starts, in the order they are laid out.</summary>
    private GridLength[] Widths()
    {
        var widths = new List<GridLength>();

        foreach (var column in _leads)
        {
            widths.Add(new GridLength(GrabColumn));
            widths.Add(new GridLength(column.Width));
        }

        widths.Add(new GridLength(IconColumn));
        widths.Add(new GridLength(1, GridUnitType.Star));

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
    /// The headings sit on the sunken ground against the top of the card, and the line under them is what the
    /// rows are read against. Every cell is inset horizontally by the same twelve the rows are, so a column's
    /// heading stands over its data.
    /// </remarks>
    private Control Header()
    {
        _header.ColumnDefinitions = Definitions();

        // The icon column is its own heading, with no word over it: the entries say what they are. A
        // contributed column that names no key is headed the same way, which is what a column of marks is —
        // what it says is said on the mark itself, and there is no word that would fit over one.
        var cells = new List<Control?>();

        foreach (var column in _leads)
        {
            cells.Add(Grip());
            cells.Add(Head(column.Header));
        }

        cells.Add(Head(string.Empty));
        cells.Add(Head("rorolala_file_system.column_name"));

        foreach (var column in _values)
        {
            cells.Add(Grip());
            cells.Add(Head(column.Header, column.Align));
        }

        Place(_header, cells);

        var band = new Border
        {
            Child = _header,
            Padding = new Thickness(12, 7),
            MinHeight = 30,
            BorderThickness = new Thickness(0, 0, 0, 1),
        };
        band[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        band[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        _heading = band;

        return band;
    }

    /// <summary>One entry as a row: its marks, its picture, its name, and what each value says.</summary>
    /// <param name="entry">The entry to draw.</param>
    private Control Row(Entry entry)
    {
        var facts = Stats.Of(entry);
        var name = Names.Show(entry);
        var icon = Icons.For(entry);
        icon.HorizontalAlignment = HorizontalAlignment.Left;

        var cells = new List<Control?>();

        // The cell over the grab is nothing: a row has no grab, only the header does.
        foreach (var column in _leads)
        {
            cells.Add(null);
            cells.Add(
                column.Draw is { } draw ? draw(entry) : Cell(column.Value(facts), column.Align)
            );
        }

        cells.Add(icon);
        cells.Add(Named(entry, name.Length > 0 ? name : entry.Path));

        foreach (var column in _values)
        {
            cells.Add(null);
            cells.Add(
                column.Draw is { } draw ? draw(entry) : Cell(column.Value(facts), column.Align)
            );
        }

        var grid = new Grid { ColumnDefinitions = Definitions() };
        Place(grid, cells);

        // Every row but the last wears the line that tells it from the next, so that the table ends in the
        // listing's own edge rather than in a line drawn against nothing.
        var last = Browser.Shown.Count > 0 &&
            string.Equals(entry.Path, Browser.Shown[Browser.Shown.Count - 1].Path, StringComparison.Ordinal);

        var row = new Border
        {
            Child = grid,
            Padding = new Thickness(12, 0),
            BorderThickness = last ? new Thickness(0) : new Thickness(0, 0, 0, 1),
        };
        row[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        return row;
    }

    /// <summary>
    /// The grip drawn between two columns of the table.
    /// </summary>
    /// <remarks>
    /// A <see cref="GridSplitter"/>, because that is the control the look is written for: the shell marks
    /// the grab between two regions with these classes and the look draws one, and a grab between two
    /// columns is that same grab — its line, and the pointer it takes when the pointer is on it.
    /// <para>
    /// It is put in a panel rather than in the header's own grid, and that is load-bearing rather than tidy.
    /// A splitter moves the columns of the grid it is in, and the table's rows are one grid each, so a
    /// splitter in the header would move the header's columns alone; put in a panel it moves nothing, which
    /// is what this leans on. The columns are moved by the view instead, which takes the press before the
    /// splitter can (see <see cref="StartResize"/>).
    /// </para>
    /// <para>
    /// The grip is four pixels, the width the look centres its line in, and that is not the width a press
    /// is taken in: a column edge is taken within the wider band <see cref="GrabZone"/> states.
    /// </para>
    /// </remarks>
    private static Control Grip() =>
        new Panel
        {
            Children =
            {
                new GridSplitter
                {
                    ResizeDirection = GridResizeDirection.Columns,
                    Width = GrabColumn,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    Classes = { GrabMark, GrabColumnMark },
                },
            },
        };

    /// <summary>
    /// Whether a press is on a column edge, and if so, takes it.
    /// </summary>
    /// <remarks>
    /// The band is the header's, because the header is where a column is named and the rows below it are the
    /// entries. It is measured from the columns rather than from the grip, so that the room to press is the
    /// band and not the four pixels the line is drawn in.
    /// </remarks>
    /// <param name="e">The press.</param>
    /// <returns>Whether the press began a resize.</returns>
    protected override bool StartResize(PointerPressedEventArgs e)
    {
        if (Edge(e.GetPosition(this)) is not var (left, right))
        {
            return false;
        }

        _grabbed = (left, right, e.GetPosition(this).X, Pixels(left), Pixels(right));

        return true;
    }

    /// <summary>Follows a resize: the two columns the edge is between take the move between them.</summary>
    /// <param name="e">The move.</param>
    protected override void FollowResize(PointerEventArgs e) => Resize(e);

    /// <summary>Ends a resize, leaving the columns as they were dragged.</summary>
    protected override void EndResize() => _grabbed = null;

    /// <summary>
    /// Says with the pointer that a column edge is within reach.
    /// </summary>
    /// <remarks>
    /// The band a press may take an edge in is wider than the line drawn in it, so without this the room to
    /// press would be room the user cannot see.
    /// </remarks>
    /// <param name="at">Where the pointer is, in the view's own coordinates.</param>
    protected override void Hovered(Point at) => Cursor = Edge(at) is null ? null : ResizeCursor;

    /// <summary>
    /// The column edge a point in the view falls on, as the pair of columns it is between.
    /// </summary>
    /// <remarks>
    /// An edge is the centre of a grab column, which the layout knows and a fixed measurement would not: the
    /// name column takes what is left, so where every other edge stands depends on the dock.
    /// </remarks>
    /// <param name="at">Where the pointer is, in the view's own coordinates.</param>
    /// <returns>The pair the edge is between, or nothing when no edge is within reach.</returns>
    private (int Left, int Right)? Edge(Point at)
    {
        if (
            _heading is null
            || this.TranslatePoint(at, _heading) is not { } band
            || band.Y < 0
            || band.Y > _heading.Bounds.Height
            || this.TranslatePoint(at, _header) is not { } header
        )
        {
            return null;
        }

        var left = 0.0;

        for (var column = 0; column < _header.ColumnDefinitions.Count; column++)
        {
            var width = _header.ColumnDefinitions[column].ActualWidth;

            // A grab column is every other one after the name, which is the table's own shape rather than a
            // measurement of it.
            if (
                column > NameColumn
                && (column - NameColumn) % 2 == 1
                && Math.Abs(header.X - (left + (width / 2))) <= GrabZone / 2
            )
            {
                return (column - 1, column + 1);
            }

            left += width;
        }

        return null;
    }

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
    /// <param name="e">The move.</param>
    private void Resize(PointerEventArgs e)
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
            // A row is the card's cell border with the table's grid inside it, so the grid is what the widths
            // are written to.
            if (row is Border { Child: Grid grid })
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

    /// <summary>One heading's text: the design's label, set by the caller in capitals.</summary>
    /// <remarks>
    /// A heading named by no key says nothing, which is what the icon column and a column of marks are
    /// headed by: what they are is read from the cells under them rather than from a word that would not
    /// fit over one.
    /// </remarks>
    /// <param name="key">The translation key the heading is read from, or nothing for no heading.</param>
    /// <param name="align">Where the text sits over its column.</param>
    private static TextBlock Head(string key, TextAlignment align = TextAlignment.Left) =>
        new()
        {
            Text = (key is { Length: > 0 } said ? RolaI18N.Get(said) : string.Empty).ToUpperInvariant(),
            TextAlignment = align,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            Classes = { "label" },
        };

    /// <summary>One entry's name: a directory reads heavier, since choosing it opens where a file does not.</summary>
    /// <remarks>
    /// The way up is left light: it is not a directory of the listing but a step out of it, and the design
    /// draws it as the one row that is a place rather than a thing.
    /// </remarks>
    /// <param name="entry">The entry the name belongs to.</param>
    /// <param name="text">The name as it is written.</param>
    private static TextBlock Named(Entry entry, string text) =>
        new()
        {
            Text = text,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            // Marked with the class rather than kept in a field, because the row is made once and reused by
            // the toolkit's virtualisation: what a rename edits is the name cell of the row as it stands.
            Classes = { "entry-name" },
            FontWeight = entry.Kind == EntryKind.Directory && !Browser.IsUp(entry.Path)
                ? FontWeight.SemiBold
                : FontWeight.Normal,
        };

    /// <summary>One cell's value: one line, cut off rather than wrapped, in the faint metadata ink.</summary>
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
            Classes = { "caption", "faint" },
        };
}

/// <summary>The entries as tiles, wrapping across the width.</summary>
internal sealed class GridBrowser : EntryView
{
    /// <summary>How much room is left around a tile's icon and name.</summary>
    private const int Around = 12;

    /// <summary>
    /// How much room is left around each tile, and so between one and the next.
    /// </summary>
    /// <remarks>
    /// The room is not only looks: a frame is begun where no entry is, so a grid whose tiles met
    /// edge to edge would be a grid a frame could never start in — every point would be on a tile. It is left
    /// on the **item** rather than on the tile drawn inside it, because it is the item's own area that answers
    /// the pointer: room left on what the item holds is room the item still covers, and a frame begun there
    /// would be a frame begun on the entry.
    /// </remarks>
    private const int Gap = 12;

    /// <summary>How many pixels wide and tall a tile's icon is, and so how wide its name is too.</summary>
    private readonly int _icon;

    /// <summary>What a tile's icon wears at its top right corner, in the order the badges are numbered.</summary>
    private readonly IReadOnlyList<IEntryMarkProvider> _badges;

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
        _badges = EntryMarkProviders.For(Browser.Current);

        // The tiles of a wrapped line fill the width it is given, so that the grid's outer inset is the room
        // around it and stays put however wide the dock is — the room a line has to spare is spent between
        // its tiles rather than at its ends, and the last line keeps that same room so a short one stands in
        // the columns above. A grid that never wrapped reads from its start (see JustifiedPanel).
        List.ItemsPanel = new FuncTemplate<Panel?>(() => new JustifiedPanel { Gap = Gap });

        // A wrapping grid needs a width to wrap against, and the base theme gives a list a horizontal
        // scrollbar instead: with one, the panel is measured at an unbounded width, lays every tile on one
        // line, and the dock scrolls sideways rather than putting the next tile on the next row. So the
        // horizontal scroll is taken off here, which is what makes the panel wrap.
        ScrollViewer.SetHorizontalScrollBarVisibility(List, ScrollBarVisibility.Disabled);

        // The panel's padding is the room around the grid, and the surface a tile wears is the item's: the list's
        // own border and inset would draw a second edge and add room of their own.
        List.Padding = new Thickness(0);
        List.BorderThickness = new Thickness(0);

        // A tile at rest is its icon and its name on the dock's own ground: the surface a tile wears is what the
        // choice draws, so it arrives with the selection and leaves with it. What is left on the
        // item is the room above and below a tile — the room across is the panel's, which is what makes the
        // outermost room the grid's own padding and nothing else (see JustifiedPanel) — and the corner the
        // surface is rounded by, which is kept for every state so that a choice does not change the shape of
        // what it is drawn on.
        List.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MarginProperty, new Thickness(0, Gap / 2.0, 0, Gap / 2.0)),
                    new Setter(Layoutable.MinHeightProperty, 0.0),
                    new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(1)),
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(8)),
                },
            }
        );

        // The card a chosen tile is drawn as: the fill is the theme's own wash over a chosen row — the same
        // colour a chosen row of the table is filled with — and the edge is the one this view draws around it.
        // The fill is the toolkit's and cannot be set from here: what a chosen item is filled with is written
        // into the theme's own template for it, which reaches the surface the tile is drawn on rather than the
        // item, so setting a background here would be a colour nothing is drawn with.
        List.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>().Class(":selected"))
            {
                Setters =
                {
                    new Setter(
                        TemplatedControl.BorderBrushProperty,
                        new DynamicResourceExtension(PaletteKeys.Splitter)
                    ),
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
    private Control Tile(Entry entry)
    {
        // The surface a chosen tile wears is the item's; what is here is the content and the room around it.
        return new Border
        {
            Padding = new Thickness(Around),
            Child = new StackPanel
            {
                Spacing = 8,
                Children = { Marked(entry), Caption(entry) },
            },
        };
    }

    /// <summary>
    /// A tile's name, on a line of its own under the picture.
    /// </summary>
    /// <remarks>
    /// What a contribution says about an entry stands on the icon here rather than beside the name, which is
    /// where the list has it: a tile is a picture with a caption, and a mark in the caption would be a mark
    /// taking room off the name at every zoom — the one thing a tile has none of. What is left is the name,
    /// as wide as the icon and not one pixel wider, so that the two read as one column rather than as a
    /// picture with a caption under it that happens to start somewhere else.
    /// </remarks>
    /// <param name="entry">The entry to draw.</param>
    /// <returns>What to draw where the name goes.</returns>
    private Control Caption(Entry entry)
    {
        var name = new TextBlock
        {
            MaxWidth = Math.Max(1, _icon),
            Text = Names.Show(entry),
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Classes = { "entry-name", "caption" },
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        return name;
    }

    /// <summary>An entry's icon, with what every contributor marked worn around its top right corner.</summary>
    /// <remarks>
    /// Every provider that speaks for the directory is asked, rather than one of them: two badges say two
    /// different things — who holds the entry and how far its version has come — and neither is the other's to
    /// replace. A picture is looked up rather than carried, and a key that names nothing draws nothing: a
    /// provider that contributed a mark but no picture is a listing without a badge on it, not a listing that
    /// fails to be drawn.
    /// <para>
    /// The order the providers were asked in is the order the badges are numbered, which is what makes a badge
    /// stand in the same place of the arc from one listing to the next: what is read at the top of the arc is
    /// the contribution that was registered first and is the same contribution every time.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry to draw.</param>
    /// <returns>What to draw where the icon goes.</returns>
    private Control Marked(Entry entry)
    {
        var icon = Icons.For(entry, _icon);
        var badges = new List<EntryBadge>();

        foreach (var provider in _badges)
        {
            if (
                provider.Mark(entry) is { IconKey: { Length: > 0 } key } mark
                && Host.Icons.Find(key) is { } picture
            )
            {
                badges.Add(new EntryBadge(picture, mark));
            }
        }

        return Icons.Marked(icon, badges, _icon);
    }
}

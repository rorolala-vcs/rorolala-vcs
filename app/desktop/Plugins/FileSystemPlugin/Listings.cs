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

/// <summary>One column of values a table shows after the icon and the name.</summary>
/// <remarks>
/// Generic in the entry because the table is: a directory's columns say how large a file is and who holds it, and
/// a plugin's place has no such columns — but the table that draws them, and the grabs that resize them, are one
/// table either way.
/// </remarks>
/// <typeparam name="TItem">What one entry of the table is.</typeparam>
/// <param name="Header">The translation key the column is headed by.</param>
/// <param name="Width">How wide the column starts.</param>
/// <param name="Value">What the column says about one entry.</param>
/// <param name="Align">Where the text sits in the column.</param>
/// <param name="Draw">
/// How the column draws one entry, where a value of its own is not enough — a contributed column draws its own
/// ink. Answered instead of <see cref="Value"/> when it is there.
/// </param>
/// <param name="Placement">Where the column sits among the listing's own.</param>
internal sealed record Column<TItem>(
    string Header,
    double Width,
    Func<TItem, string> Value,
    TextAlignment Align = TextAlignment.Left,
    Func<TItem, Control>? Draw = null,
    ColumnPlacement Placement = ColumnPlacement.AfterValues
);

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
/// What a listing is made of: the items, and everything a view has to know about one of them.
/// </summary>
/// <remarks>
/// A view is one arrangement of a listing — rows, or tiles — and a listing is one family of things arranged. The
/// two vary apart, which is why they are two: a directory and a plugin's place are both read as rows and as tiles,
/// and neither arrangement has anything to say about what an entry <em>is</em>. So a view is made of an
/// arrangement and a listing (<see cref="EntryView{TItem}"/> takes one), and what a place adds over a directory —
/// a provider's picture, a provider's menu — is said here once rather than in a second pair of views.
/// </remarks>
/// <typeparam name="TItem">What one entry of a listing is.</typeparam>
internal abstract class Listing<TItem>
    where TItem : class
{
    /// <summary>Makes a listing of what a view is to draw.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="items">What is in it, in the order it is shown.</param>
    protected Listing(IPluginHost host, IReadOnlyList<TItem> items)
    {
        Host = host;
        Items = items;
    }

    /// <summary>The host, for what cannot be done.</summary>
    public IPluginHost Host { get; }

    /// <summary>What is in the listing, in the order it is shown.</summary>
    public IReadOnlyList<TItem> Items { get; }

    /// <summary>
    /// Lights the row a drag would land on, which the view the listing is drawn in supplies.
    /// </summary>
    /// <remarks>
    /// Set by the view rather than handed in, because a row is the view's to draw: what a listing decides is which
    /// item a drag is over, and what saying so looks like belongs to the arrangement.
    /// </remarks>
    public Action<object?>? Marking { get; set; }

    /// <summary>What tells one item from another, which is what a choice is carried across a read by.</summary>
    /// <param name="item">The item.</param>
    /// <returns>What says which one it is.</returns>
    public abstract object Key(TItem item);

    /// <summary>What one item is called.</summary>
    /// <param name="item">The item.</param>
    /// <returns>The name to show.</returns>
    public abstract string Name(TItem item);

    /// <summary>The picture one item wears.</summary>
    /// <param name="item">The item.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw beside the name.</returns>
    public abstract Control Icon(TItem item, int size);

    /// <summary>The picture one item wears in a tile, which may carry more than a row's does.</summary>
    /// <param name="item">The item.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw in the tile.</returns>
    public virtual Control Marked(TItem item, int size) => Icon(item, size);

    /// <summary>Opens one item, which is what a double-click and <c>Enter</c> do.</summary>
    /// <param name="item">The item to open.</param>
    public abstract void Open(TItem item);

    /// <summary>Fills the menu opened on an item.</summary>
    /// <param name="menu">The menu to fill.</param>
    /// <param name="from">The control the menu is opened from.</param>
    /// <param name="chosen">What is chosen, the item the menu was opened on first.</param>
    /// <param name="force">Whether the forced actions are shown to begin with.</param>
    public abstract void Fill(ContextMenu menu, Control from, IReadOnlyList<TItem> chosen, bool force);

    /// <summary>How the items beyond the name are read, in the order they are shown.</summary>
    public virtual IReadOnlyList<Column<TItem>> Columns => [];

    /// <summary>Says that the choice moved, for a listing that has somewhere to publish it.</summary>
    /// <param name="chosen">What is chosen now.</param>
    public virtual void Chosen(IReadOnlyList<TItem> chosen) { }

    /// <summary>Says that a press landed on an item, for a listing that has somewhere to publish it.</summary>
    /// <param name="item">The item the press landed on.</param>
    public virtual void Clicked(TItem item) { }

    /// <summary>Whether an item leads somewhere, which is what a name reads heavier for.</summary>
    /// <param name="item">The item.</param>
    /// <returns>Whether it opens.</returns>
    public virtual bool Openable(TItem item) => false;

    /// <summary>Fades a row, which is what a listing knows and its arrangement does not.</summary>
    /// <param name="row">The row to fade.</param>
    /// <param name="item">What it stands for.</param>
    public virtual void Fade(Control row, TItem item) { }

    /// <summary>Whether a run of typed letters may land on an item.</summary>
    /// <param name="item">The item.</param>
    /// <returns>Whether it is a name rather than a place.</returns>
    public virtual bool Seekable(TItem item) => true;

    /// <summary>Whether the listing holds nothing to read.</summary>
    public virtual bool Barren => Items.Count == 0;

    /// <summary>What a listing with nothing in it says, or nothing for one that says nothing.</summary>
    /// <returns>The word, or nothing.</returns>
    public virtual Control? Notice() => null;

    /// <summary>The menu on the space around the items, or nothing.</summary>
    /// <param name="from">The control the menu is opened from.</param>
    /// <returns>The menu, or nothing.</returns>
    public virtual ContextMenu? Space(Control from) => null;

    /// <summary>Copies what is chosen, where the listing has a clipboard to put it on.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="chosen">What is chosen.</param>
    public virtual void Copy(Control from, IReadOnlyList<TItem> chosen) { }

    /// <summary>Cuts what is chosen, which a paste then moves.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="chosen">What is chosen.</param>
    public virtual void Cut(Control from, IReadOnlyList<TItem> chosen) { }

    /// <summary>Pastes what is on the clipboard into the listing.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    public virtual void Paste(Control from) { }

    /// <summary>Removes what is chosen.</summary>
    /// <param name="chosen">What is chosen.</param>
    /// <param name="ask">Whether to put the question to the user first.</param>
    public virtual void Delete(IReadOnlyList<TItem> chosen, bool ask) { }

    /// <summary>Makes a directory in the listing, which is then named where it is drawn.</summary>
    public virtual void NewFolder() { }

    /// <summary>Takes what a name was edited to.</summary>
    /// <param name="item">What is being renamed.</param>
    /// <param name="name">What the box says.</param>
    public virtual void Rename(TItem item, string name) { }

    /// <summary>How much of a name a rename chooses to begin with.</summary>
    /// <param name="item">What is being renamed.</param>
    /// <param name="name">The name it has.</param>
    /// <returns>Where the chosen part of the name ends.</returns>
    public virtual int Select(TItem item, string name) => name.Length;

    /// <summary>What is waiting to be named, if the listing made something while a view was being made.</summary>
    /// <returns>The item to name, or nothing.</returns>
    public virtual TItem? Pending() => null;

    /// <summary>Reads the listing again, which is what an edit that changed the files leaves behind.</summary>
    public virtual void Touch() { }

    /// <summary>Whether an item may be picked up and dragged away.</summary>
    /// <param name="item">The item.</param>
    /// <returns>Whether a drag may begin on it.</returns>
    public virtual bool CanDrag(TItem item) => false;

    /// <summary>Hands items over as a drag.</summary>
    /// <param name="from">The press the drag begins at.</param>
    /// <param name="carrying">What to carry, in the order the view shows it.</param>
    /// <param name="lead">The item the drag took hold of.</param>
    public virtual void DragOut(PointerPressedEventArgs from, IReadOnlyList<TItem> carrying, TItem lead) { }

    /// <summary>Whether a drag may land here, or nothing where it may not land at all.</summary>
    /// <param name="item">The item the pointer is over, or nothing where it is over the space around them.</param>
    /// <param name="e">The drag.</param>
    /// <returns>Where it would land.</returns>
    public virtual Land? Landing(TItem? item, DragEventArgs e) => null;

    /// <summary>What letting go here would do.</summary>
    /// <param name="land">Where it would land.</param>
    /// <param name="e">The drag.</param>
    /// <returns>What the platform is told.</returns>
    public virtual DragDropEffects Over(Land land, DragEventArgs e) => DragDropEffects.None;

    /// <summary>Lets a drag go where it was let go.</summary>
    /// <param name="land">Where it was let go.</param>
    /// <param name="e">The drop.</param>
    public virtual void DropAt(Land land, DragEventArgs e) { }

    /// <summary>Takes a drag off the listing: nothing is lit any more.</summary>
    public virtual void DragLeft() => Marking?.Invoke(null);
}

/// <summary>
/// What every listing view is, whichever arrangement draws it and whatever it lists.
/// </summary>
/// <remarks>
/// The root exists for what a dock holds without caring: a dock swaps one view for another, and what it does to
/// either of them — put the keyboard in it, draw it again — does not depend on what is in it.
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

    /// <summary>How many entries this view is showing, which is what a dock reports it drew.</summary>
    public abstract int Shown { get; }

    /// <summary>Draws every row again with what the listing is worth now.</summary>
    public abstract void Repaint();

    /// <summary>
    /// Puts the keyboard in the listing, so that the dock's own keys reach it again.
    /// </summary>
    /// <remarks>
    /// The listing is where a dock answers its keys from: they are taken at the top of the dock and passed to the
    /// view being read, so a keyboard that is nowhere in the dock is a dock whose keys do nothing. That is what
    /// happens when the address — which hides itself when an edit ends — was where the keyboard was
    /// (Section 7.7).
    /// </remarks>
    public abstract void Listen();
}

/// <summary>
/// What both ways of reading a listing have in common: the choice, the keyboard, the menus, and the drags.
/// </summary>
/// <remarks>
/// One listing is read two ways — as a table of rows and as tiles — and the two are one listing seen twice rather
/// than two listings. Everything a user does to an entry, short of how it is laid out, is therefore written once
/// here: what a click means, what the arrows and the typed letters mean, and what the menu on a choice offers. An
/// arrangement supplies the layout and a face for one entry, and inherits the rest.
/// <para>
/// What an entry <em>is</em> is not here either: whether it is a file, what opening it does, whether it may be
/// dragged — all of that is the <see cref="Listing{TItem}"/> the view was made over. That is what lets a plugin's
/// place be read by these very views rather than by a second set that would drift from them.
/// </para>
/// <para>
/// The choice is the toolkit's own list selection, because it is exactly the one asked for — a click
/// chooses, <c>Ctrl</c> adds or removes, <c>Shift</c> takes a range — and because the look already fills
/// a chosen row with the accent (§10). What is added here is only what the toolkit does not have: the
/// arrows within a wrapped grid, the typed letters, and a menu that knows about a set.
/// </para>
/// </remarks>
/// <typeparam name="TItem">What one entry of the listing is.</typeparam>
internal abstract class EntryView<TItem> : EntryView
    where TItem : class
{
    /// <summary>How long a run of typed letters stays one run.</summary>
    private static readonly TimeSpan Typing = TimeSpan.FromSeconds(1);

    /// <summary>How tall a row is where nothing on screen says, which is only ever a first guess.</summary>
    private const double RowGuess = 28.0;

    /// <summary>Every row on screen, so that a faded cut and a moved column reach all of them.</summary>
    private readonly List<(TItem Item, Control Row)> _rows = [];

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

    /// <summary>Where a press landed and on which entry, while it may still become a drag.</summary>
    private Point _slip;
    private int _slippedAt = -1;

    /// <summary>
    /// The choice as it stood when the pointer went down, before the toolkit collapsed it to one.
    /// </summary>
    /// <remarks>
    /// As the items themselves and not as places, because this is what a drag carries: what is handed over is what
    /// was chosen when the hand went down, and not whatever a rebuilt listing would number those places as.
    /// </remarks>
    private IReadOnlyList<TItem> _atPress = [];

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
    /// <param name="source">What is listed, which is what every entry of it is answered by.</param>
    protected EntryView(IPluginHost host, Listing<TItem> source)
    {
        Host = host;
        Source = source;
        Source.Marking = Mark;

        _ghost = new Ghost(_over);

        List = new ListBox
        {
            ItemsSource = source.Items,
            SelectionMode = SelectionMode.Multiple,
            Background = Brushes.Transparent,
            ItemTemplate = new FuncDataTemplate<TItem>((item, _) => Item(item), true),

            // Focusable on the list and not only on its rows. The toolkit focuses a row when one is pressed, and a press
            // in the space around them has no row to focus — so without this the keyboard is left nowhere, and the dock
            // answers its keys at the top of itself and passes them down, which means a keyboard that is nowhere is a
            // dock whose Ctrl+C, Ctrl+V, F2 and F5 are never heard. A listing whose rows are all focused and whose space
            // is not is a listing where the clipboard works until the user clicks past the last row.
            Focusable = true,
        };

        // What the choice holds leaves this listing as it changes, since what the rest of the program
        // does with a choice — the Version Graph's track — is about the files chosen rather than about
        // the one clicked. Every way a choice moves goes through the toolkit's selection, so this one
        // answer covers a click, a frame, the keyboard, and a listing rebuilt with the choice it had.
        List.Selection.SelectionChanged += (_, _) => source.Chosen(Chosen());

        // The menu on the space around the entries. An entry carries one of its own, and the toolkit
        // takes the nearest, so this one is the empty space by construction.
        List.ContextMenu = source.Space(this);

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

        // A name waiting to be typed, if one was made while this view was being made — which is what a new
        // folder is: its row is this view's, and this view is not laid out yet, so the box goes in a moment
        // later rather than at a row that does not exist. The listing is what knows whether the name is for what
        // it reads, since a view is handed a listing and not a directory (see `Listing.Pending`), and a view that
        // is already up hears about one through the actions' own `Naming` (see `TreeBrowser`).
        if (Source.Pending() is { } naming)
        {
            Dispatcher.UIThread.Post(() => Rename(naming), DispatcherPriority.Background);
        }
    }

    /// <summary>The host, for what cannot be done.</summary>
    protected IPluginHost Host { get; }

    /// <summary>What is listed, and what every entry of it is answered by.</summary>
    protected Listing<TItem> Source { get; }

    /// <summary>What is in the listing, in the order it is shown.</summary>
    protected IReadOnlyList<TItem> Items => Source.Items;

    /// <summary>How many entries this view is showing, which is what a dock reports it drew.</summary>
    public override int Shown => Items.Count;

    /// <summary>The list every entry is a row or a tile of.</summary>
    protected ListBox List { get; }

    /// <summary>Every row on screen, so that a column move can reach the ones drawn as a table.</summary>
    protected IReadOnlyList<(TItem Item, Control Row)> Rows => _rows;

    /// <summary>The entry the pointer or the keyboard last landed on, or nothing before either has.</summary>
    protected TItem? Lead => _lead >= 0 && _lead < Items.Count ? Items[_lead] : null;

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
        if (Source.Barren && Source.Notice() is { } notice)
        {
            grid.Children.Add(notice);
        }

        grid.Children.Add(_over);

        Content = grid;
    }

    /// <summary>
    /// One entry as a row or a tile, which is the one thing an arrangement supplies.
    /// </summary>
    /// <remarks>
    /// Called by the toolkit when it realises an item, and by no view directly, so an arrangement may build it out
    /// of anything the selection and the theme do not reach.
    /// </remarks>
    /// <param name="item">The entry to draw.</param>
    /// <returns>What stands for the entry.</returns>
    protected abstract Control Item(TItem item);

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
    /// <param name="item">What the row stands for.</param>
    /// <param name="row">The row to finish.</param>
    /// <returns>The row.</returns>
    protected Control Prepared(TItem item, Control row)
    {
        var menu = new ContextMenu();
        menu.Opening += (_, _) => Source.Fill(menu, this, Chosen(item), force: Held);
        row.ContextMenu = menu;

        // What the pointer held is read when it is pressed, because the menu that opens next is laid out before
        // anything can be asked of it, and Shift+right-click means the forced actions are wanted from the start.
        row.PointerPressed += (_, pressed) => Held = (pressed.KeyModifiers & KeyModifiers.Shift) != 0;

        // And while the menu is up, the key itself decides: Shift unfolds what it forces, and letting it go
        // folds it away again. Tunnelled, because the item under the keyboard reads the key first.
        menu.AddHandler(KeyDownEvent, (_, key) => Revealed(menu, key), RoutingStrategies.Tunnel);
        menu.AddHandler(KeyUpEvent, (_, key) => Revealed(menu, key), RoutingStrategies.Tunnel);

        Source.Fade(row, item);

        _rows.Add((item, row));
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
    /// <param name="item">The entry the menu was opened on.</param>
    protected IReadOnlyList<TItem> Chosen(TItem item)
    {
        var chosen = Chosen();

        if (!chosen.Any(other => Same(other, item)))
        {
            return [item];
        }

        return Same(chosen[0], item)
            ? chosen
            : [item, .. chosen.Where(other => !Same(other, item))];
    }

    /// <summary>Whether two items are the same one, which is what their listing says tells them apart.</summary>
    /// <param name="left">One item.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether they are the same.</returns>
    private bool Same(TItem left, TItem right) => Equals(Source.Key(left), Source.Key(right));

    /// <summary>Copies what is chosen to the clipboard.</summary>
    public void Copy() => Source.Copy(this, Offered());

    /// <summary>Cuts what is chosen to the clipboard, which a paste then moves.</summary>
    public void Cut() => Source.Cut(this, Offered());

    /// <summary>Pastes what is on the clipboard into the listing.</summary>
    public void Paste() => Source.Paste(this);

    /// <summary>Removes what is chosen, asking first unless told not to.</summary>
    /// <param name="ask">Whether to put the question to the user first.</param>
    public void Delete(bool ask) => Source.Delete(Offered(), ask);

    /// <summary>Makes a directory in the listing, which is then named where it is drawn.</summary>
    public void NewFolder() => Source.NewFolder();

    /// <summary>The box a name is being edited in, and the entry it is about, while one is.</summary>
    /// <remarks>
    /// The name is edited in the row itself rather than asked for in a window: what is being renamed is a
    /// name in a listing, and the listing is where it can be seen against the names around it.
    /// </remarks>
    private TextBox? _editor;
    private TItem? _editing;

    /// <summary>Renames what is chosen, or the entry the pointer last landed on.</summary>
    public void Rename()
    {
        if (Offered() is [{ } only])
        {
            Rename(only);
        }
    }

    /// <summary>Puts the name of `item` into a box where the listing draws it.</summary>
    /// <param name="item">What is being renamed.</param>
    public void Rename(TItem item)
    {
        // What is edited is a row, and a row that is not on screen has no container to hold a cell: bringing
        // it into view is what makes a name reachable in a listing too long to show at once.
        List.ScrollIntoView(item);

        if (Beginning(item))
        {
            return;
        }

        // The container is made by the layout the scroll asks for, so it is a moment behind: one more turn
        // of the loop is what puts the box in a row rather than nowhere.
        Dispatcher.UIThread.Post(() => Beginning(item), DispatcherPriority.Background);
    }

    /// <summary>Puts the name of `item` into a box, where its row and the cell in it are already drawn.</summary>
    /// <param name="item">What is being renamed.</param>
    /// <returns>Whether the box was made.</returns>
    private bool Beginning(TItem item)
    {
        if (_editor is not null)
        {
            return true;
        }

        var label = _rows
            .Where(row => Same(row.Item, item))
            .Select(row => row.Row)
            .SelectMany(row => row.GetVisualDescendants())
            .OfType<TextBlock>()
            .FirstOrDefault(text => text.Classes.Contains("entry-name"));

        if (label?.Parent is not Panel parent)
        {
            return false;
        }

        var at = parent.Children.IndexOf(label);
        var name = Source.Name(item);
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
        _editing = item;

        // Taken on the way down, so that Enter and Escape are read before the box's own reading of them: a
        // field that took Enter for itself would be a name nobody could finish.
        box.AddHandler(KeyDownEvent, (_, args) => Edited(args), RoutingStrategies.Tunnel);
        box.LostFocus += (_, _) => Committed();

        box.Focus();

        // The name is chosen rather than taken whole, so that what is typed over is the name and not the
        // extension where the listing leaves the extension out (see `Listing.Select`).
        box.SelectionStart = 0;
        box.SelectionEnd = Source.Select(item, name);

        return true;
    }

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
        var item = _editing;

        if (box is null || item is null)
        {
            return;
        }

        _editor = null;
        _editing = null;

        Source.Rename(item, box.Text ?? string.Empty);

        // The box belongs to a row, and the reading is what puts the name back: a rename that had nothing to
        // do would otherwise leave the field standing. The keyboard goes back to the listing with it.
        Source.Touch();
        Listen();
    }

    /// <summary>Leaves the name as it was, and reads the listing again so the box goes away.</summary>
    private void Cancelled()
    {
        if (_editor is null)
        {
            return;
        }

        _editor = null;
        _editing = null;

        Source.Touch();
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
    public override void Listen() => List.Focus();

    /// <summary>
    /// What a shortcut acts on.
    /// </summary>
    /// <remarks>
    /// The choice, and where nothing is chosen, where the last click landed: a shortcut is reached with the
    /// keyboard anywhere in the dock, so it cannot assume a choice was made, and acting on the entry the
    /// pointer last touched is what a user who has clicked one and not another means by "this".
    /// </remarks>
    private IReadOnlyList<TItem> Offered()
    {
        var chosen = Chosen();

        return chosen.Count > 0 || Lead is not { } lead ? chosen : [lead];
    }

    /// <summary>The chosen entries, in the order the listing shows.</summary>
    public IReadOnlyList<TItem> Chosen()
    {
        var chosen = new List<TItem>();

        foreach (var at in List.Selection.SelectedIndexes)
        {
            if (at >= 0 && at < Items.Count)
            {
                chosen.Add(Items[at]);
            }
        }

        return chosen;
    }

    /// <summary>
    /// Puts the choice back on what it was on before the listing was built again.
    /// </summary>
    /// <remarks>
    /// A listing is built again whenever it is read again — a window come back to, a key asking for a
    /// refresh, a change of zoom — and the choice is what the user was working with. Losing it to a read that
    /// found the same things is losing work rather than changing the subject, and the tree keeps its own choice
    /// across the same rebuild for the same reason. An entry that is gone is simply not chosen again: a listing
    /// whose entries have changed is not the place to report which of them went.
    /// </remarks>
    /// <param name="chosen">What was chosen, in the order it was.</param>
    public void Choose(IReadOnlyList<TItem> chosen)
    {
        if (chosen.Count == 0)
        {
            return;
        }

        var last = -1;

        using (List.Selection.BatchUpdate())
        {
            List.Selection.Clear();

            foreach (var item in chosen)
            {
                var at = Index(item);

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

    /// <summary>Where an entry stands in the listing, or nothing when it is not there.</summary>
    /// <param name="item">The entry to find.</param>
    /// <returns>The place it stands in, or nothing.</returns>
    private int Index(TItem item)
    {
        for (var at = 0; at < Items.Count; at++)
        {
            if (Same(Items[at], item))
            {
                return at;
            }
        }

        return -1;
    }

    /// <summary>
    /// Draws every row again with what the listing is worth now: how faded, and whether it is cut.
    /// </summary>
    /// <remarks>
    /// For a change that leaves the entries exactly as they are — the switch that shows what is hidden
    /// changes how the entries are drawn and not which ones there are, so the rows are already in the tree and
    /// need their opacity said again rather than the view being built a second time.
    /// </remarks>
    public override void Repaint()
    {
        foreach (var (item, row) in _rows)
        {
            Source.Fade(row, item);
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
                    Source.Open(lead);
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
                To(Items.Count - 1, shift);
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
    /// What a listing says is not a name — the way up — is left out: it is not a name but a place, and it is
    /// reached by its own means rather than by being spelled, so a run that matched it would answer with
    /// something the reader did not name.
    /// </remarks>
    /// <param name="typed">What was typed.</param>
    /// <param name="from">Where to start looking.</param>
    private int Seek(string typed, int from)
    {
        for (var step = 0; step < Items.Count; step++)
        {
            var at = (((from + step) % Items.Count) + Items.Count) % Items.Count;
            var item = Items[at];

            if (Source.Seekable(item) &&
                Source.Name(item).StartsWith(typed, StringComparison.OrdinalIgnoreCase))
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
            To(by < 0 ? Items.Count - 1 : 0, false);

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
        var count = Items.Count;

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

        // The file a press landed on is the one the rest of the program may be asked about: a view
        // of its version chain follows this, and a directory says nothing here because no chain
        // hangs from one.
        if (at >= 0 && at < Items.Count)
        {
            Source.Clicked(Items[at]);
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

        for (var at = 0; at < Items.Count; at++)
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
    /// What is carried is the whole choice, or — where the press did not land on a chosen entry — the one entry it
    /// landed on. Anywhere else the press begins a frame rather than a drag, which is what keeps a frame drawn
    /// beside a choice from carrying the entries it was drawn to add to (see <see cref="Pressed"/>). Everything
    /// after that is the listing's, because what a drag of a place's entries means is not what a drag of files
    /// means: a file is handed over as a file, and a place's entry is handed over as the data its provider kept
    /// (see <see cref="Listing{TItem}.DragOut"/>).
    /// </remarks>
    private void Started()
    {
        var from = _from;
        var pressed = _slippedAt;
        var chosen = _atPress;

        _slippedAt = -1;
        _from = null;
        _atPress = [];

        if (from is null || pressed < 0 || pressed >= Items.Count)
        {
            return;
        }

        var dragged = Items[pressed];
        IReadOnlyList<TItem> carrying = chosen.Any(item => Same(item, dragged))
            ? chosen
            : [dragged];

        // What the listing will not hand over — a place that is not a thing, an entry that said it is not
        // draggable — is left out here rather than refused when the drag is begun.
        carrying = [.. carrying.Where(Source.CanDrag)];

        if (carrying.Count == 0)
        {
            return;
        }

        Source.DragOut(from, carrying, dragged);
    }

    /// <summary>Says whether a drag may land here, and lights the entry it would land on.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOver(object? sender, DragEventArgs e)
    {
        // The card is put first and from the position, since it is what says where the pointer is rather than
        // what would be done there.
        _ghost.Following(e.GetPosition(this));

        var land = Source.Landing(At(e.GetPosition(this)), e);

        Source.Marking?.Invoke(land?.Row);
        e.DragEffects = land is { } where ? Source.Over(where, e) : DragDropEffects.None;
        e.Handled = e.DragEffects != DragDropEffects.None;
    }

    /// <summary>Takes the drop mark off when a drag leaves, and the card with it.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOff(object? sender, DragEventArgs e)
    {
        Source.DragLeft();
        _ghost.Unghost();
    }

    /// <summary>Lets a drag go where it was let go, and takes the card off.</summary>
    /// <param name="sender">The list.</param>
    /// <param name="e">The drop.</param>
    private void Dropped(object? sender, DragEventArgs e)
    {
        _ghost.Unghost();

        if (Source.Landing(At(e.GetPosition(this)), e) is { } land)
        {
            Source.DropAt(land, e);
        }
    }

    /// <summary>Paints the row for the entry a drag is over, or takes the paint off.</summary>
    /// <param name="key">What says which entry, or nothing.</param>
    private void Mark(object? key)
    {
        Unmark();

        if (key is null)
        {
            return;
        }

        foreach (var (item, row) in _rows)
        {
            if (!Equals(Source.Key(item), key))
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
        if (IndexAt(e.Source) is var at && at >= 0 && at < Items.Count)
        {
            Source.Open(Items[at]);
        }
    }

    /// <summary>The entry at a point in the view, or nothing where the point is on the space around them.</summary>
    /// <remarks>
    /// Walked up to the item rather than asked of the selection, because a drag in the room beside the entries
    /// has no entry and must land nowhere in particular, and because the selection may hold entries the pointer
    /// did not touch.
    /// </remarks>
    /// <param name="at">Where the pointer is, in the view's own coordinates.</param>
    /// <returns>The entry there, or nothing.</returns>
    private TItem? At(Point at)
    {
        for (var visual = this.GetVisualAt(at) as Visual; visual is not null; visual = visual.GetVisualParent())
        {
            if (visual is ListBoxItem item &&
                List.IndexFromContainer(item) is var index &&
                index >= 0 &&
                index < Items.Count)
            {
                return Items[index];
            }
        }

        return null;
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

/// <summary>The entries of a listing as a table, one to a line.</summary>
/// <typeparam name="TItem">What one entry is.</typeparam>
internal sealed class ListBrowser<TItem> : EntryView<TItem>
    where TItem : class
{
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

    /// <summary>
    /// How wide the column of names starts.
    /// </summary>
    /// <remarks>
    /// A width of its own rather than what is left over, which is what the room the table is read in is for: the
    /// dock's spare width is kept after the name and what is read with it, so that a reader finds the two together
    /// rather than a name at one edge of the dock and its other name at the other. It is taken hold of by its edge
    /// like the rest, so a reader who wants more of a name drags it open.
    /// </remarks>
    private const double NameColumnWidth = 200;

    /// <summary>The contributed columns read with the name, which stand in front of the room the table is read in.</summary>
    private readonly Column<TItem>[] _beside;

    /// <summary>The columns after the name, which every row of the table has the same of.</summary>
    private readonly IReadOnlyList<Column<TItem>> _values;

    /// <summary>How wide every column is, the grabs between them included.</summary>
    private readonly GridLength[] _widths;

    /// <summary>Which of the table's columns is the one that takes what is left over, and holds nothing.</summary>
    private readonly int _star;

    /// <summary>
    /// Where every edge between two columns is, as the grab column and the pair it is between.
    /// </summary>
    /// <remarks>
    /// Kept as the columns were laid out rather than read off the widths, because a grab is a column of the
    /// table rather than a shape of the measure: the pair a grab moves is settled where the table is built, and
    /// the room the table is read in means the edges are no longer evenly spaced.
    /// </remarks>
    private readonly Dictionary<int, (int Left, int Right)> _grabs;

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
    private readonly Column<TItem>[] _leads;

    /// <summary>Makes the table over what the listing holds.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="source">What is listed.</param>
    public ListBrowser(IPluginHost host, Listing<TItem> source)
        : base(host, source)
    {
        // A column is placed where its own contribution asks to be. The split is the view's, since the
        // arrangement knows where the room the table is read in falls and the listing that contributed the
        // column does not.
        var columns = source.Columns;

        _leads = [.. columns.Where(column => column.Placement == ColumnPlacement.Leftmost)];
        _beside = [.. columns.Where(column => column.Placement == ColumnPlacement.BesideName)];
        _values =
        [
            .. columns.Where(column =>
                column.Placement is not ColumnPlacement.Leftmost and not ColumnPlacement.BesideName
            ),
        ];

        var (widths, star, grabs) = Lay();
        _widths = widths;
        _star = star;
        _grabs = grabs;

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

    /// <inheritdoc />
    protected override Control Item(TItem item) => Prepared(item, Row(item));

    /// <summary>
    /// How wide every column starts, in the order they are laid out, together with the column that takes what
    /// is left over and where every edge between two columns is.
    /// </summary>
    /// <remarks>
    /// The order is what a row is read in: the columns read with the name, then the room the table is read in,
    /// then what is read about the entry. The room is a column of its own rather than the name's own slack, since
    /// a table whose rows are one grid each keeps every column but the one that takes what is left over at a width
    /// they all share.
    /// </remarks>
    private (GridLength[] Widths, int Star, Dictionary<int, (int Left, int Right)> Grabs) Lay()
    {
        var widths = new List<GridLength>();
        var grabs = new Dictionary<int, (int Left, int Right)>();

        // A contributed column is taken hold of by its own edge, which is the grab in front of it: the grab moves
        // that column and the one before it, and the pair is settled here rather than read off the widths.
        void Add(IReadOnlyList<Column<TItem>> columns)
        {
            foreach (var column in columns)
            {
                grabs.Add(widths.Count, (widths.Count - 1, widths.Count + 1));
                widths.Add(new GridLength(GrabColumn));
                widths.Add(new GridLength(column.Width));
            }
        }

        foreach (var column in _leads)
        {
            widths.Add(new GridLength(GrabColumn));
            widths.Add(new GridLength(column.Width));
        }

        widths.Add(new GridLength(Icons.IconColumn));
        widths.Add(new GridLength(NameColumnWidth));

        Add(_beside);

        var star = widths.Count;

        // The room the table is read in: it holds nothing and takes what is left over, so that the columns after
        // it stand against the dock's far edge however wide the dock is.
        widths.Add(new GridLength(1, GridUnitType.Star));

        Add(_values);

        return ([.. widths], star, grabs);
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

        foreach (var column in _beside)
        {
            cells.Add(Grip());
            cells.Add(Head(column.Header, column.Align));
        }

        // The room the table is read in is headed by nothing: it is the dock's own width rather than a column
        // of the entry, and there is no word that would name it.
        cells.Add(null);

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
    /// <param name="item">The entry to draw.</param>
    private Control Row(TItem item)
    {
        var name = Source.Name(item);
        var icon = Source.Icon(item, Icons.Row);
        icon.HorizontalAlignment = HorizontalAlignment.Left;

        var cells = new List<Control?>();

        // The cell over the grab is nothing: a row has no grab, only the header does.
        foreach (var column in _leads)
        {
            cells.Add(null);
            cells.Add(
                column.Draw is { } draw ? draw(item) : Cell(column.Value(item), column.Align)
            );
        }

        cells.Add(icon);
        cells.Add(Named(item, name.Length > 0 ? name : Source.Key(item)?.ToString() ?? string.Empty));

        foreach (var column in _beside)
        {
            cells.Add(null);
            cells.Add(
                column.Draw is { } draw ? draw(item) : Cell(column.Value(item), column.Align)
            );
        }

        // The room the table is read in holds nothing: a row has no cell there.
        cells.Add(null);

        foreach (var column in _values)
        {
            cells.Add(null);
            cells.Add(
                column.Draw is { } draw ? draw(item) : Cell(column.Value(item), column.Align)
            );
        }

        var grid = new Grid { ColumnDefinitions = Definitions() };
        Place(grid, cells);

        // Every row but the last wears the line that tells it from the next, so that the table ends in the
        // card's own edge rather than in a line drawn against nothing.
        var last = Items.Count > 0 && Equals(Source.Key(item), Source.Key(Items[^1]));

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
    /// An edge is the centre of a grab column, and which pair a grab moves is the layout's answer rather than a
    /// measurement of the table: the room the table is read in is not where a fixed share of the dock would put
    /// it, so where every other edge stands moves with the dock.
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

            if (
                _grabs.TryGetValue(column, out var pair)
                && Math.Abs(header.X - (left + (width / 2))) <= GrabZone / 2
            )
            {
                return pair;
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
    /// border in the table where it was. Where one of the pair is the room the table is read in — the column
    /// that takes what is left over — that one is skipped rather than moved: it takes the move for free, which
    /// is also what keeps the table as wide as the dock.
    /// <para>
    /// Neither column is dragged away entirely: the move is taken as far as the narrower of the two allows,
    /// and the room counts as a column here, which is why its width is asked of the layout.
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
        _star >= 0 && _header.ColumnDefinitions.Count > _star
            ? _header.ColumnDefinitions[_star].ActualWidth
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

    /// <summary>One entry's name: one that leads somewhere reads heavier.</summary>
    /// <remarks>
    /// The way up is left light: it is not a thing of the listing but a step out of it, and the design draws it
    /// as the one row that is a place rather than a thing.
    /// </remarks>
    /// <param name="item">The entry the name belongs to.</param>
    /// <param name="text">The name as it is written.</param>
    private TextBlock Named(TItem item, string text) =>
        new()
        {
            Text = text,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            // Marked with the class rather than kept in a field, because the row is made once and reused by
            // the toolkit's virtualisation: what a rename edits is the name cell of the row as it stands.
            Classes = { "entry-name" },
            FontWeight = Source.Openable(item) ? FontWeight.SemiBold : FontWeight.Normal,
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

/// <summary>The entries of a listing as tiles, wrapping across the width.</summary>
/// <typeparam name="TItem">What one entry is.</typeparam>
internal sealed class GridBrowser<TItem> : EntryView<TItem>
    where TItem : class
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

    /// <summary>Makes the grid layout over what the listing holds.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="source">What is listed.</param>
    /// <param name="icon">How large an icon is at the zoom this dock is at.</param>
    public GridBrowser(IPluginHost host, Listing<TItem> source, int icon)
        : base(host, source)
    {
        _icon = icon;

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
    /// <param name="item">The entry to draw.</param>
    protected override Control Item(TItem item) => Prepared(item, Tile(item));

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
        var count = Items.Count;

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

    /// <inheritdoc />
    private Control Tile(TItem item)
    {
        // The surface a chosen tile wears is the item's; what is here is the content and the room around it.
        return new Border
        {
            Padding = new Thickness(Around),
            Child = new StackPanel
            {
                Spacing = 8,
                Children = { Source.Marked(item, _icon), Caption(item) },
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
    /// <param name="item">The entry to draw.</param>
    /// <returns>What to draw where the name goes.</returns>
    private Control Caption(TItem item)
    {
        var name = new TextBlock
        {
            MaxWidth = Math.Max(1, _icon),
            Text = Source.Name(item),
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            Classes = { "entry-name", "caption" },
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        return name;
    }
}

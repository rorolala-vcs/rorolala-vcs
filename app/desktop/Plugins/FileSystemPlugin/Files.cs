using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>
/// A directory as a listing: its entries, its columns, and everything a user does to them.
/// </summary>
/// <remarks>
/// This is where being a <em>file</em> is said, and it is the only place that says it. A view arrangement
/// (<see cref="ListBrowser{TItem}"/>, <see cref="GridBrowser{TItem}"/>) knows how to lay entries out and nothing
/// about what they are; what a plugin's place adds over a directory — a provider's picture, a provider's menu, a
/// drop the provider answers — is its own listing on the other side of <see cref="Listing{TItem}"/>. The two meet
/// in the views, which is what makes a place as well read as a directory without being read twice.
/// </remarks>
internal sealed class Files : Listing<Entry>
{
    /// <summary>What a cell says when its provider has nothing to say about the entry.</summary>
    private const string Placeholder = "-";

    /// <summary>How large a dot is: a mark read at a glance rather than a thing to point at.</summary>
    private const double DotSize = 8;

    /// <summary>How wide a column of values is before anybody drags it.</summary>
    private const double PermissionsColumn = 110;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double ModifiedColumn = 150;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double SizeColumn = 88;

    /// <inheritdoc cref="PermissionsColumn" />
    private const double OwnershipColumn = 96;

    /// <summary>What opening and editing entries is done with.</summary>
    private readonly BrowserActions _actions;

    /// <summary>What a copy or a cut has put within reach of a paste.</summary>
    private readonly Clip _clip;

    /// <summary>The location, which is what the files and the columns are read through.</summary>
    private readonly Browser _browser;

    /// <summary>The lock provider that speaks for this directory, or nothing when none does.</summary>
    private readonly IEntryLockProvider? _locks;

    /// <summary>What a tile's icon wears at its top right corner, in the order the badges are numbered.</summary>
    private readonly IReadOnlyList<IEntryMarkProvider> _marks;

    /// <summary>How a drag of files is taken here.</summary>
    private readonly Drops _drops;

    /// <summary>The columns after the name, in the order they are shown.</summary>
    private readonly IReadOnlyList<Column<Entry>> _columns;

    /// <summary>
    /// What the columns said about an entry, read once per row rather than once per cell.
    /// </summary>
    /// <remarks>
    /// Every column would otherwise ask the file itself, and three columns would be three times the asking for one
    /// row. The answers are kept for as long as the listing is, which is one read of one directory.
    /// </remarks>
    private readonly Dictionary<Entry, Facts> _facts = [];

    /// <summary>Makes the listing of a directory the location is looking at.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">The location, which is where the entries are read from.</param>
    /// <param name="actions">What opening and editing entries is done with.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public Files(IPluginHost host, Browser browser, BrowserActions actions, Clip clip)
        : base(host, browser.Shown)
    {
        _actions = actions;
        _clip = clip;
        _browser = browser;
        _locks = EntryLockProviders.For(browser.Current);
        _marks = EntryMarkProviders.For(browser.Current);

        // The light on a row is the view's to put out, so the protocol is handed a way to say so rather than a
        // row it could never find: what is lit is a row this listing never draws.
        _drops = new Drops(host, browser, key => Marking?.Invoke(key));
        _columns = Values();
    }

    /// <inheritdoc />
    public override IReadOnlyList<Column<Entry>> Columns => _columns;

    /// <inheritdoc />
    public override object Key(Entry item) => item.Path;

    /// <inheritdoc />
    public override string Name(Entry item) => Names.Show(item);

    /// <inheritdoc />
    public override Control Icon(Entry item, int size) => Icons.For(item, size);

    /// <summary>An entry's icon, with what every contributor marked worn around its top right corner.</summary>
    /// <remarks>
    /// Every provider that speaks for the directory is asked, rather than one of them: two badges say two
    /// different things — who holds the entry and how far its version has come — and neither is the other's to
    /// replace. A picture is looked up rather than carried, and a key that names nothing draws nothing: a
    /// provider that contributed a mark but no picture is a listing without a badge on it, not a listing that
    /// fails to be drawn.
    /// <para>
    /// The order the providers were asked in is the order the badges are numbered, which is what makes a badge
    /// stand in the same place of the arc from one listing to the next.
    /// </para>
    /// </remarks>
    /// <param name="item">The entry to draw.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw in the tile.</returns>
    public override Control Marked(Entry item, int size)
    {
        var icon = Icons.For(item, size);
        var badges = new List<EntryBadge>();

        foreach (var provider in _marks)
        {
            if (
                provider.Mark(item) is { IconKey: { Length: > 0 } key } mark
                && Host.Icons.Find(key) is { } picture
            )
            {
                badges.Add(new EntryBadge(picture, mark));
            }
        }

        return Icons.Marked(icon, badges, size);
    }

    /// <inheritdoc />
    public override bool Openable(Entry item) =>
        item.Kind == EntryKind.Directory && !Browser.IsUp(item.Path);

    /// <inheritdoc />
    public override bool Seekable(Entry item) => !Browser.IsUp(item.Path);

    /// <summary>
    /// Whether the listing holds nothing but the way up.
    /// </summary>
    /// <remarks>
    /// The way up stands before every listing that is not the base's own and is not an entry of the directory, so
    /// a directory with nothing in it still shows one row; the word for an empty listing is due where there is
    /// nothing but that row.
    /// </remarks>
    public override bool Barren => !Items.Any(item => !Browser.IsUp(item.Path));

    /// <summary>
    /// What a listing with nothing in it says.
    /// </summary>
    /// <remarks>
    /// It takes no pointer of its own: the space it sits over is still the list's, and a word that ate the
    /// right-click there would take the empty-space menu and the start of a frame with it.
    /// </remarks>
    /// <returns>The word.</returns>
    public override Control? Notice() =>
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

    /// <inheritdoc />
    public override void Open(Entry item) => _actions.Open(item);

    /// <inheritdoc />
    public override void Fill(ContextMenu menu, Control from, IReadOnlyList<Entry> chosen, bool force) =>
        _actions.Fill(menu, from, chosen, force);

    /// <inheritdoc />
    public override ContextMenu? Space(Control from) => _actions.Empty(from);

    /// <summary>
    /// How faded a row is: one a hidden provider hides reads as secondary, and one that is cut as on its way out.
    /// </summary>
    /// <remarks>
    /// Both are said here rather than in two passes over the rows, because a row has one opacity and the second
    /// pass would be the one that decided it. Cut wins, because being on its way out is what the user just did.
    /// <para>
    /// What is hidden is asked of the providers rather than carried on the entry, so a row drawn while a provider
    /// was switched off is faded again the moment it is switched on.
    /// </para>
    /// </remarks>
    /// <param name="row">The row to fade.</param>
    /// <param name="item">What it stands for.</param>
    public override void Fade(Control row, Entry item) =>
        row.Opacity = _clip.IsCut(item.Path)
            ? _clip.Faded
            : _browser.Hides(item)
                ? EntryView.HiddenOpacity
                : 1.0;

    /// <inheritdoc />
    public override void Copy(Control from, IReadOnlyList<Entry> chosen) => _actions.Copy(from, chosen);

    /// <inheritdoc />
    public override void Cut(Control from, IReadOnlyList<Entry> chosen) => _actions.Cut(from, chosen);

    /// <inheritdoc />
    public override void Paste(Control from) => _actions.Paste(from, _browser.Current);

    /// <inheritdoc />
    public override void Delete(IReadOnlyList<Entry> chosen, bool ask) => _actions.Remove(chosen, ask);

    /// <inheritdoc />
    public override void NewFolder() => _actions.NewFolder(_browser.Current);

    /// <inheritdoc />
    public override void Rename(Entry item, string name) => _actions.Rename(item, name);

    /// <summary>How much of a name a rename chooses to begin with, which leaves a file's extension out.</summary>
    /// <param name="item">What is being renamed.</param>
    /// <param name="name">The name it has.</param>
    /// <returns>Where the chosen part of the name ends.</returns>
    public override int Select(Entry item, string name) => Names.Chosen(item.Kind, name);

    /// <inheritdoc />
    public override Entry? Pending()
    {
        // The directory as well as the entry: a name made in one directory is not one this listing's rows hold, and
        // a box put in the row of a dock the user is not looking at is worse than no box at all.
        if (
            _actions.Awaiting is not { } awaiting
            || !string.Equals(awaiting.Directory, _browser.Current, StringComparison.Ordinal)
        )
        {
            return null;
        }

        // Taken rather than read, so that the next view made for this listing does not name it a second time.
        _actions.Awaiting = null;

        return awaiting.Entry;
    }

    /// <inheritdoc />
    public override void Touch() => _browser.Touch();

    /// <inheritdoc />
    public override void Chosen(IReadOnlyList<Entry> chosen) => ChosenFiles.Publish(chosen);

    /// <inheritdoc />
    public override void Clicked(Entry item) => ChosenEntry.Publish(item);

    /// <summary>
    /// Whether an entry may be picked up and dragged away.
    /// </summary>
    /// <remarks>
    /// Places rather than things: nothing about the way up or the computer is a file to hand over.
    /// </remarks>
    /// <param name="item">The entry to consider.</param>
    /// <returns>Whether a drag may begin on it.</returns>
    public override bool CanDrag(Entry item) =>
        !Browser.IsUp(item.Path) && !Browser.IsComputer(item.Path);

    /// <inheritdoc />
    public override void DragOut(
        PointerPressedEventArgs from,
        IReadOnlyList<Entry> carrying,
        Entry lead
    ) => Drag.Away(_browser, from, carrying, lead, Host.Log.Error);

    /// <summary>
    /// Where a drag would land, or nothing where it may not land here at all.
    /// </summary>
    /// <remarks>
    /// A directory entry is a place to let go into, and so is the way up — which is not a directory of its own but
    /// the one holding the listing, so what it lands in is asked of the browser. A file and the computer are not,
    /// and neither is a drag carrying no file. The space around the entries is the directory being looked at.
    /// </remarks>
    /// <param name="item">The entry the pointer is over, or nothing where it is over the space.</param>
    /// <param name="e">The drag.</param>
    /// <returns>Where it would land.</returns>
    public override Land? Landing(Entry? item, DragEventArgs e)
    {
        if (!e.DataTransfer.Contains(DataFormat.File))
        {
            return null;
        }

        if (item is null)
        {
            return Browser.IsComputer(_browser.Current) ? null : new Land(_browser.Current, null);
        }

        if (Browser.IsUp(item.Path))
        {
            return _browser.Parent is { Length: > 0 } up ? new Land(up, item.Path) : null;
        }

        return item.Kind == EntryKind.Directory && !Browser.IsComputer(item.Path)
            ? new Land(item.Path, item.Path)
            : null;
    }

    /// <inheritdoc />
    public override DragDropEffects Over(Land land, DragEventArgs e) => Drops.Over(e);

    /// <inheritdoc />
    public override void DropAt(Land land, DragEventArgs e) => _drops.Dropped(land, e);

    /// <summary>
    /// The columns after the name, in the order they are shown.
    /// </summary>
    /// <remarks>
    /// Permissions are not a column on Windows, where the system does not carry what those nine letters say. A
    /// column blank in every row would be this table saying "not here" in a column of its own, and a table is
    /// better off with one column fewer.
    /// <para>
    /// A contributed column is placed where its own contribution asks to be, and is only there when a provider
    /// speaks for this directory: a column of placeholders would be a column of the listing saying what it does
    /// not know. The Ownership column comes last of all, since a lock is the one answer that is a state of the
    /// entry rather than a value of it.
    /// </para>
    /// </remarks>
    /// <returns>The columns, in the order the table lays them out.</returns>
    private Column<Entry>[] Values()
    {
        var contributed = EntryColumns.For(_browser.Current);
        List<Column<Entry>> columns =
        [
            .. contributed
                .Where(provider => provider.Placement == ColumnPlacement.Leftmost)
                .Select(Contributed),
            .. contributed
                .Where(provider => provider.Placement == ColumnPlacement.BesideName)
                .Select(Contributed),
            .. contributed
                .Where(provider => provider.Placement == ColumnPlacement.BeforeValues)
                .Select(Contributed),
        ];

        if (!OperatingSystem.IsWindows())
        {
            columns.Add(Permissions());
        }

        columns.Add(Modified());
        columns.Add(Size());
        columns.AddRange(
            contributed
                .Where(provider => provider.Placement == ColumnPlacement.AfterValues)
                .Select(Contributed)
        );

        if (_locks is { } locks)
        {
            columns.Add(Lock(locks));
        }

        return [.. columns];
    }

    /// <summary>The column that says what may be done with the entry and by whom.</summary>
    /// <returns>The column.</returns>
    private Column<Entry> Permissions() =>
        new("rorolala_file_system.column_permissions", PermissionsColumn, entry => Of(entry).Permissions);

    /// <summary>The column that says when an entry was last written.</summary>
    /// <returns>The column.</returns>
    private Column<Entry> Modified() =>
        new("rorolala_file_system.column_modified", ModifiedColumn, entry => Of(entry).Modified);

    /// <summary>The column that says how large an entry is, which is a number and so stands on the right.</summary>
    /// <returns>The column.</returns>
    private Column<Entry> Size() =>
        new("rorolala_file_system.column_size", SizeColumn, entry => Of(entry).Size, TextAlignment.Right);

    /// <summary>
    /// The Ownership column one lock provider contributes.
    /// </summary>
    /// <remarks>
    /// The value in it is the provider's to write and is not read from the entry's metadata, which is why the
    /// column draws its own cell: what a lock looks like — its words, and the ink they are written in — is what the
    /// provider answered, and none of it is in <see cref="Facts"/>.
    /// <para>
    /// What it says stands on the right, which is where a column of short answers is read from: the holder's name
    /// is the last thing on the row, and the edge it is read against is the one the listing ends in.
    /// </para>
    /// </remarks>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <returns>The column to add after the table's own.</returns>
    private static Column<Entry> Lock(IEntryLockProvider provider) =>
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
    /// The value in it is the provider's to write and is not read from the entry's metadata, which is why the
    /// column draws its own cell: what it says goes through <see cref="EntryColumnMark.Said"/>, which is a key
    /// translated or a literal as the contribution asked, and neither is in <see cref="Facts"/>.
    /// </remarks>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <returns>The column to add where the contribution asked.</returns>
    private static Column<Entry> Contributed(IEntryColumnProvider provider) =>
        new(
            provider.ColumnKey,
            provider.Width,
            _ => Placeholder,
            Draw: entry => Contributed(provider, entry),
            Placement: provider.Placement
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

        // A column of marks says nothing by having no mark: the dash the listing says what it does not know with
        // would be a mark of its own, and a cell with no mark has nothing else to say.
        return provider.Marks ? new Panel() : Written(mark);
    }

    /// <summary>What one entry's cell in a contributed column that is words is written as.</summary>
    /// <remarks>
    /// The ink is the contribution's to name and this plugin's to resolve: a column drawn in a colour the
    /// contribution picked for itself would be a colour the look does not hold, which is a column that vanishes
    /// somewhere the theme is another — unless the contribution has a palette of its own, which is what a colour
    /// of its own says, and then it is drawn in that. The Ownership column writes its words the same way.
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
    /// A dot rather than a word, because what it stands for is one of a handful of states and the colour is read
    /// at a glance: a column of the same few words repeated is a column of text where a mark would do, and the
    /// words are still there — said when the pointer rests on the mark, which is where a reader who wants them is
    /// already looking.
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
            // what reads around it: a hairline of the listing's would be a mark drawn differently from the one the
            // contribution asked for.
            dot.BorderThickness = new Thickness(1);
            dot.BorderBrush = new SolidColorBrush(edge);
        }

        if (mark.Hint is { Length: > 0 } hint)
        {
            ToolTip.SetTip(dot, hint);

            // At once rather than after the wait a tooltip usually takes: the mark is a colour, and a reader who
            // has to stop and wait for what a colour means is a reader who has to stop. Nothing is said by the wait
            // that is not said by the mark.
            ToolTip.SetShowDelay(dot, 0);
        }

        return dot;
    }

    /// <summary>One cell of one row, written in the column's alignment.</summary>
    /// <param name="text">What the cell says.</param>
    /// <param name="align">Where the text sits in the column.</param>
    /// <returns>The cell.</returns>
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

    /// <summary>What the columns say about one entry, read once and kept for the rest of this listing.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>What each column says.</returns>
    private Facts Of(Entry entry)
    {
        if (!_facts.TryGetValue(entry, out var facts))
        {
            facts = Stats.Of(entry);
            _facts[entry] = facts;
        }

        return facts;
    }
}

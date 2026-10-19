using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using MenuItem = Avalonia.Controls.MenuItem;

namespace FileSystemPlugin;

/// <summary>
/// A place a plugin answers for, as a listing: the entries the provider gave, and what each of them does.
/// </summary>
/// <remarks>
/// The other side of <see cref="Listing{TItem}"/> from <see cref="Files"/>, and deliberately a small one: a view
/// already knows the card, the row height, the icons and the zoom, so all a place has to say is what its entries
/// are called, what picture they wear, where opening one goes, and what its menu offers. That is the whole of the
/// logical-view point — a plugin's place read exactly as a directory is, without a second set of views to drift.
/// <para>
/// What a place adds over a directory is the provider's rather than this listing's: the columns beside the name, the
/// way into a folder row, the drag an entry may be taken out as, and what a paste of one means. All of it is asked
/// of the address being read, so an entry of a place is never mistaken for a file.
/// </para>
/// </remarks>
internal sealed class Places : Listing<LogicalEntry>
{
    /// <summary>The location, which is how a double-click goes somewhere.</summary>
    private readonly Browser _browser;

    /// <summary>The place being listed, which is the one answer read for one address.</summary>
    private readonly LogicalPlace _place;

    /// <summary>What a copy or a cut of a place's entry was put to, or nothing when none was made.</summary>
    private readonly LogicalClip? _logical;

    /// <summary>The columns the provider states, mapped onto the table's own placements.</summary>
    private readonly IReadOnlyList<Column<LogicalEntry>> _columns;

    /// <summary>A folder the place just made, waiting for its row to be named.</summary>
    private string? _made;

    /// <summary>Makes the listing of a place.</summary>
    /// <param name="host">The host, for the library an entry's picture is read from.</param>
    /// <param name="browser">The location, which is how a double-click goes somewhere.</param>
    /// <param name="place">The place to list.</param>
    /// <param name="logical">What a copy or a cut of an entry is put to, or nothing when this dock keeps none.</param>
    public Places(IPluginHost host, Browser browser, LogicalPlace place, LogicalClip? logical = null)
        : base(host, place.Entries)
    {
        ArgumentNullException.ThrowIfNull(browser);
        ArgumentNullException.ThrowIfNull(place);

        _browser = browser;
        _place = place;
        _logical = logical;
        _columns =
        [
            .. (place.Provider?.Columns(place.Address) ?? []).Select(column => Column(column)),
        ];

    }

    /// <summary>The row of a place's entries a name is, which is a folder the place just made.</summary>
    /// <param name="entries">What the place lists.</param>
    /// <param name="name">What it is called.</param>
    /// <returns>The row, or nothing when it is not there.</returns>
    private static LogicalEntry? Named(IReadOnlyList<LogicalEntry> entries, string name) =>
        entries.FirstOrDefault(entry =>
            entry.Opens is not null && string.Equals(entry.Name, name, StringComparison.Ordinal)
        );

    /// <summary>What the provider says about each entry, laid out beside the name or after the values.</summary>
    /// <remarks>
    /// A column read with the name is a second name of the entry — where a layout names it, which uuid it is known by
    /// — and one read after the values is a fact about it: who holds it. That is the provider's answer rather than
    /// this table's, so the cell is drawn from the entry each time a row is built.
    /// </remarks>
    public override IReadOnlyList<Column<LogicalEntry>> Columns => _columns;

    /// <summary>
    /// What a place lets be done to the folders it holds that are not in what it reads.
    /// </summary>
    /// <remarks>
    /// A place is read, so a folder this side made is the place's own answer rather than a directory here: what may be
    /// made, named and removed is what it says, and a provider that says nothing is a listing that can only be read.
    /// </remarks>
    /// <returns>What the place answered, or nothing when it holds no folders of its own.</returns>
    private ILogicalFolderEditor? Editor() => _place.Provider as ILogicalFolderEditor;

    /// <inheritdoc />
    public override void NewFolder()
    {
        if (Editor() is not { } editor || !editor.NewFolder(_place.Address))
        {
            return;
        }

        // What was made is asked for at once, because the place answers once: the row it is, out of what the place now
        // lists, is what the box is put on — and the view asks for it as the folder is made, one turn before a row can
        // hold a cell.
        _made = editor.Pending(_place.Address);
    }

    /// <inheritdoc />
    public override LogicalEntry? Pending()
    {
        var made = _made;
        _made = null;

        // The name is the place's own — `@/new/Photos` names a folder `Photos` — and the row is one of the place's
        // entries, which is the same record instance the listing draws, so it is found rather than made again.
        return made is null
            ? null
            : Named(
                _place.Entries,
                made[(made.LastIndexOf('/') + 1)..]
            );
    }

    /// <inheritdoc />
    public override bool Renamable(LogicalEntry item) =>
        item.Data is { } data && Editor() is { } editor && editor.Renamable(data);

    /// <inheritdoc />
    public override void Rename(LogicalEntry item, string name)
    {
        if (item.Data is { } data)
        {
            _ = Editor()?.Rename(_place.Address, data, name);
        }
    }

    /// <inheritdoc />
    public override bool Removable(LogicalEntry item) =>
        item.Data is { } data && Editor() is { } editor && editor.Removable(data);

    /// <inheritdoc />
    public override void Delete(IReadOnlyList<LogicalEntry> chosen, bool ask)
    {
        ArgumentNullException.ThrowIfNull(chosen);

        if (Editor() is not { } editor)
        {
            return;
        }

        foreach (var item in chosen)
        {
            if (item.Data is { } data)
            {
                _ = editor.Remove(_place.Address, data);
            }
        }
    }

    /// <summary>
    /// What tells one entry of a place from another.
    /// </summary>
    /// <remarks>
    /// The entry itself, which is a record: what a provider gives back for one address is what says which entry
    /// is which, and nothing here reads a name as one — two entries of a place may share a name, and a name is the
    /// provider's to repeat.
    /// </remarks>
    /// <param name="item">The entry.</param>
    /// <returns>What says which one it is.</returns>
    public override object Key(LogicalEntry item) => item;

    /// <inheritdoc />
    public override string Name(LogicalEntry item) => item.Name;

    /// <summary>How much of a name a rename chooses, which leaves a file's extension out.</summary>
    /// <remarks>
    /// Chosen the way the file browser chooses it, because the accident it avoids is the same one: typing over
    /// the whole of <c>hero.psd</c> takes the extension with it, and a place's rows are named exactly as a
    /// directory's are. A row a place opens is a folder, and one it does not is a file — a place names no other
    /// kind of thing.
    /// </remarks>
    /// <param name="item">The entry being renamed.</param>
    /// <param name="name">The name it has.</param>
    /// <returns>Where the chosen part of the name ends.</returns>
    public override int Select(LogicalEntry item, string name) =>
        Names.Chosen(item.Opens is null ? EntryKind.File : EntryKind.Directory, name);

    /// <inheritdoc />
    public override Control Icon(LogicalEntry item, int size) => Picture(item, size);

    /// <summary>Whether an entry leads somewhere, which is what its name reads heavier for.</summary>
    /// <param name="item">The entry.</param>
    /// <returns>Whether it opens.</returns>
    public override bool Openable(LogicalEntry item) => item.Opens is not null;

    /// <inheritdoc />
    public override void Open(LogicalEntry item)
    {
        if (item.Opens is { } opens)
        {
            _browser.Go(opens.ToString());
        }
    }

    /// <summary>
    /// Fills the menu opened on an entry with what the provider stated.
    /// </summary>
    /// <remarks>
    /// The items are the provider's own, so they are put in as they are: what an entry may have done to it is the
    /// plugin's answer, and the contract already carries one.
    /// <para>
    /// The action is handed the address the menu was opened at and no entries, because a logical entry is not a
    /// file: a provider that needs to know which entry its item is about built the item around that entry, which
    /// is what <see cref="LogicalEntry.Data"/> is for.
    /// </para>
    /// </remarks>
    /// <param name="menu">The menu to fill.</param>
    /// <param name="from">The control the menu is opened from.</param>
    /// <param name="chosen">What is chosen, the entry the menu was opened on first.</param>
    /// <param name="force">Whether the forced actions are shown to begin with, which a place has none of.</param>
    public override void Fill(
        ContextMenu menu,
        Control from,
        IReadOnlyList<LogicalEntry> chosen,
        bool force
    )
    {
        // Emptied first, because a menu is filled every time it is opened rather than made for one entry: a menu that
        // appended to itself would grow an item per right-click, and the same entry would be offered twice.
        menu.Items.Clear();

        if (chosen.Count == 0 || chosen[0].Menu is not { Count: > 0 } items)
        {
            return;
        }

        foreach (var item in items.OrderBy(item => item.Order))
        {
            menu.Items.Add(Item(item));
        }
    }

    /// <summary>
    /// The menu on the space around the entries: what a place can be asked of itself, which is a read again.
    /// </summary>
    /// <remarks>
    /// Everything else a menu might offer is about a file, and there is none here. The space is the place rather
    /// than an entry, so the provider is not asked — it has said what its entries offer, and nothing about the
    /// room between them.
    /// </remarks>
    /// <param name="from">The control the menu is opened from.</param>
    /// <returns>The menu.</returns>
    public override ContextMenu? Space(Control from)
    {
        var menu = new ContextMenu();
        menu.Items.Add(BrowserActions.Item("rorolala_file_system.refresh", _browser.Refresh));

        return menu;
    }

    /// <summary>
    /// What a place with nothing to read says.
    /// </summary>
    /// <remarks>
    /// Two answers, because there are two conditions: nobody claimed the prefix, or somebody did and listed
    /// nothing. It takes no pointer of its own, so that the menu and the drop under it keep answering.
    /// </remarks>
    /// <returns>The word.</returns>
    public override Control? Notice() =>
        new TextBlock
        {
            Text = _place.Provider is null
                ? RolaI18N.Get("rorolala_file_system.unknown_address", _place.Address.Scheme)
                : RolaI18N.Get("rorolala_file_system.empty"),
            Classes = { "muted" },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
        };

    /// <summary>How faded a row is: one a cut has offered to a paste reads as on its way out.</summary>
    /// <param name="row">The row to fade.</param>
    /// <param name="item">What it stands for.</param>
    public override void Fade(Control row, LogicalEntry item) =>
        row.Opacity = _logical?.IsCut(Kept(item)) == true ? Clip.FadedAmount : 1.0;

    /// <inheritdoc />
    public override void Copy(Control from, IReadOnlyList<LogicalEntry> chosen) => Put(chosen, cut: false);

    /// <inheritdoc />
    public override void Cut(Control from, IReadOnlyList<LogicalEntry> chosen) => Put(chosen, cut: true);

    /// <summary>
    /// Pastes what a place put on offer into this place, when the place takes it.
    /// </summary>
    /// <remarks>
    /// The offer is read and only then ended, so a place that refuses it leaves a cut where it was: what a paste means
    /// is the answerer's, and this side may not end an offer on its own judgement.
    /// </remarks>
    /// <param name="from">A control in the tree the clipboard would be reached through, which a place does not use.</param>
    public override void Paste(Control from)
    {
        if (_logical?.Peek() is not { Count: > 0 } items)
        {
            return;
        }

        var drag = new LogicalDrag(items.Count == 1 ? items[0] : items.ToArray());

        if (_place.Provider is null)
        {
            Refused(null, _place.Address, drag);

            return;
        }

        if (!_place.Provider.Accepts(_place.Address, drag))
        {
            Refused(_place.Provider, _place.Address, drag);

            return;
        }

        _place.Provider.Drop(_place.Address, drag);
        _logical.Took();
    }

    /// <summary>
    /// Says that the place would not take what was let go on it, in the place's own words where it has any.
    /// </summary>
    /// <remarks>
    /// Shown in the notification stack rather than as a dialog, because nothing about it is the user's to decide: the
    /// place refused, and what is owed is the reason. The content is made by the host on the window's thread, which is
    /// why what is handed over is how to build it.
    /// </remarks>
    /// <param name="provider">What answered the place, or nothing when nobody did.</param>
    /// <param name="address">The place the drop was on, or nothing when what was dropped on is not an address.</param>
    /// <param name="drag">What was dragged.</param>
    private void Refused(ILogicalViewProvider? provider, LogicalAddress? address, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(drag);

        var said =
            (provider is ILogicalRefusal refusal && address is not null
                ? refusal.Refusal(address, drag)
                : null) ?? RolaI18N.Get("rorolala_file_system.refused");

        Host.Dialogs.Notify(
            new Notification(
                NotificationLevel.Warning,
                RolaI18N.Get("rorolala_file_system.refused_title"),
                () =>
                    new TextBlock
                    {
                        Text = said,
                        TextWrapping = TextWrapping.Wrap,
                        MaxWidth = 420,
                    }
            )
        );
    }

    /// <summary>
    /// Whether a drag is something this place can take.
    /// </summary>
    /// <remarks>
    /// Asked while the pointer is still moving, which is what decides whether a drop is offered at all. A row that
    /// opens a place is a folder to drop into, and is offered to whatever answers that place's address; anywhere
    /// else is the place being read, and is offered to its own answer.
    /// <para>
    /// A drag of this program's own is offered even where the place would refuse it, so that the refusal can be said
    /// when it is let go (<see cref="DropAt"/>): a place that answered only with a forbidden pointer is a drop the user
    /// cannot tell from one the drag never reached. A drag of files is not offered, because what the platform's own
    /// refusal means is already the whole answer, and there is nothing this side knows to add.
    /// </para>
    /// <para>
    /// What answers a row's address is found by its prefix and never by reading the place: a pointer moving over rows
    /// must not start a read of each of them.
    /// </para>
    /// </remarks>
    /// <param name="item">The entry the pointer is over, or nothing where it is over the space.</param>
    /// <param name="e">The drag.</param>
    /// <returns>Where it would land, or nothing.</returns>
    public override Land? Landing(LogicalEntry? item, DragEventArgs e)
    {
        if (Dragged(e) is null || e.DataTransfer.Contains(DataFormat.File))
        {
            return null;
        }

        if (item is not null && Target(item.Opens) is { } into)
        {
            return new Land(into.Address.ToString(), item);
        }

        return new Land(_place.Address.ToString(), null);
    }

    /// <summary>
    /// What letting go here would do.
    /// </summary>
    /// <remarks>
    /// Answered as a copy, because what a place does with a drag is the provider's own business: it may keep a
    /// copy, note a name, or do nothing at all, and none of those is a reason for the source to take the
    /// originals away. A source that removed them would be deleting files on the strength of an answer it did not
    /// read.
    /// </remarks>
    /// <param name="land">Where it would land.</param>
    /// <param name="e">The drag.</param>
    /// <returns>What the platform is told.</returns>
    public override DragDropEffects Over(Land land, DragEventArgs e) => DragDropEffects.Copy;

    /// <summary>
    /// Lets a drag go: what it brought is handed to what answers the place it was dropped on.
    /// </summary>
    /// <remarks>
    /// A place that will not take it says why, in its own words where it has any: the drop was offered so that this
    /// moment could be reached, and letting it go with nothing to read is the answer this is here to avoid.
    /// </remarks>
    /// <param name="land">Where it was let go.</param>
    /// <param name="e">The drop.</param>
    public override void DropAt(Land land, DragEventArgs e)
    {
        e.Handled = true;
        Marking?.Invoke(null);

        if (Dragged(e) is not { } drag || !LogicalAddress.TryRead(land.Into, out var address))
        {
            return;
        }

        if (address is null || Answers(address) is not { } provider)
        {
            Refused(null, address, drag);

            return;
        }

        if (!provider.Accepts(address, drag))
        {
            Refused(provider, address, drag);

            return;
        }

        // Recorded as a drop this program answered, so that a source of its own does not take the originals away
        // on the strength of a drop the place has already dealt with.
        Drag.Answered = true;
        provider.Drop(address, drag);
    }

    /// <summary>
    /// Whether an entry may be picked up and dragged away.
    /// </summary>
    /// <remarks>
    /// The entry's own answer, because whether a row is a thing to hand over is what a provider knows: a folder is a
    /// place to go into, and an entry is a thing the work may take.
    /// </remarks>
    /// <param name="item">The entry to consider.</param>
    /// <returns>Whether a drag may begin on it.</returns>
    public override bool CanDrag(LogicalEntry item) => item.Draggable;

    /// <inheritdoc />
    public override void DragOut(
        PointerPressedEventArgs from,
        IReadOnlyList<LogicalEntry> carrying,
        LogicalEntry lead
    )
    {
        // What the pointer took hold of is not part of what a drop is answered with: a place takes what was chosen,
        // and which of the chosen rows the pointer began on says nothing about what to do with the rest.
        _ = lead;

        Drag.Away(from, carrying.Select(Kept).ToArray());
    }

    /// <summary>What one entry's picture is, at the size the view asked for.</summary>
    /// <remarks>
    /// A maker the provider supplied is asked first, because a picture chosen from the entry's name — which is what an
    /// extension is — cannot be looked up by a key. A key is then a picture the library holds, and neither is the folder
    /// the icon set in force draws: a row drawn as nothing is a row a reader has to read rather than see.
    /// </remarks>
    /// <param name="item">The entry to draw.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw where the picture goes.</returns>
    private Control Picture(LogicalEntry item, int size)
    {
        if (item.Picture is { } make)
        {
            return make(size);
        }

        return item.IconKey is { Length: > 0 } key && Host.Icons.Find(key) is { } picture
            ? Icons.Glyph(picture, Icons.PlainInk, size)
            : EntryIcons.Folder(size);
    }

    /// <summary>One contributed menu item, as the menu draws it.</summary>
    /// <param name="item">What the provider registered.</param>
    /// <returns>The item.</returns>
    private MenuItem Item(ContextMenuItem item)
    {
        var menu = new MenuItem { Header = RolaI18N.Get(item.LabelKey) };

        if (item.IconKey is { } key && Host.Icons.Find(key) is { } picture)
        {
            menu.Icon = Icons.Inked(picture, Icons.Menu, typeof(MenuItem));
        }

        var was = new ContextTarget(_place.Address.ToString(), []);
        menu.Click += (_, _) => item.Command(was);

        return menu;
    }

    /// <summary>What a drag carried, where it is one this program's own views started.</summary>
    /// <param name="e">The drag.</param>
    /// <returns>The object the source handed over, or nothing.</returns>
    private static LogicalDrag? Dragged(DragEventArgs e) => e.DataTransfer.TryGetValue(Drag.Payload);

    /// <summary>What one entry carries, which is what a drag and a clipboard keep of it.</summary>
    /// <remarks>
    /// The provider's own object where it kept one, and the name where it did not: a row that leads nowhere is still a
    /// row, and what is carried has to be something the answerer can read.
    /// </remarks>
    /// <param name="item">The entry.</param>
    /// <returns>What to carry or keep.</returns>
    private static object Kept(LogicalEntry item) => item.Data ?? item.Name;

    /// <summary>What a place's entry was copied or cut as, and whether it is on its way out.</summary>
    /// <param name="chosen">What is chosen.</param>
    /// <param name="cut">Whether they are to be moved rather than copied.</param>
    private void Put(IReadOnlyList<LogicalEntry> chosen, bool cut) =>
        _logical?.Put([.. chosen.Select(Kept)], cut);

    /// <summary>
    /// What answers an address, found by its prefix rather than by reading the place.
    /// </summary>
    /// <param name="opens">Where a row leads, or nothing when it leads nowhere.</param>
    /// <returns>The provider and the address, or nothing when no plugin answers it.</returns>
    private (ILogicalViewProvider Provider, LogicalAddress Address)? Target(LogicalAddress? opens)
    {
        if (opens is null || Answers(opens) is not { } provider)
        {
            return null;
        }

        return (provider, opens);
    }

    /// <summary>
    /// What answers one address, which is this place's own answer where the prefix is this place's.
    /// </summary>
    /// <param name="address">The address to answer.</param>
    /// <returns>The provider, or nothing when no plugin claimed the prefix.</returns>
    private ILogicalViewProvider? Answers(LogicalAddress address) =>
        _place.Provider is not null
        && string.Equals(address.Scheme, _place.Address.Scheme, StringComparison.Ordinal)
            ? _place.Provider
            : Host.LogicalViews.Find(address.Scheme);

    /// <summary>One column the provider stated, in the shape the table draws.</summary>
    /// <param name="column">What the provider answered.</param>
    /// <returns>The column to add.</returns>
    private static Column<LogicalEntry> Column(LogicalColumn column) =>
        new(
            column.HeaderKey,
            column.Width,
            column.Value,
            column.Align,
            Placement: column.BesideName
                ? ColumnPlacement.BesideName
                : ColumnPlacement.AfterValues
        );
}

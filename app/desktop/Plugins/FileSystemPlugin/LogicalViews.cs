using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
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
/// </remarks>
internal sealed class Places : Listing<LogicalEntry>
{
    /// <summary>The location, which is how a double-click goes somewhere.</summary>
    private readonly Browser _browser;

    /// <summary>The place being listed, which is the one answer read for one address.</summary>
    private readonly LogicalPlace _place;

    /// <summary>Makes the listing of a place.</summary>
    /// <param name="host">The host, for the library an entry's picture is read from.</param>
    /// <param name="browser">The location, which is how a double-click goes somewhere.</param>
    /// <param name="place">The place to list.</param>
    public Places(IPluginHost host, Browser browser, LogicalPlace place)
        : base(host, place.Entries)
    {
        _browser = browser;
        _place = place;
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

    /// <summary>
    /// Whether a drag is something this place can take.
    /// </summary>
    /// <remarks>
    /// Asked while the pointer is still moving, which is what decides whether a drop is offered at all. The place
    /// takes it and not a row of it: a provider's answer is about the place, and which entry the pointer happens
    /// to be over says nothing about what a file dropped there would mean.
    /// </remarks>
    /// <param name="item">The entry the pointer is over, which a place's answer does not depend on.</param>
    /// <param name="e">The drag.</param>
    /// <returns>Where it would land, or nothing.</returns>
    public override Land? Landing(LogicalEntry? item, DragEventArgs e) =>
        Dragged(e) is { } drag && Logical.Accepts(_place, drag.Data)
            ? new Land(string.Empty, null)
            : null;

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

    /// <summary>Lets a drag go: what it brought is handed to what answers the place.</summary>
    /// <param name="land">Where it was let go.</param>
    /// <param name="e">The drop.</param>
    public override void DropAt(Land land, DragEventArgs e)
    {
        e.Handled = true;
        Marking?.Invoke(null);

        if (Dragged(e) is not { } drag || !Logical.Accepts(_place, drag.Data))
        {
            return;
        }

        // Recorded as a drop this program answered, so that a source of its own does not take the originals away
        // on the strength of a drop the place has already dealt with.
        Drag.Answered = true;
        Logical.Drop(_place, drag.Data);
    }

    /// <summary>
    /// A picture an entry wears, read from the library the provider contributed it to.
    /// </summary>
    /// <remarks>
    /// A key the library does not hold draws an empty cell rather than failing: what a key names is the
    /// contributor's business, and an entry with no picture is an entry, not a listing that will not be drawn.
    /// </remarks>
    /// <param name="item">The entry to draw.</param>
    /// <param name="size">How many pixels wide and tall.</param>
    /// <returns>What to draw where the picture goes.</returns>
    private Control Picture(LogicalEntry item, int size) =>
        item.IconKey is { Length: > 0 } key && Host.Icons.Find(key) is { } picture
            ? Icons.Glyph(picture, Icons.PlainInk, size)
            : new Border { Width = size, Height = size };

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
}

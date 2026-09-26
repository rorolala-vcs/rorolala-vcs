using Avalonia.Controls;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using MenuItem = Avalonia.Controls.MenuItem;

namespace FileSystemPlugin;

/// <summary>
/// The one set of things a view does with entries: opening them, and the menus opened on them.
/// </summary>
/// <remarks>
/// Every view in this plugin does the same things with an entry — the same opening, the same menu, the
/// same clipboard — so they are made once here rather than once per view, which is also what keeps them
/// from drifting apart as one is changed.
/// <para>
/// A menu is about a <em>set</em> of entries rather than one, because a choice can be several: the menu
/// opened on a chosen entry acts on all of them, and one opened on an entry that is not chosen acts on
/// that entry alone. That is what makes a right-click on a selection do something to the selection.
/// </para>
/// </remarks>
internal sealed class BrowserActions
{
    private readonly IPluginHost _host;
    private readonly Browser _browser;
    private readonly Clip _clip;

    /// <summary>Sets up the things a view does, over one host, one location and one clipboard.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">The location the entries belong to.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public BrowserActions(IPluginHost host, Browser browser, Clip clip)
    {
        _host = host;
        _browser = browser;
        _clip = clip;
    }

    /// <summary>Opens an entry, which is what a double-click and <c>Enter</c> do.</summary>
    /// <param name="entry">What to open.</param>
    public void Open(Entry entry) => Open([entry]);

    /// <summary>
    /// Opens entries: the way up goes up, a directory is gone to, a file is handed to the system.
    /// </summary>
    /// <remarks>
    /// The way up is turned into a step here rather than resolved like a path, because it carries no path:
    /// what it leads to is the listing's parent, which only the browser knows.
    /// </remarks>
    /// <param name="entries">What to open, in the order the listing shows.</param>
    public void Open(IReadOnlyList<Entry> entries)
    {
        foreach (var entry in entries)
        {
            if (Browser.IsUp(entry.Path))
            {
                _browser.Up();

                continue;
            }

            Openers.Open(entry, _browser, _host.Log.Error);
        }
    }

    /// <summary>Copies entries to the clipboard.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What is chosen.</param>
    public void Copy(Control from, IReadOnlyList<Entry> entries)
    {
        if (Offered(entries) is { Count: > 0 } files)
        {
            _clip.Copy(from, files, _host.Log.Error);
            _host.Log.Info($"copy: {files.Count} on the clipboard");

            return;
        }

        // Said as well, so that a copy that did nothing says why rather than looking like a key that never
        // arrived (Section 7.7).
        _host.Log.Info("copy: nothing chosen");
    }

    /// <summary>Cuts entries to the clipboard, which a paste then moves.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What is chosen.</param>
    public void Cut(Control from, IReadOnlyList<Entry> entries)
    {
        if (Offered(entries) is { Count: > 0 } files)
        {
            _clip.Cut(from, files, _host.Log.Error);
            _host.Log.Info($"cut: {files.Count} on the clipboard, to be moved");

            return;
        }

        _host.Log.Info("cut: nothing chosen");
    }

    /// <summary>Pastes what is on the clipboard into a directory.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="into">The directory to paste into.</param>
    public void Paste(Control from, string into)
    {
        // The computer is where a drive is chosen from rather than a directory, so it is not a place a
        // paste can land in on any platform.
        if (Browser.IsComputer(into))
        {
            _host.Log.Info("paste: the computer is not a place to paste into");

            return;
        }

        _host.Log.Info($"paste into `{into}`");

        // Every location reads its directory again, and not only this one's: what was pasted went into a
        // directory another dock may be the one looking at, a dock being able to be out of step.
        _clip.Paste(from, into, _host.Log.Error, _browser.Touch);
    }

    /// <summary>Makes the menu opened on a set of entries.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What the menu is about.</param>
    public ContextMenu Menu(Control from, IReadOnlyList<Entry> entries)
    {
        var menu = new ContextMenu();
        Fill(menu, from, entries);

        return menu;
    }

    /// <summary>
    /// Puts the items of the menu for a set of entries into a menu.
    /// </summary>
    /// <remarks>
    /// Filled rather than made, because a view's rows are made once and reused by the toolkit's
    /// virtualisation: the menu has to be about the choice as it stands when it is opened, which is after
    /// the row was made, so a view fills it each time it opens.
    /// </remarks>
    /// <param name="menu">The menu to fill.</param>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    /// <param name="entries">What the menu is about.</param>
    public void Fill(ContextMenu menu, Control from, IReadOnlyList<Entry> entries)
    {
        menu.Items.Clear();

        // The way up is not a place: there is nothing under it to reveal, nothing to put on the clipboard
        // and nothing to root a tree at, so what can be done with it is the one thing it is for.
        if (entries.Count == 1 && Browser.IsUp(entries[0].Path))
        {
            menu.Items.Add(Item("rorolala_file_system.open", () => _browser.Up()));

            return;
        }

        menu.Items.Add(Item("rorolala_file_system.open", () => Open(entries)));

        var files = Offered(entries);

        if (files.Count > 0)
        {
            menu.Items.Add(Item("rorolala_file_system.copy", () => _clip.Copy(from, files, _host.Log.Error)));

            // Cutting the way up or the computer would be cutting a place rather than an item, which is
            // why the extra is asked for here rather than taken from what is simply offered.
            if (files.Count == entries.Count)
            {
                menu.Items.Add(Item("rorolala_file_system.cut", () => _clip.Cut(from, files, _host.Log.Error)));
            }
        }

        // Paste lands in the directory the menu was opened on when it is about exactly one and that one
        // is a directory, which is what right-clicking a folder and pasting into it means; otherwise it
        // lands in the directory being looked at.
        if (Into(entries) is { } into)
        {
            menu.Items.Add(Item("rorolala_file_system.paste", () => Paste(from, into)));
        }

        if (entries.Count == 1)
        {
            var only = entries[0];
            menu.Items.Add(Item("rorolala_file_system.reveal", () => Openers.Reveal(only, _host.Log.Error)));
            menu.Items.Add(Item("rorolala_file_system.copy_path", () => Openers.Copy(from, only.Path, _host.Log.Error)));

            // A directory can be made the base of the tree from wherever it is seen, which is what makes
            // the tree a view of the place being worked in rather than of wherever the browser started.
            if (only.Kind == EntryKind.Directory)
            {
                menu.Items.Add(Item("rorolala_file_system.set_base", () => _browser.SetBase(only.Path)));
            }
        }
        else
        {
            menu.Items.Add(Item("rorolala_file_system.copy_path", () => Openers.Copy(from, Paths(entries), _host.Log.Error)));
        }
    }

    /// <summary>The menu opened on the space around the entries.</summary>
    /// <param name="from">A control in the tree the clipboard is reached through.</param>
    public ContextMenu Empty(Control from)
    {
        var menu = new ContextMenu();
        var here = _browser.Current;

        // The computer is a place to choose a drive from rather than a directory, so there is nothing
        // there to paste into or to reveal; reading it again is the only thing it can be asked.
        if (!Browser.IsComputer(here))
        {
            menu.Items.Add(Item("rorolala_file_system.paste", () => Paste(from, here)));
            menu.Items.Add(Item("rorolala_file_system.reveal", () => Openers.Reveal(new Entry(here, EntryKind.Directory), _host.Log.Error)));
        }

        menu.Items.Add(Item("rorolala_file_system.refresh", _browser.Refresh));

        return menu;
    }

    /// <summary>
    /// The entries a copy or a cut can be about, which is every entry that is a thing rather than a place.
    /// </summary>
    /// <remarks>
    /// The way up and the computer carry no path of their own — the first is the listing's parent and the
    /// second is not a directory at all — so neither can be put on a clipboard or pasted anywhere.
    /// </remarks>
    /// <param name="entries">What is chosen.</param>
    private static List<Entry> Offered(IReadOnlyList<Entry> entries) =>
        [.. entries.Where(entry => !Browser.IsUp(entry.Path) && !Browser.IsComputer(entry.Path))];

    /// <summary>The directory a paste from this menu goes into, or nothing where there is none.</summary>
    /// <param name="entries">What the menu is about.</param>
    private string? Into(IReadOnlyList<Entry> entries)
    {
        if (entries.Count == 1 &&
            entries[0].Kind == EntryKind.Directory &&
            !Browser.IsUp(entries[0].Path) &&
            !Browser.IsComputer(entries[0].Path))
        {
            return entries[0].Path;
        }

        return Browser.IsComputer(_browser.Current) ? null : _browser.Current;
    }

    /// <summary>Paths as the text a copy-path puts on the clipboard: one to a line.</summary>
    /// <param name="entries">What is chosen.</param>
    private static string Paths(IReadOnlyList<Entry> entries) =>
        string.Join(Environment.NewLine, entries.Select(entry => entry.Path));

    /// <summary>
    /// One menu item, named by a translation key.
    /// </summary>
    /// <remarks>
    /// Open to the views as well as to the menus here, so that a view with an item of its own to add —
    /// the tree closes every step from its root — adds one that is named the way the items around it
    /// are named rather than spelling a label out.
    /// </remarks>
    /// <param name="key">The translation key the item is named by.</param>
    /// <param name="action">What choosing it does.</param>
    /// <returns>The item.</returns>
    public static MenuItem Item(string key, Action action)
    {
        var item = new MenuItem { Header = RolaI18N.Get(key) };
        item.Click += (_, _) => action();

        return item;
    }
}

using Avalonia.Controls;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using MenuItem = Avalonia.Controls.MenuItem;

namespace FileSystemPlugin;

/// <summary>
/// What a view does when an entry is opened or a menu is opened on it.
/// </summary>
/// <param name="Activate">What opening an entry does.</param>
/// <param name="Menu">The menu opened on an entry, given the view that opened it.</param>
/// <param name="Empty">The menu opened on the space around the entries, given the view that opened it.</param>
/// <remarks>
/// The view is handed back because one of the things a menu does is put a path on the clipboard, which
/// is reached through a control that is on screen and not through anything the host offers.
/// </remarks>
internal sealed record BrowserActions(
    Action<Entry> Activate,
    Func<Control, Entry, ContextMenu> Menu,
    Func<Control, ContextMenu> Empty
);

/// <summary>
/// The one set of things a view does with an entry.
/// </summary>
/// <remarks>
/// Every view in this plugin does the same things with an entry — the same four items, the same
/// opening, the same setting a directory as the base of the tree — so they are made once here rather
/// than once per view, which is also what keeps them from drifting apart as one is changed.
/// </remarks>
internal static class Actions
{
    /// <summary>Makes what a view does, over one host and one location.</summary>
    /// <param name="host">The host, for what cannot be done.</param>
    /// <param name="browser">The location the entries belong to.</param>
    public static BrowserActions For(IPluginHost host, Browser browser) =>
        new(
            entry => Open(host, browser, entry),
            (from, entry) => Menu(host, browser, from, entry),
            from => Empty(host, browser, from)
        );

    /// <summary>
    /// Opens an entry: the way up goes up, a directory is gone to, a file is handed to the system.
    /// </summary>
    /// <remarks>
    /// The way up is turned into a step here rather than resolved like a path, because it carries no path:
    /// what it leads to is the listing's parent, which only the browser knows.
    /// </remarks>
    private static void Open(IPluginHost host, Browser browser, Entry entry)
    {
        if (Browser.IsUp(entry.Path))
        {
            browser.Up();

            return;
        }

        Openers.Open(entry, browser, host.Log.Error);
    }

    /// <summary>The menu opened on one entry.</summary>
    private static ContextMenu Menu(IPluginHost host, Browser browser, Control from, Entry entry)
    {
        // The way up is not a place: there is nothing under it to reveal, nothing to put on the clipboard
        // and nothing to root a tree at, so what can be done with it is the one thing it is for.
        if (Browser.IsUp(entry.Path))
        {
            var up = new ContextMenu();
            up.Items.Add(Item("rorolala_file_system.open", () => browser.Up()));

            return up;
        }

        var menu = new ContextMenu();
        menu.Items.Add(Item("rorolala_file_system.open", () => Open(host, browser, entry)));
        menu.Items.Add(Item("rorolala_file_system.reveal", () => Openers.Reveal(entry, host.Log.Error)));
        menu.Items.Add(
            Item("rorolala_file_system.copy_path", () => Openers.Copy(from, entry.Path, host.Log.Error))
        );

        // A directory can be made the base of the tree from wherever it is seen, which is what makes
        // the tree a view of the place being worked in rather than of wherever the browser started.
        if (entry.Kind == EntryKind.Directory)
        {
            menu.Items.Add(Item("rorolala_file_system.set_base", () => browser.SetBase(entry.Path)));
        }

        return menu;
    }

    /// <summary>The menu opened on the space around the entries.</summary>
    private static ContextMenu Empty(IPluginHost host, Browser browser, Control from)
    {
        var here = new Entry(browser.Current, EntryKind.Directory);

        var menu = new ContextMenu();
        menu.Items.Add(Item("rorolala_file_system.refresh", browser.Refresh));
        menu.Items.Add(Item("rorolala_file_system.reveal", () => Openers.Reveal(here, host.Log.Error)));

        return menu;
    }

    /// <summary>One menu item, named by a translation key.</summary>
    private static MenuItem Item(string key, Action action)
    {
        var item = new MenuItem { Header = RolaI18N.Get(key) };
        item.Click += (_, _) => action();

        return item;
    }
}

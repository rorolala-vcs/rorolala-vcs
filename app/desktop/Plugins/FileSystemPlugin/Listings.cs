using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Layout;
using Avalonia.Media;
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
    /// a word rather than a piece of one.
    /// </remarks>
    /// <param name="entry">The entry to name.</param>
    /// <returns>The name to show.</returns>
    public static string Show(Entry entry) =>
        Browser.IsComputer(entry.Path)
            ? RolaI18N.Get("rorolala_file_system.computer")
            : Path.GetFileName(entry.Path) is { Length: > 0 } name
                ? name
                : entry.Path;
}

/// <summary>What a listing is: the entries, under the way up when there is one.</summary>
/// <remarks>
/// The way up is the listing's own row rather than an entry, because an entry is a path and a view
/// names it by that path's last part — and the way up is called <c>..</c> whatever it leads to.
/// </remarks>
internal static class Listings
{
    /// <summary>Puts the entries under the way up, if there is one.</summary>
    /// <param name="browser">Where the browser is, which the way up moves.</param>
    /// <param name="entries">The entries themselves.</param>
    /// <returns>What the listing shows.</returns>
    public static Control Stage(Browser browser, Control entries)
    {
        if (browser.Parent is not { } parent)
        {
            return entries;
        }

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(4, 3),
        };

        row.Children.Add(Icons.For(new Entry(parent, EntryKind.Directory)));
        row.Children.Add(
            new TextBlock { Text = "..", VerticalAlignment = VerticalAlignment.Center }
        );

        // Opened by a second tap, like the entries under it, which is what it is one of in all but
        // name.
        row.DoubleTapped += (_, _) => browser.Up();

        var staged = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(row, Dock.Top);
        staged.Children.Add(row);
        staged.Children.Add(entries);

        return staged;
    }
}

/// <summary>The entries as one row each.</summary>
internal sealed class ListBrowser : UserControl
{
    /// <summary>What an entry does when it is opened or right-clicked.</summary>
    private readonly BrowserActions _actions;

    /// <summary>Makes the list layout over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or right-clicked.</param>
    public ListBrowser(Browser browser, BrowserActions actions)
    {
        _actions = actions;

        var list = new ListBox
        {
            ItemsSource = browser.Entries,
            ItemTemplate = new FuncDataTemplate<Entry>((entry, _) => Row(entry), true),
            ContextMenu = actions.Empty(this),
        };

        // Selecting precedes the second tap, so what was activated is what is selected.
        list.DoubleTapped += (_, _) =>
        {
            if (list.SelectedItem is Entry entry)
            {
                actions.Activate(entry);
            }
        };

        Content = Listings.Stage(browser, list);
    }

    /// <summary>One entry as a row: its icon, then its name.</summary>
    private Control Row(Entry entry)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(2),
            ContextMenu = _actions.Menu(this, entry),
        };

        row.Children.Add(Icons.For(entry));
        row.Children.Add(
            new TextBlock
            {
                Text = Names.Show(entry),
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        return row;
    }
}

/// <summary>The entries as tiles, wrapping across the width.</summary>
internal sealed class GridBrowser : UserControl
{
    /// <summary>What an entry does when it is opened or right-clicked.</summary>
    private readonly BrowserActions _actions;

    /// <summary>Makes the grid layout over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or right-clicked.</param>
    public GridBrowser(Browser browser, BrowserActions actions)
    {
        _actions = actions;

        var tiles = new WrapPanel { Orientation = Orientation.Horizontal };

        foreach (var entry in browser.Entries)
        {
            tiles.Children.Add(Tile(entry));
        }

        Content = Listings.Stage(browser, new ScrollViewer { Content = tiles, ContextMenu = actions.Empty(this) });
    }

    /// <summary>One entry as a tile: its icon above its name.</summary>
    private Control Tile(Entry entry)
    {
        var icon = Icons.For(entry);
        icon.HorizontalAlignment = HorizontalAlignment.Center;

        var tile = new StackPanel
        {
            Width = 104,
            Spacing = 4,
            Margin = new Thickness(6),
            ContextMenu = _actions.Menu(this, entry),
        };

        tile.Children.Add(icon);
        tile.Children.Add(
            new TextBlock
            {
                Text = Names.Show(entry),
                TextWrapping = TextWrapping.Wrap,
                TextAlignment = TextAlignment.Center,
                HorizontalAlignment = HorizontalAlignment.Center,
            }
        );

        tile.DoubleTapped += (_, _) => _actions.Activate(entry);

        return tile;
    }
}

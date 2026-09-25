using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>What an entry is called on screen.</summary>
internal static class Names
{
    /// <summary>The last part of an entry's path, or the whole path when there is no last part.</summary>
    /// <param name="entry">The entry to name.</param>
    /// <returns>The name to show.</returns>
    public static string Show(Entry entry) =>
        Path.GetFileName(entry.Path) is { Length: > 0 } name ? name : entry.Path;
}

/// <summary>The entries as one row each.</summary>
internal sealed class ListBrowser : UserControl
{
    /// <summary>Makes the list layout over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is activated or right-clicked.</param>
    public ListBrowser(Browser browser, BrowserActions actions)
    {
        var list = new ListBox
        {
            ItemsSource = browser.Entries,
            ItemTemplate = new FuncDataTemplate<Entry>((entry, _) => Row(entry, actions), true),
            ContextMenu = actions.Empty(),
        };

        // Selecting precedes the second tap, so what was activated is what is selected.
        list.DoubleTapped += (_, _) =>
        {
            if (list.SelectedItem is Entry entry)
            {
                actions.Activate(entry);
            }
        };

        Content = list;
    }

    /// <summary>One entry as a row: its icon, then its name.</summary>
    private static Control Row(Entry entry, BrowserActions actions)
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(2),
            ContextMenu = actions.Menu(entry),
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
    /// <summary>Makes the grid layout over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is activated or right-clicked.</param>
    public GridBrowser(Browser browser, BrowserActions actions)
    {
        var tiles = new WrapPanel { Orientation = Orientation.Horizontal };

        foreach (var entry in browser.Entries)
        {
            tiles.Children.Add(Tile(entry, actions));
        }

        Content = new ScrollViewer { Content = tiles, ContextMenu = actions.Empty() };
    }

    /// <summary>One entry as a tile: its icon above its name.</summary>
    private static Control Tile(Entry entry, BrowserActions actions)
    {
        var icon = Icons.For(entry);
        icon.HorizontalAlignment = HorizontalAlignment.Center;

        var tile = new StackPanel
        {
            Width = 104,
            Spacing = 4,
            Margin = new Thickness(6),
            ContextMenu = actions.Menu(entry),
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

        tile.DoubleTapped += (_, _) => actions.Activate(entry);

        return tile;
    }
}

/// <summary>The directories under the one being looked at, as a tree opened downward.</summary>
/// <remarks>
/// A step is read when it is first opened, so a directory with many directories under it costs a
/// listing only when the user looks there. Whether a step offers an expander at all is settled before
/// that, and costs one entry of the step: a step with nothing under it is given no children, and the
/// base theme draws no chevron for one that has none.
/// </remarks>
internal sealed class TreeBrowser : UserControl
{
    /// <summary>Makes the tree layout rooted at the directory being looked at.</summary>
    /// <param name="browser">Where the browser is, which the tree reads and switches.</param>
    /// <param name="actions">What a directory does when it is chosen or right-clicked.</param>
    public TreeBrowser(Browser browser, BrowserActions actions)
    {
        var tree = new TreeView { ContextMenu = actions.Empty() };
        tree.Items.Add(Node(browser, browser.Current, actions));

        Content = tree;
    }

    /// <summary>One directory as a step of the tree, reading its children when first opened.</summary>
    private static TreeViewItem Node(Browser browser, string path, BrowserActions actions)
    {
        var item = new TreeViewItem { Header = Header(browser, path, actions) };
        var read = false;

        // A step with nothing under it is given no child at all, and that is what leaves it without an
        // expander: one that cannot be opened must not be offered as though it could. The step that
        // has something under it gets a child that is never shown, so that it can be opened at all;
        // it is replaced the first time the step is.
        if (HoldsAny(path))
        {
            item.Items.Add(new TreeViewItem { Header = "\u2026" });
        }

        item.Expanded += (_, _) =>
        {
            if (read)
            {
                return;
            }

            read = true;
            item.Items.Clear();

            foreach (var child in Subdirectories(path))
            {
                item.Items.Add(Node(browser, child, actions));
            }
        };

        return item;
    }

    /// <summary>
    /// One directory's header: its icon and its name, and what choosing it does.
    /// </summary>
    /// <remarks>
    /// A click switches the location there and then, rather than waiting for a second one: the tree
    /// is nothing but directories, so choosing one can only mean going to it, and the address says so
    /// the moment it happens. A step is opened by its own chevron, so opening one does not come
    /// through here.
    /// </remarks>
    private static Control Header(Browser browser, string path, BrowserActions actions)
    {
        var entry = new Entry(path, EntryKind.Directory);
        var name = Names.Show(entry);

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            ContextMenu = actions.Menu(entry),
        };

        row.Children.Add(Icons.For(entry));
        row.Children.Add(
            new TextBlock
            {
                Text = name.Length > 0 ? name : path,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        row.Tapped += (_, _) => browser.Go(path);

        return row;
    }

    /// <summary>Whether a directory holds any directory at all, or nothing when it cannot be read.</summary>
    /// <remarks>
    /// Read to the first entry rather than counted to the last: the whole of a large directory is not
    /// worth reading to answer what one entry already answers.
    /// </remarks>
    private static bool HoldsAny(string path)
    {
        try
        {
            return System.IO.Directory.EnumerateDirectories(path).Any();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    /// <summary>The directories directly under one, by name, or nothing when it cannot be read.</summary>
    private static IReadOnlyList<string> Subdirectories(string path)
    {
        try
        {
            return System.IO.Directory
                .EnumerateDirectories(path)
                .OrderBy(directory => Path.GetFileName(directory), StringComparer.OrdinalIgnoreCase)
                .ToArray();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }
}

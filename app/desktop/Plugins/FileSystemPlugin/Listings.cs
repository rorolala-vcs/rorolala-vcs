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
            ItemsSource = browser.Shown,
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

        Content = list;
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
    /// <summary>How much room is left around a tile's icon and name.</summary>
    private const int Around = 10;

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

    /// <summary>What an entry does when it is opened or right-clicked.</summary>
    private readonly BrowserActions _actions;

    /// <summary>How many pixels wide and tall a tile's icon is, and so how wide its name is too.</summary>
    private readonly int _icon;

    /// <summary>Makes the grid layout over what the browser holds.</summary>
    /// <param name="browser">What is being shown.</param>
    /// <param name="actions">What an entry does when it is opened or right-clicked.</param>
    /// <param name="icon">How large an icon is at the zoom this dock is at.</param>
    public GridBrowser(Browser browser, BrowserActions actions, int icon)
    {
        _actions = actions;
        _icon = icon;

        var tiles = new WrapPanel { Orientation = Orientation.Horizontal };

        foreach (var entry in browser.Shown)
        {
            tiles.Children.Add(Tile(entry));
        }

        Content = new ScrollViewer { Content = tiles, ContextMenu = actions.Empty(this) };
    }

    /// <summary>One entry as a tile: its icon above its name, on one line and cut off when it is too long.</summary>
    /// <remarks>
    /// The name is as wide as the icon and not one pixel wider, so that the two read as one column rather
    /// than as a picture with a caption under it that happens to start somewhere else. What does not fit is
    /// taken off the end rather than wrapped, because a tile of two lines is a tile of another height, and a
    /// row of tiles that are not the same height is not a row.
    /// </remarks>
    private Control Tile(Entry entry)
    {
        var tile = new Border
        {
            Padding = new Thickness(Around),
            Margin = new Thickness(4),
            ContextMenu = _actions.Menu(this, entry),
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
        tile.DoubleTapped += (_, _) => _actions.Activate(entry);

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

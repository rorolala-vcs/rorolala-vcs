using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using MenuItem = Avalonia.Controls.MenuItem;

namespace FileSystemPlugin;

/// <summary>The File System dock: the browser, as the host shows it.</summary>
internal sealed class BrowserDock : IDockView
{
    /// <summary>Makes a browser dock for one plain placement.</summary>
    /// <param name="host">The host, for logging what the browser cannot do.</param>
    /// <param name="browser">The one location, which this dock is a view onto.</param>
    public BrowserDock(IPluginHost host, Browser browser) => View = new BrowserControl(host, browser);

    /// <inheritdoc />
    public Control View { get; }

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>What a layout does when an entry is activated or a menu is opened on it.</summary>
/// <param name="Activate">What double-clicking an entry does.</param>
/// <param name="Menu">The menu opened on an entry.</param>
/// <param name="Empty">The menu opened on the space around the entries.</param>
internal sealed record BrowserActions(
    Action<Entry> Activate,
    Func<Entry, ContextMenu> Menu,
    Func<ContextMenu> Empty
);

/// <summary>
/// The browser's control: the location, laid out one of the three ways.
/// </summary>
/// <remarks>
/// Navigation is not here: back, forward, up, refresh and the address are a dock of their own
/// (Section 7.5). What is here is the layout, and the layout is the one thing about a browser dock
/// that is its own — where it is looking is the same wherever else it is looked at.
/// </remarks>
internal sealed class BrowserControl : UserControl
{
    /// <summary>The layouts an entry set can be read in, with the key each is named by.</summary>
    private static readonly (BrowserView View, string Key)[] Views =
    [
        (BrowserView.List, "rorolala_file_system.layout_list"),
        (BrowserView.Grid, "rorolala_file_system.layout_grid"),
        (BrowserView.Tree, "rorolala_file_system.layout_tree"),
    ];

    /// <summary>The host, which is where a failure the browser cannot handle is reported.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where the browser is, which every dock looks at the same one of.</summary>
    private readonly Browser _browser;

    /// <summary>What the layouts do, shared by all three so they behave alike.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The layout this dock reads the entries in.</summary>
    private BrowserView _view = BrowserView.List;

    /// <summary>The view the entries are read in.</summary>
    private readonly ComboBox _views = new();

    /// <summary>Whichever layout is being shown.</summary>
    private readonly ContentControl _content = new();

    /// <summary>Makes the browser's control.</summary>
    /// <param name="host">The host, for logging what the browser cannot do.</param>
    /// <param name="browser">The one location, which this dock is a view onto.</param>
    public BrowserControl(IPluginHost host, Browser browser)
    {
        _host = host;
        _browser = browser;
        _actions = new BrowserActions(Activate, MenuFor, EmptyMenu);

        // Listening while it is on screen rather than for as long as it exists: a dock that was
        // closed is not a view of anything, and one that was dragged to another region is taken off
        // the tree and put back, which is a departure and a return for a dock that never closed.
        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            Update();
        };
        DetachedFromVisualTree += (_, _) => _browser.Changed -= Update;

        FillViews();

        // Over the entries rather than under them, and to the right: it reads as the view of the pane
        // it sits in, and it stays out of the way of the toolbar above, which is another dock.
        var bar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(4),
        };
        bar.Children.Add(_views);

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(bar, Dock.Top);
        panel.Children.Add(bar);
        panel.Children.Add(_content);

        Content = panel;

        Update();
    }

    /// <summary>Fills the view switch with the three layouts.</summary>
    private void FillViews()
    {
        foreach (var (view, key) in Views)
        {
            _views.Items.Add(new ComboBoxItem { Content = RolaI18N.Get(key), Tag = view });
        }

        _views.SelectionChanged += (_, _) =>
        {
            // Choosing a layout draws the same entries again, which is a redraw and not a change of
            // where the browser is: the location is one, and every view of it shows the same thing.
            if (_views.SelectedItem is ComboBoxItem { Tag: BrowserView view } && view != _view)
            {
                _view = view;
                Update();
            }
        };
    }

    /// <summary>Draws what is there, the way this dock reads it.</summary>
    private void Update()
    {
        _content.Content = ContentFor(_view);
        _views.SelectedIndex = Array.FindIndex(Views, entry => entry.View == _view);
    }

    /// <summary>The control showing the entries in one layout.</summary>
    private Control ContentFor(BrowserView view) =>
        view switch
        {
            BrowserView.Grid => new GridBrowser(_browser, _actions),
            BrowserView.Tree => new TreeBrowser(_browser, _actions),
            _ => new ListBrowser(_browser, _actions),
        };

    /// <summary>Opens whatever was activated.</summary>
    private void Activate(Entry entry) => Openers.Open(entry, _browser, _host.Log.Error);

    /// <summary>The menu opened on one entry.</summary>
    private ContextMenu MenuFor(Entry entry)
    {
        var menu = new ContextMenu();
        menu.Items.Add(Item("rorolala_file_system.open", () => Activate(entry)));
        menu.Items.Add(Item("rorolala_file_system.reveal", () => Openers.Reveal(entry, _host.Log.Error)));
        menu.Items.Add(
            Item("rorolala_file_system.copy_path", () => Openers.Copy(this, entry.Path, _host.Log.Error))
        );

        return menu;
    }

    /// <summary>The menu opened on the space around the entries.</summary>
    private ContextMenu EmptyMenu()
    {
        var here = new Entry(_browser.Current, EntryKind.Directory);

        var menu = new ContextMenu();
        menu.Items.Add(Item("rorolala_file_system.refresh", _browser.Refresh));
        menu.Items.Add(Item("rorolala_file_system.reveal", () => Openers.Reveal(here, _host.Log.Error)));

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

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
    /// <param name="navigator">The browsers, which this one joins while it is shown.</param>
    public BrowserDock(IPluginHost host, Navigator navigator) =>
        View = new BrowserControl(host, navigator);

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
/// The browser's control: one directory, shown in one of the three layouts.
/// </summary>
/// <remarks>
/// Navigation is not here: back, forward, up, refresh and the address are a dock of their own
/// (Section 7.5), so that a browser keeps to what a browser is — where it has been, and what is
/// there. The view switch is here, because which layout entries are read in is the browser's own.
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

    /// <summary>Where the browser is and has been.</summary>
    private readonly Browser _browser;

    /// <summary>What the layouts do, shared by all three so they behave alike.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The view the entries are read in.</summary>
    private readonly ComboBox _views = new();

    /// <summary>Whichever layout is being shown.</summary>
    private readonly ContentControl _content = new();

    /// <summary>Makes the browser's control.</summary>
    /// <param name="host">The host, for logging what the browser cannot do.</param>
    /// <param name="navigator">The browsers, which this one joins while it is shown.</param>
    public BrowserControl(IPluginHost host, Navigator navigator)
    {
        _host = host;
        _browser = new Browser(Start());
        _actions = new BrowserActions(Activate, MenuFor, EmptyMenu);

        // Joining and leaving on the visual tree rather than in the constructor and on a close: a
        // dock dragged to another region is taken out and put back, which is a departure and a
        // return for a dock that never closed.
        AttachedToVisualTree += (_, _) => navigator.Add(_browser);
        DetachedFromVisualTree += (_, _) => navigator.Remove(_browser);

        // The last browser the user reached into is the one the navigation dock drives, and reaching
        // into one is focusing anything in it.
        GotFocus += (_, _) => navigator.Activate(_browser);

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

        _browser.Changed += Update;
        Update();
    }

    /// <summary>The directory to show when a dock is opened with nothing to say.</summary>
    /// <remarks>
    /// Where the program was started, which is where a run was made, falling back to the user's own
    /// directory when that is not somewhere that can be read.
    /// </remarks>
    private static string Start()
    {
        var here = Environment.CurrentDirectory;

        return System.IO.Directory.Exists(here)
            ? here
            : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
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
            // Choosing a layout is what changes it; being shown the browser's own layout is not a
            // choice, and acting on it would put the browser into a loop of showing itself.
            if (_views.SelectedItem is ComboBoxItem { Tag: BrowserView view } && view != _browser.View)
            {
                _browser.Show(view);
            }
        };
    }

    /// <summary>Shows the layout the browser is in.</summary>
    private void Update()
    {
        _content.Content = ContentFor(_browser.View);
        _views.SelectedIndex = Array.FindIndex(Views, entry => entry.View == _browser.View);
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

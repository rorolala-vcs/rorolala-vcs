using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
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
    public BrowserDock(IPluginHost host) => View = new BrowserControl(host);

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
/// The browser's control: a toolbar over one of the three layouts.
/// </summary>
/// <remarks>
/// The toolbar is the browser's own, not a host region: where the browser has been, and where it
/// goes next, is the browser's business, and the host has no idea what a directory is.
/// </remarks>
internal sealed class BrowserControl : UserControl
{
    /// <summary>The host, which is where a failure the browser cannot handle is reported.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where the browser is and has been.</summary>
    private readonly Browser _browser;

    /// <summary>What the layouts do, shared by all three so they behave alike.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The path being shown, and the place to type another.</summary>
    private readonly TextBox _address = new();

    private readonly Button _back = Arrow("\u2190");
    private readonly Button _forward = Arrow("\u2192");
    private readonly Button _up = Arrow("\u2191");
    private readonly Button _refresh = Arrow("\u27f3");
    private readonly ComboBox _layouts = new();
    private readonly ContentControl _content = new();

    /// <summary>Makes the browser's control.</summary>
    /// <param name="host">The host, for logging what the browser cannot do.</param>
    public BrowserControl(IPluginHost host)
    {
        _host = host;
        _browser = new Browser(Start());
        _actions = new BrowserActions(Activate, MenuFor, EmptyMenu);

        _back.Click += (_, _) => _browser.Back();
        _forward.Click += (_, _) => _browser.Forward();
        _up.Click += (_, _) => _browser.Up();
        _refresh.Click += (_, _) => _browser.Refresh();

        _address.Width = 320;
        _address.KeyDown += (_, args) =>
        {
            if (args.Key == Key.Enter)
            {
                Go(_address.Text);
            }
        };
        _address.LostFocus += (_, _) => _address.Text = _browser.Current;

        Layouts();

        var toolbar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            Margin = new Thickness(6),
        };
        toolbar.Children.Add(_back);
        toolbar.Children.Add(_forward);
        toolbar.Children.Add(_up);
        toolbar.Children.Add(_refresh);
        toolbar.Children.Add(_address);
        toolbar.Children.Add(_layouts);

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(toolbar, Dock.Top);
        panel.Children.Add(toolbar);
        panel.Children.Add(_content);

        Content = panel;

        _browser.Changed += Update;
        Update();
    }

    /// <summary>The browser's toolbar buttons: a glyph, since they are arrows and a cycle.</summary>
    private static Button Arrow(string glyph) =>
        new()
        {
            Content = glyph,
            Padding = new Thickness(8, 2),
            MinWidth = 30,
        };

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

    /// <summary>Fills the layout switch with the three layouts.</summary>
    private void Layouts()
    {
        foreach (
            var (view, key) in new (BrowserView View, string Key)[]
            {
                (BrowserView.List, "rorolala_file_system.layout_list"),
                (BrowserView.Grid, "rorolala_file_system.layout_grid"),
                (BrowserView.Tree, "rorolala_file_system.layout_tree"),
            }
        )
        {
            _layouts.Items.Add(new ComboBoxItem { Content = RolaI18N.Get(key), Tag = view });
        }

        _layouts.SelectedIndex = 0;
        _layouts.SelectionChanged += (_, _) =>
        {
            if (_layouts.SelectedItem is ComboBoxItem { Tag: BrowserView view })
            {
                _browser.Show(view);
            }
        };
    }

    /// <summary>Brings the toolbar and the content back in step with the browser.</summary>
    private void Update()
    {
        _back.IsEnabled = _browser.CanGoBack;
        _forward.IsEnabled = _browser.CanGoForward;
        _up.IsEnabled = _browser.CanGoUp;
        _address.Text = _browser.Current;
        _content.Content = ContentFor(_browser.View);
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

    /// <summary>Goes to a directory typed into the address bar.</summary>
    private void Go(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            _address.Text = _browser.Current;

            return;
        }

        if (!System.IO.Directory.Exists(path))
        {
            _host.Log.Warn(RolaI18N.Get("rorolala_file_system.not_a_directory", path));
            _address.Text = _browser.Current;

            return;
        }

        _browser.Go(path);
    }
}

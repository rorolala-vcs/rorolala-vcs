using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>The Directory dock: one directory's entries, one of two ways.</summary>
internal sealed class DirectoryDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly DirectoryControl _view;

    /// <summary>Makes a directory dock.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="browser">The one location, which this dock is a view of.</param>
    public DirectoryDock(IPluginHost host, Browser browser) =>
        _view = new DirectoryControl(host, browser);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];

    /// <inheritdoc />
    public void Restored(IDockState state) => _view.Restored(state);
}

/// <summary>
/// The directory's control: the location's entries, as a list or a grid.
/// </summary>
/// <remarks>
/// Neither navigation nor the tree is here: back, forward, up, refresh and the address are a dock of
/// their own, and so is the tree (Section 7.5). What is here is one directory, read one of two ways,
/// which is the one thing about this dock that is its own — where it is looking is the same wherever
/// else it is looked at.
/// </remarks>
internal sealed class DirectoryControl : UserControl
{
    /// <summary>The layouts a directory is read in, with the key each is named by.</summary>
    private static readonly (BrowserView View, string Key)[] Views =
    [
        (BrowserView.List, "rorolala_file_system.layout_list"),
        (BrowserView.Grid, "rorolala_file_system.layout_grid"),
    ];

    /// <summary>The dock key the layout is kept under.</summary>
    private const string LayoutKey = "layout";

    /// <summary>What the layout is kept as when it is a grid, and when it is anything else.</summary>
    private const string Grid = "grid";

    /// <summary>Where the browser is, which every dock looks at the same one of.</summary>
    private readonly Browser _browser;

    /// <summary>What the views do, so that every view of the location behaves alike.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The layout this dock reads the entries in.</summary>
    private BrowserView _view = BrowserView.List;

    /// <summary>What the dock kept, and where the next of it is kept.</summary>
    private IDockState? _state;

    /// <summary>The entries as this dock last drew them, so that a reread can be told from a step.</summary>
    private IReadOnlyList<Entry>? _drawn;

    /// <summary>The view the entries are read in.</summary>
    private readonly ComboBox _views = new();

    /// <summary>Whichever layout is being shown.</summary>
    private readonly ContentControl _content = new();

    /// <summary>Makes the directory's control.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="browser">The one location, which this dock is a view of.</param>
    public DirectoryControl(IPluginHost host, Browser browser)
    {
        _browser = browser;
        _actions = Actions.For(host, browser);

        // Listening while it is on screen rather than for as long as it exists: a dock that was
        // closed is not a view of anything, and one that was dragged to another region is taken off
        // the tree and put back, which is a departure and a return for a dock that never closed.
        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            Draw();
        };
        DetachedFromVisualTree += (_, _) => _browser.Changed -= Update;

        FillViews();

        // Over the entries rather than under them: it reads as the view of the pane it sits in, and it
        // stays out of the way of the toolbar above, which is another dock.
        var bar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(4),
        };
        bar.Children.Add(_views);

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(bar, Dock.Top);
        panel.Children.Add(bar);
        panel.Children.Add(_content);

        Content = panel;

        Draw();
    }

    /// <summary>
    /// Takes what this dock was, which is the layout it was read in.
    /// </summary>
    /// <remarks>
    /// Read before the dock is drawn, so that a directory that was a grid comes back a grid rather
    /// than being drawn a list and changing under the first frame. The host makes the view and then
    /// tells it what it was, which is why there is anything to do here at all.
    /// </remarks>
    /// <param name="state">What the dock kept, and where the next of it is kept.</param>
    public void Restored(IDockState state)
    {
        _state = state;

        if (state.Read(LayoutKey) == Grid)
        {
            _view = BrowserView.Grid;
        }

        Draw();
    }

    /// <summary>Fills the view switch with the two layouts a directory is read in.</summary>
    private void FillViews()
    {
        foreach (var (view, key) in Views)
        {
            _views.Items.Add(new ComboBoxItem { Content = RolaI18N.Get(key), Tag = view });
        }

        _views.SelectionChanged += (_, _) =>
        {
            if (_views.SelectedItem is ComboBoxItem { Tag: BrowserView view } && view != _view)
            {
                _view = view;

                // Written rather than saved: the dock keeps what it was and the host writes the
                // layout, which is the same division as everything else about where a dock is.
                _state?.Write(LayoutKey, view == BrowserView.Grid ? Grid : "list");
                Draw();
            }
        };
    }

    /// <summary>Draws the entries again when they are not the ones it drew.</summary>
    /// <remarks>
    /// Every read of a directory is a new list, so one list being another is what a step, a refresh
    /// and a change of base all look like from here.
    /// </remarks>
    private void Update()
    {
        if (!ReferenceEquals(_drawn, _browser.Entries))
        {
            Draw();
        }
    }

    /// <summary>Draws what is there, the way this dock reads it.</summary>
    private void Draw()
    {
        _content.Content = ContentFor(_view);
        _views.SelectedIndex = Array.FindIndex(Views, entry => entry.View == _view);
        _drawn = _browser.Entries;
    }

    /// <summary>The control showing the entries in one layout.</summary>
    private Control ContentFor(BrowserView view) =>
        view == BrowserView.Grid
            ? new GridBrowser(_browser, _actions)
            : new ListBrowser(_browser, _actions);
}

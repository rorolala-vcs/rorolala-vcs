using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>
/// The File Navigation dock: the directories under the base, as a tree.
/// </summary>
/// <remarks>
/// It is a dock of its own rather than a layout the directory's dock can be put into (Section 7.5).
/// A tree is not another way of reading one directory; it is a way of walking the ones under a place,
/// and it is the only view that is rooted somewhere the location is not — so it is not rearranged by
/// stepping through directories, and it is worth having on screen beside one that is.
/// </remarks>
internal sealed class TreeDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly TreeControl _view;

    /// <summary>Makes the file navigation dock.</summary>
    /// <param name="host">The host, for logging what the tree cannot do.</param>
    /// <param name="browser">The one location and base, which the tree is rooted at.</param>
    public TreeDock(IPluginHost host, Browser browser) =>
        _view = new TreeControl(browser, Actions.For(host, browser));

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>
/// The tree's control: the base, and everything under it.
/// </summary>
/// <remarks>
/// The tree is rooted at the base rather than at the location, which is what a base is for: what a
/// user has opened stays open while the location moves under it, and a step is read when it is opened
/// so that a directory with many directories under it costs a listing only when it is looked at.
/// <para>
/// One button sits over it, which takes the root to the top of the platform — the drive list on
/// Windows, the root on Unix — and roots the tree there: the way back to a place you can start
/// choosing a new base from.
/// </para>
/// </remarks>
internal sealed class TreeControl : UserControl
{
    /// <summary>Where the browser is, and where its base is.</summary>
    private readonly Browser _browser;

    /// <summary>What the rows do, which is what every other view of the location does.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The button that roots the tree at the top of the platform.</summary>
    private readonly Button _root = new();

    /// <summary>The tree, and nothing else, in the one cell under the bar.</summary>
    private readonly ContentControl _content = new();

    /// <summary>The base the tree was last rooted at, so that rooting it again can be skipped.</summary>
    private string? _drawn;

    /// <summary>Makes the tree's control.</summary>
    /// <param name="browser">The one location and base, which the tree is rooted at.</param>
    /// <param name="actions">What the rows do.</param>
    public TreeControl(Browser browser, BrowserActions actions)
    {
        _browser = browser;
        _actions = actions;

        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            Draw();
        };
        DetachedFromVisualTree += (_, _) => _browser.Changed -= Update;

        _root.Content = RolaI18N.Get("rorolala_file_system.root");
        _root.Click += (_, _) => _browser.SetBase(Browser.Root());

        var bar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            Margin = new Thickness(4),
        };
        bar.Children.Add(_root);

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(bar, Dock.Top);
        panel.Children.Add(bar);
        panel.Children.Add(_content);

        Content = panel;

        Draw();
    }

    /// <summary>Roots the tree again when the base is not the one it is rooted at.</summary>
    private void Update()
    {
        if (_drawn != _browser.BaseDir)
        {
            Draw();
        }
    }

    /// <summary>Builds the tree over the base.</summary>
    private void Draw()
    {
        _content.Content = new TreeBrowser(_browser, _browser.BaseDir, _actions);
        _drawn = _browser.BaseDir;
    }
}

/// <summary>The directories under one directory, as a tree opened downward.</summary>
/// <remarks>
/// A step is read when it is first opened, so a directory with many directories under it costs a
/// listing only when the user looks there. Whether a step offers an expander at all is settled before
/// that, and costs one entry of the step: a step with nothing under it is given no children, and the
/// base theme draws no chevron for one that has none.
/// </remarks>
internal sealed class TreeBrowser : UserControl
{
    /// <summary>Where the browser is, which the tree reads and switches.</summary>
    private readonly Browser _browser;

    /// <summary>What the rows do, which is what every other view of the location does.</summary>
    private readonly BrowserActions _actions;

    /// <summary>Makes the tree rooted at a directory.</summary>
    /// <param name="browser">Where the browser is, which the tree reads and switches.</param>
    /// <param name="root">The directory the tree is rooted at.</param>
    /// <param name="actions">What a directory does when it is chosen or right-clicked.</param>
    public TreeBrowser(Browser browser, string root, BrowserActions actions)
    {
        _browser = browser;
        _actions = actions;

        var tree = new TreeView { ContextMenu = actions.Empty(this) };
        tree.Items.Add(Node(root));

        Content = tree;
    }

    /// <summary>One directory as a step of the tree, reading its children when first opened.</summary>
    private TreeViewItem Node(string path)
    {
        var item = new TreeViewItem { Header = Header(path) };
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

            foreach (var child in Children(path))
            {
                item.Items.Add(Node(child));
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
    private Control Header(string path)
    {
        var entry = new Entry(path, EntryKind.Directory);
        var name = Names.Show(entry);

        // The menu is opened by the control the row is in rather than by the row, since one of the
        // things it does — putting a path on the clipboard — is reached through a control on screen.
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            ContextMenu = _actions.Menu(this, entry),
        };

        row.Children.Add(Icons.For(entry));
        row.Children.Add(
            new TextBlock
            {
                Text = name.Length > 0 ? name : path,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        row.Tapped += (_, _) => _browser.Go(path);

        return row;
    }

    /// <summary>Whether a directory holds any directory at all, or nothing when it cannot be read.</summary>
    /// <remarks>
    /// Read to the first entry rather than counted to the last: the whole of a large directory is not
    /// worth reading to answer what one entry already answers. The computer is the exception, holding
    /// nothing but the few drives, which are read outright.
    /// </remarks>
    private static bool HoldsAny(string path)
    {
        if (Browser.IsComputer(path))
        {
            return Browser.Read(path).Count > 0;
        }

        try
        {
            return System.IO.Directory.EnumerateDirectories(path).Any();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    /// <summary>The directories directly under one, by name, or the drives when it is the computer.</summary>
    private static IReadOnlyList<string> Children(string path) =>
        Browser.IsComputer(path)
            ? Browser.Read(path).Select(entry => entry.Path).ToArray()
            : Subdirectories(path);

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

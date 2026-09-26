using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.VisualTree;
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
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public TreeDock(IPluginHost host, Browser browser, Clip clip) =>
        _view = new TreeControl(host, browser, new BrowserActions(host, browser, clip));

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

    /// <summary>The host, which the tree is made over.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the rows do, which is what every other view of the location does.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The button that roots the tree at the top of the platform.</summary>
    private readonly Button _root = new() { Classes = { "ghost" } };

    /// <summary>The tree, and nothing else, in the one cell under the bar.</summary>
    private readonly ContentControl _content = new();

    /// <summary>The base the tree was last rooted at, so that rooting it again can be skipped.</summary>
    private string? _drawn;

    /// <summary>Makes the tree's control.</summary>
    /// <param name="host">The host, for what a drop cannot do.</param>
    /// <param name="browser">The one location and base, which the tree is rooted at.</param>
    /// <param name="actions">What the rows do.</param>
    public TreeControl(IPluginHost host, Browser browser, BrowserActions actions)
    {
        _host = host;
        _browser = browser;
        _actions = actions;

        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            Draw();
        };
        DetachedFromVisualTree += (_, _) => _browser.Changed -= Update;

        _root.Content = RolaI18N.Get("rorolala_file_system.root");
        _root.Padding = new Thickness(7, 2);
        _root.Click += (_, _) => _browser.SetBase(Browser.Root());

        var bar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(8, 8, 8, 4),
        };
        bar.Children.Add(_root);

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(bar, Dock.Top);
        panel.Children.Add(bar);
        panel.Children.Add(_content);

        // The sidebar ground: the tree sits on the elevated surface with one edge against the dock beside
        // it, which is what reads as the card the entries are on rather than as a bare pane.
        var ground = new Border
        {
            Child = panel,
            BorderThickness = new Thickness(0, 0, 1, 0),
        };
        ground[!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.elevated");
        ground[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border");

        Content = ground;

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
        _content.Content = new TreeBrowser(_host, _browser, _browser.BaseDir, _actions);
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

    /// <summary>How a drag is taken here: which step it would land in, and which row is lit.</summary>
    private readonly Drops _drops;

    /// <summary>The card that follows a drag, drawn over the tree while the pointer is in it.</summary>
    private readonly Ghost _ghost;

    /// <summary>Every step on screen, so that the one a drag is over can be lit.</summary>
    /// <remarks>
    /// A step is recorded when it is made and never taken out: a tree read a step at a time keeps the ones it
    /// has read, closed or not, so a row that was recorded is still the row the tree draws — and what is read
    /// is bounded by what a user opened.
    /// </remarks>
    private readonly List<(string Path, Border Row)> _rows = [];

    /// <summary>The row wearing the drop mark, so that it can be taken off again.</summary>
    private Border? _marked;

    /// <summary>Makes the tree rooted at a directory.</summary>
    /// <param name="host">The host, for what a drop cannot do.</param>
    /// <param name="browser">Where the browser is, which the tree reads and switches.</param>
    /// <param name="root">The directory the tree is rooted at.</param>
    /// <param name="actions">What a directory does when it is chosen or right-clicked.</param>
    public TreeBrowser(IPluginHost host, Browser browser, string root, BrowserActions actions)
    {
        _browser = browser;
        _actions = actions;
        _drops = new Drops(host, browser, Landing, Mark);

        var tree = new TreeView { ContextMenu = actions.Empty(this) };
        tree.Items.Add(Node(root));

        // A drag is taken over the whole tree and not over the rows alone: the space beside and below them is
        // not a directory, and a drag let go there is refused by saying so rather than by saying nothing.
        DragDrop.SetAllowDrop(this, true);
        AddHandler(DragDrop.DragEnterEvent, DraggedOver);
        AddHandler(DragDrop.DragOverEvent, DraggedOver);
        AddHandler(DragDrop.DragLeaveEvent, DraggedOff);
        AddHandler(DragDrop.DropEvent, Dropped);

        var over = new Canvas { IsHitTestVisible = false };
        _ghost = new Ghost(over);

        var grid = new Grid();
        grid.Children.Add(tree);

        // The base is a row of the tree whether or not it holds anything under it, so a base that holds
        // nothing still shows that one row; the word says so rather than leaving the pane reading as a
        // tree that has not finished loading.
        if (!HoldsAny(root))
        {
            grid.Children.Add(Empty());
        }

        grid.Children.Add(over);

        Content = grid;
    }

    /// <summary>Says whether a drag may land here, and lights the step it would land in.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOver(object? sender, DragEventArgs e)
    {
        _ghost.Following(e.GetPosition(this));

        e.DragEffects = _drops.Over(e);
        e.Handled = e.DragEffects != DragDropEffects.None;
    }

    /// <summary>Takes the drop mark off when a drag leaves, and the card with it.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The drag.</param>
    private void DraggedOff(object? sender, DragEventArgs e)
    {
        _drops.Off();
        _ghost.Unghost();
    }

    /// <summary>Lets a drag go into the step it was over, and takes the card off.</summary>
    /// <param name="sender">The tree.</param>
    /// <param name="e">The drop.</param>
    private void Dropped(object? sender, DragEventArgs e)
    {
        _ghost.Unghost();
        _drops.Dropped(e);
    }

    /// <summary>
    /// Where a drag would land: the directory of the step it is over.
    /// </summary>
    /// <remarks>
    /// The step itself and not the space around it — a tree holds the steps a user opened, and the space beside
    /// and below them names nothing to go into, so a drag let go there is let go nowhere rather than being read
    /// as the base. The computer is a place to choose a drive from rather than a directory, so a step standing
    /// for it is not somewhere a drag can go either.
    /// </remarks>
    /// <param name="e">The drag.</param>
    private Land? Landing(DragEventArgs e)
    {
        if (!e.DataTransfer.Contains(DataFormat.File))
        {
            return null;
        }

        var at = e.GetPosition(this);

        for (var visual = this.GetVisualAt(at) as Visual; visual is not null; visual = visual.GetVisualParent())
        {
            if (visual is TreeViewItem item && item.Tag is string path)
            {
                return Browser.IsComputer(path) ? null : new Land(path, path);
            }
        }

        return null;
    }

    /// <summary>Paints the row standing for a directory, or takes the paint off.</summary>
    /// <param name="directory">The directory being pointed at, or nothing.</param>
    private void Mark(string? directory)
    {
        Unmark();

        if (directory is null)
        {
            return;
        }

        foreach (var (path, row) in _rows)
        {
            if (!string.Equals(path, directory, StringComparison.Ordinal))
            {
                continue;
            }

            // Bound rather than read, so that the mark follows a colour changed while the program runs, the way
            // the band a frame is drawn with does.
            row[!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.selection");
            _marked = row;

            return;
        }
    }

    /// <summary>Takes the drop mark off whatever was wearing it.</summary>
    private void Unmark()
    {
        if (_marked is { } row)
        {
            // Opaque again rather than empty, because the band the row is drawn on is what a click on the space
            // after a name answers: a background taken away would take that click with it.
            row.Background = Brushes.Transparent;
            _marked = null;
        }
    }

    /// <summary>
    /// What the tree says where the base holds nothing under it.
    /// </summary>
    /// <remarks>
    /// It takes no pointer of its own: the space it sits over is still the tree's, and a word that ate the
    /// right-click there would take the tree's own menu with it.
    /// </remarks>
    private static Control Empty() =>
        new StackPanel
        {
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Children =
            {
                new TextBlock
                {
                    Text = "\U0001F5C2",
                    FontSize = 34,
                    Classes = { "faint" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
                new TextBlock
                {
                    Text = RolaI18N.Get("rorolala_file_system.empty"),
                    Classes = { "muted" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
            },
        };

    /// <summary>One directory as a step of the tree, reading its children when first opened.</summary>
    /// <param name="path">The directory the step stands for.</param>
    private TreeViewItem Node(string path)
    {
        var item = new TreeViewItem { Tag = path };
        var read = false;

        // A step with nothing under it is given no child at all, and that is what leaves it without an
        // expander: one that cannot be opened must not be offered as though it could. The step that
        // has something under it gets a child that is never shown, so that it can be opened at all;
        // it is replaced the first time the step is. What it holds is also what says whether there is
        // anything worth offering to close under it (see Header).
        var holds = HoldsAny(path);

        if (holds)
        {
            item.Items.Add(
                new TreeViewItem { Header = new TextBlock { Text = "\u2026", Classes = { "faint" } } }
            );
        }

        item.Header = Header(path, holds ? () => Collapse(item) : null);

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
    /// <para>
    /// What is clicked is the band the row is drawn in and not the word written in it, which is why the
    /// band is given a background at all: a panel with none is not hit anywhere its children are not, so
    /// a click on the space after a name would select the row and leave the location where it was.
    /// </para>
    /// <para>
    /// The base theme opens and closes a step on a double click of a row as well, which leaves the chevron
    /// as one of two ways rather than the way. That gesture is taken from it here, and taken from the row
    /// rather than from the tree: the base theme's handler is on the row's own presenter — above it and
    /// below the tree — and a gesture taken by a handler on the row is a gesture that has already been taken
    /// by the time the presenter is reached.
    /// </para>
    /// </remarks>
    /// <param name="path">The directory the row names.</param>
    /// <param name="collapse">What closes every step under the row, or nothing where it has no steps under it.</param>
    private Border Header(string path, Action? collapse)
    {
        var entry = new Entry(path, EntryKind.Directory);
        var name = Names.Show(entry);
        var menu = _actions.Menu(this, [entry]);

        // Offered by every row with steps under it, which is the same row that is given an expander, since
        // closing them all is worth offering where there is more than one to close. What is not offered is
        // the other half — opening them all — since that would read every directory under the row, which is
        // what a tree read a step at a time is for avoiding.
        if (collapse is not null)
        {
            menu.Items.Add(BrowserActions.Item("rorolala_file_system.collapse_all", collapse));
        }

        // The menu is opened by the control the row is in rather than by the row, since one of the
        // things it does — putting a path on the clipboard — is reached through a control on screen.
        var row = new Border
        {
            Background = Brushes.Transparent,
            ContextMenu = menu,
            Child = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                Children =
                {
                    Icons.For(entry),
                    new TextBlock
                    {
                        Text = name.Length > 0 ? name : path,
                        VerticalAlignment = VerticalAlignment.Center,
                    },
                },
            },
        };

        row.Tapped += (_, _) => _browser.Go(path);
        row.DoubleTapped += (_, e) => e.Handled = true;

        _rows.Add((path, row));

        return row;
    }

    /// <summary>
    /// Closes every step under a row, and the row itself.
    /// </summary>
    /// <remarks>
    /// Walked rather than closed by setting the one row: closing a step hides the steps under it whether
    /// or not they are closed, so opening it again would show the whole walk the reader had taken down
    /// through it, which is the state this is asked for to leave. Only the steps that were opened are
    /// there to walk, which is what a tree read a step at a time has.
    /// </remarks>
    /// <param name="row">The row to close everything under.</param>
    private static void Collapse(TreeViewItem row)
    {
        row.IsExpanded = false;

        foreach (var step in row.Items.OfType<TreeViewItem>())
        {
            Collapse(step);
        }
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

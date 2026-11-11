using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Tree dock: one tree's description, its staging area and the changes beside it.
/// </summary>
/// <remarks>
/// The tree is the unit of work, so what this shows is the tree rather than a listing of its files.
/// Recording is the Version Graph's and the work's own standing is the Overview's; a panel that also
/// recorded or judged the work would be a second answer to both.
/// </remarks>
internal sealed class TreePanelDock : IDockView, IDockTitle
{
    /// <summary>The panel, which the header's Refresh reaches.</summary>
    private readonly TreePanelView _view;

    /// <summary>Makes the dock.</summary>
    /// <param name="host">The host, for what the panel runs and where it reveals a path.</param>
    /// <param name="hub">What every run this panel begins reports itself to.</param>
    /// <param name="trees">What each tree stands as, shared with the listing's columns.</param>
    public TreePanelDock(IPluginHost host, IProgressHub hub, TreeReadings trees)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(trees);

        _view = new TreePanelView(host, hub, trees);
    }

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header =>
        [
            new DockHeaderCommand(
                "rorolala_vcs.tree_panel.refresh",
                _view.Reload,
                IconKey: Icons.Refresh
            ),
        ];

    /// <inheritdoc />
    public string? Title => _view.Title;

    /// <inheritdoc />
    public string? Detail => _view.Detail;

    /// <inheritdoc />
    public event Action? TitleChanged
    {
        add => _view.TitleChanged += value;
        remove => _view.TitleChanged -= value;
    }
}

/// <summary>
/// One tree's description, its staged tree and the work beside it.
/// </summary>
/// <remarks>
/// The tree is chosen by the browser rather than by this panel: the file or directory last clicked
/// in a listing or tree names the tree that holds it, and the base's own tree is shown when nothing
/// is chosen. The panel is named by that tree, since the dock's tab and a window of its own should
/// say which tree is being looked at.
/// <para>
/// The description is written as it is typed rather than on a button: it is a file of the tree's
/// mark, and a save button would be a second state to remember for a write the command line makes
/// immediately.
/// </para>
/// <para>
/// The staged area is a tree because a tree is what a version records, and the changes beside it are
/// a list because a change is one path: the two are shown at once, one above the other when the dock
/// is narrow and one beside the other when there is room for both.
/// </para>
/// </remarks>
internal sealed class TreePanelView : UserControl
{
    /// <summary>The directory a tree keeps its own data in.</summary>
    private const string Mark = ".rola-tree";

    /// <summary>The width above which the two lists stand beside each other.</summary>
    private const double Wide = 320;

    /// <summary>How tall the description's five lines and its padding are.</summary>
    private const double DescriptionHeight = 100;

    /// <summary>The host, for what a run reaches and what a failure is shown by.</summary>
    private readonly IPluginHost _host;

    /// <summary>What every run this panel begins reports itself to.</summary>
    private readonly IProgressHub _hub;

    /// <summary>What each tree stands as, read once per tree and shared with the listing.</summary>
    private readonly TreeReadings _trees;

    /// <summary>The line under the description, which tells it from the two lists.</summary>
    private readonly Border _split;

    /// <summary>The description of the next version, which fills the top of the panel.</summary>
    private readonly TextBox _description;

    /// <summary>The two lists, arranged one way when narrow and another when wide.</summary>
    private readonly Grid _lists;

    /// <summary>The staged area's pane, holding its heading and its tree.</summary>
    private readonly Border _stagedPane;

    /// <summary>The changes area's pane, holding its heading and its list.</summary>
    private readonly Border _changesPane;

    /// <summary>The staged paths as a tree, which collapses a directory at a time.</summary>
    private readonly TreeView _staged;

    /// <summary>The paths the work holds and the staging area does not, as a tree of their own.</summary>
    private readonly TreeView _changes;

    /// <summary>The checkbox that stages or unstages the whole of the staged tree.</summary>
    private readonly CheckBox _stagedBulk;

    /// <summary>The checkbox that stages every change beside the staged tree.</summary>
    private readonly CheckBox _changesBulk;

    /// <summary>The tree the panel is showing, or nothing when the selection is not in one.</summary>
    private string? _tree;

    /// <summary>What the description field held when it was last read or written.</summary>
    private string _described = string.Empty;

    /// <summary>What the lists were drawn from, so a redraw that changes nothing is not made.</summary>
    private string? _signature;

    /// <summary>The last reading of the tree, kept while a run is going and a new reading is not in yet.</summary>
    private RolaTreeStatus? _reading;

    /// <summary>The directories the staged tree has been collapsed at, so a redraw keeps them closed.</summary>
    private readonly HashSet<string> _collapsed = new(StringComparer.Ordinal);

    /// <summary>Whether the field is being filled from the file rather than typed in.</summary>
    private bool _loading;

    /// <summary>Whether a run this panel began is going.</summary>
    private bool _busy;

    /// <summary>Whether a description that could not be written has already been said.</summary>
    private bool _warned;

    /// <summary>Whether the description is to be read again even if the field has the keyboard.</summary>
    private bool _reloadDescription;

    /// <summary>Whether the events are subscribed to, which they are only while the panel is on screen.</summary>
    private bool _watching;

    /// <summary>Whether the two lists stand beside each other.</summary>
    private bool _wide = true;

    /// <summary>The name the dock says, which is the tree's name.</summary>
    private string? _title;

    /// <summary>The whole path the name stands for, for a hover.</summary>
    private string? _detail;

    /// <summary>Makes the panel.</summary>
    /// <param name="host">The host, for what a run reaches and what a failure is shown by.</param>
    /// <param name="hub">What every run this panel begins reports itself to.</param>
    /// <param name="trees">What each tree stands as, shared with the listing's columns.</param>
    public TreePanelView(IPluginHost host, IProgressHub hub, TreeReadings trees)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(trees);

        _host = host;
        _hub = hub;
        _trees = trees;

        _description = new TextBox
        {
            PlaceholderText = RolaI18N.Get("rorolala_vcs.tree_panel.description.placeholder"),
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            TextAlignment = TextAlignment.Left,
            VerticalContentAlignment = VerticalAlignment.Top,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Top,
            Height = DescriptionHeight,
            MinHeight = DescriptionHeight,
            MaxHeight = DescriptionHeight,
            Padding = new Thickness(5),
            BorderThickness = new Thickness(0),
            IsEnabled = false,
        };

        _description[!TextBox.BackgroundProperty] = new DynamicResourceExtension(
            PaletteKeys.Dark
        );
        _description.FocusAdorner = null;
        _description.Cursor = new Cursor(StandardCursorType.Ibeam);

        // The look draws a field's focus as a border and a ring on the border part of the
        // template, which a local border of nothing on the part is the only way to take off: the
        // selectors that draw it are the look's, and this field is a surface rather than a control
        // to be pointed at. The values are set on the part so that the focus style, which is
        // written on that part, cannot bring them back.
        _description.TemplateApplied += (_, _) =>
        {
            foreach (var border in _description.GetVisualDescendants().OfType<Border>())
            {
                if (string.Equals(border.Name, "border", StringComparison.Ordinal))
                {
                    border.BorderThickness = new Thickness(0);
                    border.BoxShadow = default;
                }
            }
        };

        ScrollViewer.SetVerticalScrollBarVisibility(_description, ScrollBarVisibility.Hidden);
        ScrollViewer.SetHorizontalScrollBarVisibility(_description, ScrollBarVisibility.Disabled);

        _description.TextChanged += (_, _) => QueueDescription();
        _description.LostFocus += (_, _) => WriteDescription();

        // The field fills the strip, and a press anywhere in it is a press in the field: what is
        // asked for is a large editing surface rather than a line with room beside it.
        _description.PointerPressed += (_, _) =>
        {
            if (!_description.IsFocused)
            {
                _description.Focus();
            }
        };

        _staged = new TreeView
        {
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            Padding = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Stretch,
        };

        ScrollViewer.SetHorizontalScrollBarVisibility(_staged, ScrollBarVisibility.Disabled);
        ScrollViewer.SetVerticalScrollBarVisibility(_staged, ScrollBarVisibility.Auto);

        _changes = new TreeView
        {
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            Padding = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Stretch,
        };

        ScrollViewer.SetHorizontalScrollBarVisibility(_changes, ScrollBarVisibility.Disabled);
        ScrollViewer.SetVerticalScrollBarVisibility(_changes, ScrollBarVisibility.Auto);

        _changesPane = Pane(
            "rorolala_vcs.tree_panel.changes",
            _changes,
            out var changesBulk
        );
        _changesBulk = changesBulk;

        _stagedPane = Pane(
            "rorolala_vcs.tree_panel.staged",
            _staged,
            out var stagedBulk
        );
        _stagedBulk = stagedBulk;

        _stagedBulk.Click += (_, _) => BulkUnstage();
        _changesBulk.Click += (_, _) => BulkStage();

        _lists = new Grid { Margin = new Thickness(5) };

        _lists.Children.Add(_stagedPane);
        _lists.Children.Add(_changesPane);

        _split = new Border { Height = 1 };
        _split[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        var root = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,1,*"),
            RowSpacing = 0,
        };

        Grid.SetRow(_description, 0);
        Grid.SetRow(_split, 1);
        Grid.SetRow(_lists, 2);
        root.Children.Add(_description);
        root.Children.Add(_split);
        root.Children.Add(_lists);

        Content = root;

        SizeChanged += (_, size) => Arrange(size.NewSize.Width);
        Arrange(0);

        AttachedToVisualTree += (_, _) => Watch();
        DetachedFromVisualTree += (_, _) =>
        {
            WriteDescription();
            Unwatch();
        };

        Draw();
    }

    /// <summary>What the dock's tab says, which is the tree the panel is showing.</summary>
    public string? Title => _title;

    /// <summary>Where the title is in full, which is the tree's whole path.</summary>
    public string? Detail => _detail;

    /// <summary>Whether the two lists stand beside each other, which is what a wide dock gets.</summary>
    internal bool IsWide => _wide;

    /// <summary>Raised when the title or the detail says something else.</summary>
    public event Action? TitleChanged;

    /// <summary>Reads the tree again, which is what the dock's own command asks for.</summary>
    public void Reload()
    {
        _trees.Forget();
        _host.Files.Touch();
        Draw();
    }

    /// <summary>Makes one of the two panes: its heading, its bulk checkbox and its content.</summary>
    /// <param name="labelKey">What the pane is called.</param>
    /// <param name="content">What it holds.</param>
    /// <param name="bulk">The checkbox at its heading's right.</param>
    /// <returns>The pane.</returns>
    private Border Pane(string labelKey, Control content, out CheckBox bulk)
    {
        var label = new TextBlock
        {
            Text = RolaI18N.Get(labelKey),
            Classes = { "caption" },
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 0, 0, 0),
        };

        bulk = new CheckBox
        {
            IsThreeState = false,
            IsEnabled = false,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 4, 8, 0),
        };

        var head = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            Height = 24,
        };

        head.Children.Add(label);
        Grid.SetColumn(bulk, 1);
        head.Children.Add(bulk);

        // The whole staging area is reset from the staged heading's own menu rather than from a
        // button: what the panel is for is staging, and a destructive command does not need to be
        // in the way of the paths it would clear.
        var reset = new Avalonia.Controls.MenuItem
        {
            Header = RolaI18N.Get("rorolala_vcs.tree_panel.reset"),
        };
        reset.Click += (_, _) =>
        {
            if (_tree is { Length: > 0 } tree)
            {
                AskReset(tree);
            }
        };

        head.ContextMenu = new ContextMenu();
        head.ContextMenu.Items.Add(reset);

        var pane = new Grid { RowDefinitions = new RowDefinitions("Auto,*") };

        pane.Children.Add(head);

        // The bulk checkbox is the heading's, so it sits a heading above the first row rather than
        // a row above it. The tree starts a little lower than the heading so that the gap below
        // that checkbox is the same gap the rows have between them.
        content.Margin = new Thickness(0, 5, 0, 0);
        Grid.SetRow(content, 1);
        pane.Children.Add(content);

        var border = new Border
        {
            Child = pane,
            BorderThickness = new Thickness(1),
        };

        border[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        border[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        return border;
    }

    /// <summary>Places the two panes for the width the dock has, one over the other or side by side.</summary>
    /// <param name="width">The width the dock has.</param>
    private void Arrange(double width)
    {
        var wide = width > Wide;

        if (wide == _wide)
        {
            return;
        }

        _wide = wide;
        _lists.RowDefinitions = wide ? new RowDefinitions("*") : new RowDefinitions("*,*");
        _lists.ColumnDefinitions = wide ? new ColumnDefinitions("*,*") : new ColumnDefinitions("*");
        _lists.RowSpacing = wide ? 0 : 5;
        _lists.ColumnSpacing = wide ? 5 : 0;

        Grid.SetRow(_changesPane, 0);
        Grid.SetColumn(_changesPane, 0);

        Grid.SetRow(_stagedPane, wide ? 0 : 1);
        Grid.SetColumn(_stagedPane, wide ? 1 : 0);
    }

    /// <summary>The tree the browser is pointing at, or nothing when it points outside one.</summary>
    /// <returns>The tree's root, or nothing.</returns>
    private static string? ChosenTree()
    {
        if (ChosenEntry.Current is { } file)
        {
            return InTree(file);
        }

        if (ChosenDirectory.Current is { } directory)
        {
            return InTree(directory);
        }

        var at = BaseDirectory.Current;

        return at is { Length: > 0 } && Directory.Exists(Path.Combine(at, Mark))
            ? at
            : null;
    }

    /// <summary>The tree holding an entry, or nothing when no tree does.</summary>
    /// <param name="entry">The entry the browser chose.</param>
    /// <returns>The tree's root, or nothing.</returns>
    private static string? InTree(Entry entry)
    {
        var at = BaseDirectory.Current;

        return at is { Length: > 0 }
            ? RolaTrees.RootOf(entry.Path, at)
            : RolaTrees.RootOf(entry.Path);
    }

    /// <summary>Subscribes to what changes the tree or its reading, while the panel is on screen.</summary>
    private void Watch()
    {
        if (_watching)
        {
            return;
        }

        _watching = true;
        ChosenEntry.Changed += Draw;
        ChosenDirectory.Changed += Draw;
        BaseDirectory.Changed += Draw;
        _host.Files.Changed += Draw;
        _host.Files.Redrawn += Draw;
        Draw();
    }

    /// <summary>Gives up the subscriptions a panel that is not on screen has no use for.</summary>
    private void Unwatch()
    {
        if (!_watching)
        {
            return;
        }

        _watching = false;
        ChosenEntry.Changed -= Draw;
        ChosenDirectory.Changed -= Draw;
        BaseDirectory.Changed -= Draw;
        _host.Files.Changed -= Draw;
        _host.Files.Redrawn -= Draw;
    }

    /// <summary>Draws what the chosen tree holds now, reading it when the reading is not in yet.</summary>
    private void Draw()
    {
        var tree = ChosenTree();

        if (tree is null)
        {
            ShowNoTree();

            return;
        }

        ShowTree(tree);

        if (_trees.Reading(tree) is { } reading)
        {
            ShowReading(tree, reading);
        }
    }

    /// <summary>Shows that nothing is chosen, leaving the panel empty but for its field.</summary>
    private void ShowNoTree()
    {
        WriteDescription();

        _tree = null;
        _reading = null;
        _signature = null;
        _reloadDescription = false;

        _description.IsEnabled = false;
        SetDescription(string.Empty);

        _staged.Items.Clear();
        _changes.Items.Clear();

        _stagedBulk.IsChecked = false;
        _stagedBulk.IsEnabled = false;
        _changesBulk.IsChecked = false;
        _changesBulk.IsEnabled = false;

        SetTitle(null, null);
    }

    /// <summary>Names the tree and fills the description field when it is another tree.</summary>
    /// <param name="tree">The tree's root.</param>
    private void ShowTree(string tree)
    {
        if (!string.Equals(_tree, tree, StringComparison.Ordinal))
        {
            WriteDescription();
            _tree = tree;
            _reading = null;
            _signature = null;
            _reloadDescription = true;
            _warned = false;
        }

        _description.IsEnabled = true;

        SetTitle(TreeName(tree), tree);

        if (_reloadDescription || !_description.IsFocused)
        {
            ReadDescription(tree);
            _reloadDescription = false;
        }
    }

    /// <summary>Draws what a reading of the tree came to.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="reading">What the tree says.</param>
    private void ShowReading(string tree, RolaTreeStatus reading)
    {
        _reading = reading;

        var described = File.Exists(TreePanelCommands.Description.PathOf(tree));
        var signature = SignatureOf(reading, described, _busy);

        if (string.Equals(signature, _signature, StringComparison.Ordinal))
        {
            return;
        }

        _signature = signature;

        BuildStaged(tree, reading.Staged);

        var (adds, removals, conflicts) = Work(reading);

        BuildChanges(tree, reading, conflicts);

        _stagedBulk.IsChecked = reading.Staged.Count > 0;
        _stagedBulk.IsEnabled = !_busy && reading.Staged.Count > 0;
        ToolTip.SetTip(_stagedBulk, RolaI18N.Get("rorolala_vcs.tree_panel.unstage_all"));

        _changesBulk.IsChecked = false;
        _changesBulk.IsEnabled = !_busy && adds.Length + removals.Length > 0;
        ToolTip.SetTip(_changesBulk, RolaI18N.Get("rorolala_vcs.tree_panel.stage_all"));
    }

    /// <summary>The parts of the work the two lists are made of.</summary>
    /// <param name="reading">What the tree says.</param>
    /// <returns>The paths to stage, the paths to remove, and the conflicts.</returns>
    private static (string[] Adds, string[] Removals, HashSet<string> Conflicts) Work(
        RolaTreeStatus reading
    )
    {
        var conflicts = reading
            .Conflicts.Select(conflict => conflict.Path)
            .ToHashSet(StringComparer.Ordinal);
        var unstaged = reading
            .Unstaged.Where(change => !conflicts.Contains(change.Path))
            .ToArray();

        var adds = unstaged
            .Where(change => change.Change != "deleted")
            .Select(change => change.Path)
            .Concat(
                reading
                    .Untracked.Where(entry => !conflicts.Contains(entry.Path))
                    .Select(entry => entry.Path)
            )
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        var removals = unstaged
            .Where(change => change.Change == "deleted")
            .Select(change => change.Path)
            .Concat(
                unstaged
                    .Where(change => change.Change == "renamed" && change.OldPath is { Length: > 0 })
                    .Select(change => change.OldPath!)
            )
            .Distinct(StringComparer.Ordinal)
            .ToArray();

        return (adds, removals, conflicts);
    }

    /// <summary>Draws the staged paths as the tree a version records.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="changes">The staged changes.</param>
    private void BuildStaged(string tree, IReadOnlyList<RolaTreeChange> changes)
    {
        var leaves = changes
            .Select(change => new Leaf(
                Path: change.Path,
                Change: change.Change,
                OldPath: change.OldPath,
                Staged: true,
                Check: () => Unstage(tree, PathsOf(change)),
                Menu:
                [
                    ("rorolala_vcs.tree_panel.reveal", () => Reveal(change.Path)),
                ],
                Detail: change.OldPath is { Length: > 0 } previous
                    ? $"{previous} -> {change.Path}"
                    : change.Path
            ))
            .ToArray();

        Build(_staged, leaves, staged: true);
    }

    /// <summary>Draws a set of paths as a tree, one directory at a time.</summary>
    /// <param name="view">The tree to fill.</param>
    /// <param name="leaves">The paths it holds.</param>
    /// <param name="staged">Whether the tree is the staged one.</param>
    private void Build(TreeView view, IReadOnlyList<Leaf> leaves, bool staged)
    {
        view.Items.Clear();

        var root = new Node { Directory = true };

        foreach (var leaf in leaves)
        {
            var parts = leaf.Path.Split('/', StringSplitOptions.RemoveEmptyEntries);
            var at = root;

            for (var step = 0; step + 1 < parts.Length; step++)
            {
                at = Child(at, parts[step]);
            }

            at.Children.Add(
                new Node
                {
                    Name = parts.Length > 0 ? parts[^1] : leaf.Path,
                    Path = leaf.Path,
                    Leaf = leaf,
                }
            );
        }

        foreach (var child in Sorted(root))
        {
            view.Items.Add(Item(child, staged));
        }
    }

    /// <summary>The directory node for `name` under `parent`, made when there is none.</summary>
    /// <param name="parent">The node holding it.</param>
    /// <param name="name">The directory's name.</param>
    /// <returns>The node.</returns>
    private static Node Child(Node parent, string name)
    {
        var path = parent.Path.Length == 0 ? name : parent.Path + "/" + name;
        var child = parent.Children.FirstOrDefault(node => node.Directory && node.Name == name);

        if (child is null)
        {
            child = new Node
            {
                Name = name,
                Path = path,
                Directory = true,
            };
            parent.Children.Add(child);
        }

        return child;
    }

    /// <summary>A node's children, directories before files and each by name.</summary>
    /// <param name="node">The node.</param>
    /// <returns>The children in the order they are drawn.</returns>
    private static IEnumerable<Node> Sorted(Node node) =>
        node.Children
            .OrderBy(child => child.Directory ? 0 : 1)
            .ThenBy(child => child.Name, StringComparer.Ordinal);

    /// <summary>One node of a tree as a row, with its children under it.</summary>
    /// <param name="node">The node.</param>
    /// <param name="staged">Whether the tree is the staged one.</param>
    /// <returns>The item.</returns>
    private TreeViewItem Item(Node node, bool staged)
    {
        if (node.Leaf is { } leaf)
        {
            return new TreeViewItem
            {
                Header = Row(
                    node.Name,
                    leaf.Path,
                    leaf.Detail,
                    leaf.Change,
                    leaf.Staged,
                    leaf.Check,
                    leaf.Menu
                ),
                Tag = node.Path,
                HorizontalAlignment = HorizontalAlignment.Stretch,
            };
        }

        var item = new TreeViewItem
        {
            Header = DirectoryRow(node, staged),
            IsExpanded = !_collapsed.Contains(node.Path),
            Tag = node.Path,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };

        item.Expanded += (_, _) => _collapsed.Remove(node.Path);
        item.Collapsed += (_, _) => _collapsed.Add(node.Path);

        foreach (var child in Sorted(node))
        {
            item.Items.Add(Item(child, staged));
        }

        return item;
    }

    /// <summary>One directory: its name and the checkbox that acts on everything under it.</summary>
    /// <param name="node">The directory.</param>
    /// <param name="staged">Whether the tree is the staged one.</param>
    /// <returns>The row.</returns>
    private Control DirectoryRow(Node node, bool staged)
    {
        var name = new TextBlock
        {
            Text = node.Name,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        ToolTip.SetTip(name, node.Path);

        var leaves = Subtree(node).ToArray();
        var actionable = staged
            ? leaves
            : leaves.Where(leaf => leaf.Change != "conflict").ToArray();

        var box = new CheckBox
        {
            IsChecked = staged,
            IsEnabled = !_busy && actionable.Length > 0,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 0, 6, 0),
        };

        ToolTip.SetTip(
            box,
            RolaI18N.Get(
                staged ? "rorolala_vcs.tree_panel.unstage" : "rorolala_vcs.tree_panel.stage"
            )
        );

        box.Click += (_, _) =>
        {
            if (_busy || _tree is not { Length: > 0 } tree)
            {
                return;
            }

            if (staged)
            {
                Unstage(tree, StagedPaths(leaves));
            }
            else
            {
                StageLeaves(tree, actionable);
            }
        };

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            Height = 24,
        };

        row.Children.Add(name);
        Grid.SetColumn(box, 1);
        row.Children.Add(box);

        return row;
    }

    /// <summary>Every path under a directory node.</summary>
    /// <param name="node">The directory.</param>
    /// <returns>Its leaves, in the order the tree shows them.</returns>
    private static IEnumerable<Leaf> Subtree(Node node) =>
        Sorted(node)
            .SelectMany(child =>
            {
                if (child.Leaf is { } leaf)
                {
                    return new[] { leaf };
                }

                return Subtree(child);
            });

    /// <summary>Draws the work beside the staged tree: its conflicts, its changes and its new paths.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="reading">What the tree says.</param>
    /// <param name="conflicts">The paths a merge left in conflict.</param>
    private void BuildChanges(string tree, RolaTreeStatus reading, HashSet<string> conflicts)
    {
        var leaves = new List<Leaf>();

        foreach (var conflict in reading.Conflicts)
        {
            leaves.Add(
                new Leaf(
                    Path: conflict.Path,
                    Change: "conflict",
                    OldPath: null,
                    Staged: false,
                    Check: () => Begin(
                        tree,
                        "rorolala_vcs.progress.tree_stage",
                        () => TreePanelCommands.Stage(tree, conflict.Path)
                    ),
                    Menu:
                    [
                        (
                            "rorolala_vcs.tree_panel.theirs",
                            () => Reveal(Path.Combine(tree, conflict.Theirs))
                        ),
                        ("rorolala_vcs.tree_panel.reveal", () => Reveal(conflict.Path)),
                    ],
                    Detail: conflict.Path
                )
            );
        }

        foreach (var change in reading.Unstaged.Where(change => !conflicts.Contains(change.Path)))
        {
            leaves.Add(ChangedLeaf(tree, change));
        }

        foreach (var entry in reading.Untracked.Where(entry => !conflicts.Contains(entry.Path)))
        {
            leaves.Add(UntrackedLeaf(tree, entry));
        }

        Build(_changes, leaves, staged: false);
    }

    /// <summary>One path the work changed, with what may be done about it.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="change">The work's change.</param>
    /// <returns>The leaf.</returns>
    private Leaf ChangedLeaf(string tree, RolaTreeChange change)
    {
        Action check = change.Change switch
        {
            "deleted" => () => Begin(
                tree,
                "rorolala_vcs.progress.tree_stage",
                () => TreePanelCommands.StageRemoval(tree, change.Path)
            ),
            "renamed" when change.OldPath is { Length: > 0 } from => () => Begin(
                tree,
                "rorolala_vcs.progress.tree_stage",
                () => TreePanelCommands.StageRename(tree, change.Path, from)
            ),
            _ => () => Begin(
                tree,
                "rorolala_vcs.progress.tree_stage",
                () => TreePanelCommands.Stage(tree, change.Path)
            ),
        };

        var menu = new List<(string LabelKey, Action Run)>();

        if (change.Change is not "added")
        {
            var restore = change switch
            {
                { Change: "deleted", Path: var path } => (
                    "rorolala_vcs.tree_panel.rebuild",
                    path
                ),
                { Change: "renamed", OldPath: { Length: > 0 } was } => (
                    "rorolala_vcs.tree_panel.restore",
                    was
                ),
                { Path: var path } => ("rorolala_vcs.tree_panel.restore", path),
            };

            menu.Add(
                (
                    restore.Item1,
                    () => Begin(
                        tree,
                        "rorolala_vcs.progress.tree_restore",
                        () => TreePanelCommands.Restore(tree, restore.Item2)
                    )
                )
            );
        }

        menu.Add(("rorolala_vcs.tree_panel.reveal", () => Reveal(change.Path)));

        return new Leaf(
            Path: change.Path,
            Change: change.Change,
            OldPath: change.OldPath,
            Staged: false,
            Check: check,
            Menu: menu,
            Detail: change.OldPath is { Length: > 0 } previous
                ? $"{previous} -> {change.Path}"
                : change.Path
        );
    }

    /// <summary>One path the work holds and the staging area has never held.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="entry">The untracked path.</param>
    /// <returns>The leaf.</returns>
    private Leaf UntrackedLeaf(string tree, RolaTreeUntracked entry) =>
        new(
            Path: entry.Path,
            Change: "added",
            OldPath: null,
            Staged: false,
            Check: () => Begin(
                tree,
                "rorolala_vcs.progress.tree_stage",
                () => TreePanelCommands.Stage(tree, entry.Path)
            ),
            Menu:
            [
                ("rorolala_vcs.tree_panel.reveal", () => Reveal(entry.Path)),
            ],
            Detail: entry.Path
        );

    /// <summary>Stages every non-conflict path under one directory.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="leaves">The leaves to stage.</param>
    private void StageLeaves(string tree, IReadOnlyList<Leaf> leaves)
    {
        var adds = leaves
            .Where(leaf => leaf.Change != "deleted")
            .Select(leaf => leaf.Path)
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        var removals = leaves
            .Where(leaf => leaf.Change == "deleted")
            .Select(leaf => leaf.Path)
            .Concat(
                leaves
                    .Where(leaf => leaf.Change == "renamed" && leaf.OldPath is { Length: > 0 })
                    .Select(leaf => leaf.OldPath!)
            )
            .Distinct(StringComparer.Ordinal)
            .ToArray();

        if (adds.Length + removals.Length == 0)
        {
            return;
        }

        Begin(
            tree,
            "rorolala_vcs.progress.tree_stage",
            () => TreePanelCommands.StageAll(tree, adds, removals)
        );
    }

    /// <summary>One path: its letter, its name, and the checkbox that stages or unstages it.</summary>
    /// <param name="label">What the row says, which is a leaf in a tree and a path in a list.</param>
    /// <param name="path">The path the row acts on.</param>
    /// <param name="detail">What the row says on hover, which names both ends of a rename.</param>
    /// <param name="change">What happened to it.</param>
    /// <param name="staged">Whether the row's checkbox is checked.</param>
    /// <param name="check">What checking or unchecking it does.</param>
    /// <param name="menu">What the row's context menu offers.</param>
    /// <returns>The row.</returns>
    private Control Row(
        string label,
        string path,
        string detail,
        string change,
        bool staged,
        Action check,
        IReadOnlyList<(string LabelKey, Action Run)> menu
    )
    {
        var letter = new TextBlock
        {
            Text = StatusLetter(change),
            Width = 16,
            TextAlignment = TextAlignment.Center,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(StatusColour(change)),
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(letter, ChangeName(change));

        var name = new TextBlock
        {
            Text = label,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        if (string.Equals(change, "deleted", StringComparison.Ordinal))
        {
            name.TextDecorations = TextDecorations.Strikethrough;
        }

        ToolTip.SetTip(name, detail);
        name.PointerPressed += (_, press) =>
        {
            press.Handled = true;
            Reveal(path);
        };

        var box = new CheckBox
        {
            IsChecked = staged,
            IsEnabled = !_busy,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 0, 6, 0),
        };

        ToolTip.SetTip(
            box,
            RolaI18N.Get(
                staged ? "rorolala_vcs.tree_panel.unstage" : "rorolala_vcs.tree_panel.stage"
            )
        );

        box.Click += (_, _) =>
        {
            if (!_busy)
            {
                check();
            }
        };

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto"),
            ColumnSpacing = 6,
            Height = 24,
            Background = Brushes.Transparent,
        };

        Grid.SetColumn(letter, 0);
        Grid.SetColumn(name, 1);
        Grid.SetColumn(box, 2);
        row.Children.Add(letter);
        row.Children.Add(name);
        row.Children.Add(box);

        if (menu.Count > 0)
        {
            var context = new ContextMenu();

            foreach (var (labelKey, run) in menu)
            {
                var item = new Avalonia.Controls.MenuItem { Header = RolaI18N.Get(labelKey) };

                item.Click += (_, _) => run();
                context.Items.Add(item);
            }

            row.ContextMenu = context;
        }

        return row;
    }

    /// <summary>The paths one staged change names, with both ends of a rename.</summary>
    /// <param name="change">The change.</param>
    /// <returns>The paths, once each.</returns>
    private static IReadOnlyList<string> PathsOf(RolaTreeChange change) =>
        change.OldPath is { Length: > 0 } old
            ? [change.Path, old]
            : [change.Path];

    /// <summary>The paths a set of staged changes names, once each.</summary>
    /// <param name="changes">The changes.</param>
    /// <returns>The paths.</returns>
    private static IReadOnlyList<string> StagedPaths(IEnumerable<RolaTreeChange> changes) =>
        changes
            .SelectMany(PathsOf)
            .Distinct(StringComparer.Ordinal)
            .ToArray();

    /// <summary>The paths one staged leaf names, with both ends of a rename.</summary>
    /// <param name="leaf">The leaf.</param>
    /// <returns>The paths, once each.</returns>
    private static IReadOnlyList<string> PathsOf(Leaf leaf) =>
        leaf.OldPath is { Length: > 0 } old
            ? [leaf.Path, old]
            : [leaf.Path];

    /// <summary>The paths a set of staged leaves names, once each.</summary>
    /// <param name="leaves">The leaves.</param>
    /// <returns>The paths.</returns>
    private static IReadOnlyList<string> StagedPaths(IEnumerable<Leaf> leaves) =>
        leaves
            .SelectMany(PathsOf)
            .Distinct(StringComparer.Ordinal)
            .ToArray();

    /// <summary>The one letter a change is drawn by, as a git status does.</summary>
    /// <param name="change">The change's name.</param>
    /// <returns>The letter.</returns>
    private static string StatusLetter(string change) =>
        change switch
        {
            "added" => "A",
            "modified" => "M",
            "metadata" => "m",
            "deleted" => "D",
            "type_changed" => "T",
            "renamed" => "R",
            "conflict" => "C",
            _ => "?",
        };

    /// <summary>The colour a change's letter is drawn in.</summary>
    /// <param name="change">The change's name.</param>
    /// <returns>The colour.</returns>
    private static Color StatusColour(string change) =>
        change switch
        {
            "added" => RorolalaColours.Success,
            "modified" or "metadata" => RorolalaColours.Modify,
            "deleted" => RorolalaColours.Fail,
            "type_changed" or "conflict" => RorolalaColours.Warning,
            "renamed" => RorolalaColours.Moved,
            _ => RorolalaColours.Disabled,
        };

    /// <summary>What a change is called, for the letter's hover.</summary>
    /// <param name="change">The change's name.</param>
    /// <returns>The words.</returns>
    private static string ChangeName(string change) =>
        change switch
        {
            "added" => RolaI18N.Get("rorolala_vcs.tree_panel.change.added"),
            "modified" => RolaI18N.Get("rorolala_vcs.tree_panel.change.modified"),
            "metadata" => RolaI18N.Get("rorolala_vcs.tree_panel.change.metadata"),
            "deleted" => RolaI18N.Get("rorolala_vcs.tree_panel.change.deleted"),
            "type_changed" => RolaI18N.Get("rorolala_vcs.tree_panel.change.type_changed"),
            "renamed" => RolaI18N.Get("rorolala_vcs.tree_panel.change.renamed"),
            "conflict" => RolaI18N.Get("rorolala_vcs.tree_panel.conflict"),
            _ => change,
        };

    /// <summary>What the lists were drawn from, so an unchanged reading is not drawn again.</summary>
    /// <param name="reading">What the tree says.</param>
    /// <param name="described">Whether a description is staged.</param>
    /// <param name="busy">Whether a run is going.</param>
    /// <returns>The signature.</returns>
    private static string SignatureOf(RolaTreeStatus reading, bool described, bool busy)
    {
        var signature = new System.Text.StringBuilder();

        signature.Append(described).Append('|').Append(busy);

        foreach (var change in reading.Staged)
        {
            signature
                .Append("|s:")
                .Append(change.Path)
                .Append(':')
                .Append(change.Change)
                .Append(':')
                .Append(change.OldPath);
        }

        foreach (var change in reading.Unstaged)
        {
            signature
                .Append("|u:")
                .Append(change.Path)
                .Append(':')
                .Append(change.Change)
                .Append(':')
                .Append(change.OldPath);
        }

        foreach (var entry in reading.Untracked)
        {
            signature.Append("|n:").Append(entry.Path);
        }

        foreach (var conflict in reading.Conflicts)
        {
            signature.Append("|c:").Append(conflict.Path);
        }

        return signature.ToString();
    }

    /// <summary>Stages the whole of the work beside the staged tree, conflicts excepted.</summary>
    private void BulkStage()
    {
        if (_busy || _tree is not { Length: > 0 } tree || _reading is not { } reading)
        {
            return;
        }

        var (adds, removals, _) = Work(reading);

        Begin(
            tree,
            "rorolala_vcs.progress.tree_stage",
            () => TreePanelCommands.StageAll(tree, adds, removals)
        );
    }

    /// <summary>Unstages every path the staged tree holds.</summary>
    private void BulkUnstage()
    {
        if (_busy || _tree is not { Length: > 0 } tree || _reading is not { } reading)
        {
            return;
        }

        Begin(
            tree,
            "rorolala_vcs.progress.tree_unstage",
            () => TreePanelCommands.Unstage(tree, StagedPaths(reading.Staged))
        );
    }

    /// <summary>Unstages the paths one row names.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="paths">The paths.</param>
    private void Unstage(string tree, IReadOnlyList<string> paths) =>
        Begin(
            tree,
            "rorolala_vcs.progress.tree_unstage",
            () => TreePanelCommands.Unstage(tree, paths)
        );

    /// <summary>The tree's name, which is its part under the base or its whole path outside it.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <returns>The name.</returns>
    private static string TreeName(string tree)
    {
        var at = BaseDirectory.Current;

        if (at is not { Length: > 0 })
        {
            return tree;
        }

        var relative = Path.GetRelativePath(at, tree);

        // The base itself is the tree when the two are one directory, and its name is the leaf's
        // rather than the dot a relative path gives it.
        return relative.StartsWith("..", StringComparison.Ordinal) || Path.IsPathRooted(relative)
            ? tree
            : relative is "." or ""
                ? Path.GetFileName(Path.TrimEndingDirectorySeparator(tree))
                : relative;
    }

    /// <summary>Reads the description a tree has staged into the field.</summary>
    /// <param name="tree">The tree's root.</param>
    private void ReadDescription(string tree)
    {
        var described = TreePanelCommands.Description.Read(tree) ?? string.Empty;

        _described = described;
        _loading = true;
        _description.Text = described;
        _loading = false;
    }

    /// <summary>Replaces the field's text without writing it back to a tree that did not say it.</summary>
    /// <param name="text">What the field is to say.</param>
    private void SetDescription(string text)
    {
        _described = text;
        _loading = true;
        _description.Text = text;
        _loading = false;
    }

    /// <summary>Writes the description as it is typed, with no save button and no waiting.</summary>
    /// <remarks>
    /// A write is a few bytes to a file of the tree's mark, and a keystroke that has not reached it
    /// is a description the tree does not hold: the text is written on every change, and the write
    /// is skipped only while the field is being filled from the file.
    /// </remarks>
    private void QueueDescription()
    {
        if (_loading || _tree is null)
        {
            return;
        }

        WriteDescription();
    }

    /// <summary>Writes the description the field holds, when it says something else than the tree.</summary>
    private void WriteDescription()
    {
        if (_loading || _tree is not { Length: > 0 } tree)
        {
            return;
        }

        var text = _description.Text ?? string.Empty;

        if (string.Equals(text, _described, StringComparison.Ordinal))
        {
            return;
        }

        var was = File.Exists(TreePanelCommands.Description.PathOf(tree));

        if (!TreePanelCommands.Description.Write(tree, text, out var said))
        {
            if (!_warned)
            {
                _warned = true;
                FailureNotice.Show(
                    _host.Dialogs,
                    _host.Icons,
                    RolaI18N.Get("rorolala_vcs.tree_panel.description.failed"),
                    said is { Length: > 0 } ? said : RolaI18N.Get("rorolala_vcs.tree_panel.no_words")
                );
            }

            return;
        }

        _warned = false;
        _described = text;

        // The graph draws a draft for a staged description, and it reads whether the file is there
        // rather than what it says: a write that left the answer the same is nothing to read again.
        // The event is posted rather than raised here, since a panel that is being given up may be
        // writing on its way out and a redraw made in that write would arrive before the field is.
        if (was != File.Exists(TreePanelCommands.Description.PathOf(tree)))
        {
            Dispatcher.UIThread.Post(() => _host.Files.Touch());
        }
    }

    /// <summary>Begins one run, and draws the tree again when it ends.</summary>
    /// <param name="tree">The tree the run acts on.</param>
    /// <param name="titleKey">What the run is called on the progress stack.</param>
    /// <param name="run">What it runs.</param>
    /// <param name="cleared">Whether the run clears the description with the staging area.</param>
    private void Begin(
        string tree,
        string titleKey,
        Func<(bool Done, string Said)> run,
        bool cleared = false
    )
    {
        if (_busy)
        {
            return;
        }

        _busy = true;
        _signature = null;

        if (_reading is { } reading)
        {
            ShowReading(tree, reading);
        }

        var card = _hub.Begin(titleKey);

        _ = Task.Run(() =>
        {
            (bool Done, string Said) answer;

            try
            {
                answer = run();
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                answer = (false, error.Message);
            }

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(card);
                _busy = false;
                _signature = null;
                _trees.Forget();

                if (_reading is { } held)
                {
                    ShowReading(tree, held);
                }

                if (cleared && answer.Done)
                {
                    SetDescription(string.Empty);
                }

                _host.Files.Touch();

                if (!answer.Done)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.tree_panel.failed"),
                        answer.Said is { Length: > 0 }
                            ? answer.Said
                            : RolaI18N.Get("rorolala_vcs.tree_panel.no_words")
                    );
                }

                Draw();
            });
        });
    }

    /// <summary>Asks before the staging area, description and conflicts are cleared.</summary>
    /// <param name="tree">The tree the run acts on.</param>
    private void AskReset(string tree) =>
        _host.Dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.tree_panel.reset_title"),
                RolaI18N.Get("rorolala_vcs.tree_panel.reset_message"),
                () => Begin(
                    tree,
                    "rorolala_vcs.progress.tree_reset",
                    () => TreePanelCommands.Reset(tree),
                    cleared: true
                )
            )
        );

    /// <summary>Shows a path in the listing, or the place it would be in.</summary>
    /// <param name="path">The path to show.</param>
    private void Reveal(string path)
    {
        _host.Windows.Reveal(
            new DockRequest(
                global::FileSystemPlugin.FileSystemPlugin.DirectoryDockNameId,
                Path.IsPathRooted(path) || _tree is null ? path : Path.Combine(_tree, path),
                Select: true
            )
        );
    }

    /// <summary>Sets what the dock's tab says, and says that it changed.</summary>
    /// <param name="title">The tree's name, or nothing for the registered name.</param>
    /// <param name="detail">The tree's whole path.</param>
    private void SetTitle(string? title, string? detail)
    {
        if (
            string.Equals(_title, title, StringComparison.Ordinal)
            && string.Equals(_detail, detail, StringComparison.Ordinal)
        )
        {
            return;
        }

        _title = title;
        _detail = detail;
        TitleChanged?.Invoke();
    }

    /// <summary>One path in a tree: what it is, and what its row does.</summary>
    /// <param name="Path">The path under the tree, with forward slashes.</param>
    /// <param name="Change">What happened to it.</param>
    /// <param name="OldPath">Where it was, for a rename.</param>
    /// <param name="Staged">Whether its checkbox is checked.</param>
    /// <param name="Check">What checking or unchecking it does.</param>
    /// <param name="Menu">What its context menu offers.</param>
    /// <param name="Detail">What its row says on hover, which names both ends of a rename.</param>
    private sealed record Leaf(
        string Path,
        string Change,
        string? OldPath,
        bool Staged,
        Action Check,
        IReadOnlyList<(string LabelKey, Action Run)> Menu,
        string Detail
    );

    /// <summary>One node of a tree: a directory or one path.</summary>
    private sealed class Node
    {
        /// <summary>The leaf's name, or the directory's.</summary>
        public string Name { get; init; } = string.Empty;

        /// <summary>The path under the tree, with forward slashes.</summary>
        public string Path { get; init; } = string.Empty;

        /// <summary>Whether the node is a directory holding other paths.</summary>
        public bool Directory { get; init; }

        /// <summary>The path, for a leaf.</summary>
        public Leaf? Leaf { get; init; }

        /// <summary>What the node holds.</summary>
        public List<Node> Children { get; } = [];
    }
}

using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Menus;

using ShapePath = Avalonia.Controls.Shapes.Path;

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

    /// <summary>The staged paths as a tree, drawn as the rows that are open.</summary>
    private readonly ListBox _staged;

    /// <summary>The paths the work holds and the staging area does not, drawn the same way.</summary>
    private readonly ListBox _changes;

    /// <summary>The roots the staged rows are flattened from, kept so opening a step refills them.</summary>
    private readonly List<Node> _stagedRoots = [];

    /// <summary>The roots the change rows are flattened from.</summary>
    private readonly List<Node> _changesRoots = [];

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

        _staged = TreeList();
        _staged.ItemTemplate = new FuncDataTemplate<Flat>(
            (row, _) => TreeRow(row),
            supportsRecycling: true
        );

        _changes = TreeList();
        _changes.ItemTemplate = new FuncDataTemplate<Flat>(
            (row, _) => TreeRow(row),
            supportsRecycling: true
        );

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

        // A list with a scrollbar keeps its rows inside it, so the heading's checkbox has to move
        // in by the bar's width to stand over them. The bar comes and goes with what the list holds,
        // which is why this is read at every layout rather than set once.
        _staged.LayoutUpdated += (_, _) => AlignBulk(_staged, _stagedBulk);
        _changes.LayoutUpdated += (_, _) => AlignBulk(_changes, _changesBulk);

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

    /// <summary>Makes one of the two tree lists: virtualized rows over the paths that are open.</summary>
    /// <remarks>
    /// A tree of controls is only worth making for what can be seen. The list holds the rows that are
    /// open, and the panel materializes a row's controls only while it is in the viewport, so a tree
    /// with a directory of ten thousand files costs the rows on screen rather than ten thousand.
    /// </remarks>
    /// <returns>The list.</returns>
    private static ListBox TreeList()
    {
        var list = new ListBox
        {
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            Padding = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Stretch,
            ItemsPanel = new FuncTemplate<Panel?>(() => new VirtualizingStackPanel()),
        };

        list.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MarginProperty, new Thickness(0)),
                    new Setter(Layoutable.MinHeightProperty, 24.0),
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                    new Setter(
                        ContentControl.HorizontalContentAlignmentProperty,
                        HorizontalAlignment.Stretch
                    ),
                },
            }
        );

        list.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>().Class(":selected"))
            {
                Setters =
                {
                    new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                },
            }
        );

        ScrollViewer.SetHorizontalScrollBarVisibility(list, ScrollBarVisibility.Disabled);
        ScrollViewer.SetVerticalScrollBarVisibility(list, ScrollBarVisibility.Auto);

        return list;
    }

    /// <summary>Places a heading's checkbox over the row checkboxes, scrollbar and all.</summary>
    /// <param name="list">The list the rows are in.</param>
    /// <param name="bulk">The checkbox at the heading's right.</param>
    private static void AlignBulk(ListBox list, CheckBox bulk)
    {
        var bar = list
            .GetVisualDescendants()
            .OfType<ScrollBar>()
            .FirstOrDefault(bar => bar.Orientation == Orientation.Vertical && bar.IsVisible);
        var inset = bar?.Bounds.Width ?? 0;
        var margin = new Thickness(6, 4, 6 + inset, 0);

        if (!bulk.Margin.Equals(margin))
        {
            bulk.Margin = margin;
        }
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
            Margin = new Thickness(6, 4, 6, 0),
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
        // in the way of the paths it would clear. It is the delete category's, because clearing the
        // staging area is removing what it holds.
        head.ContextMenu = Menu(
            [
                new MenuLeaf(
                    ["menu.entry.delete", "menu.entry.staging"],
                    10,
                    () =>
                    {
                        if (_tree is { Length: > 0 } tree)
                        {
                            AskReset(tree);
                        }
                    }
                ),
            ]
        );

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

        _staged.ItemsSource = null;
        _changes.ItemsSource = null;
        _stagedRoots.Clear();
        _changesRoots.Clear();

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
                    new MenuLeaf(["menu.entry.reveal", "menu.entry.in_list"], 10, () => Reveal(change.Path)),
                ],
                Detail: change.OldPath is { Length: > 0 } previous
                    ? $"{previous} -> {change.Path}"
                    : change.Path
            ))
            .ToArray();

        Build(_staged, _stagedRoots, leaves, staged: true);
    }

    /// <summary>Draws a set of paths as a tree, as the rows that are open.</summary>
    /// <param name="view">The list to fill.</param>
    /// <param name="roots">Where the roots are kept for opening a step.</param>
    /// <param name="leaves">The paths it holds.</param>
    /// <param name="staged">Whether the tree is the staged one.</param>
    private void Build(
        ListBox view,
        List<Node> roots,
        IReadOnlyList<Leaf> leaves,
        bool staged
    )
    {
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

        roots.Clear();
        roots.AddRange(Sorted(root));

        var rows = new List<Flat>();
        Fill(rows, roots, 0, staged);
        view.ItemsSource = rows;
    }

    /// <summary>Adds the open rows of a tree, depth first, and no row for a closed step.</summary>
    /// <param name="rows">The rows being collected.</param>
    /// <param name="nodes">The nodes at this depth.</param>
    /// <param name="level">How far in the depth is.</param>
    /// <param name="staged">Whether the tree is the staged one.</param>
    private void Fill(List<Flat> rows, IEnumerable<Node> nodes, int level, bool staged)
    {
        foreach (var node in nodes)
        {
            var expanded = node.Directory && !_collapsed.Contains(node.Path);

            rows.Add(new Flat(node, level, expanded, staged));

            if (expanded)
            {
                Fill(rows, Sorted(node), level + 1, staged);
            }
        }
    }

    /// <summary>Opens or closes one step and draws the rows that are open now.</summary>
    /// <param name="item">The step's row.</param>
    private void Toggle(Flat item)
    {
        if (!item.Directory)
        {
            return;
        }

        if (!_collapsed.Remove(item.Path))
        {
            _collapsed.Add(item.Path);
        }

        var rows = new List<Flat>();
        Fill(rows, item.Staged ? _stagedRoots : _changesRoots, 0, item.Staged);
        (item.Staged ? _staged : _changes).ItemsSource = rows;
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

    /// <summary>One open row: a step of the tree or one path under it.</summary>
    /// <param name="item">The row.</param>
    /// <returns>The row's controls.</returns>
    private Control TreeRow(Flat item)
    {
        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,Auto,*,Auto"),
            Height = 24,
            Margin = new Thickness(item.Level * 16, 0, 0, 0),
            Background = Brushes.Transparent,
        };

        if (item.Leaf is not { } leaf)
        {
            var chevron = Chevron(item);

            Grid.SetColumn(chevron, 0);
            row.Children.Add(chevron);

            var step = new TextBlock
            {
                Text = item.Name,
                VerticalAlignment = VerticalAlignment.Center,
                TextTrimming = TextTrimming.CharacterEllipsis,
            };

            ToolTip.SetTip(step, item.Path);

            Grid.SetColumn(step, 1);
            Grid.SetColumnSpan(step, 2);
            row.Children.Add(step);

            var directory = DirectoryBox(item);

            Grid.SetColumn(directory, 3);
            row.Children.Add(directory);

            return row;
        }

        var lead = new Border
        {
            Width = 16,
            Height = 24,
            Background = Brushes.Transparent,
        };

        Grid.SetColumn(lead, 0);
        row.Children.Add(lead);

        var letter = new TextBlock
        {
            Text = StatusLetter(leaf.Change),
            Width = 16,
            TextAlignment = TextAlignment.Center,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(StatusColour(leaf.Change)),
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(letter, ChangeName(leaf.Change));

        Grid.SetColumn(letter, 1);
        row.Children.Add(letter);

        var name = new TextBlock
        {
            Text = item.Name,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        if (string.Equals(leaf.Change, "deleted", StringComparison.Ordinal))
        {
            name.TextDecorations = TextDecorations.Strikethrough;
        }

        ToolTip.SetTip(name, leaf.Detail);
        name.PointerPressed += (_, press) =>
        {
            press.Handled = true;
            Reveal(leaf.Path);
        };

        Grid.SetColumn(name, 2);
        row.Children.Add(name);

        var box = FileBox(leaf);

        Grid.SetColumn(box, 3);
        row.Children.Add(box);

        if (leaf.Menu.Count > 0)
        {
            row.ContextMenu = Menu(leaf.Menu);
        }

        return row;
    }

    /// <summary>The chevron that opens or closes one step.</summary>
    /// <param name="item">The step's row.</param>
    /// <returns>The control.</returns>
    private Control Chevron(Flat item)
    {
        var mark = new ShapePath
        {
            Data = Geometry.Parse("M 0 0 L 4 4 L 0 8 Z"),
            Fill = new SolidColorBrush(RorolalaColours.Disabled),
            Width = 4,
            Height = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        if (item.Expanded)
        {
            mark.RenderTransform = new RotateTransform(90);
            mark.RenderTransformOrigin = new RelativePoint(0.5, 0.5, RelativeUnit.Relative);
        }

        var button = new Button
        {
            Width = 16,
            Height = 24,
            Padding = new Thickness(0),
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(0),
            HorizontalAlignment = HorizontalAlignment.Left,
            Content = mark,
        };

        button.Click += (_, _) => Toggle(item);

        return button;
    }

    /// <summary>The checkbox that acts on everything under one step.</summary>
    /// <param name="item">The step's row.</param>
    /// <returns>The checkbox.</returns>
    private CheckBox DirectoryBox(Flat item)
    {
        var leaves = Subtree(item.Node).ToArray();
        var actionable = item.Staged
            ? leaves
            : leaves.Where(leaf => leaf.Change != "conflict").ToArray();
        var underConflict = _reading?.Conflicts.Any(
            conflict => Under(item.Path, conflict.Path)
        ) == true;
        var crosses = leaves.Any(leaf =>
            leaf.OldPath is { Length: > 0 } old && !Under(item.Path, old)
        );

        var box = new CheckBox
        {
            IsChecked = item.Staged,
            IsEnabled = !_busy && actionable.Length > 0,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 0, 6, 0),
        };

        ToolTip.SetTip(
            box,
            RolaI18N.Get(
                item.Staged ? "rorolala_vcs.tree_panel.unstage" : "rorolala_vcs.tree_panel.stage"
            )
        );

        box.Click += (_, _) =>
        {
            if (_busy || _tree is not { Length: > 0 } tree)
            {
                return;
            }

            if (item.Staged)
            {
                if (underConflict)
                {
                    Unstage(tree, StagedPaths(leaves));
                }
                else
                {
                    var paths = new List<string> { item.Path };

                    foreach (var leaf in leaves)
                    {
                        if (leaf.OldPath is { Length: > 0 } old && !Under(item.Path, old))
                        {
                            paths.Add(old);
                        }
                    }

                    Unstage(tree, paths);
                }
            }
            else if (actionable.Length == leaves.Length && !crosses)
            {
                Stage(tree, item.Path);
            }
            else
            {
                StageLeaves(tree, actionable);
            }
        };

        return box;
    }

    /// <summary>The checkbox that stages or unstages one path.</summary>
    /// <param name="leaf">The path.</param>
    /// <returns>The checkbox.</returns>
    private CheckBox FileBox(Leaf leaf)
    {
        var box = new CheckBox
        {
            IsChecked = leaf.Staged,
            IsEnabled = !_busy,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(6, 0, 6, 0),
        };

        ToolTip.SetTip(
            box,
            RolaI18N.Get(
                leaf.Staged ? "rorolala_vcs.tree_panel.unstage" : "rorolala_vcs.tree_panel.stage"
            )
        );

        box.Click += (_, _) =>
        {
            if (!_busy)
            {
                leaf.Check();
            }
        };

        return box;
    }

    /// <summary>The menu one path's row offers.</summary>
    /// <param name="items">What it offers.</param>
    /// <returns>The menu.</returns>
    private static ContextMenu Menu(IReadOnlyList<MenuLeaf> items)
    {
        var menu = new ContextMenu();

        new MenuRenderer(RolaI18N.Get).Fill(menu, items, force: false);

        return menu;
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
                        new MenuLeaf(
                            ["menu.entry.restore", "menu.entry.theirs"],
                            10,
                            () => Reveal(Path.Combine(tree, conflict.Theirs))
                        ),
                        new MenuLeaf(["menu.entry.reveal", "menu.entry.in_list"], 10, () => Reveal(conflict.Path)),
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

        Build(_changes, _changesRoots, leaves, staged: false);
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

        var menu = new List<MenuLeaf>();

        if (change.Change is not "added")
        {
            var restore = change switch
            {
                { Change: "deleted", Path: var path } => ("menu.entry.rebuild", path),
                { Change: "renamed", OldPath: { Length: > 0 } was } => (
                    "menu.entry.baseline",
                    was
                ),
                { Path: var path } => ("menu.entry.baseline", path),
            };

            menu.Add(
                new MenuLeaf(
                    ["menu.entry.restore", restore.Item1],
                    10,
                    () => Begin(
                        tree,
                        "rorolala_vcs.progress.tree_restore",
                        () => TreePanelCommands.Restore(tree, restore.Item2)
                    )
                )
            );
        }

        menu.Add(new MenuLeaf(["menu.entry.reveal", "menu.entry.in_list"], 10, () => Reveal(change.Path)));

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
                new MenuLeaf(["menu.entry.reveal", "menu.entry.in_list"], 10, () => Reveal(entry.Path)),
            ],
            Detail: entry.Path
        );

    /// <summary>Whether a path is a directory or one under it.</summary>
    /// <param name="directory">The directory's path under the tree.</param>
    /// <param name="path">The path to test.</param>
    /// <returns>Whether it is under the directory.</returns>
    private static bool Under(string directory, string path) =>
        string.Equals(path, directory, StringComparison.Ordinal)
        || path.StartsWith(directory + "/", StringComparison.Ordinal);

    /// <summary>Stages one path or one directory.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="path">The path or directory inside it.</param>
    private void Stage(string tree, string path) =>
        Begin(
            tree,
            "rorolala_vcs.progress.tree_stage",
            () => TreePanelCommands.Stage(tree, path)
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

        var (adds, removals, conflicts) = Work(reading);

        if (adds.Length + removals.Length == 0)
        {
            return;
        }

        // Staging the whole tree is one short command. The paths are only named one by one when a
        // conflict is in the tree, since `tree add .` would settle it, and the panel's bulk
        // checkbox deliberately leaves conflicts alone.
        Begin(
            tree,
            "rorolala_vcs.progress.tree_stage",
            conflicts.Count == 0
                ? () => TreePanelCommands.StageAll(tree)
                : () => TreePanelCommands.StageAll(tree, adds, removals)
        );
    }

    /// <summary>Unstages every path the staged tree holds.</summary>
    private void BulkUnstage()
    {
        if (_busy || _tree is not { Length: > 0 } tree || _reading is not { } reading)
        {
            return;
        }

        // Resetting the whole tree is one short command and keeps the description. A conflict is
        // not staged work, so a tree holding one is reset path by path instead, which leaves the
        // conflict where it is.
        IReadOnlyList<string> paths = reading.Conflicts.Count == 0
            ? ["."]
            : StagedPaths(reading.Staged);

        Begin(
            tree,
            "rorolala_vcs.progress.tree_unstage",
            () => TreePanelCommands.Unstage(tree, paths)
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
        IReadOnlyList<MenuLeaf> Menu,
        string Detail
    );

    /// <summary>One row of a tree that is open: a step, or a path under one.</summary>
    /// <param name="Node">The node the row stands for.</param>
    /// <param name="Level">How far in the tree it is.</param>
    /// <param name="Expanded">Whether the step's children are open.</param>
    /// <param name="Staged">Whether the row is in the staged tree.</param>
    private sealed record Flat(Node Node, int Level, bool Expanded, bool Staged)
    {
        /// <summary>Whether the row is a step.</summary>
        public bool Directory => Node.Directory;

        /// <summary>The path, for a leaf.</summary>
        public Leaf? Leaf => Node.Leaf;

        /// <summary>The row's name.</summary>
        public string Name => Node.Name;

        /// <summary>The row's path under the tree.</summary>
        public string Path => Node.Path;
    }

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

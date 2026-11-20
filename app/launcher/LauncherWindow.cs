using System.Globalization;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Dialogs;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;
using ShapeRectangle = Avalonia.Controls.Shapes.Rectangle;

namespace RorolalaLauncher;

/// <summary>
/// The launcher's window: the Vaults this machine knows on the left, the Workspaces it has opened on the
/// right, and the account and the create button in the strip at the bottom.
/// </summary>
/// <remarks>
/// What the window is made of is made again whenever the look or the list changes: a launcher is a small
/// window over a small state, and one statement of what it looks like is easier to keep true than a hundred
/// brushes updated in place. What is read from files and programs is read once by <see cref="Load"/>, and
/// the handshakes that follow are spoken in the background, because a window that waited for each address
/// before drawing would be a window that appeared late.
/// </remarks>
internal sealed class LauncherWindow : Window
{
    /// <summary>The window's width when it is not a tiling window manager's.</summary>
    private const double WidthOfWindow = 1100;

    /// <summary>The window's height when it is not a tiling window manager's.</summary>
    private const double HeightOfWindow = 720;

    /// <summary>The columns of the body: the Vaults dock, the splitter, and what is left.</summary>
    private const string ColumnsOfBody = "340,1,*";

    /// <summary>The rows of the window: the title, and the body.</summary>
    private const string RowsOfWindow = "34,*";

    /// <summary>The rows of a dock: its head, its body, and its foot.</summary>
    private const string RowsOfDock = "36,*,40";

    /// <summary>How wide the mark the window waits under is.</summary>
    private const double LoadingMarkDiameter = 26;

    /// <summary>How long a window is waited for before this window takes the wait back.</summary>
    private static readonly TimeSpan WaitForWindow = TimeSpan.FromSeconds(30);

    /// <summary>Where this program keeps its own files.</summary>
    private readonly LauncherFiles _files;

    /// <summary>The command line program.</summary>
    private readonly RolaRunner _rola;

    /// <summary>The daemon, which is the address book's own program.</summary>
    private readonly Daemon _daemon;

    /// <summary>The accounts on this machine.</summary>
    private readonly Accounts _accounts;

    /// <summary>What a connection is checked through.</summary>
    private readonly Connection _connection;

    /// <summary>The look.</summary>
    private readonly LauncherTheme _theme;

    /// <summary>What this program was told.</summary>
    private readonly LauncherSettings _settings;

    /// <summary>What the machine draws a frame in, which is what an unchosen frame is.</summary>
    private WindowStyle _machine;

    /// <summary>How this window's frame is drawn.</summary>
    private WindowStyle _frame;

    /// <summary>The colours of the variant in force.</summary>
    private Skin _skin;

    /// <summary>The Workspaces this machine has opened.</summary>
    private WorkspacesFile _opened = null!;

    /// <summary>The addresses this launcher hosts.</summary>
    private VaultsFile _hosted = null!;

    /// <summary>Every address in the dock, from the registry and from the file.</summary>
    private readonly List<VaultEntry> _vaults = [];

    /// <summary>The slots the racks were last drawn as, so that a handshake can redraw one.</summary>
    private readonly Dictionary<VaultEntry, VaultSlot> _slots = [];

    /// <summary>The account the work acts as, or nothing when none has been chosen.</summary>
    private string? _account;

    /// <summary>Whether the window's own work has been read and drawn.</summary>
    private bool _ready;

    /// <summary>Whether a Desktop window is being opened, which is what makes the rest of this one unavailable.</summary>
    private bool _busy;

    /// <summary>The start this window is waiting on, or nothing when it is not waiting.</summary>
    private LaunchAttempt? _launch;

    /// <summary>The cards the list was last drawn as, which is what being busy is said to.</summary>
    private readonly List<WorkspaceCard> _cards = [];

    /// <summary>The buttons that are out of reach while a window is being opened.</summary>
    private readonly List<FlatButton> _buttons = [];

    /// <summary>The language button and the menu it opens, out of reach for the same reason.</summary>
    private Control? _language;

    /// <summary>The window's own title row.</summary>
    private TitleChrome _chrome = null!;

    /// <summary>What the left dock shows, which is rebuilt when the address list changes.</summary>
    private StackPanel _leftBody = null!;

    /// <summary>The rack of addresses the daemon serves.</summary>
    private StackPanel _registryRack = null!;

    /// <summary>The rack of addresses this launcher hosts.</summary>
    private StackPanel _hostedRack = null!;

    /// <summary>The list of Workspaces.</summary>
    private StackPanel _workspaceList = null!;

    /// <summary>How many addresses the dock holds.</summary>
    private TextBlock _vaultCount = null!;

    /// <summary>How many Workspaces the list holds.</summary>
    private TextBlock _workspaceCount = null!;

    /// <summary>What the dock's foot says about the handshakes.</summary>
    private TextBlock _summary = null!;

    /// <summary>The lamp beside the summary.</summary>
    private VaultLed _summaryLed = null!;

    /// <summary>The button that creates a Workspace, which an account is needed for.</summary>
    private FlatButton _create = null!;

    /// <summary>Where toasts are laid over the window.</summary>
    private StackPanel _toastLayer = null!;

    /// <summary>The menu that opens over an address.</summary>
    private Popup _vaultMenu = null!;

    /// <summary>The menu the language button opens, so that Escape closes it before the window.</summary>
    private Popup? _languageMenu;

    /// <summary>The create button the empty state carries, which an account is also needed for.</summary>
    private FlatButton? _emptyCreate;

    /// <summary>Makes the window, holding only the mark it waits under.</summary>
    /// <param name="files">Where this program keeps its own files.</param>
    /// <param name="rola">The command line program.</param>
    /// <param name="accounts">The accounts on this machine.</param>
    /// <param name="connection">What a connection is checked through.</param>
    /// <param name="theme">The look.</param>
    /// <param name="settings">What this program was told.</param>
    public LauncherWindow(
        LauncherFiles files,
        RolaRunner rola,
        Accounts accounts,
        Connection connection,
        LauncherTheme theme,
        LauncherSettings settings
    )
    {
        ArgumentNullException.ThrowIfNull(files);
        ArgumentNullException.ThrowIfNull(rola);
        ArgumentNullException.ThrowIfNull(accounts);
        ArgumentNullException.ThrowIfNull(connection);
        ArgumentNullException.ThrowIfNull(theme);
        ArgumentNullException.ThrowIfNull(settings);

        _files = files;
        _rola = rola;
        _daemon = new Daemon(files.Daemon);
        _accounts = accounts;
        _connection = connection;
        _theme = theme;
        _settings = settings;

        _machine = WindowStyle.Windows;
        _frame = settings.Frame ?? _machine;
        _skin = theme.Skin;

        Title = "ROLA Launcher";
        Width = WidthOfWindow;
        Height = HeightOfWindow;
        MinWidth = 900;
        MinHeight = 560;
        CanResize = Program.Tiling;
        WindowDecorations = Avalonia.Controls.WindowDecorations.None;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = _skin.Mid;

        KeyDown += (_, e) =>
        {
            if (e.Key != Key.Escape)
            {
                return;
            }

            if (_vaultMenu is not null && _vaultMenu.IsOpen)
            {
                _vaultMenu.IsOpen = false;

                return;
            }

            if (_languageMenu is not null && _languageMenu.IsOpen)
            {
                _languageMenu.IsOpen = false;

                return;
            }

            if (!_busy)
            {
                Close();
            }
        };

        Closing += (_, leave) =>
        {
            if (_busy)
            {
                leave.Cancel = true;

                return;
            }

            _launch?.Dispose();
            _launch = null;
        };

        _theme.Changed += (_, _) =>
        {
            _skin = _theme.Skin;

            if (!_ready)
            {
                return;
            }

            Build();
        };

        ShowLoading();
    }

    /// <summary>
    /// Reads what the window shows, once it is on the screen, and draws it.
    /// </summary>
    /// <returns>A task that ends when the window has been built.</returns>
    internal async Task Load()
    {
        if (_ready)
        {
            return;
        }

        var machine = Task.Run(() => _rola.ProbeWindowStyle(_files.Beside));
        var account = Task.Run(() => _accounts.List(_files.Root).Current);
        var registry = Task.Run(() => _daemon.List(_files.Beside, _rola.Language));

        _machine = (await machine) ?? WindowStyle.Windows;
        _frame = _settings.Frame ?? _machine;

        try
        {
            _opened = WorkspacesFile.Read(_files.Workspaces);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            _opened = new WorkspacesFile();
        }

        try
        {
            _hosted = VaultsFile.Read(_files.Vaults);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            _hosted = new VaultsFile();
        }

        var served = await registry;

        _vaults.Clear();

        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (var space in served.Namespaces)
        {
            var address = $"rola://localhost/{space}";

            if (seen.Add(address))
            {
                _vaults.Add(new VaultEntry(space, address, VaultSource.Registry));
            }
        }

        foreach (var hosted in _hosted.Entries)
        {
            if (seen.Add(hosted.Address))
            {
                _vaults.Add(hosted);
            }
        }

        _account = await account;

        if (!IsVisible)
        {
            return;
        }

        Build();

        _ready = true;

        CheckAll();
    }

    /// <summary>Puts up the mark the window waits under, which is all it holds before it is read.</summary>
    private void ShowLoading()
    {
        Content = new Grid
        {
            Background = _skin.Mid,
            Children = { new TurningMark(new SolidColorBrush(_skin.Palette.Primary), LoadingMarkDiameter) },
        };
    }

    /// <summary>Builds the whole window from what is in force.</summary>
    private void Build()
    {
        if (_busy)
        {
            return;
        }

        _chrome = new TitleChrome(this, _skin);

        _chrome.Arrange(_skin, _frame);
        _chrome.SettingsAsked += (_, _) => _ = OpenSettings();

        var body = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions(ColumnsOfBody),
        };

        body.Children.Add(LeftDock());

        var splitter = new Border { Background = _skin.Splitter };

        Grid.SetColumn(splitter, 1);
        body.Children.Add(splitter);

        var workspaces = RightDock();

        Grid.SetColumn(workspaces, 2);
        body.Children.Add(workspaces);

        _toastLayer = new StackPanel
        {
            Orientation = Orientation.Vertical,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, 14, 54),
            IsHitTestVisible = false,
        };

        _vaultMenu = new Popup
        {
            Placement = PlacementMode.Pointer,
            IsLightDismissEnabled = true,
        };

        var root = new Grid
        {
            RowDefinitions = new RowDefinitions(RowsOfWindow),
            Background = _skin.Mid,
        };

        root.Children.Add(_chrome);

        Grid.SetRow(body, 1);
        root.Children.Add(body);

        Grid.SetRow(_toastLayer, 1);
        root.Children.Add(_toastLayer);
        root.Children.Add(_vaultMenu);

        Content = root;

        Fill();
    }

    /// <summary>The left dock: the Vaults head, the racks, and the handshake summary.</summary>
    /// <returns>The dock.</returns>
    private Control LeftDock()
    {
        _vaultCount = new TextBlock
        {
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            Foreground = _skin.Muted,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var add = IconButton(
            Marks.Plus(_skin.Muted, 14),
            RolaI18N.Get("launcher.vaults.add"),
            () => _ = OpenVaultDialog(null)
        );

        _leftBody = new StackPanel
        {
            Margin = new Thickness(12),
            Spacing = 0,
        };

        _registryRack = new StackPanel();
        _hostedRack = new StackPanel();

        _leftBody.Children.Add(GroupHead(RolaI18N.Get("launcher.vaults.registry"), first: true));
        _leftBody.Children.Add(Rack(_registryRack));
        _leftBody.Children.Add(GroupHead(RolaI18N.Get("launcher.vaults.hosted"), first: false));
        _leftBody.Children.Add(Rack(_hostedRack));

        var tile = new AddTile(RolaI18N.Get("launcher.vaults.add"), _skin);

        tile.Clicked += (_, _) => _ = OpenVaultDialog(null);
        _leftBody.Children.Add(tile);

        _summaryLed = new VaultLed(6);

        _summary = new TextBlock
        {
            FontSize = 11.5,
            Foreground = _skin.Muted,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        var dock = new Grid
        {
            RowDefinitions = new RowDefinitions(RowsOfDock),
        };

        dock.Children.Add(
            DockHead(
                Marks.Vault(_skin.Muted, 15),
                "Vaults",
                _vaultCount,
                add
            )
        );

        var scroll = new ScrollViewer
        {
            Content = _leftBody,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Background = _skin.Mid,
        };

        Grid.SetRow(scroll, 1);
        dock.Children.Add(scroll);

        var foot = DockFoot();

        Grid.SetRow(foot, 2);
        dock.Children.Add(foot);

        return dock;
    }

    /// <summary>The right dock: the Workspaces head, the cards, and the toolbar.</summary>
    /// <returns>The dock.</returns>
    private Control RightDock()
    {
        _workspaceCount = new TextBlock
        {
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            Foreground = _skin.Muted,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        _workspaceList = new StackPanel
        {
            Spacing = 10,
            Margin = new Thickness(12),
        };

        var dock = new Grid
        {
            RowDefinitions = new RowDefinitions(RowsOfDock),
        };

        dock.Children.Add(
            DockHead(
                Marks.Workspace(_skin.Muted, 15),
                RolaI18N.Get("launcher.workspaces.title"),
                _workspaceCount,
                null
            )
        );

        var scroll = new ScrollViewer
        {
            Content = _workspaceList,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Background = _skin.Mid,
        };

        Grid.SetRow(scroll, 1);
        dock.Children.Add(scroll);

        var toolbar = Toolbar();

        Grid.SetRow(toolbar, 2);
        dock.Children.Add(toolbar);

        return dock;
    }

    /// <summary>One dock's head: its mark, its name, how many it holds, and what may be done there.</summary>
    /// <param name="icon">The mark.</param>
    /// <param name="title">What the dock is called.</param>
    /// <param name="count">How many it holds.</param>
    /// <param name="tool">What may be done there, or nothing.</param>
    /// <returns>The head.</returns>
    private Control DockHead(Control icon, string title, TextBlock count, Control? tool)
    {
        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,Auto,Auto,*,Auto"),
            Margin = new Thickness(12, 0, 8, 0),
        };

        icon.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(icon);

        var name = new TextBlock
        {
            Text = title,
            FontSize = 13,
            FontWeight = FontWeight.SemiBold,
            Foreground = _skin.Front,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(8, 0, 0, 0),
        };

        Grid.SetColumn(name, 1);
        grid.Children.Add(name);

        var badge = new Border
        {
            Height = 18,
            MinWidth = 20,
            Padding = new Thickness(6, 0, 6, 0),
            CornerRadius = new CornerRadius(9),
            Background = _skin.Dark,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(1),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(8, 0, 0, 0),
            Child = count,
        };

        Grid.SetColumn(badge, 2);
        grid.Children.Add(badge);

        if (tool is not null)
        {
            tool.VerticalAlignment = VerticalAlignment.Center;

            Grid.SetColumn(tool, 4);
            grid.Children.Add(tool);
        }

        return new Border
        {
            Background = _skin.Bright,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Child = grid,
        };
    }

    /// <summary>The left dock's foot: what the handshakes came to, and the button that asks again.</summary>
    /// <returns>The foot.</returns>
    private Control DockFoot()
    {
        var recheck = IconButton(
            Marks.Refresh(_skin.Muted, 14),
            RolaI18N.Get("launcher.vaults.recheck_all"),
            () =>
            {
                CheckAll();
                Toast(RolaI18N.Get("launcher.vaults.rechecking"));
            }
        );

        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto"),
            Margin = new Thickness(12, 0, 8, 0),
            ColumnSpacing = 8,
        };

        grid.Children.Add(_summaryLed);

        Grid.SetColumn(_summary, 1);
        grid.Children.Add(_summary);

        Grid.SetColumn(recheck, 2);
        grid.Children.Add(recheck);

        return new Border
        {
            Background = _skin.Mid,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Child = grid,
        };
    }

    /// <summary>A group's name above a rack.</summary>
    /// <param name="label">What the group is called.</param>
    /// <param name="first">Whether it is the first group, which carries no room above it.</param>
    /// <returns>The label.</returns>
    private Control GroupHead(string label, bool first)
    {
        return new TextBlock
        {
            Text = label,
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 1,
            Foreground = _skin.Muted,
            Margin = new Thickness(2, first ? 1 : 16, 2, 7),
        };
    }

    /// <summary>A rack: a rounded surface that holds slots, or says it is empty.</summary>
    /// <param name="host">Where the slots are put.</param>
    /// <returns>The rack.</returns>
    private Control Rack(StackPanel host)
    {
        return new Border
        {
            Background = _skin.Bright,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            ClipToBounds = true,
            Child = host,
        };
    }

    /// <summary>Draws both racks from the addresses in force.</summary>
    private void FillVaults()
    {
        foreach (var slot in _slots.Values)
        {
            slot.Stop();
        }

        _slots.Clear();
        _registryRack.Children.Clear();
        _hostedRack.Children.Clear();

        AddSlots(_registryRack, _vaults.FindAll(entry => entry.Source == VaultSource.Registry), RolaI18N.Get("launcher.vaults.registry_empty"));
        AddSlots(_hostedRack, _vaults.FindAll(entry => entry.Source == VaultSource.Hosted), RolaI18N.Get("launcher.vaults.hosted_empty"));

        _vaultCount.Text = _vaults.Count.ToString(CultureInfo.InvariantCulture);

        UpdateSummary();
    }

    /// <summary>Puts one group's slots into its rack.</summary>
    /// <param name="host">The rack's stack.</param>
    /// <param name="entries">The addresses.</param>
    /// <param name="empty">What the rack says when it holds none.</param>
    private void AddSlots(StackPanel host, List<VaultEntry> entries, string empty)
    {
        if (entries.Count == 0)
        {
            host.Children.Add(
                new TextBlock
                {
                    Text = empty,
                    FontSize = 12,
                    Foreground = _skin.Faint,
                    Margin = new Thickness(14, 16, 14, 16),
                }
            );

            return;
        }

        for (var index = 0; index < entries.Count; index++)
        {
            var entry = entries[index];
            var slot = new VaultSlot(entry, _skin, index == 0, OpenVaultMenu);

            entry.Changed -= OnVaultChanged;
            entry.Changed += OnVaultChanged;

            _slots[entry] = slot;
            host.Children.Add(slot);
        }
    }

    /// <summary>Draws the Workspaces from the list in force.</summary>
    private void FillWorkspaces()
    {
        _workspaceList.Children.Clear();
        _cards.Clear();
        _emptyCreate = null;

        if (_opened.Paths.Count == 0)
        {
            _workspaceList.Children.Add(EmptyWorkspaces());

            _workspaceCount.Text = "0";
            UpdateCreate();

            return;
        }

        foreach (var path in _opened.Paths)
        {
            var card = new WorkspaceCard(path, _skin, chosen => _ = Open(chosen), Remove);

            _cards.Add(card);
            _workspaceList.Children.Add(card);
        }

        _workspaceCount.Text = _opened.Paths.Count.ToString(CultureInfo.InvariantCulture);

        UpdateCreate();
    }

    /// <summary>What a machine with no Workspaces is shown.</summary>
    /// <returns>The empty state.</returns>
    private Control EmptyWorkspaces()
    {
        var icon = Marks.Workspace(_skin.Faint, 26);

        var title = new TextBlock
        {
            Text = RolaI18N.Get("launcher.workspace.empty"),
            FontSize = 15,
            FontWeight = FontWeight.SemiBold,
            Foreground = _skin.Front,
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var sub = new TextBlock
        {
            Text = RolaI18N.Get("launcher.workspace.empty_hint"),
            FontSize = 12.5,
            Foreground = _skin.Muted,
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            MaxWidth = 300,
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        var create = new FlatButton(
            new TextBlock { Text = RolaI18N.Get("launcher.workspace.create"), FontSize = 13, FontWeight = FontWeight.SemiBold },
            _skin,
            ButtonKind.Primary
        )
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, 10, 0, 0),
        };

        create.Enabled = _account is not null;
        create.Clicked += (_, _) => _ = OpenCreate();

        _emptyCreate = create;

        var stack = new StackPanel
        {
            Spacing = 6,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(20, 32, 20, 32),
        };

        stack.Children.Add(icon);
        stack.Children.Add(title);
        stack.Children.Add(sub);
        stack.Children.Add(create);

        var outline = new ShapeRectangle
        {
            StrokeThickness = 1,
            Stroke = _skin.Splitter,
            StrokeDashArray = new Avalonia.Collections.AvaloniaList<double> { 4, 3 },
            RadiusX = 10,
            RadiusY = 10,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsHitTestVisible = false,
            Margin = new Thickness(0.5),
        };

        var root = new Panel { Margin = new Thickness(0, 12, 0, 0) };

        root.Children.Add(outline);
        root.Children.Add(stack);

        return root;
    }

    /// <summary>The right dock's foot: the language, the account, importing and creating.</summary>
    /// <returns>The foot.</returns>
    private Control Toolbar()
    {
        _buttons.Clear();

        _language = LanguageMenu();

        var account = ToolbarButton(
            LauncherIcons.Of(Icons.AccountCircle),
            17,
            RolaI18N.Get("launcher.toolbar.account"),
            () => _ = OpenAccount()
        );

        var import = ToolbarButton(
            LauncherIcons.Of(Icons.FolderOpen),
            17,
            RolaI18N.Get("launcher.toolbar.import"),
            () => _ = Import()
        );

        _create = new FlatButton(CreateFace(), _skin, ButtonKind.Primary)
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
            Padding = new Thickness(14, 0, 14, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };

        _create.Clicked += (_, _) => _ = OpenCreate();

        _buttons.Add(account);
        _buttons.Add(_create);
        _buttons.Add(import);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };

        buttons.Children.Add(_language);
        buttons.Children.Add(account);
        buttons.Children.Add(import);
        buttons.Children.Add(_create);

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            Margin = new Thickness(10, 0, 10, 0),
        };

        Grid.SetColumn(buttons, 1);
        row.Children.Add(buttons);

        return new Border
        {
            Background = _skin.Mid,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Child = row,
        };
    }

    /// <summary>What the create button carries: a plus and its name.</summary>
    /// <returns>The face.</returns>
    private Control CreateFace()
    {
        var face = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };

        face.Children.Add(LauncherIcons.Glyph(LauncherIcons.Of(Icons.AddCircle), 14, _skin.OnPrimary));
        face.Children.Add(
            new TextBlock
            {
                Text = RolaI18N.Get("launcher.workspace.create"),
                FontSize = 13,
                FontWeight = FontWeight.SemiBold,
                Foreground = _skin.OnPrimary,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        return face;
    }

    /// <summary>One of the foot's icon buttons: a picture, what it is for, and what pressing it does.</summary>
    /// <param name="picture">The picture.</param>
    /// <param name="extent">How wide it is drawn.</param>
    /// <param name="tip">What it is for.</param>
    /// <param name="pressed">What pressing it does.</param>
    /// <returns>The button.</returns>
    private FlatButton ToolbarButton(Bitmap picture, double extent, string tip, Action pressed)
    {
        var button = new FlatButton(LauncherIcons.Glyph(picture, extent, _skin.Front), _skin, ButtonKind.Surface)
        {
            Width = 42,
            Height = 32,
            Padding = default,
            CornerRadius = new CornerRadius(5),
        };

        button.Clicked += (_, _) => pressed();

        ToolTip.SetTip(button, tip);
        AutomationProperties.SetName(button, tip);

        return button;
    }

    /// <summary>A small button that carries a drawn mark.</summary>
    /// <param name="mark">The mark.</param>
    /// <param name="tip">What it is for.</param>
    /// <param name="pressed">What pressing it does.</param>
    /// <returns>The button.</returns>
    private FlatButton IconButton(Avalonia.Controls.Shapes.Path mark, string tip, Action pressed)
    {
        var button = new FlatButton(mark, _skin, ButtonKind.Quiet)
        {
            Width = 26,
            Height = 26,
            Padding = default,
            CornerRadius = new CornerRadius(5),
        };

        button.Clicked += (_, _) => pressed();

        mark.Stroke = _skin.Muted;

        ToolTip.SetTip(button, tip);
        AutomationProperties.SetName(button, tip);

        return button;
    }

    /// <summary>
    /// The language the program speaks, as a button that opens the two it may speak.
    /// </summary>
    /// <returns>The button, and the menu it opens.</returns>
    private Control LanguageMenu()
    {
        var current = LauncherLanguage.Of(LauncherLanguage.Current);

        var face = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 3,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        face.Children.Add(LauncherIcons.Glyph(LauncherIcons.Of(Icons.Language), 16, _skin.Front));
        face.Children.Add(Marks.Caret(_skin.Muted, 7));

        var button = new FlatButton(face, _skin, ButtonKind.Surface)
        {
            Width = 42,
            Height = 32,
            Padding = default,
            CornerRadius = new CornerRadius(5),
        };

        var named = RolaI18N.Get("launcher.language");

        ToolTip.SetTip(button, named);
        AutomationProperties.SetName(button, named);

        var options = new StackPanel { Spacing = 2 };

        var menu = new Popup
        {
            PlacementTarget = button,
            Placement = PlacementMode.TopEdgeAlignedLeft,
            IsLightDismissEnabled = true,
        };

        _languageMenu = menu;

        foreach (var option in LauncherLanguage.Options)
        {
            var chosen = string.Equals(option.Locale, current.Locale, StringComparison.Ordinal);

            var item = new FlatButton(
                new TextBlock { Text = option.Name, FontSize = 13 },
                _skin,
                chosen ? ButtonKind.Primary : ButtonKind.Quiet
            )
            {
                Height = 30,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(9, 0, 9, 0),
                CornerRadius = new CornerRadius(5),
            };

            item.Clicked += (_, _) =>
            {
                menu.IsOpen = false;

                Choose(option.Locale);
            };

            options.Children.Add(item);
        }

        menu.Child = new Border
        {
            Background = _skin.Bright,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(1),
            Padding = new Thickness(4),
            Width = 168,
            CornerRadius = new CornerRadius(8),
            Child = options,
        };

        button.Clicked += (_, _) => menu.IsOpen = !menu.IsOpen;

        return new Panel { Children = { button, menu } };
    }

    /// <summary>Switches the language the window speaks, and keeps the choice.</summary>
    /// <param name="locale">The locale chosen.</param>
    private void Choose(string locale)
    {
        try
        {
            Program.SetLanguage(locale);
        }
        catch (ConfigurationFailure failure)
        {
            _ = Notice(RolaI18N.Get("launcher.language"), failure.Message);

            return;
        }

        Build();
    }

    /// <summary>Draws what the window shows: both docks.</summary>
    private void Fill()
    {
        if (_busy)
        {
            return;
        }

        FillVaults();
        FillWorkspaces();
    }

    /// <summary>Sets the create button and the empty state's button to what the account allows.</summary>
    private void UpdateCreate()
    {
        if (_create is null)
        {
            return;
        }

        _create.Enabled = _account is not null;

        if (_emptyCreate is not null)
        {
            _emptyCreate.Enabled = _account is not null;
        }

        ToolTip.SetTip(
            _create,
            _account is null
                ? RolaI18N.Get("launcher.toolbar.create_needs_account")
                : RolaI18N.Get("launcher.workspace.create")
        );
    }

    /// <summary>Draws one slot again when its handshake landed, and the summary that counts it.</summary>
    /// <param name="sender">The entry that changed.</param>
    /// <param name="e">The event.</param>
    private void OnVaultChanged(object? sender, EventArgs e)
    {
        if (sender is VaultEntry entry && _slots.TryGetValue(entry, out var slot))
        {
            slot.Repaint(_skin);
        }

        UpdateSummary();
    }

    /// <summary>Counts the handshakes and says what the foot says about them.</summary>
    private void UpdateSummary()
    {
        if (_summary is null)
        {
            return;
        }

        if (_vaults.Count == 0)
        {
            _summary.Text = RolaI18N.Get("launcher.vaults.summary_empty");
            _summaryLed.Show(VaultStatus.Unchecked, _skin);

            return;
        }

        var checking = _vaults.Count(entry => entry.Status == VaultStatus.Checking);

        if (checking > 0)
        {
            _summary.Text = RolaI18N.Get("launcher.vaults.summary_checking", checking);
            _summaryLed.Show(VaultStatus.Checking, _skin);

            return;
        }

        var reachable = _vaults.Count(entry => entry.Status == VaultStatus.Reachable);
        var failed = _vaults.Count(entry => entry.Status == VaultStatus.Failed);
        var uncheckedCount = _vaults.Count(entry => entry.Status == VaultStatus.Unchecked);

        var parts = new List<string>();

        if (reachable > 0)
        {
            parts.Add(RolaI18N.Get("launcher.vaults.summary_reachable", reachable));
        }

        if (failed > 0)
        {
            parts.Add(RolaI18N.Get("launcher.vaults.summary_failed", failed));
        }

        if (uncheckedCount > 0)
        {
            parts.Add(RolaI18N.Get("launcher.vaults.summary_unchecked", uncheckedCount));
        }

        _summary.Text = string.Join(RolaI18N.Get("launcher.vaults.summary_separator"), parts);

        _summaryLed.Show(
            failed > 0
                ? VaultStatus.Failed
                : reachable > 0
                    ? VaultStatus.Reachable
                    : VaultStatus.Unchecked,
            _skin
        );
    }

    /// <summary>Checks every address at once, each in the background.</summary>
    private void CheckAll()
    {
        foreach (var entry in _vaults)
        {
            entry.Token++;
            entry.Reason = string.Empty;
            entry.Status = VaultStatus.Checking;
        }

        foreach (var entry in _vaults)
        {
            _ = CheckAsync(entry, entry.Token);
        }
    }

    /// <summary>Checks one address again.</summary>
    /// <param name="entry">The address.</param>
    private void CheckOne(VaultEntry entry)
    {
        entry.Token++;
        entry.Reason = string.Empty;
        entry.Status = VaultStatus.Checking;

        _ = CheckAsync(entry, entry.Token);
    }

    /// <summary>Speaks one handshake in the background and applies it if it is still the newest.</summary>
    /// <param name="entry">The address.</param>
    /// <param name="token">Which handshake this is.</param>
    /// <returns>A task that ends when the answer has been applied.</returns>
    private async Task CheckAsync(VaultEntry entry, int token)
    {
        var report = await Task.Run(() => _connection.Check(entry.Address));

        if (entry.Token != token)
        {
            return;
        }

        entry.Reason = report.State == ConnectionState.NoAccount
            ? RolaI18N.Get("launcher.create.need_account")
            : report.Reason;

        entry.Status = report.Reached ? VaultStatus.Reachable : VaultStatus.Failed;
    }

    /// <summary>Opens the menu over an address.</summary>
    /// <param name="entry">The address.</param>
    /// <param name="target">What the menu is placed against.</param>
    private void OpenVaultMenu(VaultEntry entry, Control target)
    {
        var items = new StackPanel { Spacing = 0 };

        items.Children.Add(
            MenuItem(
                RolaI18N.Get("launcher.vaults.check"),
                Marks.Refresh(_skin.Muted, 13),
                () =>
                {
                    _vaultMenu.IsOpen = false;
                    CheckOne(entry);
                },
                danger: false
            )
        );

        if (entry.Source == VaultSource.Hosted)
        {
            items.Children.Add(
                MenuItem(
                    RolaI18N.Get("launcher.vaults.edit"),
                    Marks.Pencil(_skin.Muted, 13),
                    () =>
                    {
                        _vaultMenu.IsOpen = false;
                        _ = OpenVaultDialog(entry);
                    },
                    danger: false
                )
            );

            items.Children.Add(
                new Border
                {
                    Height = 1,
                    Margin = new Thickness(4, 4, 4, 4),
                    Background = _skin.Splitter,
                }
            );

            items.Children.Add(
                MenuItem(
                    RolaI18N.Get("launcher.vaults.remove"),
                    Marks.Trash(_skin.Alarm, 13),
                    () =>
                    {
                        _vaultMenu.IsOpen = false;
                        RemoveVault(entry);
                    },
                    danger: true
                )
            );
        }

        _vaultMenu.Child = new Border
        {
            Width = 184,
            Background = _skin.Bright,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(4),
            Child = items,
        };

        _vaultMenu.PlacementTarget = target;
        _vaultMenu.IsOpen = true;
    }

    /// <summary>One item of the address menu.</summary>
    /// <param name="label">What it says.</param>
    /// <param name="mark">The mark before it.</param>
    /// <param name="pressed">What pressing it does.</param>
    /// <param name="danger">Whether it takes something away.</param>
    /// <returns>The item.</returns>
    private FlatButton MenuItem(string label, Avalonia.Controls.Shapes.Path mark, Action pressed, bool danger)
    {
        var face = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };

        face.Children.Add(mark);
        face.Children.Add(
            new TextBlock
            {
                Text = label,
                FontSize = 12.5,
                Foreground = danger ? _skin.Alarm : _skin.Front,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        var item = new FlatButton(
            face,
            _skin,
            danger ? ButtonKind.QuietDanger : ButtonKind.Quiet
        )
        {
            Height = 30,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(9, 0, 9, 0),
            CornerRadius = new CornerRadius(5),
        };

        mark.Stroke = danger ? _skin.Alarm : _skin.Muted;

        item.Clicked += (_, _) => pressed();

        return item;
    }

    /// <summary>Adds an address the launcher hosts, or edits one it already hosts.</summary>
    /// <param name="editing">The entry being edited, or nothing when one is being added.</param>
    /// <returns>A task that ends when the window is closed.</returns>
    private async Task OpenVaultDialog(VaultEntry? editing)
    {
        var dialog = new VaultDialog(_skin, editing, _vaults);

        await dialog.ShowDialog(this);

        if (!dialog.Saved)
        {
            return;
        }

        if (editing is null)
        {
            var made = new VaultEntry(dialog.SavedName, dialog.SavedAddress, VaultSource.Hosted);

            _hosted.Entries.Add(made);
            _vaults.Add(made);

            SaveVaults();
            FillVaults();
            CheckOne(made);

            Toast(RolaI18N.Get("launcher.vaults.added", made.Name));

            return;
        }

        editing.Name = dialog.SavedName;
        editing.Address = dialog.SavedAddress;

        SaveVaults();
        FillVaults();
        CheckOne(editing);

        Toast(RolaI18N.Get("launcher.vaults.saved"));
    }

    /// <summary>Removes an address the launcher hosts, and says so.</summary>
    /// <param name="entry">The entry.</param>
    private void RemoveVault(VaultEntry entry)
    {
        var name = entry.Name;

        _hosted.Remove(entry);
        _vaults.Remove(entry);

        SaveVaults();
        FillVaults();

        Toast(RolaI18N.Get("launcher.vaults.removed", name));
    }

    /// <summary>Writes the hosted addresses, saying so when they could not be written.</summary>
    private void SaveVaults()
    {
        try
        {
            _hosted.Write(_files.Vaults);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            _ = Notice(RolaI18N.Get("launcher.vaults.title"), error.Message);
        }
    }

    /// <summary>Opens the window that makes sure there is an account.</summary>
    private async Task OpenAccount()
    {
        var dialog = new AccountDialog(_skin, _files, _accounts);

        await dialog.ShowDialog(this);

        _account = _accounts.List(_files.Root).Current;

        FillWorkspaces();
        CheckAll();
    }

    /// <summary>Opens the window that creates a Workspace.</summary>
    private async Task OpenCreate()
    {
        if (_account is null)
        {
            await Notice(RolaI18N.Get("launcher.toolbar.account"), RolaI18N.Get("launcher.toolbar.create_needs_account"));

            return;
        }

        var dialog = new CreateWorkspaceDialog(_skin, _rola, _vaults, _opened);

        await dialog.ShowDialog(this);

        if (dialog.Created is { } made)
        {
            _opened.Add(made);
            Save();
            FillWorkspaces();

            Toast(RolaI18N.Get("launcher.workspace.created", Workspaces.Named(made)));
        }
    }

    /// <summary>Asks for a directory and adds it to the list, if it is a Workspace.</summary>
    private async Task Import()
    {
        var folders = await StorageProvider.OpenFolderPickerAsync(
            new FolderPickerOpenOptions
            {
                Title = RolaI18N.Get("launcher.toolbar.import"),
                AllowMultiple = false,
            }
        );

        if (folders.Count == 0 || folders[0].TryGetLocalPath() is not { } picked)
        {
            return;
        }

        if (Workspaces.Of(picked) is not (WorkspaceState.Workspace or WorkspaceState.GitProject))
        {
            await Notice(RolaI18N.Get("launcher.import.title"), RolaI18N.Get("launcher.import.not_a_workspace", picked));

            return;
        }

        if (_opened.Add(picked))
        {
            Save();
        }

        FillWorkspaces();
    }

    /// <summary>Opens the window that says what this program may be told.</summary>
    private async Task OpenSettings()
    {
        var dialog = new SettingsDialog(_skin, _theme, _settings, _files, _machine);

        await dialog.ShowDialog(this);

        _frame = dialog.Frame;

        _chrome.Arrange(_skin, _frame);
    }

    /// <summary>
    /// Opens the Desktop program onto a Workspace, and closes this one once its window is up.
    /// </summary>
    /// <param name="path">The Workspace.</param>
    private async Task Open(string path)
    {
        if (_busy)
        {
            return;
        }

        if (Workspaces.Of(path) is not (WorkspaceState.Workspace or WorkspaceState.GitProject))
        {
            _ = Notice(RolaI18N.Get("launcher.workspace.title"), RolaI18N.Get("launcher.workspace.gone", path));

            return;
        }

        _launch?.Dispose();
        _launch = _rola.Launch(path, _frame);

        if (_launch is null)
        {
            _ = Notice(
                RolaI18N.Get("launcher.workspace.title"),
                RolaI18N.Get("launcher.workspace.launch_failed", _rola.Program)
            );

            return;
        }

        SetBusy(path);

        var outcome = await _launch.WaitAsync(WaitForWindow);

        SetIdle();

        if (outcome.Ready)
        {
            Close();

            return;
        }

        _ = Notice(
            RolaI18N.Get("launcher.workspace.title"),
            outcome.Ended is { } ended
                ? RolaI18N.Get("launcher.workspace.launch_ended", ended)
                : RolaI18N.Get("launcher.workspace.launch_timeout")
        );
    }

    /// <summary>Puts all of the window but the wait out of reach.</summary>
    /// <param name="path">The Workspace whose window is being opened.</param>
    private void SetBusy(string path)
    {
        _busy = true;

        foreach (var card in _cards)
        {
            var chosen = string.Equals(card.Path, path, StringComparison.Ordinal);

            card.SetBusy(chosen);

            if (!chosen)
            {
                card.IsEnabled = false;
            }
        }

        foreach (var button in _buttons)
        {
            button.Enabled = false;
        }

        if (_emptyCreate is not null)
        {
            _emptyCreate.Enabled = false;
        }

        if (_language is not null)
        {
            _language.IsEnabled = false;
        }

        _chrome.IsEnabled = false;
    }

    /// <summary>Puts the window back the way it was, once there is nothing left to wait for.</summary>
    private void SetIdle()
    {
        _busy = false;

        foreach (var card in _cards)
        {
            card.SetBusy(false);
            card.IsEnabled = true;
        }

        foreach (var button in _buttons)
        {
            button.Enabled = true;
        }

        if (_create is not null)
        {
            _create.Enabled = _account is not null;
        }

        if (_emptyCreate is not null)
        {
            _emptyCreate.Enabled = _account is not null;
        }

        if (_language is not null)
        {
            _language.IsEnabled = true;
        }

        _chrome.IsEnabled = true;
    }

    /// <summary>Takes a Workspace off the list, and says so if the list could not be written.</summary>
    /// <param name="path">The Workspace.</param>
    private void Remove(string path)
    {
        if (_opened.Remove(path))
        {
            Save();
        }

        FillWorkspaces();

        Toast(RolaI18N.Get("launcher.workspace.removed", Workspaces.Named(path)));
    }

    /// <summary>Writes the list of Workspaces, saying so when it could not be written.</summary>
    private void Save()
    {
        try
        {
            _opened.Write(_files.Workspaces);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            _ = Notice(RolaI18N.Get("launcher.workspace.title"), error.Message);
        }
    }

    /// <summary>Says one thing over the window.</summary>
    /// <param name="text">What it says.</param>
    private void Toast(string text)
    {
        if (_toastLayer is null)
        {
            return;
        }

        var face = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };

        face.Children.Add(Marks.Check(_skin.Success, 13));
        face.Children.Add(
            new TextBlock
            {
                Text = text,
                FontSize = 12.5,
                Foreground = _skin.Front,
                TextWrapping = TextWrapping.Wrap,
                MaxWidth = 340,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        var toast = new Border
        {
            Background = _skin.Bright,
            BorderBrush = _skin.Splitter,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(12, 9, 12, 9),
            HorizontalAlignment = HorizontalAlignment.Right,
            Child = face,
        };

        _toastLayer.Children.Add(toast);

        var timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(3.2) };

        timer.Tick += (_, _) =>
        {
            timer.Stop();
            _toastLayer.Children.Remove(toast);
        };

        timer.Start();
    }

    /// <summary>Says one thing over the window, in a window of its own.</summary>
    /// <param name="heading">What it is about.</param>
    /// <param name="message">What it says.</param>
    /// <returns>A task that ends when it is read.</returns>
    private Task Notice(string heading, string message) =>
        NoticeDialog.Show(this, _skin, heading, message, RolaI18N.Get("launcher.common.ok"));
}

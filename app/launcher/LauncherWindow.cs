using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform.Storage;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Banner;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Dialogs;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher;

/// <summary>
/// The launcher's window: a picture at the top, the Workspaces this machine has opened under it, and what
/// can be done about them at the bottom.
/// </summary>
/// <remarks>
/// The window is one size, always, and draws its own frame: the picture is the frame, so a window whose
/// frame was the desktop's would be two frames around one picture. It is opened as a dialog over nothing,
/// which is what keeps a tiling window manager from tiling it with everything else — the same way it keeps
/// the windows the program opens over itself where they were put.
/// <para>
/// What the window is made of is made again whenever the look or the list changes: a launcher is a small
/// window over a small state, and one statement of what it looks like is easier to keep true than a hundred
/// brushes updated in place.
/// </para>
/// </remarks>
internal sealed class LauncherWindow : Window
{
    /// <summary>The window's width, which is also the width the list is laid out in.</summary>
    private const double WidthOfWindow = 540;

    /// <summary>The window's height.</summary>
    private const double HeightOfWindow = 600;

    /// <summary>The height of the banner, which is the window's width over four.</summary>
    private const double HeightOfBanner = WidthOfWindow / 4;

    /// <summary>The height of the row the three buttons sit in.</summary>
    private const double HeightOfToolbar = 64;

    /// <summary>How many times the theme artwork's own size the welcome draws the look it is chosen in.</summary>
    private const double ModeScale = 1.5;

    /// <summary>How far apart the three pictures of the look stand.</summary>
    private const double ModeSpacing = 10;

    /// <summary>
    /// How tall the welcome is when it centres something: the window, less the banner and the strip at the
    /// bottom, less the room the list keeps above and below what it holds.
    /// </summary>
    private const double WelcomeHeight = HeightOfWindow - HeightOfBanner - HeightOfToolbar - 36;

    /// <summary>How wide each of the two buttons under the welcome is drawn, which is one width for both.</summary>
    private const double ActionWidth = 190;

    /// <summary>Where this program keeps its own files.</summary>
    private readonly LauncherFiles _files;

    /// <summary>The command line program.</summary>
    private readonly RolaRunner _rola;

    /// <summary>The accounts on this machine.</summary>
    private readonly Accounts _accounts;

    /// <summary>What a connection is checked through.</summary>
    private readonly Connection _connection;

    /// <summary>The look.</summary>
    private readonly LauncherTheme _theme;

    /// <summary>What this program was told.</summary>
    private readonly LauncherSettings _settings;

    /// <summary>What this machine draws a frame in, which is what an unchosen frame is.</summary>
    private readonly WindowStyle _machine;

    /// <summary>The Workspaces this machine has opened.</summary>
    private WorkspacesFile _opened = null!;

    /// <summary>The account the work acts as, or nothing when none has been chosen.</summary>
    private string? _account;

    /// <summary>How this window's frame is drawn.</summary>
    private WindowStyle _frame;

    /// <summary>The picture the banner plays, if one could be read.</summary>
    private AsepriteAnimation? _animation;

    /// <summary>What went wrong with the picture, if anything did.</summary>
    private string? _bannerProblem;

    /// <summary>The colours of the variant in force.</summary>
    private Skin _skin;

    /// <summary>The banner.</summary>
    private BannerView _banner = null!;

    /// <summary>The window's own buttons.</summary>
    private TitleChrome _chrome = null!;

    /// <summary>What the list or the welcome is drawn in.</summary>
    private StackPanel _content = null!;

    /// <summary>The button that creates a Workspace, which an account is needed for.</summary>
    private FlatButton _create = null!;

    /// <summary>The welcome's own button for the same thing, while the welcome is what is shown.</summary>
    private FlatButton? _welcomeCreate;

    /// <summary>Makes the window.</summary>
    /// <param name="files">Where this program keeps its own files.</param>
    /// <param name="rola">The command line program.</param>
    /// <param name="accounts">The accounts on this machine.</param>
    /// <param name="connection">What a connection is checked through.</param>
    /// <param name="theme">The look.</param>
    /// <param name="settings">What this program was told.</param>
    /// <param name="machine">How this machine draws a frame.</param>
    public LauncherWindow(
        LauncherFiles files,
        RolaRunner rola,
        Accounts accounts,
        Connection connection,
        LauncherTheme theme,
        LauncherSettings settings,
        WindowStyle machine
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
        _accounts = accounts;
        _connection = connection;
        _theme = theme;
        _settings = settings;
        _machine = machine;
        _frame = settings.Frame ?? machine;
        _skin = theme.Skin;

        Title = "Rola Launcher";
        Width = WidthOfWindow;
        Height = HeightOfWindow;
        CanResize = false;
        WindowDecorations = Avalonia.Controls.WindowDecorations.None;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        Background = _skin.Mid;

        KeyDown += (_, e) =>
        {
            if (e.Key == Key.Escape)
            {
                Close();
            }
        };

        _theme.Changed += (_, _) =>
        {
            _skin = _theme.Skin;

            Build();
        };

        _opened = WorkspacesFile.Read(_files.Workspaces);
        _account = _accounts.List(_files.Root).Current;

        LoadBanner();
        Build();
    }

    /// <summary>
    /// Builds the whole window from what is in force.
    /// </summary>
    private void Build()
    {
        _banner = new BannerView();

        _banner.Play(_animation);
        _banner.Repaint(_skin);
        _banner.ReplacementAsked += (_, _) => _ = ReplaceBanner();

        _chrome = new TitleChrome(this, _skin);

        _chrome.Arrange(_skin, _frame);
        _chrome.SettingsAsked += (_, _) => _ = OpenSettings();

        _content = new StackPanel { Spacing = 10, Margin = new Thickness(20, 18, 20, 18) };

        var scroll = new ScrollViewer
        {
            Content = _content,
            VerticalScrollBarVisibility = ScrollBarVisibility.Hidden,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Background = _skin.Mid,
        };

        var banner = new Grid { Height = HeightOfBanner };

        banner.Children.Add(new Border { Background = _skin.Dark });
        banner.Children.Add(_banner);
        banner.Children.Add(OverBanner());
        banner.Children.Add(_chrome);

        var root = new Grid
        {
            RowDefinitions = new RowDefinitions($"{HeightOfBanner},*,{HeightOfToolbar}"),
            Background = _skin.Mid,
        };

        var toolbar = Toolbar();

        root.Children.Add(banner);
        Grid.SetRow(scroll, 1);
        root.Children.Add(scroll);
        Grid.SetRow(toolbar, 2);
        root.Children.Add(toolbar);

        Content = root;

        Fill();
    }

    /// <summary>
    /// What is written over the picture when there is none, or when the one there could not be read.
    /// </summary>
    /// <returns>The layer.</returns>
    private Control OverBanner()
    {
        if (_bannerProblem is null && _animation is not null)
        {
            return new Panel();
        }

        var says = _bannerProblem is null
            ? RolaI18N.Get("launcher.banner.empty")
            : RolaI18N.Get("launcher.banner.problem", _bannerProblem);

        return new Border
        {
            Background = _skin.Dark,
            Child = new TextBlock
            {
                Text = says,
                FontSize = 12,
                Foreground = _skin.Muted,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                TextWrapping = TextWrapping.Wrap,
                MaxWidth = WidthOfWindow - 120,
                TextAlignment = TextAlignment.Center,
            },
            IsHitTestVisible = false,
        };
    }

    /// <summary>The row at the bottom: the language on the left, and what can be done on the right.</summary>
    /// <returns>The row.</returns>
    private Control Toolbar()
    {
        var account = ToolbarButton(
            LauncherIcons.Of(Icons.AccountCircle),
            RolaI18N.Get("launcher.toolbar.account"),
            () => _ = OpenAccount()
        );

        _create = ToolbarButton(
            LauncherIcons.Of(Icons.AddCircle),
            RolaI18N.Get("launcher.toolbar.create"),
            () => _ = OpenCreate()
        );

        var import = ToolbarButton(
            LauncherIcons.Of(Icons.FolderOpen),
            RolaI18N.Get("launcher.toolbar.import"),
            () => _ = Import()
        );

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };

        buttons.Children.Add(account);
        buttons.Children.Add(_create);
        buttons.Children.Add(import);

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(20, 0, 20, 0),
        };

        row.Children.Add(LanguageMenu());

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

    /// <summary>
    /// The language the program speaks, as a button that opens the two it may speak.
    /// </summary>
    /// <remarks>
    /// The button is the size of the three beside it and carries no word: the language a window is written
    /// in is not something a person reads the name of once they are in it, and the three buttons it stands
    /// beside are marks too. What it is, and what it would say, is the mark and the caret under it. What is
    /// chosen is written to the preference file the Desktop program reads, so the two programs go on
    /// speaking one language rather than each its own.
    /// </remarks>
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
            Width = 46,
            Height = 38,
            Padding = default,
            HorizontalAlignment = HorizontalAlignment.Left,
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

        foreach (var option in LauncherLanguage.Options)
        {
            var chosen = string.Equals(option.Locale, current.Locale, StringComparison.Ordinal);

            var item = new FlatButton(
                new TextBlock { Text = option.Name, FontSize = 13 },
                _skin,
                chosen ? ButtonKind.Primary : ButtonKind.Quiet
            )
            {
                Height = 32,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(12, 0, 12, 0),
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
            Child = options,
        };

        button.Clicked += (_, _) => menu.IsOpen = !menu.IsOpen;

        return new Panel { Children = { button, menu } };
    }

    /// <summary>
    /// Switches the language the window speaks, and keeps the choice.
    /// </summary>
    /// <remarks>
    /// The window is drawn again whole rather than having its words replaced: every word it shows is read
    /// while it is built, and one statement of what it says is easier to keep true than a hundred labels
    /// changed in place.
    /// </remarks>
    /// <param name="locale">The locale chosen.</param>
    private void Choose(string locale)
    {
        try
        {
            Program.SetLanguage(locale);
        }
        catch (ConfigurationFailure failure)
        {
            // The file the choice is kept in could not be written, so the window is left speaking what it was
            // speaking: a choice the next run would not find is a choice it did not make.
            _ = Notice(RolaI18N.Get("launcher.language"), failure.Message);

            return;
        }

        Build();
    }


    /// <summary>One of the toolbar's buttons: a picture, and what pressing it does.</summary>
    /// <param name="picture">The picture.</param>
    /// <param name="tip">What it is for.</param>
    /// <param name="pressed">What pressing it does.</param>
    /// <returns>The button.</returns>
    private FlatButton ToolbarButton(Bitmap picture, string tip, Action pressed)
    {
        var button = new FlatButton(LauncherIcons.Glyph(picture, 16, _skin.Front), _skin, ButtonKind.Surface)
        {
            Width = 46,
            Height = 38,
            Padding = default,
        };

        button.Clicked += (_, _) => pressed();

        ToolTip.SetTip(button, tip);

        return button;
    }

    /// <summary>
    /// Draws what the list area shows: the Workspaces, or the welcome a machine with none is met with.
    /// </summary>
    private void Fill()
    {
        _content.Children.Clear();
        _welcomeCreate = null;

        if (_create is not null)
        {
            // A Workspace is made against a Vault as somebody, so a machine with no account has nothing to
            // make one as. Importing one needs no account: it is a directory that already exists.
            _create.Enabled = _account is not null;

            var tip = _account is null
                ? RolaI18N.Get("launcher.toolbar.create_needs_account")
                : RolaI18N.Get("launcher.toolbar.create");

            ToolTip.SetTip(_create, tip);
        }

        if (_opened.Paths.Count == 0)
        {
            Welcome();

            return;
        }

        foreach (var path in _opened.Paths)
        {
            var card = new WorkspaceCard(path, _skin, Open, Remove);

            card.Margin = new Thickness(0, 0, 0, 0);
            _content.Children.Add(card);
        }
    }

    /// <summary>
    /// The welcome a machine with no Workspaces is met with: the program's own name and line, who the machine
    /// is, the look it is drawn in, and what there is to do with no work in it yet.
    /// </summary>
    /// <remarks>
    /// It is the one screen that is read before anything is done, so each of its parts is one thing: the name
    /// at the top, the line written under it, and then what the machine can do. The buttons are here as well
    /// as in the strip at the bottom because this is the screen that offers them — what a person meets first
    /// should not have to be found in a row of marks.
    /// <para>
    /// A machine with no account is shown the one thing such a machine can do, and it is shown it in the
    /// middle of the window: there is no work yet, and the account is what the work would be done as. Once
    /// there is an account the greeting is not said again — the window is not being met for the first time
    /// any more, and what it has to offer is the look and the work.
    /// </para>
    /// </remarks>
    private void Welcome()
    {
        var header = new StackPanel
        {
            Spacing = 8,
            Margin = new Thickness(0, 24, 0, 0),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };

        header.Children.Add(
            new TextBlock
            {
                Text = "- ROLA -",
                FontSize = 12,
                LetterSpacing = 2.4,
                Foreground = _skin.Muted,
                HorizontalAlignment = HorizontalAlignment.Center,
            }
        );

        var slogan = new SloganView(_skin) { Margin = new Thickness(0, 2, 0, 0) };

        header.Children.Add(slogan);

        slogan.Say(Program.SloganLines);

        if (_account is null)
        {
            Start(header);

            return;
        }

        // The body is the width of the three pictures and centred as a column, so that what is written over
        // them starts where they start whatever the window is: the three are the widest thing the screen has,
        // and a column of one width is what makes a line of copy and a row of pictures read as one block.
        var pictures = ThemePreview.Of(_theme.Configuration.LightTheme);
        var body = new StackPanel
        {
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            Width = ((pictures.PixelSize.Width * ModeScale) + 4) * 3 + (ModeSpacing * 2),
        };

        body.Children.Add(
            new TextBlock
            {
                Text = RolaI18N.Get("launcher.welcome.appearance"),
                FontSize = 12,
                Foreground = _skin.Muted,
                HorizontalAlignment = HorizontalAlignment.Left,
                Margin = new Thickness(0, 18, 0, 2),
            }
        );

        body.Children.Add(Modes());

        body.Children.Add(
            new TextBlock
            {
                Text = RolaI18N.Get("launcher.welcome.empty"),
                FontSize = 13,
                Foreground = _skin.Muted,
                HorizontalAlignment = HorizontalAlignment.Left,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 18, 0, 2),
            }
        );

        body.Children.Add(Actions());

        var panel = new StackPanel { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Stretch };

        panel.Children.Add(header);
        panel.Children.Add(body);

        _content.Children.Add(panel);
    }

    /// <summary>
    /// What a machine with no account is shown: the one thing it can do, in the middle of the window.
    /// </summary>
    /// <remarks>
    /// The header stays where it is and the rest is centred in what is left, rather than the whole screen
    /// being centred as one group: the name and the line are where they are on every screen the program has,
    /// and what is different about this one is that there is something waiting to be done.
    /// </remarks>
    /// <param name="header">The name and the line, which are drawn at the top.</param>
    private void Start(StackPanel header)
    {
        var middle = new StackPanel
        {
            Spacing = 14,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        middle.Children.Add(
            new TextBlock
            {
                Text = RolaI18N.Get("launcher.welcome.no_account"),
                FontSize = 15,
                Foreground = _skin.Front,
                HorizontalAlignment = HorizontalAlignment.Center,
            }
        );

        middle.Children.Add(
            LabeledButton(
                LauncherIcons.Of(Icons.AccountCircle),
                RolaI18N.Get("launcher.welcome.account"),
                () => _ = OpenAccount()
            )
        );

        var screen = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*"),
            MinHeight = WelcomeHeight,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };

        screen.Children.Add(header);

        Grid.SetRow(middle, 1);
        screen.Children.Add(middle);

        _content.Children.Add(screen);
    }

    /// <summary>
    /// The three pictures of the window a look is chosen between.
    /// </summary>
    /// <remarks>
    /// Three pictures of this window rather than three of its words: what the look would be is easier to see
    /// than to be told, and the one in force says so by the frame it wears. They are drawn smaller than the
    /// welcome used to draw them, because the screen has more to say than a choice of look.
    /// </remarks>
    /// <returns>The row.</returns>
    private Control Modes()
    {
        var modes = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = ModeSpacing,
            HorizontalAlignment = HorizontalAlignment.Left,
        };

        var light = _theme.Configuration.LightTheme;
        var dark = _theme.Configuration.Theme;

        modes.Children.Add(
            new ThemeButton(
                ColorMode.Light,
                ThemePreview.Of(light),
                _theme.Mode == ColorMode.Light,
                _skin,
                Choose,
                ModeScale
            )
        );
        modes.Children.Add(
            new ThemeButton(
                ColorMode.Dark,
                ThemePreview.Of(dark),
                _theme.Mode == ColorMode.Dark,
                _skin,
                Choose,
                ModeScale
            )
        );
        modes.Children.Add(
            new ThemeButton(
                ColorMode.System,
                ThemePreview.HalfAndHalf(light, dark),
                _theme.Mode == ColorMode.System,
                _skin,
                Choose,
                ModeScale
            )
        );

        return modes;
    }

    /// <summary>
    /// The two things there are to do with no work in the window: make a Workspace, or take one that is there.
    /// </summary>
    /// <remarks>
    /// The two are centred as a pair rather than set against the column the copy is read in: they are what the
    /// screen is asking for, and what it asks for is the one thing on it that is not read left to right. They
    /// are the same width as one another with what they say against their own left edges, so that the pair
    /// reads as one row of choices rather than as two words of two lengths.
    /// </remarks>
    /// <returns>The row.</returns>
    private Control Actions()
    {
        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, 6, 0, 0),
        };

        _welcomeCreate = LabeledButton(
            LauncherIcons.Of(Icons.AddCircle),
            RolaI18N.Get("launcher.welcome.create"),
            () => _ = OpenCreate()
        );

        _welcomeCreate.Enabled = _account is not null;

        ToolTip.SetTip(
            _welcomeCreate,
            _account is null
                ? RolaI18N.Get("launcher.toolbar.create_needs_account")
                : RolaI18N.Get("launcher.toolbar.create")
        );

        var import = LabeledButton(
            LauncherIcons.Of(Icons.FolderOpen),
            RolaI18N.Get("launcher.welcome.import"),
            () => _ = Import()
        );

        row.Children.Add(_welcomeCreate);
        row.Children.Add(import);

        return row;
    }

    /// <summary>
    /// A button that says what it does: one of the pictures at its left edge, and the word for it centred.
    /// </summary>
    /// <remarks>
    /// The picture and the word are laid over one another rather than set side by side, because a row of the
    /// two would put the word after the picture instead of in the middle of the button: what is being read is
    /// the word, and the picture is the mark that says which of the buttons it is. The room the picture takes
    /// is left out of what the word is centred in only at its left, so a long word still has the whole of the
    /// rest to itself.
    /// </remarks>
    /// <param name="picture">The picture.</param>
    /// <param name="label">What it says.</param>
    /// <param name="pressed">What pressing it does.</param>
    /// <returns>The button.</returns>
    private FlatButton LabeledButton(Bitmap picture, string label, Action pressed)
    {
        var face = new Grid
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var glyph = LauncherIcons.Glyph(picture, 16, _skin.Front);

        glyph.HorizontalAlignment = HorizontalAlignment.Left;

        var text = new TextBlock
        {
            Text = label,
            FontSize = 13,
            Foreground = _skin.Front,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            TextAlignment = TextAlignment.Center,
        };

        face.Children.Add(text);
        face.Children.Add(glyph);

        var button = new FlatButton(face, _skin, ButtonKind.Surface)
        {
            Height = 38,
            MinWidth = ActionWidth,
            Padding = new Thickness(16, 0, 16, 0),
            HorizontalAlignment = HorizontalAlignment.Center,
        };

        button.Clicked += (_, _) => pressed();

        return button;
    }

    /// <summary>Chooses the day/night variant, which writes it and draws the window again.</summary>
    /// <param name="mode">The variant.</param>
    private void Choose(ColorMode mode)
    {
        try
        {
            _theme.Choose(mode);
        }
        catch (ConfigurationFailure failure)
        {
            _ = Notice(RolaI18N.Get("launcher.settings.title"), failure.Message);
        }
    }

    /// <summary>
    /// Opens the Desktop program onto a Workspace, and closes this one.
    /// </summary>
    /// <remarks>
    /// The window is asked for through the command line rather than started by name, because opening a
    /// window onto a Workspace is what that command is. What this program draws a frame in is handed over, so
    /// the window that opens is drawn the way this one is.
    /// </remarks>
    /// <param name="path">The Workspace.</param>
    private void Open(string path)
    {
        if (Workspaces.Of(path) != WorkspaceState.Workspace)
        {
            _ = Notice(RolaI18N.Get("launcher.workspace.title"), RolaI18N.Get("launcher.workspace.gone", path));

            return;
        }

        if (_rola.Launch(path, _frame))
        {
            Close();

            return;
        }

        _ = Notice(
            RolaI18N.Get("launcher.workspace.title"),
            RolaI18N.Get("launcher.workspace.launch_failed", _rola.Program)
        );
    }

    /// <summary>
    /// Takes a Workspace off the list, and says so if the list could not be written.
    /// </summary>
    /// <remarks>
    /// What is taken away is the entry, not the directory: a person tidying the list is not asking for their
    /// work to be deleted, and importing the same directory puts it back where it was.
    /// </remarks>
    /// <param name="path">The Workspace.</param>
    private void Remove(string path)
    {
        if (_opened.Remove(path))
        {
            Save();
        }

        Fill();
    }

    /// <summary>Opens the window that makes sure there is an account.</summary>
    private async Task OpenAccount()
    {
        var dialog = new AccountDialog(_skin, _files, _accounts);

        await dialog.ShowDialog(this);

        _account = _accounts.List(_files.Root).Current;

        Fill();
    }

    /// <summary>Opens the window that creates a Workspace.</summary>
    private async Task OpenCreate()
    {
        if (_account is null)
        {
            await Notice(RolaI18N.Get("launcher.toolbar.account"), RolaI18N.Get("launcher.toolbar.create_needs_account"));

            return;
        }

        var dialog = new CreateWorkspaceDialog(_skin, _rola, _connection);

        await dialog.ShowDialog(this);

        if (dialog.Created is { } made)
        {
            _opened.Add(made);
            Save();
            Fill();
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

        if (Workspaces.Of(picked) != WorkspaceState.Workspace)
        {
            await Notice(RolaI18N.Get("launcher.import.title"), RolaI18N.Get("launcher.import.not_a_workspace", picked));

            return;
        }

        if (_opened.Add(picked))
        {
            Save();
        }

        Fill();
    }

    /// <summary>Opens the window that says what this program may be told.</summary>
    private async Task OpenSettings()
    {
        var dialog = new SettingsDialog(_skin, _theme, _settings, _files, _machine);

        await dialog.ShowDialog(this);

        _frame = dialog.Frame;

        // The look may have changed while the window was open, and the window's own buttons are drawn
        // where the frame puts them rather than where they were.
        _chrome.Arrange(_skin, _frame);
    }

    /// <summary>
    /// Asks for another picture for the banner, and puts it up when it is one that can be read.
    /// </summary>
    /// <remarks>
    /// What is chosen is read before it is copied over the picture that is there: a file that is not an
    /// aseprite one would otherwise be the end of a banner that was working.
    /// </remarks>
    private async Task ReplaceBanner()
    {
        // The pictures that shipped with the program are where a replacement is looked for first, since
        // they are the ones a person is choosing between when they have not drawn one of their own.
        var start = Directory.Exists(_files.Banners)
            ? await StorageProvider.TryGetFolderFromPathAsync(_files.Banners)
            : null;

        var files = await StorageProvider.OpenFilePickerAsync(
            new FilePickerOpenOptions
            {
                Title = RolaI18N.Get("launcher.banner.pick"),
                AllowMultiple = false,
                SuggestedStartLocation = start,
                FileTypeFilter =
                [
                    new FilePickerFileType(RolaI18N.Get("launcher.banner.type"))
                    {
                        Patterns = ["*.aseprite"],
                    },
                ],
            }
        );

        if (files.Count == 0 || files[0].TryGetLocalPath() is not { } chosen)
        {
            return;
        }

        AsepriteAnimation animation;

        try
        {
            animation = Aseprite.Read(chosen);
        }
        catch (Exception error)
            when (error is IOException or InvalidDataException or UnauthorizedAccessException)
        {
            await Notice(RolaI18N.Get("launcher.banner.pick"), RolaI18N.Get("launcher.banner.problem", error.Message));

            return;
        }

        try
        {
            Directory.CreateDirectory(_files.Root);
            File.Copy(chosen, _files.Banner, overwrite: true);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            await Notice(RolaI18N.Get("launcher.banner.pick"), RolaI18N.Get("launcher.banner.problem", error.Message));

            return;
        }

        _animation = animation;
        _bannerProblem = null;

        _banner.Play(animation);

        Build();
    }

    /// <summary>
    /// Reads the picture the banner plays, and the one that ships with the program when there is none.
    /// </summary>
    /// <remarks>
    /// The picture is the user's own once there is one at the path the program reads. A machine that has
    /// never opened the launcher has none, so the first of the pictures that shipped with it is copied
    /// there: what the program shows is then a file the person can replace, in the place the program reads.
    /// </remarks>
    private void LoadBanner()
    {
        try
        {
            if (!File.Exists(_files.Banner))
            {
                Ship();
            }

            if (!File.Exists(_files.Banner))
            {
                _animation = null;
                _bannerProblem = null;

                return;
            }

            _animation = Aseprite.Read(_files.Banner);
            _bannerProblem = null;
        }
        catch (Exception error)
            when (error is IOException or InvalidDataException or UnauthorizedAccessException)
        {
            _animation = null;
            _bannerProblem = error.Message;
        }
    }

    /// <summary>Copies the first of the pictures that shipped with the program where the banner is read.</summary>
    private void Ship()
    {
        if (!Directory.Exists(_files.Banners))
        {
            return;
        }

        var first = Directory
            .EnumerateFiles(_files.Banners, "*.aseprite")
            .OrderBy(path => path, StringComparer.Ordinal)
            .FirstOrDefault();

        if (first is null)
        {
            return;
        }

        Directory.CreateDirectory(_files.Root);
        File.Copy(first, _files.Banner);
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
    /// <param name="heading">What it is about.</param>
    /// <param name="message">What it says.</param>
    /// <returns>A task that ends when it is read.</returns>
    private Task Notice(string heading, string message) =>
        NoticeDialog.Show(this, _skin, heading, message, RolaI18N.Get("launcher.common.ok"));
}

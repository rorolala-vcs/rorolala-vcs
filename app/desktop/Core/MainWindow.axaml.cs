using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;
using RorolalaDesktop.Notifications;
using RorolalaDesktop.Progress;
// Named rather than imported whole, because the contract has a menu item of its own and the toolkit has one
// too: what a window's menu is built from is the toolkit's, and only these two are the contract's.
using DockHeaderCommand = RorolalaDesktop.Contract.DockHeaderCommand;
using DockHeaderView = RorolalaDesktop.Contract.DockHeaderView;
using RorolalaDesktop.Theming;
using ContractMenuItem = RorolalaDesktop.Contract.MenuItem;
using DockOpenMode = RorolalaDesktop.Contract.DockOpenMode;
using DockRegistration = RorolalaDesktop.Contract.DockRegistration;
using Notification = RorolalaDesktop.Contract.Notification;
using NotificationLevel = RorolalaDesktop.Contract.NotificationLevel;
using PluginId = RorolalaDesktop.Contract.PluginId;
using Shape = Avalonia.Controls.Shapes.Shape;
using ShapePath = Avalonia.Controls.Shapes.Path;

namespace RorolalaDesktop;

/// <summary>
/// A window of the program: the row its menu and its own buttons share, and the dock area below it.
/// </summary>
/// <remarks>
/// Both are filled from what was registered, the kernel's own entries and the plugins', and a
/// plugin registers during initialization, which is before this window is made, so the menu is built
/// once here and the area is kept in step by the dock manager.
/// <para>
/// There is one window class for the whole program, because the main window and a window a dock was moved
/// into are the same window: the same row, the same frame, the same buttons. What tells them apart is the
/// dock the window is for, and nothing else, a window with no such dock is the main one, and shows
/// everything; a window that is for a dock belongs to that dock's plugin, and shows that plugin's own
/// menus. What follows from that is the dock area, which draws the docks that are in this window whichever
/// window it is.
/// </para>
/// <para>
/// No title bar of the desktop's is drawn: the row above the dock area is the window's whole frame for
/// as much of it as is seen, and what the toolkit takes from it is said in the markup. What is left
/// here is the part of it the host owns, the buttons' words and their pictures, which is why the
/// window is the one that says them.
/// </para>
/// </remarks>
public partial class MainWindow : Window
{
    /// <summary>
    /// The height of the row the menu and the window's own buttons share.
    /// </summary>
    /// <remarks>
    /// One number rather than three that have to agree: the row is this tall, the decorations are told
    /// to reserve this much for a title bar, and the window is told the same, because a title bar the
    /// framework believes is a different height from the one that is drawn leaves a strip at the top
    /// that drags nothing and a strip under it that drags the window. It is a field rather than a
    /// property because the markup reads it with <c>x:Static</c>.
    /// <para>
    /// Every window of the host has this row at this height, the main one and a window that is one dock
    /// alike: a window of one dock has no strip of tabs to stand in for, so there is nothing for its row to
    /// be taller than the menu bar the two share, and two rows of one program that stood at two heights
    /// would step as the eye moved between them.
    /// </para>
    /// </remarks>
    public const double TitleBarHeight = 32.0;

    /// <summary>How large the menu button's picture is, which is the size a toolbar's tools are drawn at.</summary>
    private const double MenuGlyphSize = 18;

    /// <summary>How much of the toolbar size the menu's mark takes.</summary>
    private const double MenuGlyphScale = 0.8;

    /// <summary>The label key of the menu the registered docks appear under.</summary>
    private const string WindowMenu = "menu.window";

    /// <summary>Everything the host holds.</summary>
    private readonly HostServices _services;

    /// <summary>
    /// This window's key, or nothing for the main one.
    /// </summary>
    /// <remarks>
    /// Carried on the window itself as well as here, because the one that hears a window closed is the
    /// window: what a closed window has to say about itself is which window it was.
    /// </remarks>
    private readonly string? _key;

    /// <summary>
    /// The plugin whose menus this window shows, or nothing for every one of them.
    /// </summary>
    /// <remarks>
    /// A window that is for a dock belongs to that dock's plugin, and its row is that plugin's own: what a
    /// window of one dock is for is that dock, and a menu about a tree has nothing to say in a window that
    /// holds a browser. The kernel's own menus are shown either way, the Window menu is where the docks and
    /// the way back to the main window are, in every window there is.
    /// </remarks>
    private readonly PluginId? _scope;

    /// <summary>
    /// The dock this window is for, or nothing for the main one.
    /// </summary>
    /// <remarks>
    /// Held because a window of one dock is that dock's window: what the dock says about itself is said in
    /// this window's own row, which is read long after the window was made.
    /// </remarks>
    private readonly Docking.DockInstance? _dock;

    /// <summary>
    /// This window's frame, which is what draws its own buttons.
    /// </summary>
    /// <remarks>
    /// Made by the window rather than named as a resource, because every other window of the program, a
    /// plugin's question, a notice, is a card with a title bar of the desktop's. It is held because the
    /// buttons it builds are the ones this window has to say the words and the pictures of.
    /// </remarks>
    private readonly RorolalaTheme.Frame _frame;

    /// <summary>
    /// The stack notifications are shown in, or nothing for a window that is one dock.
    /// </summary>
    /// <remarks>
    /// Notifications belong to the program's main window rather than to every window the host may open: a
    /// dock torn into a window of its own is a view of one part of the work, and a failure reported while the
    /// user is there is still the program's, not that dock's.
    /// </remarks>
    private readonly NotificationStack? _notifications;

    /// <summary>The docks as they appear in the Window menu, so their checks can be kept in step.</summary>
    private readonly List<(DockRegistration Registration, MenuItem Entry)> _docks = [];

    /// <summary>The menus the title row's button opens.</summary>
    private readonly MenuFlyout _menus = new() { Placement = PlacementMode.BottomEdgeAlignedLeft };

    /// <summary>
    /// The drag the name of the one dock this window holds has begun, if it has begun one.
    /// </summary>
    /// <remarks>
    /// The name in the row is the handle a dock of its own is brought back to the main window by, and what
    /// the pointer did with it is this gesture: held only while a press may still become a drag, which is
    /// what <see cref="DockDrag"/> is.
    /// </remarks>
    private DockDrag? _drag;

    /// <summary>The covering the window was made with, or nothing for a window with nothing to cover.</summary>
    private readonly LoadingOverlay? _loading;


    /// <summary>Makes the window over what was registered.</summary>
    /// <param name="services">Everything the host holds.</param>
    internal MainWindow(HostServices services)
        : this(services, null, null) { }

    /// <summary>Makes the window, either the main one or one a dock of its own is shown in.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="key">
    /// This window's key, or nothing for the main one. Written on the window, because the window is what is
    /// heard from when it closes and the one thing it has to say is which window it was.
    /// </param>
    /// <param name="dock">The dock this window is for, or nothing for the main one.</param>
    /// <param name="loading">
    /// Whether the window is drawn under a covering until the host says its work is done. True for the main
    /// window made while the host is still assembling, and false for one opened over a host that is assembled:
    /// a covering there would be covering work that has already been done.
    /// </param>
    internal MainWindow(HostServices services, string? key, Docking.DockInstance? dock, bool loading = false)
    {
        InitializeComponent();

        _services = services;
        _key = key;
        _dock = dock;
        _scope = dock?.Registration.Owner;

        // The key travels with the window, since the window outlives the list it is in: what it is closes
        // over nothing, and the set of windows remembers it by what it says about itself.
        Tag = key;

        // The host's own picture, and the one every other window of the program is drawn with; the
        // dialogs this window shows take it from the same place rather than making one of their own.
        Icon = services.ProgramIcon;

        // The window is named where every other word is, so changing what it is called is a change
        // to a translation rather than to this file. A window built with no translations to read,
        // which is what the designer builds, keeps the name the markup gives it.
        if (RolaI18N.TranslationDirectory is not null)
        {
            // The main window is the program; a window that is one dock is that dock, and says which: what a
            // window of a dock's own is for is the dock in it, and a row of them all called the same thing is
            // a row nobody can tell apart. The dock's name is the one its own tab would have carried.
            Title =
                _dock is { } alone
                    ? _services.I18n.Get("window.titled", alone.Title)
                    : _services.I18n.Get("window.title");
        }

        // The height the toolkit reserves for the title bar is the height the row above the dock area is drawn
        // at, and both are one number: the row is a window's own, and every window of the host draws it at the
        // menu bar's height. The frame is told the same number, because what it keeps for that row is where its
        // own buttons are drawn: a frame that kept room for a shorter row than this window draws would draw its
        // buttons where the row is not.
        TitleBar.Height = TitleBarHeight;
        WindowTitleLayer.Height = TitleBarHeight;
        ExtendClientAreaTitleBarHeightHint = TitleBarHeight;
        _frame = RorolalaTheme.DecorationsTheme(Program.WindowStyle, TitleBarHeight);

        // There is no title bar of the desktop's left to move the window, so the row this program draws
        // in its place does it: a press on the row that no menu and no button claimed is a press on the
        // window, and the second click of a double click is what maximizes or restores it, the two
        // gestures a title bar of the desktop's would have answered by itself. A press on a menu's own row
        // is the menu's and is left to it (see `MoveOrMaximize`).
        //
        // One press, one listener, and it is the row's own surface rather than the menu bar over it: the
        // menu is inside the row, so a press on the one passes through the other, and two listeners would
        // answer the same double click twice, maximizing the window and restoring it again. Taken on the
        // way down so that the window is asked to move before the menu reads the press, which is the order
        // the paragraph above is about; the press is left unhandled, so a menu still opens under it.
        //
        // Asked here rather than left to the role, which moves the window only on a display server that
        // draws its own decorations: a desktop that draws none is the one this is written for, and it
        // reads no role. Both are asked, so a press moves the window either way.
        TitleBar.AddHandler(
            InputElement.PointerPressedEvent,
            (_, press) => MoveOrMaximize(press),
            RoutingStrategies.Tunnel
        );

        // What each of the window's buttons says and what it is drawn as. The buttons are the frame's,
        // see the look, which builds them out of the program's own pictures, and what is said here is the
        // part of them the host owns: the words a reader hears them by, which are translated, and the
        // picture that says what the one that maximizes will do next.
        //
        // A frame drawn for a tiling window manager has none of them, which is why each is asked for rather
        // than assumed: what the window says about a button it was not given is nothing.
        Describe(_frame.Minimize, "window.minimize");
        Describe(_frame.Close, "window.close");
        WindowControls.Picture(_frame.Minimize, Glyph(WindowControls.MinimizeKey));
        WindowControls.Picture(_frame.Close, Glyph(WindowControls.CloseKey));
        MaximizePicture();

        // The button that maximizes says what it will do next rather than what it did, so what it carries
        // follows the state the window is in: a press leaves the window maximized, and the same button then
        // has to offer the way back. The state is watched rather than the click answered to, because a
        // window can be maximized without this button at all, a double click on the row, the window
        // manager's own key, and a button still offering to maximize an already maximized window lies.
        PropertyChanged += (_, change) =>
        {
            if (change.Property == WindowStateProperty)
            {
                MaximizePicture();
            }
        };

        // The frame: no title bar of the desktop's, and the row above the dock area to stand in for it. It
        // is put on the window rather than in the look's own resources because a dialog of a plugin's has a
        // title bar of its own, and this would take it away.
        WindowDecorationsTheme = _frame.Theme;

        // The row no longer carries a menu bar: one button opens the same top-level menus as a flyout. The
        // button is the row's own, so a press on it is answered by the button before the row asks whether to
        // move the window, and the flyout is drawn over the docks rather than in the row's visual tree.
        MenuButton.Flyout = _menus;
        MenuButton[!TemplatedControl.ForegroundProperty] =
            new DynamicResourceExtension(RorolalaDesktop.Contract.PaletteKeys.Front);
        MenuButton.Content = MenuGlyph();

        // The macOS frame puts its three lights at the left, so the menu belongs at the right. Every other
        // frame leaves the left of the row free and keeps its buttons at the right.
        var (menuColumn, menuSide) = TitlePlacement(Program.WindowStyle);

        Grid.SetColumn(MenuGroup, menuColumn);
        MenuGroup.HorizontalAlignment = menuSide;

        // The macOS frame draws its own edge over the last few pixels of the row, so the button is kept clear of
        // it rather than being clipped by the frame it sits in.
        if (Program.WindowStyle == Theming.WindowStyle.Mac)
        {
            MenuButton.Margin = new Thickness(0, 0, 8, 0);
        }
        MenuButton.Click += (_, _) => _menus.ShowAt(MenuButton);
        ToolTip.SetTip(MenuButton, _services.I18n.Get("window.menu"));
        AutomationProperties.SetName(MenuButton, _services.I18n.Get("window.menu"));

        // The main window is the program, so its row carries the program's name and the workspace it was
        // started on. A window of one dock is that dock, and its row carries the dock's title instead.
        if (_dock is null)
        {
            var brand = Brand();

            // The menu is at the left on every frame but the macOS one, where the lights are, so the brand
            // stands beside it there and at the row's left end under macOS.
            if (menuColumn == 0)
            {
                MenuGroup.Children.Add(brand);
            }
            else
            {
                Grid.SetColumn(brand, 0);
                TitleRow.Children.Add(brand);
            }
        }

        BuildMenu();
        Caption();
        // A window of one dock is named by that dock, and a dock that names itself by where it is names the window
        // again whenever it goes somewhere else. Subscribed after the row is drawn, since what it says is the row's.
        if (_dock is not null)
        {
            _services.Docks.Renamed += Renamed;
        }

        DockAreaHost.Content = new DockArea(
            _services.Docks,
            _services.I18n,
            _services.Windows,
            _key
        );

        // Every window shows the same host-wide runs: a command begun from a menu has to reach a window that may
        // not exist yet, and a window opened after it began has to see it.
        ProgressLayer.Children.Add(new ProgressStack(_services.Progress, _services.Icons));

        if (_key is null)
        {
            _notifications = new NotificationStack();
            NotificationLayer.Children.Add(_notifications);

            // A window made while the host is still assembling is drawn under a black covering, so that the
            // first frame is the window rather than what a host with nothing in it looks like. The ground is
            // black as well, because the window's own buttons are the frame's: they are drawn over the client
            // area rather than in it, and the covering is not what paints under them.
            if (loading)
            {
                Background = Brushes.Black;

                _loading = new LoadingOverlay();
                Root.Children.Add(_loading);
            }
        }

        // A dock can be closed, opened, or dragged to another region by hand as well as from the
        // menu, so the checks are refreshed and the layout written whenever the docks change, a
        // dock moved by hand is remembered from then on, not only if the run ends tidily.
        _services.Docks.Changed += DocksChanged;

        // Coming back to the window is when something outside the program is most likely to have changed what
        // a plugin reads, so what the host is told about it is said on to the plugins here.
        Activated += (_, _) => _services.Refocus.Regain();

        Opened += (_, _) =>
        {
            ShowPopups();
            ShowNotifications();
        };

        // What is raised once this window is up is shown as it is raised, rather than waiting for another
        // window to be opened: a failure met while the program is running is one to act on then.
        _services.Popups.Raised += PopupsRaised;

        if (_key is null)
        {
            _services.Notifications.Posted += NotificationsPosted;
        }

        // The layout is what the main window's regions were, so it is written when the main window is the one
        // going: a window of a dock's own has no regions to remember, and writing the layout from it would be
        // writing the main window's arrangement from a window that is not the main one.
        Closing += (_, _) =>
        {
            if (_key is null)
            {
                LayoutStore.Save(_services.Log, _services.Docks.Snapshot());
            }
        };

        // And a window that has gone stops being told about the docks: it has no regions to keep in step, and
        // what is left of it needs no waking, a window that kept listening would be one the dock manager
        // holds for the rest of the run, and one that answered a change by rebuilding a row nobody can see.
        Closed += (_, _) =>
        {
            _services.Docks.Changed -= DocksChanged;
            _services.Docks.Renamed -= Renamed;
            _services.Popups.Raised -= PopupsRaised;

            if (_key is null)
            {
                _services.Notifications.Posted -= NotificationsPosted;
            }
        };
    }

    /// <summary>
    /// Says the window's name again where the dock it holds has gone somewhere else.
    /// </summary>
    /// <remarks>
    /// A window of one dock is that dock, and a dock named by where it is says the window's name too: both the name the
    /// desktop is given and the word in the row are the dock's, which is what keeps a row of such windows readable.
    /// </remarks>
    /// <param name="instance">The dock whose name says something else.</param>
    private void Renamed(Docking.DockInstance instance)
    {
        if (!ReferenceEquals(_dock, instance))
        {
            return;
        }

        Title = _services.I18n.Get("window.titled", instance.Title);

        if (_dock is not null)
        {
            WindowCaption.Text = instance.Title;
            ToolTip.SetTip(WindowCaption, instance.Detail);
        }
    }

    /// <summary>
    /// The program's name and the workspace it was started on, in the row the window draws for itself.
    /// </summary>
    /// <remarks>
    /// The name is written out rather than translated: it is the program's, and a translation of a name is a
    /// second name. The workspace is the directory the program was started in, which is what the command line
    /// hands over when it opens the window.
    /// </remarks>
    /// <returns>The row's brand.</returns>
    private static Control Brand()
    {
        var brand = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Margin = new Thickness(10, 0, 0, 0),
        };

        brand.Children.Add(
            new TextBlock
            {
                Text = "Rorolala",
                Classes = { "title" },
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        var workspace = WorkspaceName();

        if (workspace.Length == 0)
        {
            return brand;
        }

        var rule = new Border
        {
            Width = 1,
            Height = 14,
            Margin = new Thickness(2, 0, 2, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        rule[!Border.BackgroundProperty] = new DynamicResourceExtension(
            RorolalaDesktop.Contract.PaletteKeys.Splitter
        );

        brand.Children.Add(rule);
        brand.Children.Add(
            new TextBlock
            {
                Text = workspace,
                Classes = { "caption", "muted" },
                VerticalAlignment = VerticalAlignment.Center,
                TextTrimming = TextTrimming.CharacterEllipsis,
                MaxWidth = 280,
            }
        );

        return brand;
    }

    /// <summary>The name of the directory the program was started in, or that directory when it has no name.</summary>
    /// <returns>The workspace's name.</returns>
    private static string WorkspaceName()
    {
        var directory = string.IsNullOrWhiteSpace(Program.CurrentDirectory)
            ? Environment.CurrentDirectory
            : Program.CurrentDirectory;
        var trimmed = Path.TrimEndingDirectorySeparator(directory);
        var name = Path.GetFileName(trimmed);

        return name.Length > 0 ? name : trimmed;
    }

    /// <summary>
    /// Puts the title of the one dock this window holds over the row, and the handle it is dragged back to
    /// the main window by beside the menu.
    /// </summary>
    /// <remarks>
    /// A window of one dock has no strip of tabs, so the dock's name has nowhere to be but the row the
    /// desktop's title bar would have drawn. The name is a title here rather than a handle: a title says what
    /// the window is, and a title that was also a drag was a word that looked pressable and moved the dock when
    /// a reader meant only to read it. The drag is the hand beside the menu, which is where the row's own
    /// controls are.
    /// <para>
    /// The title is laid over the row rather than in it: the row is inset by the room the frame's buttons take
    /// on one side and the menu on the other, so a title laid out in it would be off the window's centre. Equal
    /// margins keep it clear of both ends.
    /// </para>
    /// </remarks>
    private void Caption()
    {
        if (_dock is not { } instance)
        {
            return;
        }

        WindowTitleLayer.IsVisible = true;
        WindowCaption.Text = instance.Title;
        ToolTip.SetTip(WindowCaption, instance.Detail);
        WindowCaption.Margin = Program.WindowStyle switch
        {
            Theming.WindowStyle.Mac => new Thickness(80, 0, 80, 0),
            Theming.WindowStyle.TillingWm => new Thickness(8, 0, 8, 0),
            _ => new Thickness(150, 0, 150, 0),
        };

        // The title is over the row rather than in it, so a press on its words is the window's to move as
        // well: the row's own handler never sees a press on a sibling drawn above it.
        WindowCaption.PointerPressed += (_, press) => MoveOrMaximize(press);

        var handle = DragHandle(instance);

        // The hand stands beside the menu, on the inside of it: the menu keeps the frame's own edge, and the
        // hand is the next thing inwards under macOS.
        if (Program.WindowStyle == Theming.WindowStyle.Mac)
        {
            MenuGroup.Children.Insert(0, handle);
        }
        else
        {
            MenuGroup.Children.Add(handle);
        }

        // The drag is followed over the whole window rather than over the button: a button answers a press
        // with a click of its own, and what its class handlers do to a move is not this gesture's business.
        // The window's tunnel handlers see the move first and whether or not the button marks it handled.
        AddHandler(
            InputElement.PointerMovedEvent,
            (_, moved) => _drag?.Follow(moved, this),
            RoutingStrategies.Tunnel,
            handledEventsToo: true
        );
        AddHandler(
            InputElement.PointerReleasedEvent,
            (_, _) => _drag = null,
            RoutingStrategies.Tunnel,
            handledEventsToo: true
        );
    }

    /// <summary>The hand a window of one dock is dragged back to the main window by.</summary>
    /// <remarks>
    /// A press on it begins the dock's drag rather than the window's move, which is what the row's own press
    /// does everywhere else: the button takes the press before the row reads it, the same way the menu does. The
    /// pointer is kept until it is let go of, so a hand that leaves the button before it has moved far enough is
    /// still a drag of the dock.
    /// </remarks>
    /// <param name="instance">The dock this window holds.</param>
    /// <returns>The handle.</returns>
    private Button DragHandle(Docking.DockInstance instance)
    {
        var handle = new Button
        {
            Classes = { "tool" },
            VerticalAlignment = VerticalAlignment.Center,
            Content = HandGlyph(),
        };

        // The picture is a mask over the button's foreground, and the row does not set one for this button
        // the way it does for the menu's: a foreground left to the window is one the macOS frame may answer
        // with nothing, which is a hand drawn in nothing.
        handle[!TemplatedControl.ForegroundProperty] = new DynamicResourceExtension(
            RorolalaDesktop.Contract.PaletteKeys.Front
        );

        ToolTip.SetTip(handle, _services.I18n.Get("window.drag"));
        AutomationProperties.SetName(handle, _services.I18n.Get("window.drag"));

        // Taken on the way down, before the button's own handlers read the press: a button answers a press
        // with a click, and a click is not a drag. The pointer is captured by the window, so a hand that
        // leaves the button before it has moved far enough is still followed.
        handle.AddHandler(
            InputElement.PointerPressedEvent,
            (_, press) =>
            {
                if (!press.GetCurrentPoint(handle).Properties.IsLeftButtonPressed)
                {
                    return;
                }

                press.Handled = true;
                _drag = new DockDrag(_services.Windows, instance, _key);
                _drag.Take(press, this);
                press.Pointer.Capture(handle);
            },
            RoutingStrategies.Tunnel,
            handledEventsToo: true
        );

        handle.PointerCaptureLost += (_, _) => _drag = null;

        return handle;
    }

    /// <summary>The hand a window of one dock is dragged back to the main window by, in the row's own ink.</summary>
    /// <remarks>
    /// Drawn rather than read from the picture library, for the reason the menu's three dots are: the mark is part
    /// of the row, and a picture put through an opacity mask is a second brush the row's ink has to find its way
    /// into. The geometry is the `back_hand` outline the icon manifest names, so the two are the same drawing.
    /// </remarks>
    /// <returns>The hand.</returns>
    private static Control HandGlyph()
    {
        var hand = new ShapePath
        {
            Data = Hand,
            Stretch = Stretch.Uniform,
            Width = WindowControls.GlyphSize,
            Height = WindowControls.GlyphSize,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        hand[!Shape.FillProperty] = new DynamicResourceExtension(
            RorolalaDesktop.Contract.PaletteKeys.Front
        );

        return hand;
    }

    /// <summary>The `back_hand` outline, in the twenty-four unit box the set draws in.</summary>
    private static readonly Geometry Hand = Geometry.Parse(
        "M13 24c-3.26 0-6.19-1.99-7.4-5.02l-3.03-7.61a1 1 0 0 1 1.24-1.32l.79.26c.56.18 1.02.61 1.24 1.16L7.25 15H8V3.25a1.25 1.25 0 0 1 2.5 0V12h1V1.25a1.25 1.25 0 0 1 2.5 0V12h1V2.75a1.25 1.25 0 0 1 2.5 0V12h1V5.75a1.25 1.25 0 0 1 2.5 0V16c0 4.42-3.58 8-8 8z"
    );

    /// <summary>
    /// Moves the window for a press on the row, or maximizes it for the second click of a double click.
    /// </summary>
    /// <remarks>
    /// The move is asked of the toolkit rather than done by hand, because on a desktop it is the window
    /// manager's own move and dragging it out by arithmetic would lose the snap to a screen edge and the
    /// bounds a maximized window restores to. A press that landed on a menu, on one of the window's own
    /// buttons, or on the name of the dock this window holds is left alone rather than moved, because each
    /// of the three is a part of the row that answers a press itself, a menu answers the release of one, a
    /// button is the toolkit's, and the name begins a drag of the dock.
    /// <para>
    /// The press is left unhandled, because a menu still has to open under a press that landed on one of
    /// its menus, and what is answered here is the moving of the window rather than the press.
    /// </para>
    /// </remarks>
    /// <param name="press">What was pressed.</param>
    private void MoveOrMaximize(PointerPressedEventArgs press)
    {
        // A press that landed on a menu, on one of the window's own buttons, or on the hand a window of one
        // dock is dragged back by is left alone rather than moved, because each of the three is a part of the
        // row that answers a press itself: a menu answers the release of one, a button is the toolkit's, and
        // the hand begins a drag of the dock.
        if (press.Handled || !press.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            return;
        }

        if (ButtonUnder(press.Source) is not null)
        {
            return;
        }

        if (press.ClickCount == 2)
        {
            WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
            return;
        }

        if (press.ClickCount == 1)
        {
            BeginMoveDrag(press);
        }
    }

    /// <summary>The button a press landed in, or nothing when it landed elsewhere on the row.</summary>
    /// <remarks>
    /// The press reaches the row before the button handles it, because the row's handler is on the way down and the
    /// button's is on the way up. What the pointer hit is part of the button's own template rather than the button,
    /// so the parents are walked.
    /// </remarks>
    /// <param name="pressed">What the press landed on.</param>
    /// <returns>The button holding it, or nothing.</returns>
    private static Button? ButtonUnder(object? pressed)
    {
        for (var at = pressed as StyledElement; at is not null; at = at.Parent)
        {
            if (at is Button button)
            {
                return button;
            }
        }

        return null;
    }

    /// <summary>The menu button's picture: the three dots of a vertical more menu, in the ordinary ink.</summary>
    /// <remarks>
    /// Drawn rather than read from the picture library, because this is the one mark in the row that is not a
    /// window's button: a picture masked by a resource binding is a picture that can be present and still draw
    /// nothing, and three dots are the whole of what the mark is.
    /// </remarks>
    /// <returns>The mark.</returns>
    private static Control MenuGlyph()
    {
        var extent = MenuGlyphSize * MenuGlyphScale;
        var dot = extent / 5;
        var glyph = new Canvas { Width = extent, Height = extent };

        for (var at = 0; at < 3; at++)
        {
            var mark = new Border
            {
                Width = dot,
                Height = dot,
                CornerRadius = new CornerRadius(dot / 2),
            };

            mark[!Border.BackgroundProperty] =
                new DynamicResourceExtension(RorolalaDesktop.Contract.PaletteKeys.Front);
            Canvas.SetLeft(mark, (extent - dot) / 2);
            Canvas.SetTop(mark, (extent * (0.2 + (at * 0.3))) - (dot / 2));
            glyph.Children.Add(mark);
        }

        return glyph;
    }

    /// <summary>
    /// Draws the maximize button as what pressing it would do to the window as it stands.
    /// </summary>
    /// <remarks>
    /// The picture follows the window's state rather than the press, because a window can be maximized
    /// without this button (a double click on the row, the window manager's own shortcut) and a button
    /// still offering to maximize a maximized window is a button that lies. The words follow it for the
    /// same reason: what a reader is told the button does has to be what pressing it does.
    /// </remarks>
    private void MaximizePicture()
    {
        // A frame drawn for a tiling window manager has no such button, and a window with none has nothing
        // to say about it.
        if (_frame.Maximize is not { } button)
        {
            return;
        }

        var maximized = WindowState == WindowState.Maximized;

        WindowControls.Picture(
            button,
            Glyph(maximized ? WindowControls.RestoreKey : WindowControls.MaximizeKey)
        );
        AutomationProperties.SetName(
            button,
            _services.I18n.Get(maximized ? "window.restore" : "window.maximize")
        );
    }

    /// <summary>
    /// Gives one of the window's buttons the words it is read and reached by, where it has one.
    /// </summary>
    /// <param name="button">The button, or nothing where the frame has none.</param>
    /// <param name="key">The translated key the name is read from.</param>
    private void Describe(Button? button, string key)
    {
        if (button is not null)
        {
            WindowControls.Describe(button, key, _services.I18n);
        }
    }

    /// <summary>
    /// The picture a button carries, or nothing when the program was not drawn with one.
    /// </summary>
    /// <remarks>
    /// Nothing rather than a failure: a button without a picture is still a button, and the window it
    /// closes or minimizes is the same window. What must not happen is a window that does not open because
    /// a picture it was drawn from was not beside the program.
    /// <para>
    /// Drawn smaller where the frame is the three lights of a macOS one, because a light is smaller than a
    /// button: see <see cref="WindowControls.LightGlyphSize"/>.
    /// </para>
    /// </remarks>
    /// <param name="key">The key the picture is held under.</param>
    /// <param name="extent">How large to draw it, or nothing for the frame's own size.</param>
    /// <returns>What to put in the button, or nothing.</returns>
    private Control? Glyph(string key, double? extent = null) =>
        WindowControls.Find(_services.Icons, key) is { } picture
            ? WindowControls.Glyph(
                picture,
                extent
                    ?? (
                        Program.WindowStyle == Theming.WindowStyle.Mac
                            ? WindowControls.LightGlyphSize
                            : WindowControls.GlyphSize
                    )
            )
            : null;

    /// <summary>
    /// The top-level menus a window shows, given the plugin whose window it is.
    /// </summary>
    /// <remarks>
    /// A window that is for a dock shows that dock's plugin's menus and the kernel's own, and a window that
    /// is for nothing, the main one, shows every menu there is. What a window of one dock is for is that
    /// dock, so a menu about a tree has nothing to say in a window that holds a browser; what the program
    /// itself has to say, which is the Window menu and the way back to the main window, is said everywhere.
    /// <para>
    /// A question about what was registered and nothing about a window, so that what a window's row is made
    /// of can be asked without making one, and answered here rather than in the loop below, so that the
    /// answer and what is drawn cannot come apart.
    /// </para>
    /// </remarks>
    /// <param name="menu">What was registered.</param>
    /// <param name="scope">The plugin whose window this is, or nothing for every one of them.</param>
    /// <returns>The menus to show, in the order they are shown.</returns>
    internal static IReadOnlyList<TopLevelMenu> Visible(MenuRegistry menu, PluginId? scope)
    {
        ArgumentNullException.ThrowIfNull(menu);

        return
        [
            .. menu.TopLevels.Where(top => scope is not { } mine || top.Owner == mine || top.Owner == Core.Id),
        ];
    }

    /// <summary>
    /// Builds the menu bar again from what is registered.
    /// </summary>
    /// <remarks>
    /// The bar was built when the window was made, which is before the plugins have registered the menus they
    /// contribute; this puts theirs in once there are any. It is safe to call again because the bar is built
    /// from the registries rather than added to, see <see cref="BuildMenu"/>, which takes the old one down
    /// first, and it is what a main window made before the plugins were initialized needs.
    /// </remarks>
    internal void RebuildMenu() => BuildMenu();

    /// <summary>The top-level menus this window shows, in order, for the button and for tests.</summary>
    internal IReadOnlyList<MenuItem> MenuItems => [.. _menus.Items.OfType<MenuItem>()];

    /// <summary>Where the menu stands in the title row, by the frame's style.</summary>
    /// <remarks>
    /// A macOS frame draws its three lights at the window's left corner, so the menu moves to the right. The
    /// other two styles are the other way round. The title is not placed by this: it is centred over the whole
    /// window rather than laid out in the row the frame insets.
    /// </remarks>
    /// <param name="style">The frame's style.</param>
    /// <returns>The menu's column and side.</returns>
    internal static (int MenuColumn, HorizontalAlignment MenuSide) TitlePlacement(
        Theming.WindowStyle style
    ) =>
        style == Theming.WindowStyle.Mac
            ? (1, HorizontalAlignment.Right)
            : (0, HorizontalAlignment.Left);

    /// <summary>
    /// Ends the covering this window was made with, once the work behind it is done.
    /// </summary>
    /// <remarks>
    /// A window made without one, a dock's own, or the main window opened again over a host that is already
    /// assembled, has nothing to end. A second call is answered by the covering itself, which goes once.
    /// </remarks>
    internal void CompleteLoading()
    {
        if (_loading is null)
        {
            return;
        }

        // The ground goes back to what the look says, now that the covering is the only thing that was black:
        // what it fades away to reveal is the window the covering stood in for, rather than a black one.
        ClearValue(BackgroundProperty);

        _loading.Finish();
    }

    /// <summary>
    /// The room between the picture of a dock and the dock's word, in the item that opens it.
    /// </summary>
    private const double DockMarkGap = 6.0;

    /// <summary>
    /// The height of the line a menu item's word stands on, in pixels, at the size a menu's words are set at.
    /// </summary>
    /// <remarks>
    /// A dock's picture is sixteen pixels tall, and a menu's line is shorter than that: it is centred on the line and
    /// the pixel it takes past it above and below is drawn into the item's own padding, so that an item with a picture
    /// is the height of an item without one. What the height itself is, is the look's, a menu's word is not set at the
    /// size the program's words are, and this is that size's line.
    /// </remarks>
    private const double MenuLine = 14.0;

    /// <summary>
    /// The header of the item that opens a dock in the Window menu: the dock's word, and the picture of the dock before it.
    /// </summary>
    /// <remarks>
    /// The picture is part of the item's own header rather than its <c>Icon</c>, because the look's base theme draws an
    /// item's check and its icon in one column: the tick of a dock that is open would be drawn over the picture that says
    /// which dock it is. After the check there is a column for the word and room beside it, and the row keeps the height
    /// every other item of a menu has, since the picture is fitted to the line the word stands on.
    /// <para>
    /// The word is an <see cref="AccessText"/>, which is what the base theme draws a menu's own header as: it is set at
    /// the size a menu's words are set at, and a plain text block would be set at the program's size instead, larger
    /// than every other item of a menu, which is what makes a row of those look as if it belonged in another program.
    /// </para>
    /// </remarks>
    /// <param name="registration">The registration the item is made from.</param>
    /// <returns>What the item says.</returns>
    private Control DockHeader(DockRegistration registration)
    {
        var words = new AccessText
        {
            Text = _services.Docks.Text(registration.DisplayNameKey),
            VerticalAlignment = VerticalAlignment.Center,
        };

        if (
            registration.IconKey is not { Length: > 0 } key
            || _services.Icons.Find(key) is not { } picture
        )
        {
            return words;
        }

        var fitted = new Panel
        {
            Width = Marks.MenuSize,
            Height = MenuLine,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { Marks.Menu(picture) },
        };

        return new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = DockMarkGap,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { fitted, words },
        };
    }

    /// <summary>Builds the menu bar from what was registered, for the window this is.</summary>
    /// <remarks>
    /// The dock's own header commands are added under the Window menu rather than as menus of their own:
    /// they are what the dock says about the thing it is showing, and there is no other place in a window
    /// that is one dock for them to be.
    /// </remarks>
    private void BuildMenu()
    {
        _menus.Items.Clear();

        // The entries the last bar was built with are gone with it, and a bar built again would add the
        // registrations to the ones it no longer shows: what is kept is the entries this bar holds, so it
        // starts with none of them.
        _docks.Clear();

        foreach (var top in Visible(_services.Menu, _scope))
        {
            var menu = new MenuItem { Header = _services.I18n.Get(top.LabelKey) };

            foreach (var item in _services.Menu.Items(top.LabelKey))
            {
                menu.Items.Add(Item(item));
            }

            // The registered docks appear under the Window menu, since that is what the dock area
            // holds; that menu's items are therefore not registered as ordinary items.
            if (top.LabelKey == WindowMenu)
            {
                AddDocks(menu);
                AddCommands(menu);
            }

            _menus.Items.Add(menu);
        }
    }

    /// <summary>Makes one item, running its command when chosen.</summary>
    private MenuItem Item(ContractMenuItem item)
    {
        var made = new MenuItem { Header = _services.I18n.Get(item.LabelKey) };
        made.Click += (_, _) => item.Command();

        return made;
    }

    /// <summary>
    /// Adds what the dock this window is for says about itself.
    /// </summary>
    /// <remarks>
    /// A dock's header commands are the ones its own strip would carry, and a window that is that dock has
    /// no strip but the menu bar: what the dock offers about the thing it is showing belongs there, or a
    /// window of one dock would be a window with less in it than the region it came from.
    /// </remarks>
    /// <param name="menu">The Window menu to add them under.</param>
    private void AddCommands(MenuItem menu)
    {
        if (_dock is not { } instance)
        {
            return;
        }

        // A menu entry is a word and an action, so what a dock's header carries as a strip is represented here by
        // the one command the strip offered in its place (the reading again, for this dock) and by nothing
        // where it offered none: a window of one dock has no strip to put a control in.
        var commands = instance
            .View.Header.Select(item =>
                item switch
                {
                    DockHeaderCommand command => command,
                    DockHeaderView { Command: { } offered } => offered,
                    _ => null,
                }
            )
            .OfType<DockHeaderCommand>()
            .ToArray();

        if (commands.Length == 0)
        {
            return;
        }

        menu.Items.Add(new Separator());

        foreach (var command in commands)
        {
            var entry = new MenuItem { Header = _services.I18n.Get(command.LabelKey) };

            entry.Click += (_, _) => command.Command();
            menu.Items.Add(entry);
        }
    }

    /// <summary>Adds an entry for each registered dock, in the main window.</summary>
    /// <remarks>
    /// A toggle appears as a toggle, checked while its dock is shown; a dock with open mode New
    /// appears as a plain entry, since choosing it makes another instance rather than switching one.
    /// <para>
    /// A window that is for a dock has no list of docks and no regions to put one in: what its Window menu
    /// holds is the way back to the main window, where the docks are, and what the dock it is for says about
    /// itself.
    /// </para>
    /// </remarks>
    private void AddDocks(MenuItem menu)
    {
        if (_key is not null)
        {
            AddReturn(menu);

            return;
        }

        var registrations = _services.Docks.Registrations;

        if (registrations.Count > 0)
        {
            menu.Items.Add(new Separator());
        }

        foreach (var registration in registrations)
        {
            var entry = new MenuItem { Header = DockHeader(registration) };

            if (registration.OpenMode == DockOpenMode.Toggle)
            {
                entry.ToggleType = MenuItemToggleType.CheckBox;
                entry.IsChecked = IsShown(registration.DockNameId);
            }

            var dock = registration;
            entry.Click += (_, _) =>
            {
                // The dock opened is given the keyboard, for the reason choosing one is: a dock answers its own
                // keys inside itself, and what was just asked for from this menu is what a user is about to use.
                if (_services.Docks.Activate(dock) is { } opened)
                {
                    opened.View.View.Focus();
                }
            };

            _docks.Add((registration, entry));
            menu.Items.Add(entry);
        }
    }

    /// <summary>
    /// Adds the item that brings the dock a window of its own holds back to the main window.
    /// </summary>
    /// <remarks>
    /// A window of one dock is not the main window and has no list of docks in its Window menu, so what it
    /// offers there is the one move its dock can make: back where it came from. The move is a move of the
    /// dock rather than a window being opened, which is why it is not beside the kernel's own
    /// <c>Open Main Window</c>: that one asks for a window that may be up already, and this one puts a dock
    /// into it.
    /// <para>
    /// Built here rather than registered, because what it acts on is the dock this window is for and a
    /// registered item is handed nothing that says which window it was chosen from.
    /// </para>
    /// </remarks>
    /// <param name="menu">The Window menu to add it to.</param>
    private void AddReturn(MenuItem menu)
    {
        if (_dock is not { } instance)
        {
            return;
        }

        menu.Items.Add(new Separator());

        var entry = new MenuItem { Header = _services.I18n.Get("menu.window_items.move_to_main") };

        entry.Click += (_, _) => _services.Windows.MoveToMain(instance);
        menu.Items.Add(entry);
    }

    /// <summary>
    /// Keeps the menu in step with the docks, and writes the layout they came to.
    /// </summary>
    /// <remarks>
    /// The layout is written by the main window alone: what it holds is where the main window's regions are
    /// and which docks are in them, and a window of a dock's own has neither to say.
    /// </remarks>
    private void DocksChanged()
    {
        RefreshDocks();

        if (_key is null)
        {
            LayoutStore.Save(_services.Log, _services.Docks.Snapshot());
        }
    }

    /// <summary>Keeps the toggle checks in step with which docks are shown.</summary>
    private void RefreshDocks()
    {
        foreach (var (registration, entry) in _docks)
        {
            if (registration.OpenMode == DockOpenMode.Toggle)
            {
                entry.IsChecked = IsShown(registration.DockNameId);
            }
        }
    }

    /// <summary>Whether a dock name has an instance that is shown.</summary>
    private bool IsShown(string nameId) =>
        _services.Docks.Instances.Any(instance => instance.DockNameId == nameId && instance.IsOpen);

    /// <summary>
    /// Shows what has been raised and not yet shown, in the notification stack.
    /// </summary>
    /// <remarks>
    /// A plugin that was not loaded, or a skin that could not be read, is raised while there is nowhere to
    /// put a notification. It is held and shown here, once, all together. What is raised after the window is
    /// up comes by the same road: the queue announces it, and this takes it when there is a window to show
    /// it in.
    /// </remarks>
    private void ShowPopups()
    {
        // A window that is not shown has nowhere to put a card, so what is waiting is left for the window
        // that is up, this one once it is opened.
        if (_notifications is null || !IsVisible)
        {
            return;
        }

        foreach (var notice in _services.Popups.Take())
        {
            Add(
                new Notification(
                    Level(notice.Level),
                    notice.Source,
                    () =>
                        new TextBlock
                        {
                            Text = notice.Message,
                            TextWrapping = TextWrapping.Wrap,
                        }
                )
            );
        }
    }

    /// <summary>
    /// Shows a notice as it is raised, on the window's own thread.
    /// </summary>
    /// <remarks>
    /// A plugin may raise from any thread, as it may ask a question from any one. What it does here is
    /// only start the showing; whether there is anywhere to show it is <see cref="ShowPopups"/>'s to say.
    /// </remarks>
    /// <param name="sender">The queue.</param>
    /// <param name="e">Not read.</param>
    private void PopupsRaised(object? sender, EventArgs e)
    {
        if (Dispatcher.UIThread.CheckAccess())
        {
            ShowPopups();

            return;
        }

        Dispatcher.UIThread.Post(ShowPopups);
    }

    /// <summary>Shows every notification posted and not yet shown, on the window's own thread.</summary>
    private void ShowNotifications()
    {
        if (_notifications is null || !IsVisible)
        {
            return;
        }

        foreach (var notification in _services.Notifications.Take())
        {
            Add(notification);
        }
    }

    /// <summary>Shows a notification posted since the window was last looked at.</summary>
    /// <param name="sender">The center.</param>
    /// <param name="e">Not read.</param>
    private void NotificationsPosted(object? sender, EventArgs e)
    {
        if (Dispatcher.UIThread.CheckAccess())
        {
            ShowNotifications();

            return;
        }

        Dispatcher.UIThread.Post(ShowNotifications);
    }

    /// <summary>Puts one notification in the stack.</summary>
    /// <param name="notification">What to show.</param>
    private void Add(Notification notification)
    {
        _notifications?.Push(new NotificationCard(notification, _services.I18n, _services.Icons));
    }

    /// <summary>What a log level is marked as in the notification stack.</summary>
    /// <param name="level">How serious it is.</param>
    /// <returns>The level the card is drawn with.</returns>
    private static NotificationLevel Level(LogLevel level) =>
        level switch
        {
            LogLevel.Error => NotificationLevel.Error,
            LogLevel.Warn => NotificationLevel.Warning,
            _ => NotificationLevel.Message,
        };
}

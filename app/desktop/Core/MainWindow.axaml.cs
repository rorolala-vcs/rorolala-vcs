using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;
// Named rather than imported whole, because the contract has a menu item of its own and the toolkit has one
// too: what a window's menu is built from is the toolkit's, and only these two are the contract's.
using DockHeaderCommand = RorolalaDesktop.Contract.DockHeaderCommand;
using DockHeaderView = RorolalaDesktop.Contract.DockHeaderView;
using RorolalaDesktop.Theming;
using ContractMenuItem = RorolalaDesktop.Contract.MenuItem;
using DockOpenMode = RorolalaDesktop.Contract.DockOpenMode;
using DockRegistration = RorolalaDesktop.Contract.DockRegistration;
using PluginId = RorolalaDesktop.Contract.PluginId;

namespace RorolalaDesktop;

/// <summary>
/// A window of the program: the row its menu and its own buttons share, and the dock area below it.
/// </summary>
/// <remarks>
/// Both are filled from what was registered — the kernel's own entries and the plugins' — and a
/// plugin registers during initialization, which is before this window is made, so the menu is built
/// once here and the area is kept in step by the dock manager.
/// <para>
/// There is one window class for the whole program, because the main window and a window a dock was moved
/// into are the same window: the same row, the same frame, the same buttons. What tells them apart is the
/// dock the window is for, and nothing else — a window with no such dock is the main one, and shows
/// everything; a window that is for a dock belongs to that dock's plugin, and shows that plugin's own
/// menus. What follows from that is the dock area, which draws the docks that are in this window whichever
/// window it is.
/// </para>
/// <para>
/// No title bar of the desktop's is drawn: the row above the dock area is the window's whole frame for
/// as much of it as is seen, and what the toolkit takes from it is said in the markup. What is left
/// here is the part of it the host owns — the buttons' words and their pictures — which is why the
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
    public const double TitleBarHeight = 29.0;

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
    /// holds a browser. The kernel's own menus are shown either way — the Window menu is where the docks and
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
    /// Made by the window rather than named as a resource, because every other window of the program — a
    /// plugin's question, a notice — is a card with a title bar of the desktop's. It is held because the
    /// buttons it builds are the ones this window has to say the words and the pictures of.
    /// </remarks>
    private readonly RorolalaTheme.Frame _frame;

    /// <summary>The docks as they appear in the Window menu, so their checks can be kept in step.</summary>
    private readonly List<(DockRegistration Registration, MenuItem Entry)> _docks = [];

    /// <summary>
    /// The drag the name of the one dock this window holds has begun, if it has begun one.
    /// </summary>
    /// <remarks>
    /// The name in the row is the handle a dock of its own is brought back to the main window by, and what
    /// the pointer did with it is this gesture: held only while a press may still become a drag, which is
    /// what <see cref="DockDrag"/> is.
    /// </remarks>
    private DockDrag? _drag;

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
    internal MainWindow(HostServices services, string? key, Docking.DockInstance? dock)
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
        // to a translation rather than to this file. A window built with no translations to read —
        // which is what the designer builds — keeps the name the markup gives it.
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
        ExtendClientAreaTitleBarHeightHint = TitleBarHeight;
        _frame = RorolalaTheme.DecorationsTheme(Program.WindowStyle, TitleBarHeight);

        // There is no title bar of the desktop's left to move the window, so the row this program draws
        // in its place does it: a press on the row that no menu and no button claimed is a press on the
        // window, and the second click of a double click is what maximizes or restores it — the two
        // gestures a title bar of the desktop's would have answered by itself. A press on a menu's own row
        // is the menu's and is left to it (see `MoveOrMaximize`).
        //
        // One press, one listener, and it is the row's own surface rather than the menu bar over it: the
        // menu is inside the row, so a press on the one passes through the other, and two listeners would
        // answer the same double click twice — maximizing the window and restoring it again. Taken on the
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

        // What each of the window's buttons says and what it is drawn as. The buttons are the frame's —
        // see the look, which builds them out of the program's own pictures — and what is said here is the
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
        // window can be maximized without this button at all — a double click on the row, the window
        // manager's own key — and a button still offering to maximize an already maximized window lies.
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

        BuildMenu();
        Caption();
        DockAreaHost.Content = new DockArea(
            _services.Docks,
            _services.I18n,
            _services.Windows,
            _key
        );

        // A dock can be closed, opened, or dragged to another region by hand as well as from the
        // menu, so the checks are refreshed and the layout written whenever the docks change — a
        // dock moved by hand is remembered from then on, not only if the run ends tidily.
        _services.Docks.Changed += DocksChanged;

        // Coming back to the window is when something outside the program is most likely to have changed what
        // a plugin reads, so what the host is told about it is said on to the plugins here.
        Activated += (_, _) => _services.Refocus.Regain();

        Opened += (_, _) => ShowPopups();

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
        // what is left of it needs no waking — a window that kept listening would be one the dock manager
        // holds for the rest of the run, and one that answered a change by rebuilding a row nobody can see.
        Closed += (_, _) => _services.Docks.Changed -= DocksChanged;
    }

    /// <summary>
    /// Puts the name of the one dock this window holds in the window's own row, and makes it the handle it
    /// is dragged back to the main window by.
    /// </summary>
    /// <remarks>
    /// A window of one dock has no strip of tabs, so the name the dock's tab would have carried has nowhere
    /// to be but the row the desktop's title bar would have drawn — and the row is the only part of the
    /// window that is not the dock itself. The drag begins on that name rather than on the row, because the
    /// row is also what moves the window: what a press landed on is what tells the two gestures apart, and a
    /// row that did both would do one of them by accident.
    /// <para>
    /// The press is taken here as well as in <see cref="MoveOrMaximize"/>, which reads the same class off the
    /// thing that was pressed: a press that tunnels to the row arrives before the one that reaches this name,
    /// so what tells the window not to move is the name rather than the taking of the press.
    /// </para>
    /// </remarks>
    private void Caption()
    {
        if (_dock is not { } instance)
        {
            return;
        }

        var caption = RorolalaTheme.Caption(Program.WindowStyle, instance.Title);

        caption.PointerPressed += (_, press) =>
        {
            press.Handled = true;
            _drag = new DockDrag(_services.Windows, instance, _key);
            _drag.Take(press, caption);

            // The pointer is kept until it is let go of, so that a hand that leaves the name before it has
            // moved far enough is still a drag of the dock: what is being followed is the gesture and not
            // the pixels the name happens to cover.
            press.Pointer.Capture(caption);
        };

        caption.PointerMoved += (_, moved) => _drag?.Follow(moved, caption);
        caption.PointerCaptureLost += (_, _) => _drag = null;

        TitleRow.Children.Add(caption);
    }

    /// <summary>
    /// Moves the window for a press on the row, or maximizes it for the second click of a double click.
    /// </summary>
    /// <remarks>
    /// The move is asked of the toolkit rather than done by hand, because on a desktop it is the window
    /// manager's own move and dragging it out by arithmetic would lose the snap to a screen edge and the
    /// bounds a maximized window restores to. A press that landed on a menu, on one of the window's own
    /// buttons, or on the name of the dock this window holds is left alone rather than moved, because each
    /// of the three is a part of the row that answers a press itself — a menu answers the release of one, a
    /// button is the toolkit's, and the name begins a drag of the dock.
    /// <para>
    /// The press is left unhandled, because a menu still has to open under a press that landed on one of
    /// its menus, and what is answered here is the moving of the window rather than the press.
    /// </para>
    /// </remarks>
    /// <param name="press">What was pressed.</param>
    private void MoveOrMaximize(PointerPressedEventArgs press)
    {
        // A press something inside the row has already taken means a menu or a button, and the window
        // must not move out from under the thing that was aimed at.
        if (press.Handled || !press.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            return;
        }

        // Nothing has taken a press on a menu's row yet, though, and nothing will before this: a menu
        // answers the *release*, and the handler that opens it is subscribed when the menu is attached to
        // the visual tree — which is after this window was built, and so after this handler. Moving the
        // window here takes that release away with it: the submenu opens under a window being moved, the
        // items in it cannot be clicked, and the second click of a double click maximizes the window. The
        // name of the dock this window holds is the same question asked of the other part of the row: it
        // begins a drag of that dock, and the window must not be moved by the same press.
        if (ItemUnder(press.Source) is not null || RorolalaTheme.TitleUnder(press.Source))
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

    /// <summary>
    /// The menu item a press landed in, or nothing when it landed elsewhere on the row.
    /// </summary>
    /// <remarks>
    /// The same question the toolkit asks itself about a press it handles — its
    /// <c>DefaultMenuInteractionHandler</c> walks the parents the same way — and answered the same way,
    /// because what the pointer hit is a part of the item's own template rather than the item.
    /// </remarks>
    /// <param name="pressed">What the press landed on.</param>
    /// <returns>The menu item holding it, or nothing.</returns>
    private static MenuItem? ItemUnder(object? pressed)
    {
        for (var at = pressed as StyledElement; at is not null; at = at.Parent)
        {
            if (at is MenuItem item)
            {
                return item;
            }
        }

        return null;
    }

    /// <summary>
    /// Draws the maximize button as what pressing it would do to the window as it stands.
    /// </summary>
    /// <remarks>
    /// The picture follows the window's state rather than the press, because a window can be maximized
    /// without this button — a double click on the row, the window manager's own shortcut — and a button
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
    /// <returns>What to put in the button, or nothing.</returns>
    private Control? Glyph(string key) =>
        WindowControls.Find(_services.Icons, key) is { } picture
            ? WindowControls.Glyph(
                picture,
                Program.WindowStyle == Theming.WindowStyle.Mac
                    ? WindowControls.LightGlyphSize
                    : WindowControls.GlyphSize
            )
            : null;

    /// <summary>
    /// The top-level menus a window shows, given the plugin whose window it is.
    /// </summary>
    /// <remarks>
    /// A window that is for a dock shows that dock's plugin's menus and the kernel's own, and a window that
    /// is for nothing — the main one — shows every menu there is. What a window of one dock is for is that
    /// dock, so a menu about a tree has nothing to say in a window that holds a browser; what the program
    /// itself has to say, which is the Window menu and the way back to the main window, is said everywhere.
    /// <para>
    /// A question about what was registered and nothing about a window, so that what a window's row is made
    /// of can be asked without making one — and answered here rather than in the loop below, so that the
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
    /// Builds the menu bar from what was registered, for the window this is.
    /// </summary>
    /// <remarks>
    /// The dock's own header commands are added under the Window menu rather than as menus of their own:
    /// they are what the dock says about the thing it is showing, and there is no other place in a window
    /// that is one dock for them to be.
    /// </remarks>
    private void BuildMenu()
    {
        MainMenu.Items.Clear();

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

            MainMenu.Items.Add(menu);
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
        // the one command the strip offered in its place — the reading again, for this dock — and by nothing
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
            var entry = new MenuItem
            {
                Header = _services.Docks.Text(registration.DisplayNameKey),
            };

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
    /// Shows what was raised before the window existed, all in one dialog.
    /// </summary>
    /// <remarks>
    /// A plugin that was not loaded, or a skin that could not be read, is raised while there is
    /// nowhere to put a dialog. It is held and shown here, once, all together — one dialog listing
    /// the trouble is easier to take in than a stack of them, and deduplication has already removed
    /// the repeats.
    /// </remarks>
    private void ShowPopups()
    {
        var pending = _services.Popups.Take();

        if (pending.Count == 0)
        {
            return;
        }

        _ = Notice(pending);
    }

    /// <summary>
    /// Shows what was raised as one card: a title, a stripe per line, and one way out.
    /// </summary>
    /// <remarks>
    /// One card rather than a dialog per notification, because trouble is usually one thing met many times
    /// and a stack of identical dialogs is the one shape a notice must not have. It is drawn from the same
    /// tokens as everything else — one border, one radius, a soft shadow, and a stripe rather than an icon
    /// — so a failure reads as part of the program rather than as something the toolkit put on top of it.
    /// </remarks>
    /// <param name="pending">What was raised and not yet shown.</param>
    private async Task Notice(IReadOnlyList<Notice> pending)
    {
        var lines = new StackPanel();

        foreach (var notice in pending)
        {
            lines.Children.Add(NoticeLine(notice));
        }

        var dismiss = new Button
        {
            Content = _services.I18n.Get("window.dismiss"),
            Classes = { "primary" },
            HorizontalAlignment = HorizontalAlignment.Right,
        };

        var heading = new TextBlock
        {
            Text = _services.I18n.Get("window.notice"),
            Classes = { "title" },
            Margin = new Thickness(16, 16, 16, 8),
        };

        var foot = new Border
        {
            Padding = new Thickness(12, 12, 16, 16),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Child = dismiss,
        };
        foot[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border");

        var body = new ScrollViewer { Content = lines };

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(heading, Dock.Top);
        DockPanel.SetDock(foot, Dock.Bottom);
        panel.Children.Add(heading);
        panel.Children.Add(foot);
        panel.Children.Add(body);

        var dialog = new Window
        {
            Title = _services.I18n.Get("window.notice"),
            Icon = _services.ProgramIcon,
            Width = 560,
            MaxHeight = 640,
            SizeToContent = SizeToContent.Height,
            Content = panel,
        };

        // The dialog is a card of its own, so it takes the elevated ground the design puts a card on
        // rather than the ground the window behind it is drawn on.
        dialog[!Window.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.elevated");
        dismiss.Click += (_, _) => dialog.Close();

        await dialog.ShowDialog(this);
    }

    /// <summary>One notification: a stripe in the colour of its level, and what it was and said.</summary>
    /// <remarks>
    /// The stripe is left unstained for a level that is not trouble, so that the line still has room for
    /// it: a mark that appears only sometimes is a line that shifts under the eye when it does.
    /// </remarks>
    /// <param name="notice">What to draw.</param>
    private static Control NoticeLine(Notice notice)
    {
        var stripe = new Border { Width = 3 };

        if (Severity(notice.Level) is { } severity)
        {
            stripe[!Border.BackgroundProperty] = new DynamicResourceExtension(severity);
        }

        var text = new StackPanel
        {
            Margin = new Thickness(16, 8, 16, 8),
            Spacing = 2,
            Children =
            {
                new TextBlock { Text = notice.Source, Classes = { "label" } },
                new TextBlock { Text = notice.Message, TextWrapping = TextWrapping.Wrap },
            },
        };

        var grid = new Grid { ColumnDefinitions = new ColumnDefinitions("3,*") };
        Grid.SetColumn(stripe, 0);
        Grid.SetColumn(text, 1);
        grid.Children.Add(stripe);
        grid.Children.Add(text);

        var line = new Border
        {
            BorderThickness = new Thickness(0, 0, 0, 1),
            Child = grid,
        };
        line[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border");

        return line;
    }

    /// <summary>What a level is marked with: the one red, the one amber, or nothing at all.</summary>
    /// <param name="level">How serious it is.</param>
    private static string? Severity(LogLevel level) =>
        level switch
        {
            LogLevel.Error => "rorolala.del",
            LogLevel.Warn => "rorolala.warn",
            _ => null,
        };
}

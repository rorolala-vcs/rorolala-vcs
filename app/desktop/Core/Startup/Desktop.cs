using Avalonia;
using Avalonia.Threading;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.CoreDocks;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;
using RorolalaDesktop.Notifications;
using RorolalaDesktop.Plugins;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop;

/// <summary>
/// Assembles the host: the shared services, the kernel's own docks and menus, the plugins, and how the
/// program is drawn.
/// </summary>
/// <remarks>
/// The work is in two parts, because a window that is on the screen is worth more than a window that is
/// assembled: what the window can be drawn over is made first, the window is shown, and only then is the rest
/// of the host made — each step of it on the window's own thread, one turn of it at a time, so that the
/// covering the window puts up is drawn while the work goes on.
/// <para>
/// The order inside each part is still the order the steps must run in. The kernel's docks and menus are
/// registered first, then the look is applied, and then the window is made — the look before the window
/// because a style added to the application after a window exists stops applying to what is already on the
/// screen, without saying so. The plugins are initialized after the window, because everything they register
/// is drawn over a window that is already there.
/// </para>
/// <para>
/// The services outlive the window, and are made once for the whole run: the main window can be closed and
/// opened again while the program goes on, and what it is built over is the same host either way. What is
/// assembled once is the arrangement of docks, and what a window is made from is that.
/// </para>
/// </remarks>
internal sealed class Desktop
{
    /// <summary>The identity the kernel's own dock and menu entries are attributed to.</summary>
    private static readonly PluginId Kernel = Core.Id;

    /// <summary>The label key of the menu the docks and the way back to the main window appear under.</summary>
    private const string WindowMenu = "menu.window";

    /// <summary>What the run worked out before Avalonia started.</summary>
    private readonly DesktopState _state;

    /// <summary>Everything the host holds, made once and kept.</summary>
    private readonly HostServices _services;

    /// <summary>Whether the first half of the assembly has been done: the kernel, the look, and the window.</summary>
    private bool _shellMade;

    /// <summary>Whether the second half — everything the window is drawn over — has been started.</summary>
    private bool _assembling;

    /// <summary>Whether the second half has finished, so that a window opened now is one with nothing to cover.</summary>
    private bool _assembled;

    /// <summary>The arrangement of docks the window's work is restored from, read before the window was made.</summary>
    private DockLayout? _layout;

    /// <summary>Makes the host over what the run worked out.</summary>
    /// <param name="state">What the run worked out before Avalonia started.</param>
    public Desktop(DesktopState state)
    {
        _state = state;
        _services = Services();
    }

    /// <summary>Everything the host holds, which a window is made over.</summary>
    public HostServices Host => _services;

    /// <summary>
    /// Raised when the main window has been made and is on the screen.
    /// </summary>
    /// <remarks>
    /// Said while the window is being shown rather than after it: the toolkit announces an opening once, and a
    /// listener added afterwards is a listener that has already missed it. What waits for it is the work the
    /// window is drawn over, and the caller that asked to be told the window is up.
    /// </remarks>
    public event Action? ShellOpened;

    /// <summary>
    /// Makes what a window can be opened over — the kernel's own docks and menus, the look, and the
    /// arrangement the docks are to be given — and opens the main window.
    /// </summary>
    /// <returns>The main window.</returns>
    /// <remarks>
    /// Only what may not wait for the window's work is here. The look is the one such thing: it is added to
    /// the application's styles, and a style added after a window exists stops applying to what is already on
    /// the screen without saying so — see <see cref="ThemeService"/>. Everything else the host is made of is
    /// work a window is shown over; see <see cref="Assemble"/>.
    /// </remarks>
    public MainWindow StartShell()
    {
        if (_shellMade)
        {
            // A second opening of the window is not a second shell: what the run is made of was decided the
            // first time and every window is made over the same host, so what is left is opening the window.
            _services.Windows.OpenMain();

            return _services.Windows.Main;
        }

        _shellMade = true;

        RegisterKernel(_services);

        // Before the window, rather than after the plugins where it used to sit: nothing may be added to the
        // application's styles once a window exists, and the window is what this method goes on to make. It
        // reads nothing of the plugins — the variant and the colours are the file's and the application's —
        // which is what lets it be applied this early.
        _services.Theme.Apply();

        // Read here rather than in the assembly, because it is a file and nothing else: it says where the
        // docks go, not the docks themselves. Read on this thread rather than one of its own, because the log
        // a file that will not read is reported to is the window's own collection, and a file this small is
        // not worth a thread to lose that.
        _layout = LayoutStore.Load(_services.Log);

        _services.Windows.OpenMain();

        return _services.Windows.Main;
    }

    /// <summary>
    /// Does the work the window is drawn over: the plugins, the docks they register, and what there is to
    /// report.
    /// </summary>
    /// <param name="done">What to do once every step has returned, which is the window letting its covering go.</param>
    /// <remarks>
    /// One step per turn of the window's thread, so that what is drawn between them is drawn: the covering
    /// writes itself while the work goes on. A step that throws is written down and the rest go on — one
    /// plugin that could not be started is not a reason to leave the user under a black window.
    /// <para>
    /// Steps do not need the main window to be up: it may have been closed while this ran, and what is left of
    /// the work is still the work. A window opened afterwards is made over the same host, and is one with
    /// nothing to cover.
    /// </para>
    /// </remarks>
    public void Assemble(Action done)
    {
        ArgumentNullException.ThrowIfNull(done);

        if (_assembling)
        {
            return;
        }

        _assembling = true;

        Run(done, 0);
    }

    /// <summary>
    /// Ends the covering the main window was made with, which is what the work being done looks like.
    /// </summary>
    /// <remarks>
    /// A window that is gone has no covering to end, and that is not a failure: the entire shell may be closed
    /// while the work goes on, and what is left of it is over a window nothing can be drawn in.
    /// </remarks>
    public void FinishLoading()
    {
        if (_services.Windows.MainIsUp)
        {
            _services.Windows.Main.CompleteLoading();
        }
    }

    /// <summary>The number of steps the work is: the cases of <see cref="Step"/>.</summary>
    private const int Steps = 5;

    /// <summary>Does one step and puts the next turn of the window's thread in.</summary>
    /// <param name="done">What to do once there are no steps left.</param>
    /// <param name="step">Which step this turn is.</param>
    private void Run(Action done, int step)
    {
        try
        {
            Step(step);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _services.Log.Record(
                LogLevel.Error,
                Core.Source,
                $"the window's work did not finish a step: {error.Message}"
            );
        }

        if (step + 1 < Steps)
        {
            Dispatcher.UIThread.Post(() => Run(done, step + 1), DispatcherPriority.Background);

            return;
        }

        _assembled = true;

        done();
    }

    /// <summary>Does one step of the work, in the order the steps must run.</summary>
    /// <param name="step">Which step this is.</param>
    private void Step(int step)
    {
        switch (step)
        {
            case 0:
                _state.Plugins.InitializeAll(_services);

                // The menu was built when the window was made, which was before there were plugin menus to
                // put in it: it is built again now that there are. A window that is gone has nothing to build,
                // and asking for it would be asking the set of windows for one it does not have.
                if (_services.Windows.MainIsUp)
                {
                    _services.Windows.Main.RebuildMenu();
                }

                break;

            case 1:
                // A later start of the program is answered from here on: what a call is answered with is a
                // dock in a window, and every dock was registered by the step above.
                SingleInstance.Listen(Program.CalledFrom, _services.Log);
                break;

            case 2:
                // What a dock keeps about itself is written down as soon as it is kept, so that a run that
                // ends badly is not the thing that loses it. Subscribed before the restore rather than after
                // it, because restoring a dock is itself a dock keeping something.
                _services.Docks.StateChanged += () =>
                    LayoutStore.Save(_services.Log, _services.Docks.Snapshot());
                break;

            case 3:
                // The layout is restored once every dock is registered, since a saved layout addresses docks
                // by name and a name nothing answers to is dropped. On a first run there is no layout and no
                // dock is opened: which docks should start open is not something this program decides yet.
                _services.Docks.Restore(_layout ?? DockLayout.Starting());
                break;

            default:
                // What went wrong with the plugins is said last, because it is what the user may want to act
                // on: the popups it raises belong to the window, and the covering is still over it.
                Report(_services);
                break;
        }
    }

    /// <summary>Makes the shared services, all over one deduplication service.</summary>
    private HostServices Services()
    {
        var notifications = new NotificationService();
        var i18n = new I18nService();
        var log = new LogService(notifications);
        var popups = new PopupService(notifications);
        var notices = new NotificationCenter();

        // Made here and not by a window, because the windows it is for are not all the host's: a plugin
        // reaches it through the contract, so the one picture is decoded once and every window — the
        // host's, a dock's own, a question — is drawn with the same one.
        var icon = AppIcon.Load(log);

        // Made before the object they are put in, because a dialog is shown on the window and the window is
        // named once it exists: the shell is the one place both sides agree on, so it is made here and
        // handed to each.
        var shell = new Shell();
        var dialogs = new Dialogs(shell, i18n, icon, notices);

        // The look has to be applied before anything is drawn, so the service that applies it is made
        // here, with the rest of what a run is assembled from, rather than in the window. It is made by
        // its own factory because what a plugin is handed is not the service but the palette it answers
        // from: both come out of the one statement of how a colour in force is found.
        var making = ThemeService.Make(
            Application.Current ?? throw new InvalidOperationException("the host was assembled without an application"),
            _state.Theme
        );

        var theme = making.Service;

        var docks = new DockManager(new DockRegistry(), i18n, log, popups);
        var menus = new MenuRegistry();
        var dockMenus = new DockMenuRegistry();
        var icons = new IconLibrary();
        var refocus = new Refocus();

        var services = new HostServices
        {
            Log = log,
            Popups = popups,
            Notifications = notices,
            I18n = i18n,
            ProgramIcon = icon,
            Theme = theme,
            Palette = making.Palette,
            Rola = new RolaCapability(),
            Preference = _state.Preference,
            Settings = new SettingRegistry(_state.Preference),
            Shell = shell,
            Menu = menus,
            ContextMenus = new ContextMenuRegistry(),
            DockMenus = dockMenus,
            Navigation = new NavigationRegistry(),
            Docks = docks,
            OpenHooks = new OpenHookRegistry(),
            Refocus = refocus,
            FileChanges = new FileChanges(),
            Dialogs = dialogs,
            IconBadges = new IconBadgeRegistry(),
            Icons = icons,
            LogicalViews = new LogicalViewRegistry(),
            Drops = new DropTargetRegistry(),
            // The set of windows is made over the services rather than handed them, and how one is made is
            // written onto it here: what a window is drawn from is this object, which does not exist until
            // this line is done. The shell is named by the set itself, which is the one that hears a window
            // close and can move the name to one that is still up.
            Windows = new DockWindows(docks, i18n, refocus, log, icons, dockMenus, shell, icon),
        };

        services.Windows.Make = (key, dock) =>
        {
            // The window is given the covering only while the shell's work is still to be done: one opened
            // after it — the main window closed and asked for again from the Window menu — is a window over a
            // host that is already assembled, and covering it would be covering work that has been done.
            var window = new MainWindow(services, key, dock, loading: key is null && !_assembled);

            // Every window of the host's, and not only the main one: a dock in a window of its own is come
            // back to as much as the main window is, and a plugin's reading of the world outside is the same
            // question either way.
            window.Activated += (_, _) => refocus.Regain();

            // Told before the window is shown, because the showing is what is being told about: a listener
            // added after it has already missed it. Only the main window is spoken of, since opening a window
            // for a dock is a thing a user asked for rather than the shell being up.
            if (key is null)
            {
                window.Opened += (_, _) => ShellOpened?.Invoke();
            }

            return window;
        };

        return services;
    }

    /// <summary>
    /// Registers what the kernel itself provides: the top menu bar and the two core docks.
    /// </summary>
    private void RegisterKernel(HostServices services)
    {
        var plugins = _state.Plugins;

        // The host's own pictures are contributed before anything reads them: a window's buttons are drawn
        // from the library when the window is made, which is after this.
        Icons.Declare(services.Icons);

        services.Docks.Register(
            new DockRegistration(
                Kernel,
                "rorolala.core.plugin_manager",
                "dock.plugin_manager",
                DockOpenMode.Toggle,
                DockSide.Center,
                () =>
                    new PluginManagerDock(
                        plugins,
                        services.I18n,
                        plugins.Configuration
                    ),
                Icons.Extension
            )
        );

        services.Docks.Register(
            new DockRegistration(
                Kernel,
                "rorolala.core.log",
                "dock.log",
                DockOpenMode.Toggle,
                DockSide.Bottom,
                () => new LogDock(services.Log),
                Icons.ListAlt
            )
        );

        services.Docks.Register(
            new DockRegistration(
                Kernel,
                "rorolala.core.preference",
                "dock.preferences",
                DockOpenMode.Toggle,
                DockSide.Center,
                () =>
                    new PreferenceDock(
                        services.Settings,
                        plugins,
                        services.I18n,
                        services.Theme
                    ),
                Icons.Settings
            )
        );

        services.Menu.AddTopLevel(Kernel, Core.Position, WindowMenu, 100);

        // Where a window that was closed is asked for again. It is an item of the Window menu rather than one
        // of its own, because what it is about is the window the docks are in: a dock's own window carries
        // the same menu, so the way back is in every window there is.
        services.Menu.AddItem(
            Kernel,
            Core.Position,
            WindowMenu,
            new MenuItem("menu.window_items.open_main", 0, services.Windows.OpenMain)
        );

        // The two things the host itself says about a dock's tab, offered for every dock: moving one into a
        // window of its own is the host's answer about windows and not a plugin's, and closing one is what
        // the close on the tab does — which a menu that has no close of its own would leave unsaid.
        services.DockMenus.Add(
            Kernel,
            Core.Position,
            DockMenuTarget.Tab,
            new DockMenuItem(
                null,
                new ContextMenuItem(
                    "menu.dock.own_window",
                    10,
                    was => OpenInWindow(services, was)
                )
            )
        );

        services.DockMenus.Add(
            Kernel,
            Core.Position,
            DockMenuTarget.Tab,
            new DockMenuItem(
                null,
                new ContextMenuItem("menu.dock.close", 20, was => services.Windows.CloseDock(was.Directory))
            )
        );
    }

    /// <summary>
    /// Moves the dock a menu was opened on into a window of its own.
    /// </summary>
    /// <remarks>
    /// The dock is named by the one contextual shape the program has, which for a tab states the dock's
    /// stable name id as the directory: what is being looked up is the instance that name is open as.
    /// </remarks>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="was">What the menu was opened on.</param>
    private static void OpenInWindow(HostServices services, ContextTarget was)
    {
        ArgumentNullException.ThrowIfNull(was);

        if (services.Docks.InstanceOf(was.Directory) is { } instance)
        {
            services.Windows.ToOwnWindow(instance);
        }
    }

    /// <summary>
    /// Reports what went wrong with the plugins, to the log and to the user.
    /// </summary>
    /// <remarks>
    /// A plugin that was not loaded is something the user may want to act on, so it is raised as well
    /// as logged. What the user's order costs is not reported here: the plugin manager shows it on
    /// the card it belongs to, which is where it can be fixed.
    /// </remarks>
    private void Report(HostServices services)
    {
        foreach (var problem in _state.Plugins.Problems)
        {
            services.Log.Record(
                LogLevel.Warn,
                Core.Source,
                $"{problem.Subject}: {problem.Description}"
            );
            services.Popups.Raise(
                LogLevel.Warn,
                Core.Source,
                $"{problem.Subject}: {problem.Description}"
            );
        }
    }
}

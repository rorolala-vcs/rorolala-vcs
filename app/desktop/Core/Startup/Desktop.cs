using Avalonia;
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
/// The order here is the order the steps must run in. The kernel's docks and menus are registered
/// first, then the plugins are initialized and register theirs, and then the look is applied — the run
/// is assembled in one direction, and how it is drawn comes after what it is made of. The window is
/// made last, because the look has to be applied before anything is drawn.
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

    /// <summary>Whether the one-time half of the assembly has been done.</summary>
    private bool _assembled;

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
    /// Starts the plugins, applies the look, and opens the main window.
    /// </summary>
    /// <returns>The main window.</returns>
    public MainWindow Start()
    {
        if (_assembled)
        {
            // A second Start is not assembling anything: what the run is made of was decided the first time
            // and every window is made over the same host, so what is left is opening the window.
            _services.Windows.OpenMain();

            return _services.Windows.Main;
        }

        _assembled = true;

        RegisterKernel(_services);
        _state.Plugins.InitializeAll(_services);

        // What a dock keeps about itself is written down as soon as it is kept, so that a run that
        // ends badly is not the thing that loses it. Subscribed here rather than on the window, and
        // before the restore rather than after it, because restoring a dock is itself a dock keeping
        // something: a dock that keeps anything while it is being restored would be keeping it before
        // there was a window to hear about it.
        _services.Docks.StateChanged += () =>
            LayoutStore.Save(_services.Log, _services.Docks.Snapshot());

        // The layout is restored once every dock is registered, since a saved layout addresses docks
        // by name and a name nothing answers to is dropped. On a first run there is no layout and no
        // dock is opened: which docks should start open is not something this program decides yet.
        _services.Docks.Restore(LayoutStore.Load(_services.Log));

        Report(_services);

        _services.Theme.Apply();
        _services.Windows.OpenMain();

        return _services.Windows.Main;
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
            // The set of windows is made over the services rather than handed them, and how one is made is
            // written onto it here: what a window is drawn from is this object, which does not exist until
            // this line is done. The shell is named by the set itself, which is the one that hears a window
            // close and can move the name to one that is still up.
            Windows = new DockWindows(docks, i18n, refocus, log, icons, dockMenus, shell, icon),
        };

        services.Windows.Make = (key, dock) =>
        {
            var window = new MainWindow(services, key, dock);

            // Every window of the host's, and not only the main one: a dock in a window of its own is come
            // back to as much as the main window is, and a plugin's reading of the world outside is the same
            // question either way.
            window.Activated += (_, _) => refocus.Regain();

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
                DockPlacement.Center,
                _ =>
                    new PluginManagerDock(
                        plugins,
                        services.I18n,
                        plugins.Configuration
                    )
            )
        );

        services.Docks.Register(
            new DockRegistration(
                Kernel,
                "rorolala.core.log",
                "dock.log",
                DockOpenMode.Toggle,
                DockPlacement.Bottom,
                _ => new LogDock(services.Log)
            )
        );

        services.Docks.Register(
            new DockRegistration(
                Kernel,
                "rorolala.core.preference",
                "dock.preferences",
                DockOpenMode.Toggle,
                DockPlacement.Center,
                _ =>
                    new PreferenceDock(
                        services.Settings,
                        plugins,
                        services.I18n,
                        services.Theme
                    )
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

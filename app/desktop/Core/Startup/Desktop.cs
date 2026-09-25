using Avalonia.Platform.Storage;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.CoreDocks;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;
using RorolalaDesktop.Plugins;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop;

/// <summary>
/// Assembles the host: the shared services, the kernel's own docks and menus, the plugins, and the
/// theme.
/// </summary>
/// <remarks>
/// The order here is the one Section 4.6 lays down. The kernel's docks and menus are registered
/// first, then the plugins are initialized and register theirs, and only then is the theme applied —
/// because the theme the user chose may be one a plugin supplies. The window is made last, so a
/// theme that no provider supplies still stops the program before there is a window to show.
/// </remarks>
internal sealed class Desktop
{
    /// <summary>The identity the kernel's own dock and menu entries are attributed to.</summary>
    private static readonly PluginId Kernel = Core.Id;

    /// <summary>What the run worked out before Avalonia started.</summary>
    private readonly DesktopState _state;

    /// <summary>Makes the host over what the run worked out.</summary>
    /// <param name="state">What the run worked out before Avalonia started.</param>
    public Desktop(DesktopState state) => _state = state;

    /// <summary>
    /// Builds the services, starts the plugins, applies the theme, and makes the window.
    /// </summary>
    /// <returns>The window, which the caller shows.</returns>
    /// <exception cref="ConfigurationFailure">The named theme has no provider.</exception>
    public MainWindow Start()
    {
        var services = Services();

        RegisterKernel(services);
        _state.Plugins.InitializeAll(services);

        // What a dock keeps about itself is written down as soon as it is kept, so that a run that
        // ends badly is not the thing that loses it. Subscribed here rather than on the window, and
        // before the restore rather than after it, because restoring a dock is itself a dock keeping
        // something: a dock that keeps anything while it is being restored would be keeping it before
        // there was a window to hear about it.
        services.Docks.StateChanged += () =>
            LayoutStore.Save(services.Log, services.Docks.Snapshot());

        // The layout is restored once every dock is registered, since a saved layout addresses docks
        // by name and a name nothing answers to is dropped. On a first run there is no layout and no
        // dock is opened: which docks should start open is not something this program decides yet.
        services.Docks.Restore(LayoutStore.Load(services.Log));

        Report(services);

        new ThemeService(services.Themes).Apply(_state.Preference.Theme);

        var window = new MainWindow(services);

        services.Shell.Window = window;

        return window;
    }

    /// <summary>Makes the shared services, all over one deduplication service.</summary>
    private HostServices Services()
    {
        var notifications = new NotificationService();
        var i18n = new I18nService();
        var log = new LogService(notifications);
        var popups = new PopupService(notifications);

        return new HostServices
        {
            Log = log,
            Popups = popups,
            I18n = i18n,
            Rola = new RolaCapability(),
            Preference = _state.Preference,
            Shell = new Shell(),
            Menu = new MenuRegistry(),
            ContextMenus = new ContextMenuRegistry(),
            Navigation = new NavigationRegistry(),
            Docks = new DockManager(new DockRegistry(), i18n, log, popups),
            OpenHooks = new OpenHookRegistry(),
            IconBadges = new IconBadgeRegistry(),
            Themes = new ThemeRegistry(),
        };
    }

    /// <summary>
    /// Registers what the kernel itself provides: the top menu bar, the two core docks, and the
    /// built-in theme.
    /// </summary>
    private void RegisterKernel(HostServices services)
    {
        var plugins = _state.Plugins;

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

        services.Menu.AddTopLevel(Kernel, Core.Position, "menu.file", 0);
        services.Menu.AddTopLevel(Kernel, Core.Position, "menu.window", 100);
        services.Menu.AddItem(
            Kernel,
            Core.Position,
            "menu.file",
            new MenuItem("item.open_directory", 0, () => OpenDirectory(services))
        );

        services.Themes.Add(Kernel, new RorolalaTheme());
    }

    /// <summary>
    /// Reports what went wrong with the plugins, to the log and to the user.
    /// </summary>
    /// <remarks>
    /// A plugin that was not loaded is something the user may want to act on, so it is raised as well
    /// as logged; an ordering note is information, so it is only logged. The plugin manager shows
    /// both.
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

        foreach (var note in _state.Plugins.OrderingNotes)
        {
            services.Log.Record(
                LogLevel.Info,
                Core.Source,
                $"{note.Subject} {note.Description}"
            );
        }
    }

    /// <summary>
    /// Asks the user for a directory and makes it the one being browsed.
    /// </summary>
    /// <remarks>
    /// Until the File System plugin is loaded there is no browser to show it in, so the directory is
    /// recorded in the log; the browser reads it once that plugin arrives.
    /// </remarks>
    private static async void OpenDirectory(HostServices services)
    {
        var window = services.Shell.Window;

        if (window is null)
        {
            return;
        }

        try
        {
            var folders = await window.StorageProvider.OpenFolderPickerAsync(
                new FolderPickerOpenOptions
                {
                    Title = services.I18n.Get("item.open_directory"),
                    AllowMultiple = false,
                }
            );

            if (folders.Count == 0)
            {
                return;
            }

            var chosen = folders[0].TryGetLocalPath() ?? folders[0].Path.LocalPath;

            services.Log.Record(LogLevel.Info, Core.Source, $"opened directory {chosen}");
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            services.Log.Record(
                LogLevel.Error,
                Core.Source,
                $"the directory could not be chosen: {error.Message}"
            );
        }
    }
}

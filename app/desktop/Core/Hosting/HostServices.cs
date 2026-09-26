using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Logging;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// Everything the host holds, shared by every plugin and handed to each one.
/// </summary>
/// <remarks>
/// The registries are one set with an owner on each entry, rather than one set per plugin: that is
/// what lets items from different plugins be ordered against each other by load order rather than
/// being gathered plugin by plugin.
/// </remarks>
internal sealed class HostServices
{
    /// <summary>The host's one log.</summary>
    public required LogService Log { get; init; }

    /// <summary>Notifications the user must act on, shown once there is a window.</summary>
    public required PopupService Popups { get; init; }

    /// <summary>The host's translations.</summary>
    public required I18nService I18n { get; init; }

    /// <summary>
    /// How the program looks, and what the preference panel changes about it.
    /// </summary>
    /// <remarks>
    /// It is the host's rather than the window's because a colour is changed from the preference dock,
    /// which is opened long after the window, and the change has to reach every control rather than the
    /// dock it was made in.
    /// </remarks>
    public required ThemeService Theme { get; init; }

    /// <summary>What Rorolala can do.</summary>
    public required IRola Rola { get; init; }

    /// <summary>The user's preferences.</summary>
    public required PreferenceConfiguration Preference { get; init; }

    /// <summary>What every owner declared as settable, and what each declaration is worth.</summary>
    /// <remarks>
    /// It is the host's rather than a plugin's: the kernel declares into it before any plugin is started, and
    /// the preference panel reads all of it at once.
    /// </remarks>
    public required SettingRegistry Settings { get; init; }

    /// <summary>The window the shell's own commands act on.</summary>
    public required Shell Shell { get; init; }

    /// <summary>The top-level menus and their items.</summary>
    public required MenuRegistry Menu { get; init; }

    /// <summary>The context-menu items.</summary>
    public required ContextMenuRegistry ContextMenus { get; init; }

    /// <summary>The navigation buttons.</summary>
    public required NavigationRegistry Navigation { get; init; }

    /// <summary>The dock registrations and the docks that are open.</summary>
    public required Docking.DockManager Docks { get; init; }

    /// <summary>The open hooks.</summary>
    public required OpenHookRegistry OpenHooks { get; init; }

    /// <summary>The icon badge providers.</summary>
    public required IconBadgeRegistry IconBadges { get; init; }

    /// <summary>When one of the host's windows is come back to.</summary>
    public required Refocus Refocus { get; init; }

    /// <summary>The host as one plugin sees it.</summary>
    /// <param name="id">The plugin's identity.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <returns>A host for that plugin.</returns>
    public PluginHost For(PluginId id, int position) => new(this, id, position);
}

/// <summary>
/// The host as one plugin sees it, and the registries that plugin may add to.
/// </summary>
/// <remarks>
/// One instance per plugin, remembering which plugin that is, so everything registered through it
/// is attributed: menu and dock items sort against other plugins' by load order, and a failure
/// names the plugin it came from rather than the host as a whole.
/// </remarks>
internal sealed class PluginHost
    : IPluginHost,
        IMenuRegistry,
        IContextMenuRegistry,
        INavigationRegistry,
        IDockRegistry,
        IOpenHookRegistry,
        IIconBadgeRegistry
{
    /// <summary>Everything the host holds.</summary>
    private readonly HostServices _services;

    /// <summary>The plugin this host is for.</summary>
    private readonly PluginId _id;

    /// <summary>The plugin's place in the load order.</summary>
    private readonly int _position;

    /// <summary>Makes the host one plugin sees.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="id">The plugin's identity.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    public PluginHost(HostServices services, PluginId id, int position)
    {
        _services = services;
        _id = id;
        _position = position;
        Log = services.Log.For(id.Value);
        Config = new PluginConfigView(services.Settings, id);
    }

    /// <inheritdoc />
    public ILog Log { get; }

    /// <inheritdoc />
    public II18n I18n => _services.I18n;

    /// <inheritdoc />
    public IRola Rola => _services.Rola;

    /// <inheritdoc />
    public IPluginConfig Config { get; }

    /// <inheritdoc />
    public IMenuRegistry Menu => this;

    /// <inheritdoc />
    public IContextMenuRegistry ContextMenus => this;

    /// <inheritdoc />
    public INavigationRegistry Navigation => this;

    /// <inheritdoc />
    public IDockRegistry Docks => this;

    /// <inheritdoc />
    public IOpenHookRegistry OpenHooks => this;

    /// <inheritdoc />
    public IIconBadgeRegistry IconBadges => this;

    /// <inheritdoc />
    public IRefocus Refocus => _services.Refocus;

    /// <inheritdoc />
    void IMenuRegistry.AddTopLevel(string labelKey, int order) =>
        _services.Menu.AddTopLevel(_id, _position, labelKey, order);

    /// <inheritdoc />
    void IMenuRegistry.AddItem(string menuPath, MenuItem item) =>
        _services.Menu.AddItem(_id, _position, menuPath, item);

    /// <inheritdoc />
    void IContextMenuRegistry.Add(ContextMenuTarget target, ContextMenuItem item) =>
        _services.ContextMenus.Add(_id, _position, target, item);

    /// <inheritdoc />
    void INavigationRegistry.Add(NavigationButton button) =>
        _services.Navigation.Add(_id, _position, button);

    /// <inheritdoc />
    void IDockRegistry.Register(DockRegistration registration) =>
        _services.Docks.Register(registration);

    /// <inheritdoc />
    void IOpenHookRegistry.Add(IOpenHook hook) => _services.OpenHooks.Add(_id, _position, hook);

    /// <inheritdoc />
    void IIconBadgeRegistry.Add(IIconBadgeProvider provider) =>
        _services.IconBadges.Add(_id, _position, provider);
}

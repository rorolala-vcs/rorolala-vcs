using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// Something a plugin registered, remembered with who registered it and where its plugin sits.
/// </summary>
/// <param name="Owner">The plugin that registered it.</param>
/// <param name="Position">The plugin's place in the load order.</param>
/// <param name="Value">What was registered.</param>
internal sealed record Owned<T>(PluginId Owner, int Position, T Value);

/// <summary>What a plugin registered as a top-level menu.</summary>
/// <param name="Owner">The plugin that registered it.</param>
/// <param name="Position">The plugin's place in the load order.</param>
/// <param name="LabelKey">An i18n key for the menu's label, which is also its path.</param>
/// <param name="Order">Where the menu sits among the top-level menus.</param>
internal sealed record TopLevelMenu(PluginId Owner, int Position, string LabelKey, int Order);

/// <summary>
/// The top-level menus and their items.
/// </summary>
/// <remarks>
/// Items are ordered by the registering plugin's load order and then by the item's own order, so
/// two plugins that both add under <c>File</c> appear in a stable order rather than fighting over
/// one number.
/// </remarks>
internal sealed class MenuRegistry
{
    /// <summary>Every top-level menu, in registration order.</summary>
    private readonly List<TopLevelMenu> _topLevels = [];

    /// <summary>Every item, by the label key of the menu it was added under.</summary>
    private readonly Dictionary<string, List<Owned<MenuItem>>> _items = new(StringComparer.Ordinal);

    /// <summary>Adds a top-level menu.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="labelKey">An i18n key for the label, which is also the menu's path.</param>
    /// <param name="order">Where the menu sits among the top-level menus.</param>
    public void AddTopLevel(PluginId owner, int position, string labelKey, int order) =>
        _topLevels.Add(new TopLevelMenu(owner, position, labelKey, order));

    /// <summary>Adds an item under a top-level menu.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="menuPath">The label key of the menu to add under.</param>
    /// <param name="item">The item to add.</param>
    public void AddItem(PluginId owner, int position, string menuPath, MenuItem item)
    {
        if (!_items.TryGetValue(menuPath, out var items))
        {
            items = [];
            _items[menuPath] = items;
        }

        items.Add(new Owned<MenuItem>(owner, position, item));
    }

    /// <summary>Whether a top-level menu with this label key was registered.</summary>
    /// <param name="labelKey">The label key to look for.</param>
    /// <returns>Whether it is there.</returns>
    public bool HasTopLevel(string labelKey) =>
        _topLevels.Any(menu => menu.LabelKey == labelKey);

    /// <summary>Every top-level menu, in the order they are shown.</summary>
    public IReadOnlyList<TopLevelMenu> TopLevels =>
        _topLevels
            .OrderBy(menu => menu.Order)
            .ThenBy(menu => menu.Position)
            .ThenBy(menu => menu.LabelKey, StringComparer.Ordinal)
            .ToArray();

    /// <summary>Every item under a menu, in the order they are shown.</summary>
    /// <param name="menuPath">The label key of the menu.</param>
    /// <returns>The items, or nothing when the menu has none.</returns>
    public IReadOnlyList<MenuItem> Items(string menuPath) =>
        _items.TryGetValue(menuPath, out var items)
            ? items
                .OrderBy(item => item.Position)
                .ThenBy(item => item.Value.Order)
                .ThenBy(item => item.Value.LabelKey, StringComparer.Ordinal)
                .Select(item => item.Value)
                .ToArray()
            : [];

    /// <summary>Every item registered under a menu, whatever the order.</summary>
    /// <param name="menuPath">The label key of the menu.</param>
    /// <returns>Whether the menu has any item at all.</returns>
    public bool HasItems(string menuPath) =>
        _items.TryGetValue(menuPath, out var items) && items.Count > 0;
}

/// <summary>One context-menu item, with who registered it.</summary>
internal sealed record OwnedContextItem(
    PluginId Owner,
    int Position,
    ContextMenuTarget Target,
    ContextMenuItem Item
);

/// <summary>
/// The context-menu items, by the context they belong to.
/// </summary>
internal sealed class ContextMenuRegistry
{
    /// <summary>Every item, in registration order.</summary>
    private readonly List<OwnedContextItem> _items = [];

    /// <summary>Adds an item to one of the three contexts.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="target">The context the item belongs to.</param>
    /// <param name="item">The item to add.</param>
    public void Add(PluginId owner, int position, ContextMenuTarget target, ContextMenuItem item) =>
        _items.Add(new OwnedContextItem(owner, position, target, item));

    /// <summary>Every item in a context, in the order they are shown.</summary>
    /// <param name="target">The context.</param>
    /// <returns>The items, or nothing when the context has none.</returns>
    public IReadOnlyList<ContextMenuItem> Items(ContextMenuTarget target) =>
        _items
            .Where(entry => entry.Target == target)
            .OrderBy(entry => entry.Position)
            .ThenBy(entry => entry.Item.Order)
            .ThenBy(entry => entry.Item.LabelKey, StringComparer.Ordinal)
            .Select(entry => entry.Item)
            .ToArray();
}

/// <summary>The navigation buttons plugins contribute.</summary>
internal sealed class NavigationRegistry
{
    /// <summary>Every button, in registration order.</summary>
    private readonly List<Owned<NavigationButton>> _buttons = [];

    /// <summary>Adds a button.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="button">The button to add.</param>
    public void Add(PluginId owner, int position, NavigationButton button) =>
        _buttons.Add(new Owned<NavigationButton>(owner, position, button));

    /// <summary>Every button, in the order they are shown.</summary>
    public IReadOnlyList<NavigationButton> Buttons =>
        _buttons
            .OrderBy(button => button.Position)
            .ThenBy(button => button.Value.Order)
            .ThenBy(button => button.Value.LabelKey, StringComparer.Ordinal)
            .Select(button => button.Value)
            .ToArray();
}

/// <summary>
/// The dock registrations.
/// </summary>
/// <remarks>
/// A dock's name id is globally unique, and registering one twice is refused here rather than
/// silently keeping the first: the duplicate is a plugin's mistake, and refusing it fails that
/// plugin's initialization, which is reported and harmless to the rest of the program.
/// </remarks>
internal sealed class DockRegistry
{
    /// <summary>Every registration, in registration order.</summary>
    private readonly List<DockRegistration> _registrations = [];

    /// <summary>Every registration, by its name id.</summary>
    private readonly Dictionary<string, DockRegistration> _byNameId = new(StringComparer.Ordinal);

    /// <summary>Every registration, in registration order.</summary>
    public IReadOnlyList<DockRegistration> Registrations => _registrations;

    /// <summary>Registers a dock.</summary>
    /// <param name="registration">What the plugin says about the dock.</param>
    /// <exception cref="InvalidOperationException">The name id was already taken.</exception>
    public void Register(DockRegistration registration)
    {
        if (!_byNameId.TryAdd(registration.DockNameId, registration))
        {
            throw new InvalidOperationException(
                $"the dock name id `{registration.DockNameId}` is already taken"
            );
        }

        _registrations.Add(registration);
    }

    /// <summary>The registration a name id names, or nothing.</summary>
    /// <param name="nameId">The dock's name id.</param>
    /// <returns>The registration, or nothing when no dock has that name.</returns>
    public DockRegistration? Find(string nameId) => _byNameId.GetValueOrDefault(nameId);
}

/// <summary>The open hooks plugins contribute.</summary>
internal sealed class OpenHookRegistry
{
    /// <summary>Every hook, in registration order.</summary>
    private readonly List<Owned<IOpenHook>> _hooks = [];

    /// <summary>Adds a hook.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="hook">The hook to add.</param>
    public void Add(PluginId owner, int position, IOpenHook hook) =>
        _hooks.Add(new Owned<IOpenHook>(owner, position, hook));

    /// <summary>Every hook in a stage, in the order they run.</summary>
    /// <param name="stage">The stage.</param>
    /// <returns>The hooks, or nothing when the stage has none.</returns>
    public IReadOnlyList<Owned<IOpenHook>> Hooks(OpenStage stage) =>
        _hooks
            .Where(entry => entry.Value.Stage == stage)
            .OrderBy(entry => entry.Position)
            .ToArray();
}

/// <summary>The icon badge providers plugins contribute.</summary>
internal sealed class IconBadgeRegistry
{
    /// <summary>Every provider, in registration order.</summary>
    private readonly List<Owned<IIconBadgeProvider>> _providers = [];

    /// <summary>Adds a provider.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="provider">The provider to add.</param>
    public void Add(PluginId owner, int position, IIconBadgeProvider provider) =>
        _providers.Add(new Owned<IIconBadgeProvider>(owner, position, provider));

    /// <summary>Every provider, in the order their badges are placed.</summary>
    public IReadOnlyList<IIconBadgeProvider> Providers =>
        _providers.OrderBy(provider => provider.Position).Select(provider => provider.Value).ToArray();
}

/// <summary>The icon badge providers plugins contribute.</summary>
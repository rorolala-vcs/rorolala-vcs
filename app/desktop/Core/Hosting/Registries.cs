using Avalonia.Media.Imaging;
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

    /// <summary>
    /// The items of one context, in the order they are shown.
    /// </summary>
    /// <remarks>
    /// Every plugin's, and deliberately not one plugin's: the menu a context is drawn in is another plugin's
    /// — the File System's is what folds these in — so what is asked for is what there is rather than what
    /// one of the plugins that contributed it has to say.
    /// </remarks>
    /// <param name="target">The context.</param>
    /// <returns>The items, or nothing when that context has none.</returns>
    public IReadOnlyList<ContextMenuItem> Items(ContextMenuTarget target) =>
        _items
            .Where(entry => entry.Target == target)
            .OrderBy(entry => entry.Position)
            .ThenBy(entry => entry.Item.Order)
            .ThenBy(entry => entry.Item.LabelKey, StringComparer.Ordinal)
            .Select(entry => entry.Item)
            .ToArray();
}

/// <summary>One dock-menu item, with who registered it.</summary>
internal sealed record OwnedDockItem(
    PluginId Owner,
    int Position,
    DockMenuTarget Target,
    DockMenuItem Item
);

/// <summary>
/// The items plugins add to the menus a dock's own chrome opens.
/// </summary>
/// <remarks>
/// Kept apart from <see cref="ContextMenuRegistry"/> rather than folded into its three contexts, because
/// the two are asked for different things: a file menu is built for a choice inside a listing, and this one
/// for a dock's own tab, which no listing knows about. What an item of either is, is the same shape.
/// </remarks>
internal sealed class DockMenuRegistry
{
    /// <summary>Every item, in registration order.</summary>
    private readonly List<OwnedDockItem> _items = [];

    /// <summary>Adds an item to one of a dock's own menus.</summary>
    /// <param name="owner">The plugin registering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="target">The place the item belongs to.</param>
    /// <param name="item">The item to add.</param>
    public void Add(PluginId owner, int position, DockMenuTarget target, DockMenuItem item) =>
        _items.Add(new OwnedDockItem(owner, position, target, item));

    /// <summary>Every item offered for one place and one dock, in the order they are shown.</summary>
    /// <param name="target">The place.</param>
    /// <param name="dockNameId">The dock the menu belongs to.</param>
    /// <returns>The items, or nothing when that place has none for that dock.</returns>
    public IReadOnlyList<ContextMenuItem> Items(DockMenuTarget target, string dockNameId) =>
        _items
            .Where(entry =>
                entry.Target == target
                && (entry.Item.DockNameId is null
                    || string.Equals(entry.Item.DockNameId, dockNameId, StringComparison.Ordinal))
            )
            .OrderBy(entry => entry.Position)
            .ThenBy(entry => entry.Item.Item.Order)
            .ThenBy(entry => entry.Item.Item.LabelKey, StringComparer.Ordinal)
            .Select(entry => entry.Item.Item)
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

/// <summary>The pictures plugins contribute, keyed by name.</summary>
/// <remarks>
/// A picture is held rather than made: what a plugin contributes is already the image, and a key is
/// looked up every time an entry of a listing is drawn, so making one there would make it again for
/// every row. What a reader does with the size — a badge is as large as the icon it sits on — is the
/// reader's, since the same picture is read at more than one size.
/// </remarks>
internal sealed class IconLibrary : IIconLibrary
{
    /// <summary>Every picture, by key.</summary>
    private readonly Dictionary<string, Bitmap> _pictures = new(StringComparer.Ordinal);

    /// <inheritdoc />
    public void Add(string key, Bitmap picture)
    {
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(picture);

        _pictures[key] = picture;
    }

    /// <inheritdoc />
    public Bitmap? Find(string key)
    {
        ArgumentNullException.ThrowIfNull(key);

        return _pictures.GetValueOrDefault(key);
    }
}

/// <summary>The places plugins answer for, by the prefix their addresses begin with.</summary>
internal sealed class LogicalViewRegistry
{
    /// <summary>Every claim, in the order it was made.</summary>
    private readonly List<(PluginId Owner, int Position, string Scheme, ILogicalViewProvider Provider)> _claims = [];

    /// <summary>Adds a claim.</summary>
    /// <param name="owner">The plugin claiming it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="scheme">The prefix being claimed.</param>
    /// <param name="provider">What answers for it.</param>
    /// <exception cref="ArgumentException">The prefix is not one a plugin may claim.</exception>
    public void Add(PluginId owner, int position, string scheme, ILogicalViewProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (!LogicalAddress.IsScheme(scheme))
        {
            throw new ArgumentException(
                $"`{scheme}` is not a prefix an address may begin with, so `{owner.Value}` cannot answer for it",
                nameof(scheme)
            );
        }

        _claims.Add((owner, position, scheme, provider));
    }

    /// <summary>What answers a prefix.</summary>
    /// <param name="scheme">The prefix.</param>
    /// <returns>The provider, or nothing when no plugin answers it.</returns>
    public ILogicalViewProvider? Find(string scheme) =>
        _claims
            .Where(claim => string.Equals(claim.Scheme, scheme, StringComparison.Ordinal))
            .OrderBy(claim => claim.Position)
            .Select(claim => claim.Provider)
            .FirstOrDefault();

    /// <summary>Every prefix that has an answer, in the order they were offered.</summary>
    public IReadOnlyList<string> Schemes =>
    [
        .. _claims.OrderBy(claim => claim.Position).Select(claim => claim.Scheme),
    ];
}

/// <summary>What plugins take a non-file drag dropped on a directory for.</summary>
/// <remarks>
/// A list rather than one answer, because a directory is not the taker: any plugin may say which directories it
/// takes what for, and the first that says it takes this one is the one that does.
/// </remarks>
internal sealed class DropTargetRegistry
{
    /// <summary>Every target, in the order it was offered.</summary>
    private readonly List<(PluginId Owner, int Position, IDropTarget Target)> _targets = [];

    /// <summary>Offers a target.</summary>
    /// <param name="owner">The plugin offering it.</param>
    /// <param name="position">The plugin's place in the load order.</param>
    /// <param name="target">What answers for the directories it takes.</param>
    public void Add(PluginId owner, int position, IDropTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        _targets.Add((owner, position, target));
    }

    /// <summary>Every target offered, in the order they are asked.</summary>
    public IReadOnlyList<IDropTarget> Items =>
    [
        .. _targets.OrderBy(target => target.Position).Select(target => target.Target),
    ];
}

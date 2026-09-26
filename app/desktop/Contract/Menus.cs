namespace RorolalaDesktop.Contract;

/// <summary>Which part of the browser a context-menu item belongs to.</summary>
public enum ContextMenuTarget
{
    /// <summary>Right-click on a directory.</summary>
    Directory,

    /// <summary>Right-click on a file.</summary>
    File,

    /// <summary>Right-click on empty area.</summary>
    EmptySpace,
}

/// <summary>What a context-menu item was opened on.</summary>
/// <param name="Directory">The directory the menu was opened in.</param>
/// <param name="Entry">
/// The file or directory that was right-clicked, or nothing when empty space was.
/// </param>
public sealed record ContextTarget(string Directory, Entry? Entry);

/// <summary>One context-menu item.</summary>
/// <param name="LabelKey">An i18n key for the item's label.</param>
/// <param name="Order">Where the item sits among those of the same context.</param>
/// <param name="Command">What the item does, given what was right-clicked.</param>
/// <param name="IconKey">An optional key naming the item's icon.</param>
public sealed record ContextMenuItem(
    string LabelKey,
    int Order,
    Action<ContextTarget> Command,
    string? IconKey = null
);

/// <summary>Where a plugin adds context-menu items.</summary>
public interface IContextMenuRegistry
{
    /// <summary>
    /// Adds an item to one of the three contexts.
    /// </summary>
    /// <remarks>
    /// Items from different plugins are ordered by plugin load order and then by the item's own
    /// order value.
    /// </remarks>
    /// <param name="target">The context the item belongs to.</param>
    /// <param name="item">The item to add.</param>
    void Add(ContextMenuTarget target, ContextMenuItem item);
}

/// <summary>One item under a menu.</summary>
/// <param name="LabelKey">An i18n key for the item's label.</param>
/// <param name="Order">Where the item sits among its siblings.</param>
/// <param name="Command">What the item does.</param>
/// <param name="IconKey">An optional key naming the item's icon.</param>
public sealed record MenuItem(
    string LabelKey,
    int Order,
    Action Command,
    string? IconKey = null
);

/// <summary>Where a plugin adds top-level menus and items.</summary>
public interface IMenuRegistry
{
    /// <summary>
    /// Adds a top-level menu.
    /// </summary>
    /// <param name="labelKey">An i18n key for the menu's label, which is also its path.</param>
    /// <param name="order">Where the menu sits among the top-level menus.</param>
    void AddTopLevel(string labelKey, int order);

    /// <summary>
    /// Adds an item under a top-level menu.
    /// </summary>
    /// <param name="menuPath">The label key of the top-level menu to add under.</param>
    /// <param name="item">The item to add.</param>
    void AddItem(string menuPath, MenuItem item);
}

/// <summary>One navigation button.</summary>
/// <param name="LabelKey">An i18n key for the button's label.</param>
/// <param name="Order">Where the button sits among the navigation buttons.</param>
/// <param name="Command">What the button does, given the current directory.</param>
/// <param name="IconKey">An optional key naming the button's icon.</param>
public sealed record NavigationButton(
    string LabelKey,
    int Order,
    Action<string> Command,
    string? IconKey = null
);

/// <summary>Where a plugin adds buttons to the navigation area.</summary>
public interface INavigationRegistry
{
    /// <summary>
    /// Adds a button.
    /// </summary>
    /// <param name="button">The button to add.</param>
    void Add(NavigationButton button);
}

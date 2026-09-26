namespace RorolalaDesktop.Contract;

/// <summary>
/// The host, as one plugin sees it: the services a plugin reaches during initialization.
/// </summary>
/// <remarks>
/// An instance is handed to one plugin and remembers which plugin that is, so what the plugin
/// registers is attributed to it — for ordering against the plugins that registered before it, and
/// for naming it in a report when something it did fails.
/// </remarks>
public interface IPluginHost
{
    /// <summary>Where the plugin writes what it wants to say.</summary>
    ILog Log { get; }

    /// <summary>Where the plugin adds the translations it carries.</summary>
    II18n I18n { get; }

    /// <summary>What Rorolala can do, injected over the C ABI.</summary>
    IRola Rola { get; }

    /// <summary>The plugin's own section of <c>preference.json</c>.</summary>
    IPluginConfig Config { get; }

    /// <summary>Where the plugin adds top-level menus and their items.</summary>
    IMenuRegistry Menu { get; }

    /// <summary>Where the plugin adds context-menu items.</summary>
    IContextMenuRegistry ContextMenus { get; }

    /// <summary>Where the plugin adds navigation buttons.</summary>
    INavigationRegistry Navigation { get; }

    /// <summary>Where the plugin registers docks.</summary>
    IDockRegistry Docks { get; }

    /// <summary>Where the plugin adds open hooks.</summary>
    IOpenHookRegistry OpenHooks { get; }

    /// <summary>Where the plugin adds icon badges.</summary>
    IIconBadgeRegistry IconBadges { get; }

    /// <summary>When one of the host's windows is come back to.</summary>
    IRefocus Refocus { get; }

    /// <summary>Where the plugin asks the user something.</summary>
    IDialogs Dialogs { get; }
}

using Avalonia.Controls;

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

    /// <summary>Where the plugin adds the pictures the program draws, and reads them back.</summary>
    IIconLibrary Icons { get; }

    /// <summary>When one of the host's windows is come back to.</summary>
    IRefocus Refocus { get; }

    /// <summary>Where a plugin says that what it did changed the files, and hears another say so.</summary>
    IFileChanges Files { get; }

    /// <summary>Where the plugin asks the user something.</summary>
    IDialogs Dialogs { get; }

    /// <summary>
    /// The picture the program is known by, for the windows the plugin shows of its own.
    /// </summary>
    /// <remarks>
    /// The host makes it once and hands the same one to every plugin, so a window a plugin shows is
    /// drawn as the program rather than as something beside it, and the picture is decoded once
    /// however many windows are drawn with it. It is nothing when the platform would not take it,
    /// which is a window without an icon rather than a reason not to open one.
    /// </remarks>
    WindowIcon? ProgramIcon { get; }
}

using Avalonia.Controls;

namespace RorolalaDesktop.Contract;

/// <summary>
/// The host, as one plugin sees it: the services a plugin reaches during initialization.
/// </summary>
/// <remarks>
/// An instance is handed to one plugin and remembers which plugin that is, so what the plugin
/// registers is attributed to it, for ordering against the plugins that registered before it, and
/// for naming it in a report when something it did fails.
/// </remarks>
public interface IPluginHost
{
    /// <summary>Where the plugin writes what it wants to say.</summary>
    ILog Log { get; }

    /// <summary>
    /// The progress every window of the program shows.
    /// </summary>
    /// <remarks>
    /// It is the program's rather than the plugin's: a run begun here is drawn by the stack of whichever windows
    /// are open, and a run begun by another plugin is in the same list. What a plugin adds is its own runs, not a
    /// panel of its own.
    /// </remarks>
    IProgressHub Progress { get; }

    /// <summary>The program's previews, which a tile asks for and a source answers.</summary>
    IPreviewService Previews { get; }

    /// <summary>Where the plugin adds its own preview renderer, after the system's.</summary>
    IPreviewRegistry PreviewSources { get; }

    /// <summary>Where the plugin adds the translations it carries.</summary>
    II18n I18n { get; }

    /// <summary>What Rorolala can do, injected over the C ABI.</summary>
    IRola Rola { get; }

    /// <summary>The plugin's own section of <c>preference.json</c>.</summary>
    IPluginConfig Config { get; }

    /// <summary>Where the plugin keeps what it can make again rather than what a person decided.</summary>
    ICache Cache { get; }

    /// <summary>Where the plugin adds top-level menus and their items.</summary>
    IMenuRegistry Menu { get; }

    /// <summary>Where the plugin adds context-menu items.</summary>
    IContextMenuRegistry ContextMenus { get; }

    /// <summary>Where the plugin adds navigation buttons.</summary>
    INavigationRegistry Navigation { get; }

    /// <summary>Where the plugin registers docks.</summary>
    IDockRegistry Docks { get; }

    /// <summary>Which dock the user is working in, which is what a dock's own keys are read against.</summary>
    IDockFocus Focus { get; }

    /// <summary>Where the plugin adds items to the menus a dock's own chrome opens.</summary>
    IDockMenuRegistry DockMenus { get; }

    /// <summary>Where the plugin asks for a dock to be shown in a window of its own.</summary>
    IWindows Windows { get; }

    /// <summary>Where the plugin adds open hooks.</summary>
    IOpenHookRegistry OpenHooks { get; }

    /// <summary>Where the plugin adds icon badges.</summary>
    IIconBadgeRegistry IconBadges { get; }

    /// <summary>Where the plugin adds the pictures the program draws, and reads them back.</summary>
    IIconLibrary Icons { get; }

    /// <summary>Where the plugin offers the places it answers for, by the prefix of their addresses.</summary>
    ILogicalViewRegistry LogicalViews { get; }

    /// <summary>Where the plugin offers what it takes a place's entry dropped on a directory for.</summary>
    IDropTargetRegistry Drops { get; }

    /// <summary>When one of the host's windows is come back to.</summary>
    IRefocus Refocus { get; }

    /// <summary>Where a plugin says that what it did changed the files, and hears another say so.</summary>
    IFileChanges Files { get; }

    /// <summary>Where the plugin asks the user something.</summary>
    IDialogs Dialogs { get; }

    /// <summary>
    /// The colours the program is drawn in, in the variant it is drawn in.
    /// </summary>
    /// <remarks>
    /// What a plugin draws itself with, when it needs the colour rather than a resource to name: the
    /// five members answer for whichever of the two looks is in force, which is what keeps a plugin
    /// from having to know which one that is, and they are answered afresh each time they are read, so
    /// a colour the user changes is the next one drawn. What a plugin says about its own marks is its
    /// own and is not here.
    /// </remarks>
    IPalette Palette { get; }

    /// <summary>
    /// Raised when the look has been written and its colours replaced, after an edit in the panel is settled.
    /// </summary>
    /// <remarks>
    /// The resources a control names in a <c>DynamicResource</c> follow a colour on their own. A mark that was
    /// built from a colour value, which is what a badge or a state dot is, does not, so this is the point at
    /// which a plugin draws those again. It is raised after the write rather than on every drag of the plate:
    /// what a reader is choosing is the settled colour, and a redraw per pointer move would be a redraw for
    /// colours that were passed through.
    /// </remarks>
    event EventHandler? ThemeChanged;

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

using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace GitVCSPlugin;

/// <summary>
/// The Git plugin: it offers Git's own notion of what is ignored to the File System plugin.
/// </summary>
/// <remarks>
/// It carries no dock and no window of its own. What it does is contribute one hide provider, the
/// entries Git ignores, through the File System plugin's extension point, which it reaches by
/// depending on that plugin: the reference is what lets it name <see cref="HideRegistry"/>, and the
/// manifest's dependency is what has the host start this one after it. A plugin of its own rather than
/// a part of the browser, because Git's rules are the version control system's to offer, not the file
/// manager's, and a browser that knew about <c>.gitignore</c> would know about one system only.
/// </remarks>
public sealed class GitVCSPlugin : IRolaPlugin
{
    /// <summary>This plugin's identity.</summary>
    public const string Identity = "rorolala.git";

    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId(Identity),
            "rorolala_git.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId(FileSystem.Identity)]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.I18n.RegisterDirectory(Translations());

        // The pictures this plugin draws, which the badge and the restore's menu item are read by:
        // a picture is looked up by key, so the library has to hold it before a listing asks.
        Icons.Declare(host.Icons);

        // The File System plugin's catalogue, reached the way a preset is: this plugin is started after
        // that one, so the catalogue it adds to is the one the setting was declared over, and the option
        // this adds is offered the moment the preference panel is drawn (see HideRegistry).
        HideRegistry.Register(new GitIgnored());

        // The git-rola@ place: the Vaults a git-rola project declares and each one's Layout, read
        // through `git rola upstream`. The prefix carries a hyphen, which the contract's grammar was
        // widened to allow for exactly this name.
        var place = new GitRolaPlace(host);

        host.LogicalViews.Register(GitRolaPlace.Scheme, place);
        host.Drops.Add(new GitRolaDrops(host));

        host.Menu.AddTopLevel("rorolala_git.menu.root", 70);
        host.Menu.AddItem(
            "rorolala_git.menu.root",
            new MenuItem("rorolala_git.menu.open", 10, place.OpenRoot)
        );

        // From a directory of the work, where a person stands when they wonder what upstream holds,
        // and only where there is a project to ask: a git repository with no rola.toml has nothing
        // for this place to read.
        host.ContextMenus.Add(
            ContextMenuTarget.Directory,
            new ContextMenuItem(
                ["menu.entry.open", "menu.entry.git_rola"],
                70,
                _ => place.OpenRoot(),
                Icons.ArrowDown,
                Applies: target => GitRolaToml.ProjectOf(GitRolaActions.Chosen(target)) is not null
            )
        );

        // What a signed-in material is marked with in the tree: grey while it holds what the lock
        // records, the program's error red when it does not. The check is one answer read by both
        // the colour and the badge, so it is made once and handed to both.
        var check = new GitRolaCheck(host);

        EntryColourProviders.Register(new GitRolaColour(host.Palette, check));
        EntryMarkProviders.Register(new GitRolaBadge(check, host.Palette));
        host.ThemeChanged += (_, _) => host.Files.Redraw();

        // The commands a right-click on the work offers, in the command line's own words: the two
        // whole-project commands under one category, and the restore under its own, since it is
        // about the files chosen rather than the project.
        foreach (var target in new[] { ContextMenuTarget.Directory, ContextMenuTarget.File })
        {
            host.ContextMenus.Add(
                target,
                new ContextMenuItem(
                    [GitRolaActions.Category, GitRolaActions.Refresh],
                    80,
                    choice => GitRolaActions.RunRefresh(host, choice),
                    Applies: GitRolaActions.InProject
                )
            );
            host.ContextMenus.Add(
                target,
                new ContextMenuItem(
                    [GitRolaActions.Category, GitRolaActions.Update],
                    90,
                    choice => GitRolaActions.RunUpdate(host, choice),
                    Applies: GitRolaActions.InProject
                )
            );
        }

        host.ContextMenus.Add(
            ContextMenuTarget.File,
            new ContextMenuItem(
                [GitRolaActions.RestoreCategory, GitRolaActions.RestoreToLock],
                100,
                choice => GitRolaActions.RunRestore(host, choice),
                GitRolaActions.RestoreIcon,
                Applies: GitRolaActions.FilesInProject
            )
        );

        // A tree is one material, so its own directory is what a restore of the whole tree names:
        // a file inside it restores that file, and this puts the tree back the way a file's item
        // puts the file back.
        host.ContextMenus.Add(
            ContextMenuTarget.Directory,
            new ContextMenuItem(
                [GitRolaActions.RestoreCategory, GitRolaActions.RestoreToLock],
                100,
                choice => GitRolaActions.RunRestore(host, choice),
                GitRolaActions.RestoreIcon,
                Applies: GitRolaActions.TreeInProject
            )
        );
    }

    /// <summary>
    /// The directory this plugin's translations sit in, beside its assembly.
    /// </summary>
    /// <remarks>
    /// Namespaced by the plugin's identity, as the File System plugin's are: every plugin's files are
    /// laid out under one <c>plugins/</c> directory, and a directory two plugins named would be one set
    /// of nodes, read once.
    /// </remarks>
    private static string Translations() =>
        Path.Combine(
            Path.GetDirectoryName(typeof(GitVCSPlugin).Assembly.Location)!,
            "i18n",
            "rorolala_git"
        );
}

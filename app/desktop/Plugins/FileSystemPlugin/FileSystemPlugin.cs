using System.Reflection;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The File System plugin: the location, and the docks that show it.
/// </summary>
/// <remarks>
/// It is a plugin like any other — the host discovers it, checks its contract and Avalonia versions,
/// and starts it — but it ships with the program and is enabled by default, because a browser is
/// what the shell is for (Section 7.5).
/// <para>
/// What it owns is deliberately more than a dock: the location every dock looks at until one is taken out of
/// step, the answers that stay everybody's either way — where the tree is rooted, and whether hidden entries
/// are shown — the entries at a location, their icons, the menus opened on them, and the navigation that
/// switches where one is. The host is the shell around it.
/// </para>
/// </remarks>
public sealed class FileSystemPlugin : IRolaPlugin
{
    /// <summary>
    /// The stable name of the directory dock.
    /// </summary>
    /// <remarks>
    /// Written into dock layout, so it must not change once shipped. It says <c>browser</c> because
    /// that is what the dock was called before the tree was taken out of it (Section 7.5), and a name
    /// written into a user's layout is not a name to correct.
    /// </remarks>
    public const string DirectoryDockNameId = "rorolala.file_system.browser";

    /// <summary>
    /// The stable name of the folder tree dock.
    /// </summary>
    /// <remarks>
    /// Written into dock layout, so it must not change once shipped.
    /// </remarks>
    public const string TreeDockNameId = "rorolala.file_system.tree";

    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("rorolala.file_system"),
            "rorolala_file_system.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            []
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
        host.I18n.RegisterDirectory(Translations());

        // The commands the agent carries the file operations out with, and the reading of them. They are
        // settings rather than constants because the tool that does the work is another program's, and a
        // system may keep it elsewhere — or a user may prefer another. What they are until they are changed
        // is this program's own operations, named in full so that the default works without a path.
        FileOps.Configure(host.Config);
        host.Config.Add(new PluginSetting(FileOps.CopySetting, SettingKind.Text, "rorolala_file_system.setting.copy", FileOps.DefaultCopy, 10));
        host.Config.Add(new PluginSetting(FileOps.MoveSetting, SettingKind.Text, "rorolala_file_system.setting.move", FileOps.DefaultMove, 20));
        host.Config.Add(new PluginSetting(FileOps.RemoveDirsSetting, SettingKind.Text, "rorolala_file_system.setting.remove_dirs", FileOps.DefaultRemoveDirs, 30));
        host.Config.Add(new PluginSetting(FileOps.RemoveFilesSetting, SettingKind.Text, "rorolala_file_system.setting.remove_files", FileOps.DefaultRemoveFiles, 40));

        // One location for the whole plugin, and one set of answers every location shares. Every dock is a
        // view onto the location — the browser docks differ in layout and in nothing else, and the navigation
        // dock has one address and one history to show — unless a dock has been taken out of step, in which
        // case it is given a location of its own (Section 7.5) and only the answers stay everybody's. The
        // clipboard is shared the same way: a copy made in one directory dock is a copy the other can paste.
        var at = Start();
        var shared = new Shared(at);
        var browser = new Browser(host.Log, shared, at);
        var clip = new Clip(host.Log);

        // A browser with nothing watching the filesystem is told to look again when the program is come back to:
        // a change another program made is most likely to have happened while it was in front.
        host.Refocus.Regained += browser.Touch;

        host.Docks.Register(
            new DockRegistration(
                Manifest.Id,
                DirectoryDockNameId,
                "rorolala_file_system.directories",
                DockOpenMode.New,
                DockPlacement.Center,
                _ => new DirectoryDock(host, shared, browser, clip)
            )
        );

        host.Docks.Register(
            new DockRegistration(
                Manifest.Id,
                TreeDockNameId,
                "rorolala_file_system.tree",
                DockOpenMode.Toggle,
                DockPlacement.Left,
                _ => new TreeDock(host, browser, clip)
            )
        );
    }

    /// <summary>The directory to look at when nothing has said otherwise.</summary>
    /// <remarks>
    /// Where the program was started, which is where a run was made, falling back to the user's own
    /// directory when that is not somewhere that can be read.
    /// </remarks>
    private static string Start()
    {
        var here = Environment.CurrentDirectory;

        return System.IO.Directory.Exists(here)
            ? here
            : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
    }

    /// <summary>
    /// The directory this plugin's translations sit in, beside its assembly.
    /// </summary>
    /// <remarks>
    /// Namespaced by the plugin's identity, since every plugin's files are laid out under one
    /// <c>plugins/</c> directory and a directory registered by two plugins would be one set of
    /// nodes, read once.
    /// </remarks>
    private static string Translations() =>
        Path.Combine(
            Path.GetDirectoryName(typeof(FileSystemPlugin).Assembly.Location)!,
            "i18n",
            "rorolala_file_system"
        );
}

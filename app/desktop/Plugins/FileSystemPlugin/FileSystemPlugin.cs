using System.Diagnostics.CodeAnalysis;
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
    /// This plugin's identity, which a plugin depending on it names in its manifest.
    /// </summary>
    /// <remarks>
    /// It is public so that a dependent plugin — Rorolala's own among them — need not spell the string
    /// again: a dependency that disagreed with this would be one the host could not satisfy.
    /// </remarks>
    public const string Identity = "rorolala.file_system";

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
            new PluginId(Identity),
            "rorolala_file_system.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            []
        );

    /// <inheritdoc />
    /// <remarks>
    /// The one location made here is the plugin's for the whole run: every dock is made over it, and it is
    /// handed on rather than handed over, so there is no moment at which it stops being needed and nothing to
    /// let it go at. The rule cannot tell that from a location that leaked, so it is excused beside the method.
    /// </remarks>
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the whole location lives as long as the plugin does, which is why it is made once here and never let go of"
    )]
    public void Initialize(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.I18n.RegisterDirectory(Translations());

        // The commands the file operations are carried out with, and the reading of them. They are
        // settings rather than constants because the tool that does the work is another program's, and a
        // system may keep it elsewhere — or a user may prefer another. What they are until they are changed
        // is the system's own tools, chosen for the platform this is running on, and the preset above the
        // four is the way to fill all of them at once — or the way a dependent plugin, Rorolala's own
        // among them, offers a set of its own (see FileOperationPresets).
        // The host's own picture goes in with them: the one window this plugin shows — the question a
        // conflict puts — is drawn as the program rather than as something of its own. So does the
        // host's windows for the same reason: what a command said when it refused is put over the
        // window the run was started from rather than left in a log.
        FileOps.Configure(host.Config, host.ProgramIcon, host.Dialogs);
        FileOperationPresets.BuiltIn();
        host.Config.Add(
            new PluginSetting(
                FileOps.PresetSetting,
                SettingKind.Preset,
                "rorolala_file_system.setting.operations",
                null,
                5,
                false,
                FileOperationPresets.Options
            )
        );
        host.Config.Add(new PluginSetting(FileOps.CopySetting, SettingKind.Text, "rorolala_file_system.setting.copy", FileOps.DefaultCopy, 10));
        host.Config.Add(new PluginSetting(FileOps.MoveSetting, SettingKind.Text, "rorolala_file_system.setting.move", FileOps.DefaultMove, 20));
        host.Config.Add(new PluginSetting(FileOps.RemoveDirsSetting, SettingKind.Text, "rorolala_file_system.setting.remove_dirs", FileOps.DefaultRemoveDirs, 30));
        host.Config.Add(new PluginSetting(FileOps.RemoveFilesSetting, SettingKind.Text, "rorolala_file_system.setting.remove_files", FileOps.DefaultRemoveFiles, 40));

        // The hide providers are the plugin's own defaults, and the settings they are read through are
        // declared here; a dependent plugin adds its provider during its own start, which the host runs after
        // this one, and the setting's options are the live catalogue (see HideRegistry).
        var hides = new HideRegistry(host.Config);
        HideRegistry.Declare(host);

        // One location for the whole plugin, and one set of answers every location shares. Every dock is a
        // view onto the location — the browser docks differ in layout and in nothing else, and the navigation
        // dock has one address and one history to show — unless a dock has been taken out of step, in which
        // case it is given a location of its own (Section 7.5) and only the answers stay everybody's. The
        // clipboard is shared the same way: a copy made in one directory dock is a copy the other can paste.
        var at = Start();
        var shared = new Shared(at, hides, host.Config);
        var browser = new Browser(host.Log, shared, at);
        var clip = new Clip(host.Log);

        // Either setting changes what is shown rather than what is there: every location stages its listing
        // again out of what was read, and no directory is read a second time.
        host.Config.Changed += setting =>
        {
            if (
                string.Equals(setting, HideRegistry.Setting, StringComparison.Ordinal)
                || string.Equals(setting, Shared.ShowSetting, StringComparison.Ordinal)
            )
            {
                shared.Hidden();
            }
        };

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

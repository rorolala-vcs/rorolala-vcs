using System.Reflection;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The File System plugin: the browser, and everything it contributes to the host.
/// </summary>
/// <remarks>
/// It is a plugin like any other — the host discovers it, checks its contract and Avalonia versions,
/// and starts it — but it ships with the program and is enabled by default, because a browser is
/// what the shell is for (Section 7.5).
/// <para>
/// What it owns is deliberately more than a dock: the entries, their icons, the menus opened on
/// them, and the navigation that moves between directories. The host is the shell around it.
/// </para>
/// </remarks>
public sealed class FileSystemPlugin : IRolaPlugin
{
    /// <summary>
    /// The stable name of the browser dock.
    /// </summary>
    /// <remarks>
    /// Written into dock layout, so it must not change once shipped.
    /// </remarks>
    public const string BrowserDockNameId = "rorolala.file_system.browser";

    /// <summary>
    /// The stable name of the navigation dock.
    /// </summary>
    /// <remarks>
    /// Written into dock layout, so it must not change once shipped.
    /// </remarks>
    public const string NavigationDockNameId = "rorolala.file_system.navigation";

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

        // One navigator for the whole plugin. The browsers are made per dock and the navigation dock
        // is made once, in whatever order the user opens them, so the only way the toolbar can reach
        // a browser is through something both factories were given.
        var navigator = new Navigator();

        host.Docks.Register(
            new DockRegistration(
                Manifest.Id,
                BrowserDockNameId,
                "rorolala_file_system.dock",
                DockOpenMode.New,
                DockPlacement.Center,
                _ => new BrowserDock(host, navigator)
            )
        );

        host.Docks.Register(
            new DockRegistration(
                Manifest.Id,
                NavigationDockNameId,
                "rorolala_file_system.navigation",
                DockOpenMode.Toggle,
                DockPlacement.Top,
                _ => new NavigationDock(host, navigator)
            )
        );
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

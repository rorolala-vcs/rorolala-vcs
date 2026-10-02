using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace GitVCSPlugin;

/// <summary>
/// The Git plugin: it offers Git's own notion of what is ignored to the File System plugin.
/// </summary>
/// <remarks>
/// It carries no dock and no window of its own. What it does is contribute one hide provider — the
/// entries Git ignores — through the File System plugin's extension point, which it reaches by
/// depending on that plugin: the reference is what lets it name <see cref="HideRegistry"/>, and the
/// manifest's dependency is what has the host start this one after it. A plugin of its own rather than
/// a part of the browser, because Git's rules are the version control system's to offer, not the file
/// manager's — and a browser that knew about <c>.gitignore</c> would know about one system only.
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

        // The File System plugin's catalogue, reached the way a preset is: this plugin is started after
        // that one, so the catalogue it adds to is the one the setting was declared over, and the option
        // this adds is offered the moment the preference panel is drawn (see HideRegistry).
        HideRegistry.Register(new GitIgnored());
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

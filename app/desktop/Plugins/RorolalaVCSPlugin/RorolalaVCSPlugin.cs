using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Rorolala plugin: it offers Rorolala's own file operations to the File System plugin, and shows
/// the work through three docks of its own.
/// </summary>
/// <remarks>
/// What it contributes to the browser is a preset — the <c>rola fs-ops</c> commands — and what it adds
/// beside it is the three views of the work: Overview, Workspace and Version Graph. Both are reached by
/// depending on the File System plugin: the reference is what lets it name
/// <see cref="FileOperationPresets"/>, and the manifest's dependency is what has the host start this one
/// after it. A plugin of its own rather than a part of the browser, because Rorolala's operations and
/// what Rorolala works on are the version control system's to offer, not the file manager's.
/// </remarks>
public sealed class RorolalaVCSPlugin : IRolaPlugin
{
    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId("rorolala.vcs"),
            "rorolala_vcs.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId(FileSystem.Identity)]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.I18n.RegisterDirectory(Translations());
        FileOperationPresets.Add(new RorolalaOperations());

        // What a state is coloured is this plugin's answer rather than the theme's, so the four colours are its
        // own settings: declaring them is what puts them in the preference panel, and what draws them reads
        // them as it draws, so a colour chosen there is a colour the next pill or dot is drawn in.
        RorolalaColours.Declare(host.Config);

        // And what the work holds: the Ownership column of a directory listing, and the corner a
        // locked tile wears. It is this plugin's to answer rather than the File System plugin's,
        // because who holds an entry is Rorolala's question and the answer comes through the C ABI.
        RorolalaLocks.Declare(host);

        // What an entry's menu offers about who holds it: the two ownership commands, plain and forced.
        OwnershipActions.Declare(host);

        // And what the work is looked at through: the three docks, which are this plugin's own windows
        // rather than anything the File System plugin is asked for. What they will hold is Rorolala's,
        // while the base directory they are read against is that plugin's — which is why the two are one
        // plugin depending on another (see RorolalaDocks).
        RorolalaDocks.Declare(host, Manifest.Id);
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
            Path.GetDirectoryName(typeof(RorolalaVCSPlugin).Assembly.Location)!,
            "i18n",
            "rorolala_vcs"
        );
}

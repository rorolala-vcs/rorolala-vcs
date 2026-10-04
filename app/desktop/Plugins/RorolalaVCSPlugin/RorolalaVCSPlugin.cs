using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Rorolala plugin: it offers Rorolala's own file operations to the File System plugin.
/// </summary>
/// <remarks>
/// It carries no dock and no window of its own. What it does is contribute a preset — the
/// <c>rola fs-ops</c> commands — through the File System plugin's extension point, which it reaches by
/// depending on that plugin: the reference is what lets it name <see cref="FileOperationPresets"/>, and
/// the manifest's dependency is what has the host start this one after it. A plugin of its own rather
/// than a part of the browser, because Rorolala's operations are the version control system's to offer,
/// not the file manager's.
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

        // And what the work holds: the Ownership column of a directory listing, and the corner a
        // locked tile wears. It is this plugin's to answer rather than the File System plugin's,
        // because who holds an entry is Rorolala's question and the answer comes through the C ABI.
        RorolalaLocks.Declare(host);

        // What an entry's menu offers about who holds it: the two ownership commands, plain and forced.
        OwnershipActions.Declare(host);
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

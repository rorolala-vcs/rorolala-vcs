using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace SightPlugin;

/// <summary>
/// The Sight plugin: one dock that shows what the File System's chosen file is.
/// </summary>
/// <remarks>
/// Its only plugin dependency is the File System plugin, which is what publishes the choice and owns
/// the listing. It reads the files itself and asks the C ABI for nothing. The previewers it registers
/// are the built-in ones; another plugin adds its own by referencing this one and calling
/// <see cref="SightPreviewers.Register"/>, and another kind of metadata by calling
/// <see cref="SightMetadataProviders.Register"/>.
/// </remarks>
public sealed class SightPlugin : IRolaPlugin
{
    /// <summary>This plugin's identity, which a plugin depending on it names in its manifest.</summary>
    public const string Identity = "rorolala.sight";

    /// <summary>
    /// The stable name of the preview dock.
    /// </summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string PreviewDockNameId = "rorolala.sight.preview";

    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId(Identity),
            "rorolala_sight.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId(FileSystem.Identity)]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.I18n.RegisterDirectory(Translations());
        Icons.Declare(host.Icons);
        SightSettings.Declare(host.Config);

        // The built-in previewers, in the order they are offered; the ranks decide which is tried
        // first when more than one cares for the same file.
        SightPreviewers.Register(new SightText());
        SightPreviewers.Register(new SightImage());
        SightPreviewers.Register(new SightDiff());
        SightPreviewers.Register(new SightModel());
        SightPreviewers.Register(new SightPsd());

        // What the corner says: the document's own numbers, then the three documents that carry
        // more than a format.
        SightMetadataProviders.Register(new SightDocumentMetadata());
        SightMetadataProviders.Register(new SightDiffMetadata());
        SightMetadataProviders.Register(new SightModelMetadata());
        SightMetadataProviders.Register(new SightPsdMetadata());

        host.Docks.Register(
            new DockRegistration(
                Manifest.Id,
                PreviewDockNameId,
                "rorolala_sight.dock.preview",
                DockOpenMode.Toggle,
                DockSide.Right,
                () => new SightDock(host),
                Icons.Eye
            )
        );

        host.ContextMenus.Add(
            ContextMenuTarget.File,
            new ContextMenuItem(
                "rorolala_sight.menu.preview",
                50,
                _ => host.Windows.Show(PreviewDockNameId, null),
                IconKey: Icons.Eye,
                Applies: target => target.Entries.Any(entry => entry.Kind == EntryKind.File)
            )
        );
        host.ContextMenus.Add(
            ContextMenuTarget.File,
            new ContextMenuItem(
                "rorolala_sight.menu.compare",
                60,
                _ => host.Windows.Show(PreviewDockNameId, null),
                IconKey: Icons.ArrowsLeftRight,
                Applies: target =>
                    target.Entries.Count(entry => entry.Kind == EntryKind.File) == 2
            )
        );
    }

    /// <summary>The directory this plugin's translations sit in, beside its assembly.</summary>
    /// <returns>The directory.</returns>
    private static string Translations() =>
        Path.Combine(
            Path.GetDirectoryName(typeof(SightPlugin).Assembly.Location)!,
            "i18n",
            "rorolala_sight"
        );
}

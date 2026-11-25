using System.Globalization;
using RorolalaDesktop.I18n;

namespace SightPlugin;

/// <summary>One thing the preview says about what it is showing.</summary>
/// <param name="LabelKey">An i18n key naming the thing.</param>
/// <param name="Value">What it says, already written as a person reads it.</param>
public sealed record SightMetadataItem(string LabelKey, string Value);

/// <summary>What a metadata provider is asked about.</summary>
/// <param name="Primary">The file chosen last.</param>
/// <param name="Second">The other chosen file, when two are chosen.</param>
/// <param name="Document">What the previewer answered, or nothing before one has.</param>
/// <param name="Previewer">The previewer that answered, when one did.</param>
public sealed record SightMetadataContext(
    SightFile Primary,
    SightFile? Second,
    SightDocument? Document,
    ISightPreviewer? Previewer
);

/// <summary>
/// One thing that adds metadata to the preview's corner.
/// </summary>
/// <remarks>
/// A provider reads what the document already holds, and is given nothing to reach a file, a Vault
/// or the network with: a preview is a reading, and its metadata is a second reading of the same
/// answer rather than a new one. Providers run in registration order.
/// </remarks>
public interface ISightMetadataProvider
{
    /// <summary>The stable value this provider is known by, unique among the providers offered.</summary>
    string Id { get; }

    /// <summary>The items to show for this preview, in the order they are read.</summary>
    /// <param name="context">What is being previewed.</param>
    /// <returns>The items, or nothing when this provider has nothing to say.</returns>
    IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context);
}

/// <summary>
/// The metadata providers in force, and where a dependent plugin adds its own.
/// </summary>
public static class SightMetadataProviders
{
    /// <summary>Every provider, in the order it was offered.</summary>
    private static readonly List<ISightMetadataProvider> Catalogue = [];

    /// <summary>Every provider, in the order it was offered.</summary>
    public static IReadOnlyList<ISightMetadataProvider> All => Catalogue;

    /// <summary>Adds a provider after the ones already offered.</summary>
    /// <param name="provider">The provider.</param>
    public static void Register(ISightMetadataProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (Catalogue.Any(other => string.Equals(other.Id, provider.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(provider);
    }

    /// <summary>Everything every provider says, in registration order.</summary>
    /// <param name="context">What is being previewed.</param>
    /// <returns>The items.</returns>
    public static IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        var items = new List<SightMetadataItem>();

        foreach (var provider in Catalogue)
        {
            items.AddRange(provider.Read(context));
        }

        return items;
    }
}

/// <summary>The format, encoding and line count of a text or an image preview.</summary>
internal sealed class SightDocumentMetadata : ISightMetadataProvider
{
    /// <inheritdoc />
    public string Id => "sight.metadata.document";

    /// <inheritdoc />
    public IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        return context.Document switch
        {
            SightTextDocument text =>
            [
                new SightMetadataItem("rorolala_sight.meta.format", SightFormatName.OfText(context.Primary.Extension)),
                new SightMetadataItem("rorolala_sight.meta.encoding", "UTF-8"),
                new SightMetadataItem("rorolala_sight.meta.lines", Number(text.Lines.Count)),
            ],
            SightImageDocument image =>
            [
                new SightMetadataItem("rorolala_sight.meta.format", image.Format),
                new SightMetadataItem(
                    "rorolala_sight.meta.size",
                    $"{Number(image.Picture.PixelSize.Width)} × {Number(image.Picture.PixelSize.Height)}"
                ),
            ],
            _ => [],
        };
    }

    /// <summary>A count, written the way a reader reads one.</summary>
    /// <param name="value">The number.</param>
    /// <returns>The number with its groups.</returns>
    internal static string Number(int value) => value.ToString("N0", CultureInfo.InvariantCulture);
}

/// <summary>What a diff added and removed, and how much context it keeps.</summary>
internal sealed class SightDiffMetadata : ISightMetadataProvider
{
    /// <inheritdoc />
    public string Id => "sight.metadata.diff";

    /// <inheritdoc />
    public IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        return context.Document is SightDiffDocument diff
            ?
            [
                new SightMetadataItem("rorolala_sight.meta.format", "Diff"),
                new SightMetadataItem("rorolala_sight.meta.added", SightDocumentMetadata.Number(diff.Added)),
                new SightMetadataItem("rorolala_sight.meta.removed", SightDocumentMetadata.Number(diff.Removed)),
            ]
            : [];
    }
}

/// <summary>What a model holds.</summary>
internal sealed class SightModelMetadata : ISightMetadataProvider
{
    /// <inheritdoc />
    public string Id => "sight.metadata.model";

    /// <inheritdoc />
    public IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        return context.Document is SightModelDocument model
            ?
            [
                new SightMetadataItem("rorolala_sight.meta.format", model.Format),
                new SightMetadataItem("rorolala_sight.meta.vertices", SightDocumentMetadata.Number(model.Mesh.Vertices.Count)),
                new SightMetadataItem("rorolala_sight.meta.triangles", SightDocumentMetadata.Number(model.Mesh.TriangleCount)),
            ]
            : [];
    }
}

/// <summary>What a PSD header holds, and whether the merged image was decoded.</summary>
internal sealed class SightPsdMetadata : ISightMetadataProvider
{
    /// <inheritdoc />
    public string Id => "sight.metadata.psd";

    /// <inheritdoc />
    public IReadOnlyList<SightMetadataItem> Read(SightMetadataContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        if (context.Document is not SightPsdDocument psd)
        {
            return [];
        }

        var items = new List<SightMetadataItem>
        {
            new("rorolala_sight.meta.format", "PSD"),
            new("rorolala_sight.meta.size", $"{SightDocumentMetadata.Number(psd.Width)} × {SightDocumentMetadata.Number(psd.Height)}"),
            new("rorolala_sight.meta.channels", SightDocumentMetadata.Number(psd.Channels)),
            new("rorolala_sight.meta.depth", $"{SightDocumentMetadata.Number(psd.Depth)}-bit"),
            new("rorolala_sight.meta.mode", psd.Mode),
            new("rorolala_sight.psd.composite", RolaI18N.Get(psd.Picture is null ? "rorolala_sight.psd.none" : "rorolala_sight.psd.composite")),
            new("rorolala_sight.meta.layers", RolaI18N.Get("rorolala_sight.psd.note")),
        };

        return items;
    }
}

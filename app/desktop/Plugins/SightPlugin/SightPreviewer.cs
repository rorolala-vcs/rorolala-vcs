using RorolalaDesktop.Contract;

namespace SightPlugin;

/// <summary>One file a preview may be made of, as the previewers see it.</summary>
/// <param name="Path">Where it is.</param>
/// <param name="Length">How many bytes it holds.</param>
/// <param name="Extension">Its extension, without the dot and in lower case.</param>
/// <param name="Name">Its name, which the dock never draws and a previewer may read.</param>
/// <param name="WriteTicks">When it was last written, which is part of the cache key.</param>
public sealed record SightFile(string Path, long Length, string Extension, string Name, long WriteTicks)
{
    /// <summary>
    /// Reads what a path holds now.
    /// </summary>
    /// <param name="path">The file.</param>
    /// <returns>The file, as the previewers see it.</returns>
    public static SightFile Read(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var info = new FileInfo(path);

        return new SightFile(
            path,
            info.Exists ? info.Length : 0,
            info.Extension.TrimStart('.').ToLowerInvariant(),
            info.Name,
            info.Exists ? info.LastWriteTimeUtc.Ticks : 0
        );
    }
}

/// <summary>What one preview is asked for.</summary>
/// <param name="Primary">The file chosen last, which is what a single-file preview is of.</param>
/// <param name="Second">The other chosen file, when two are chosen and a diff may be made.</param>
/// <param name="Host">The host, for a previewer that needs a service.</param>
public sealed record SightOpenRequest(SightFile Primary, SightFile? Second, IPluginHost Host);

/// <summary>
/// One thing that can show a file: a previewer.
/// </summary>
/// <remarks>
/// It is the list's entry and the extension point at once. <see cref="Cares"/> is asked before
/// anything is read, so it looks at the name and the few magic bytes a previewer already knows and no
/// more; <see cref="OpenAsync"/> is where a file whose content does not match its name is refused, by
/// answering nothing.
/// </remarks>
public interface ISightPreviewer
{
    /// <summary>The stable value this previewer is known by, unique among the previewers offered.</summary>
    string Id { get; }

    /// <summary>An i18n key the previewer is listed under.</summary>
    string NameKey { get; }

    /// <summary>A picture in the icon library naming it in the list, or nothing for no picture.</summary>
    string? IconKey { get; }

    /// <summary>How early it is tried when several previewers care for the same file.</summary>
    int Rank { get; }

    /// <summary>Whether it may answer for these files, cheaply.</summary>
    /// <param name="primary">The file chosen last.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <returns>Whether it is worth asking <see cref="OpenAsync"/>.</returns>
    bool Cares(SightFile primary, SightFile? second);

    /// <summary>The preview, or nothing when this previewer cannot make one.</summary>
    /// <param name="request">The files to preview.</param>
    /// <param name="cancel">Cancels the work.</param>
    /// <returns>The document, or nothing.</returns>
    ValueTask<SightDocument?> OpenAsync(SightOpenRequest request, CancellationToken cancel);
}

/// <summary>
/// The previewers in force, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// A previewer registered twice is ignored rather than replaced, so two plugins claiming one name
/// cannot have the list show one and the dock draw the other. The catalogue is filled during
/// initialization, before the dock is made, so a previewer added after the window is shown is not
/// re-listed.
/// </remarks>
public static class SightPreviewers
{
    /// <summary>Every previewer, in the order it was offered.</summary>
    private static readonly List<ISightPreviewer> Catalogue = [];

    /// <summary>Every previewer, in the order it was offered.</summary>
    public static IReadOnlyList<ISightPreviewer> All => Catalogue;

    /// <summary>Adds a previewer after the ones already offered.</summary>
    /// <param name="previewer">The previewer.</param>
    public static void Register(ISightPreviewer previewer)
    {
        ArgumentNullException.ThrowIfNull(previewer);

        if (Catalogue.Any(other => string.Equals(other.Id, previewer.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(previewer);
    }

    /// <summary>The previewer known by `id`, or nothing when none is.</summary>
    /// <param name="id">The stable value.</param>
    /// <returns>The previewer, or nothing.</returns>
    public static ISightPreviewer? ById(string id) =>
        Catalogue.FirstOrDefault(previewer =>
            string.Equals(previewer.Id, id, StringComparison.Ordinal)
        );

    /// <summary>
    /// The previewers that may answer for these files, the highest rank first.
    /// </summary>
    /// <remarks>
    /// The order is the answer to an automatic choice: the first previewer that returns a document
    /// is the one drawn. A tie keeps registration order, which is load order, so a plugin cannot
    /// move ahead of a built-in without saying it ranks higher.
    /// </remarks>
    /// <param name="primary">The file chosen last.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <returns>The previewers to try, in order.</returns>
    public static IReadOnlyList<ISightPreviewer> Ordered(SightFile primary, SightFile? second) =>
        Catalogue
            .Where(previewer => previewer.Cares(primary, second))
            .OrderByDescending(previewer => previewer.Rank)
            .ToArray();
}

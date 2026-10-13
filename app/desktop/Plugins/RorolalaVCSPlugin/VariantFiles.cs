using RolaSharp;

namespace RorolalaVCSPlugin;

/// <summary>
/// The variant files a reading holds: which merge a file of the tree is the variant file of.
/// </summary>
/// <remarks>
/// A merge in progress is a record of a target — the file the Layout names that the variant is to be joined
/// into — and of a variant whose content is written beside it as a file of its own. The variant file is not
/// a path either Layout names, so it is the one thing about a merge that no path of the work answers: the
/// listing shows it as a file nobody has heard of, and what makes it legible — its badge, the column that
/// names it, and the button that ends the merge — is read from here.
/// <para>
/// What the reading names is relative to the Workspace it was made of, and what a listing has is a path the
/// system names, so the two are joined before they are compared: the same thing the reading itself does for
/// a state or a standing, done here because a merge is reported as a list rather than asked about a path.
/// </para>
/// </remarks>
internal static class VariantFiles
{
    /// <summary>How much of a variant's hash is shown.</summary>
    /// <remarks>
    /// The head of it, which is what a person recognises a variant by: the whole hash is sixty-four characters
    /// and says nothing more in a pill, a badge or a column than its first few do.
    /// </remarks>
    private const int Short = 7;

    /// <summary>
    /// The merge whose variant file lies at `path`, when one does.
    /// </summary>
    /// <remarks>
    /// A merge whose variant file has moved is found where the reading says it now is rather than where the
    /// record puts it: the record is what `--rename` settles, and the file is what a reader is looking at.
    /// </remarks>
    /// <param name="merges">The merges the reading holds.</param>
    /// <param name="root">Where the Workspace the reading was made of is rooted.</param>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>The merge, or nothing when the path is not a variant file of one.</returns>
    public static Merge? At(IReadOnlyList<Merge> merges, string root, string path)
    {
        ArgumentNullException.ThrowIfNull(merges);
        ArgumentNullException.ThrowIfNull(root);
        ArgumentNullException.ThrowIfNull(path);

        return merges.FirstOrDefault(merge => StatusReadings.Names(root, Place(merge), path));
    }

    /// <summary>Where the tree holds a merge's variant file, which is where it moved to when it moved.</summary>
    /// <param name="merge">The merge.</param>
    /// <returns>The path, relative to the Workspace the reading was made of.</returns>
    public static string Place(Merge merge)
    {
        ArgumentNullException.ThrowIfNull(merge);

        return merge.Moved is { Length: > 0 } moved ? moved : merge.Path;
    }

    /// <summary>What a variant is known by, which is the head of the hash it is named by.</summary>
    /// <param name="variant">The hash as the reading gives it.</param>
    /// <returns>The head of it.</returns>
    public static string Head(string variant)
    {
        ArgumentNullException.ThrowIfNull(variant);

        return variant.Length > Short ? variant[..Short] : variant;
    }
}

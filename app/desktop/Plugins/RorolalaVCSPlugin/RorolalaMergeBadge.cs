using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Rorolala's answer to what is waiting to be joined into a file: the badge a variant file's icon wears.
/// </summary>
/// <remarks>
/// A merge in progress is two files: the one the Layout names, which wears the state dot of work under way,
/// and the variant file beside it, which holds the content waiting to become part of it. The second is a
/// file neither Layout names, so nothing else says what it is — a listing draws it as an untracked file,
/// which it is not — and the badge is where that is said: the set's two lines coming together into one,
/// drawn in the amber every other mark of a merge is drawn in.
/// <para>
/// It is read through the same <see cref="StatusReadings"/> the State and Standing columns read through, so
/// a directory listed twice pays for one reading: what a merge is waiting for is a walk of the work, and the
/// three marks about a file are read from the one walk.
/// </para>
/// </remarks>
internal sealed class RorolalaMergeBadge : IEntryMarkProvider
{
    /// <summary>What the set draws for two things coming together into one.</summary>
    private const string Join = Icons.Merge;

    /// <summary>What the work says about the directories being listed, shared with the list's own columns.</summary>
    private readonly StatusReadings _readings;

    /// <summary>Makes the provider.</summary>
    /// <param name="readings">What the work says about the directories being listed.</param>
    public RorolalaMergeBadge(StatusReadings readings) => _readings = readings;

    /// <inheritdoc />
    public string Id => "rorolala.merge.badge";

    /// <inheritdoc />
    public bool Applies(string directory) => _readings.Reading(directory) is not null;

    /// <inheritdoc />
    public EntryMark Mark(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        // What a merge is about is a file of the work, so an entry that names no directory of its own — the
        // way up, which is a step rather than a place — is nothing this speaks about.
        if (
            Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
            || _readings.Reading(directory) is not { } status
        )
        {
            return new EntryMark();
        }

        try
        {
            return VariantFiles.At(status.Merging, status.Root, entry.Path) is { } merge
                ? Say(merge)
                : new EntryMark();
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new EntryMark();
        }
    }

    /// <summary>
    /// What a variant file is worn as: the two lines coming together, in the amber of work under way.
    /// </summary>
    /// <remarks>
    /// Worn on the dark ground the other badges wear, since a tile carries marks of several kinds at once and
    /// a bare picture beside a badge would be two of them. The words are the mark's own rather than a phrase
    /// read around them: there is nothing to say beside which variant it is, because the variant <em>is</em>
    /// what the file is.
    /// </remarks>
    /// <param name="merge">The merge the file is the variant file of.</param>
    /// <returns>The mark to draw.</returns>
    internal static EntryMark Say(Merge merge)
    {
        ArgumentNullException.ThrowIfNull(merge);

        return new EntryMark(
            IconKey: Join,
            Ink: MarkInk.Warn,
            Tagged: true,
            Text: RolaI18N.Get("rorolala_vcs.variant.pending", VariantFiles.Head(merge.Variant))
        );
    }

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared with the list's own columns, so this may be the third letting go of the same
    /// set: what is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget() => _readings.Forget();
}

using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>
/// How one entry is marked: the words for it, the picture for it, and the ink.
/// </summary>
/// <remarks>
/// The words and the picture are told apart because the two views want different ones — a table has a
/// column of text and a tile has a badge to draw — and a contribution says both so that neither view has to
/// decide what a mark looks like. The picture is named by a key rather than carried, since what is drawn is
/// the icon library's to hold and the key is how everything else in the contract names a picture.
/// <para>
/// They are drawn in two inks for the same reason they are two things: what a picture says by being
/// coloured — the reader's own, which is the accent — is not always what the words want, which may be to
/// read as any other text. A contribution that wants both the same says so twice.
/// </para>
/// <para>
/// One record serves both places an entry is marked, because the two are one statement about one entry: a
/// contribution asked twice would be one whose column and whose badge could disagree. What a mark with
/// nothing in it means is the view's to read — nothing drawn on a badge, the dash the listing writes for
/// what it does not know in a column.
/// </para>
/// </remarks>
/// <param name="TextKey">An i18n key for the words, when they are the contributor's own.</param>
/// <param name="Text">
/// The words themselves, when they are not a phrase — an account's name, which is data. Said, it
/// wins over <paramref name="TextKey"/>.
/// </param>
/// <param name="IconKey">A key naming the picture in the icon library, or nothing for no picture.</param>
/// <param name="Ink">What the picture is drawn in.</param>
/// <param name="TextInk">What the words are drawn in.</param>
/// <param name="Tagged">
/// Whether the picture is drawn on the dark ground a badge wears rather than bare on the tile. A badge that
/// says something is wrong wants saying, and one that says an entry is the reader's own does not.
/// </param>
/// <param name="HoverKey">
/// A key for what the badge says when the pointer rests on it, with the mark's own words as the one value it
/// leaves a place for, or nothing to be hovered for the mark's own words as they stand.
/// </param>
/// <param name="Colour">
/// The picture's own colour, where the role the look holds is not the one it is said in: a contribution with a
/// palette of its own says what its marks are coloured rather than which of the look's roles they wear. It wins
/// over <paramref name="Ink"/>, and nothing means the role is what is drawn.
/// </param>
public sealed record EntryMark(
    string? TextKey = null,
    string? Text = null,
    string? IconKey = null,
    MarkInk Ink = MarkInk.Quiet,
    MarkInk TextInk = MarkInk.Quiet,
    bool Tagged = false,
    string? HoverKey = null,
    Color? Colour = null
)
{
    /// <summary>
    /// What the mark says in words: a holder's name, or what the column calls a state that has no name.
    /// </summary>
    /// <remarks>
    /// Read by the column and by what the badge says when the pointer rests on it, because the two are one
    /// answer: a mark that is a name in the column is that name on the badge as well. The literal comes first
    /// because an account's name is data — a name that happens to read like a key is still what somebody is
    /// called.
    /// </remarks>
    /// <returns>What to write, or nothing when the mark says nothing at all.</returns>
    public string? Said() =>
        Text is { Length: > 0 } literal ? literal
        : TextKey is { Length: > 0 } key ? RolaI18N.Get(key)
        : null;

    /// <summary>
    /// What the mark says when the pointer rests on its badge, which may be a phrase around its own words.
    /// </summary>
    /// <remarks>
    /// A phrase of its own rather than the words the column is read for, because the two are read in different
    /// places: a column beside a name says only the name, while a badge that is a picture of a lock says nothing
    /// about who holds it until it is given the words. The mark's own words are the argument, so a phrase can be
    /// written around a name without the renderer having to know which part of it is the name — and one place in
    /// a phrase is all the translation formatter takes.
    /// </remarks>
    /// <returns>What to say, which is the mark's own words when it names no phrase of its own.</returns>
    public string? Hover() =>
        HoverKey is { Length: > 0 } key && Said() is { Length: > 0 } said
            ? RolaI18N.Get(key, said)
            : Said();
}

/// <summary>
/// One plugin's answer to what an entry wears on its icon.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c> may hand
/// one of these to <see cref="EntryMarkProviders.Register"/>. What is contributed is a badge of an entry's
/// icon in a grid, and what the contribution is about is its own — the listing knows how to lay the badges
/// out and never what one means.
/// <para>
/// Every provider that speaks for a directory is asked, and the order they were registered in is the order
/// their badges are numbered: what a contribution says is its own, and two of them marking one tile say two
/// things rather than one answer to one question.
/// </para>
/// <para>
/// A provider answers about a directory as much as about an entry, because whether an entry has a badge at
/// all is not always a fact about the entry: whether it is in a work is a fact about the directory it sits
/// in, and a listing outside any such work has no badges.
/// </para>
/// </remarks>
public interface IEntryMarkProvider
{
    /// <summary>The stable value this provider is known by, unique among the providers offered.</summary>
    string Id { get; }

    /// <summary>
    /// Whether this provider speaks for the entries of a directory.
    /// </summary>
    /// <remarks>
    /// Asked once for the directory a listing is of, not once per entry: it is what decides whether an answer
    /// is looked for at all, and a badge that appeared and disappeared row by row would not be a badge. A
    /// provider that cannot answer — a directory it may not inspect — says no rather than failing the
    /// listing.
    /// </remarks>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether this provider has anything to say about its entries.</returns>
    bool Applies(string directory);

    /// <summary>
    /// How one entry is shown on the icon it wears.
    /// </summary>
    /// <remarks>
    /// A mark with nothing in it says the provider has nothing to say about the entry, and nothing is drawn
    /// for it. It is an answer rather than an absence so that a provider which says nothing about an entry —
    /// a directory, which no Layout names — has no second way of saying it.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>The mark to draw.</returns>
    EntryMark Mark(Entry entry);

    /// <summary>
    /// Says that what this provider answered about entries may have changed.
    /// </summary>
    /// <remarks>
    /// A listing is read again when the files may have changed, and what a provider answered about them is
    /// read from somewhere such a change reaches: a Workspace's Layout names each path by a <c>Uuid</c>, and a
    /// move or a removal renames one of them, so an answer about a path that was moved is an answer about
    /// nothing. A provider that keeps anything is told to let go of it here; one that reads afresh each time
    /// has nothing to do, which is what the default does.
    /// <para>
    /// It is said before the entries are read rather than after, since a listing built from an answer that was
    /// already stale would be drawn wrong until the next one.
    /// </para>
    /// </remarks>
    void Forget() { }
}

/// <summary>
/// The mark providers in force, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// Every provider that speaks for a directory is asked, in the order they were registered: the badges of one
/// tile are numbered in that order, so a plugin added later is read after the ones already there rather than
/// taking a place of its own. A provider of an identity already offered is ignored rather than replacing it,
/// so that two plugins claiming one name cannot have a listing show one and a menu name the other.
/// </remarks>
public static class EntryMarkProviders
{
    /// <summary>Every provider registered this run, in the order they are offered.</summary>
    private static readonly List<IEntryMarkProvider> Catalogue = [];

    /// <summary>Every provider, in the order they are offered.</summary>
    public static IReadOnlyList<IEntryMarkProvider> All => Catalogue;

    /// <summary>
    /// Adds a provider, after the ones already added.
    /// </summary>
    /// <param name="provider">The provider to add.</param>
    public static void Register(IEntryMarkProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (Catalogue.Any(other => string.Equals(other.Id, provider.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(provider);
    }

    /// <summary>
    /// The providers that speak for a directory's entries, in the order their badges are numbered.
    /// </summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The providers to ask.</returns>
    public static IReadOnlyList<IEntryMarkProvider> For(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        return [.. Catalogue.Where(provider => provider.Applies(directory))];
    }

    /// <summary>
    /// Says to every provider that what it answered may have changed.
    /// </summary>
    /// <remarks>
    /// Said to all of them rather than to the ones a directory is answered by: what is read again is not
    /// known here, and what a provider is holding is its own business — one with nothing to let go of
    /// answers nothing to it.
    /// </remarks>
    public static void Forget()
    {
        foreach (var provider in Catalogue)
        {
            provider.Forget();
        }
    }

    /// <summary>
    /// Forgets every provider, which the suite that drives this plugin needs.
    /// </summary>
    /// <remarks>
    /// The catalogue is the run's, and a run starts a program once: only a suite that starts the plugin
    /// over and over in one process has a reason to clear it, and it has to, or one test's provider would
    /// answer for the next test's listing.
    /// </remarks>
    internal static void Clear() => Catalogue.Clear();
}

/// <summary>One badge a tile's icon wears: the picture to draw, and what the contribution said about the entry.</summary>
/// <remarks>
/// The picture is looked up by the view rather than carried by the contribution, because the library is what
/// holds a picture and a key is how the contract names one; what is brought together here is the picture a
/// key resolved to and the mark it is drawn for, which is all the drawing needs.
/// </remarks>
/// <param name="Picture">What to draw inside the badge.</param>
/// <param name="Mark">What the contribution said about the entry.</param>
internal sealed record EntryBadge(Bitmap Picture, EntryMark Mark);

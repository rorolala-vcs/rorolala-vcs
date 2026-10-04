using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>What ink a lock mark is drawn in.</summary>
/// <remarks>
/// The colour is the contribution's, the brush is not: a plugin names a role and this plugin picks
/// the theme's resource for it, so a mark follows the user's colours rather than being the one
/// colour the plugin was written against.
/// </remarks>
public enum LockInk
{
    /// <summary>The list's own metadata ink: nothing is being drawn attention to.</summary>
    Quiet,

    /// <summary>The ordinary foreground, which reads as any other words on the card.</summary>
    Plain,

    /// <summary>The accent, which marks what is the reader's own.</summary>
    Accent,

    /// <summary>The error red, which marks what is somebody else's.</summary>
    Error,
}

/// <summary>
/// How one entry's lock is shown: the words for it, the picture for it, and the ink.
/// </summary>
/// <remarks>
/// The words and the picture are told apart because the two views want different ones — a table has
/// a column of text and a tile has a corner to draw in — and a contribution says both so that
/// neither view has to decide what a lock looks like. The picture is named by a key rather than
/// carried, since what is drawn is the icon library's to hold and the key is how everything else in
/// the contract names a picture.
/// <para>
/// They are drawn in two inks for the same reason they are two things: what a picture says by being
/// coloured — the reader's own, which is the accent — is not always what the words want, which may be
/// to read as any other text. A contribution that wants both the same says so twice.
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
public sealed record EntryLockMark(
    string? TextKey = null,
    string? Text = null,
    string? IconKey = null,
    LockInk Ink = LockInk.Quiet,
    LockInk TextInk = LockInk.Quiet
);

/// <summary>
/// One plugin's answer to who holds an entry, and how it is shown.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c>
/// may hand one of these to <see cref="EntryLockProviders.Register"/>. What is contributed is the
/// Ownership column of a directory dock's list, and the mark a tile wears in its grid.
/// <para>
/// A provider answers about a directory as much as about an entry, because whether an entry has a
/// lock at all is not a fact about the entry: it is a fact about the work the directory sits in, and
/// a listing outside any such work has no column and no marks.
/// </para>
/// </remarks>
public interface IEntryLockProvider
{
    /// <summary>The stable value this provider is known by, unique among the providers offered.</summary>
    string Id { get; }

    /// <summary>An i18n key naming the column this provider heads.</summary>
    string ColumnKey { get; }

    /// <summary>
    /// Whether this provider speaks for the entries of a directory.
    /// </summary>
    /// <remarks>
    /// Asked once for the directory a listing is of, not once per entry: it is what decides whether
    /// the column is there at all, and a column that appeared and disappeared row by row would not
    /// be a column. A provider that cannot answer — a directory it may not inspect — says no rather
    /// than failing the listing.
    /// </remarks>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether this provider has anything to say about its entries.</returns>
    bool Applies(string directory);

    /// <summary>
    /// How one entry's lock is shown.
    /// </summary>
    /// <remarks>
    /// A mark with nothing in it says the provider has nothing to say about the entry, and is drawn
    /// the way a listing draws what it does not know. It is an answer rather than an absence so that a
    /// reader has one shape to handle, and so that a provider which says nothing about an entry — a
    /// directory, which no Layout names — has no second way of saying it.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>The mark to draw.</returns>
    EntryLockMark Mark(Entry entry);
}

/// <summary>
/// The lock providers in force, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// One at a time is shown, which is why a directory is asked for the provider rather than for all
/// of them: the column is the listing's and there is one of it, and a second provider that spoke
/// for the same directory would be a second answer to one question. The first registered that does
/// is the one shown, and registration is plugin load order.
/// </remarks>
public static class EntryLockProviders
{
    /// <summary>Every provider registered this run, in the order they are offered.</summary>
    private static readonly List<IEntryLockProvider> Catalogue = [];

    /// <summary>Every provider, in the order they are offered.</summary>
    public static IReadOnlyList<IEntryLockProvider> All => Catalogue;

    /// <summary>
    /// Adds a provider, after the ones already added.
    /// </summary>
    /// <remarks>
    /// A provider of an identity already offered is ignored rather than replacing it, so that two
    /// plugins claiming one name cannot have a listing show one and a menu name the other.
    /// </remarks>
    /// <param name="provider">The provider to add.</param>
    public static void Register(IEntryLockProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (Catalogue.Any(other => string.Equals(other.Id, provider.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(provider);
    }

    /// <summary>
    /// The provider that speaks for a directory's entries, or nothing when none does.
    /// </summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The provider to ask, or nothing.</returns>
    public static IEntryLockProvider? For(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        return Catalogue.FirstOrDefault(provider => provider.Applies(directory));
    }

    /// <summary>
    /// Forgets every provider, which the suite that drives this plugin needs.
    /// </summary>
    /// <remarks>
    /// The catalogue is the run's, and a run starts a program once: only a suite that starts the
    /// plugin over and over in one process has a reason to clear it, and it has to, or one test's
    /// provider would answer for the next test's listing.
    /// </remarks>
    internal static void Clear() => Catalogue.Clear();
}

using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>Where a contributed column sits among the listing's own.</summary>
/// <remarks>
/// The place is the contribution's rather than the listing's, because what a column says decides how it is
/// read: a column saying where an entry is is read with the name, and one saying something about the entry
/// is read once what it is has been read. The room the table is read in falls between the two — after what
/// is read with the name and before what is read about the entry — so that the dock's spare width is spent
/// in one place rather than scattered between the columns.
/// </remarks>
public enum ColumnPlacement
{
    /// <summary>
    /// Before everything, the picture included, where a mark that is about the entry stands.
    /// </summary>
    /// <remarks>
    /// A column here is a mark rather than a value — a shape in a colour, with what it means said when the
    /// pointer rests on it — so it is narrow, and it is read with the entry rather than after it. What it
    /// says is what the entry is, and the name beside it is what the entry is called.
    /// </remarks>
    Leftmost,

    /// <summary>Right after the name, before the room the table is read in.</summary>
    /// <remarks>
    /// A column here is the entry's second name — where it is named elsewhere — so it stands with the name
    /// rather than after the listing's own columns: the name and what is read with it are one thing, and the
    /// room the table has to spare is left after them rather than between them.
    /// </remarks>
    BesideName,

    /// <summary>After the room the table is read in, before the listing's own columns.</summary>
    /// <remarks>
    /// A column here is about what the entry is rather than what it is called, so it is read with the
    /// listing's own columns — the time it was written, its size — which it stands in front of, being the
    /// same kind of answer about the same entry.
    /// </remarks>
    BeforeValues,

    /// <summary>After the listing's own columns, which is where what an entry is has been said.</summary>
    AfterValues,
}

/// <summary>What one entry's cell in a contributed column says.</summary>
/// <remarks>
/// The words and the ink, and nothing else: a column of values is text where the listing's own columns are,
/// and one that grew a picture or a tag would be a second kind of thing in a table of one kind. What a
/// contribution wants drawn rather than written — a mark with a picture in it, a badge on a tile — is the
/// Ownership column's business, which is another question asked in another way.
/// </remarks>
/// <param name="TextKey">An i18n key for the words, when they are the contributor's own.</param>
/// <param name="Text">
/// The words themselves, when they are not a phrase — a path, a name, which is data. Said, it wins over
/// <paramref name="TextKey"/>.
/// </param>
/// <param name="Ink">What the words are drawn in, and what a dot is filled with where no colour is given.</param>
/// <param name="Dot">
/// Whether the cell is a round mark in the ink rather than the words: a state that is one of a handful, where
/// the colour is read at a glance and the words would be a column of the same few words repeated. What the
/// mark means is then said by <paramref name="Hint"/> when the pointer rests on it.
/// </param>
/// <param name="Hint">
/// What the pointer resting on the mark says, which is the words the cell does not hold. Nothing to hover for
/// when the words are the cell.
/// </param>
/// <param name="Colour">
/// The mark's own colour, where the role the theme holds is not the one it is said in: a contribution with a
/// palette of its own says what its marks are coloured rather than which of the look's roles they wear. It wins
/// over <paramref name="Ink"/>, and nothing means the role is what is drawn.
/// </param>
/// <param name="Edge">
/// The line around the mark, which is the contribution's for the same reason its colour is — the two are one
/// thing drawn — or nothing for a mark with no line at all.
/// </param>
public sealed record EntryColumnMark(
    string? TextKey = null,
    string? Text = null,
    MarkInk Ink = MarkInk.Quiet,
    bool Dot = false,
    string? Hint = null,
    Color? Colour = null,
    Color? Edge = null
)
{
    /// <summary>What the cell reads as, or nothing when the contribution has nothing to say.</summary>
    /// <remarks>
    /// The literal comes first because a path is data: one that happens to read like a key is still the
    /// path of a file. A mark that is a dot says nothing at all: what it means is the words it is hovered
    /// for, and a cell holding both a dot and a word would be a dot drawn for nothing.
    /// </remarks>
    /// <returns>What to write, or nothing when there is nothing to say about the entry.</returns>
    public string? Said() =>
        Dot ? null
        : Text is { Length: > 0 } literal ? literal
        : TextKey is { Length: > 0 } key ? RolaI18N.Get(key)
        : null;
}

/// <summary>
/// One plugin's column of values in a directory dock's list.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c> may hand
/// one of these to <see cref="EntryColumns.Register"/>. What is contributed is a column of the list, drawn
/// beside the ones the listing has of its own; what a listing may say about an entry from the filesystem
/// alone is the listing's, and what only the work a directory sits in knows is a contribution's.
/// <para>
/// A provider answers about a directory as much as about an entry, because whether a column has anything
/// to say at all is not a fact about the entry: it is a fact about the work the directory sits in, and a
/// listing outside any such work has no column and no cells.
/// </para>
/// </remarks>
public interface IEntryColumnProvider
{
    /// <summary>The stable value this provider is known by, unique among the providers offered.</summary>
    string Id { get; }

    /// <summary>An i18n key naming the column this provider heads.</summary>
    string ColumnKey { get; }

    /// <summary>How wide the column starts, which a reader may change by taking its edge.</summary>
    /// <remarks>
    /// The contribution's rather than the listing's, since what a column holds is the contribution's to
    /// know: a listing that guessed would size a column of paths for a column of marks, or the reverse.
    /// </remarks>
    double Width { get; }

    /// <summary>Where the column sits among the listing's own.</summary>
    ColumnPlacement Placement { get; }

    /// <summary>
    /// Whether this column's cells are marks rather than values.
    /// </summary>
    /// <remarks>
    /// A column of marks says nothing about an entry by having no mark, where a column of values says it the
    /// way the listing says everything it does not know — with a dash. A dash where a mark would be is a mark
    /// of its own, and it says the one thing a cell with no mark cannot mean twice.
    /// </remarks>
    bool Marks { get; }

    /// <summary>
    /// Whether this provider speaks for the entries of a directory.
    /// </summary>
    /// <remarks>
    /// Asked once for the directory a listing is of, not once per entry: it is what decides whether the
    /// column is there at all, and a column that appeared and disappeared row by row would not be a
    /// column. A provider that cannot answer — a directory it may not inspect — says no rather than
    /// failing the listing.
    /// </remarks>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether this provider has anything to say about its entries.</returns>
    bool Applies(string directory);

    /// <summary>
    /// What one entry's cell says.
    /// </summary>
    /// <remarks>
    /// A mark with nothing in it says the provider has nothing to say about the entry, and is drawn the
    /// way a listing draws what it does not know. It is an answer rather than an absence so that a reader
    /// has one shape to handle, and so that a provider which says nothing about an entry — a directory,
    /// which no Layout names — has no second way of saying it.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>What the cell says.</returns>
    EntryColumnMark Value(Entry entry);

    /// <summary>
    /// Says that what this provider answered about entries may have changed.
    /// </summary>
    /// <remarks>
    /// A listing is read again when the files may have changed, and what a provider answered about them is
    /// read from somewhere such a change reaches: a Layout names each path by a `Uuid`, so an answer about
    /// a path that was moved is an answer about nothing. A provider that keeps anything is told to let go
    /// of it here; one that reads afresh each time has nothing to do, which is what the default does.
    /// </remarks>
    void Forget() { }
}

/// <summary>
/// The column providers in force, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// Every provider that speaks for a directory is shown, one column each: the columns are told apart by the
/// key each is headed by, so a second provider is a second column rather than a second answer to one
/// question. A provider of an identity already offered is ignored rather than replacing it, so that two
/// plugins claiming one name cannot have a listing show one and a menu name the other.
/// </remarks>
public static class EntryColumns
{
    /// <summary>Every provider registered this run, in the order they are offered.</summary>
    private static readonly List<IEntryColumnProvider> Catalogue = [];

    /// <summary>Every provider, in the order they are offered.</summary>
    public static IReadOnlyList<IEntryColumnProvider> All => Catalogue;

    /// <summary>
    /// Adds a provider, after the ones already added.
    /// </summary>
    /// <param name="provider">The provider to add.</param>
    public static void Register(IEntryColumnProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (Catalogue.Any(other => string.Equals(other.Id, provider.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(provider);
    }

    /// <summary>
    /// The providers that speak for a directory's entries, in the order they are offered.
    /// </summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The providers to ask.</returns>
    public static IReadOnlyList<IEntryColumnProvider> For(string directory)
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

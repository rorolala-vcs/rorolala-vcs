using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// One plugin's answer to what colour an entry is marked with.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c> may hand one
/// of these to <see cref="EntryColourProviders.Register"/>. A colour is not exclusive: a provider says which
/// entries it marks, and an entry that no provider marks inherits the nearest marked directory above it.
/// <para>
/// What a provider answers is a colour rather than a hue, because what a hue is worth is the provider's own
/// decision: a rule that hashes a name picks the hue, while a rule about a state may say the colour itself.
/// What the file manager does with the answer is fixed: the icon is drawn in it, the row carries a wash of it,
/// and the entries under a marked directory inherit it.
/// </para>
/// </remarks>
public interface IEntryColourProvider
{
    /// <summary>The stable value this provider is known by, unique among the providers offered.</summary>
    string Id { get; }

    /// <summary>
    /// The colour this provider marks an entry with, or nothing when it marks none.
    /// </summary>
    /// <remarks>
    /// The entry is the entry itself and not a directory above it: a provider that marks a directory is asked
    /// about the directory once, and the entries under it are answered by inheritance rather than by a second
    /// question. A provider that cannot answer says nothing rather than throwing at a listing.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>The colour to mark it with, or nothing.</returns>
    Color? Colour(Entry entry);

    /// <summary>
    /// Says that what this provider answered may have changed.
    /// </summary>
    /// <remarks>
    /// A listing is read again when the files may have changed, and what a provider answered about them is
    /// read from somewhere such a change reaches. A provider that keeps anything lets go of it here; one that
    /// reads afresh each time has nothing to do, which is what the default does.
    /// </remarks>
    void Forget() { }
}

/// <summary>
/// The colour providers in force, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// Every provider that marks an entry is asked, in the order they were registered, and the first answer for one
/// path is the one used: when two providers claim one directory, the one registered first is what is drawn. A
/// provider that marks a directory marks everything under it as well, unless a provider marks something under
/// it, which is the rule a nested assignment follows.
/// </remarks>
public static class EntryColourProviders
{
    /// <summary>Every provider registered this run, in the order they were offered.</summary>
    private static readonly List<IEntryColourProvider> Catalogue = [];

    /// <summary>Every provider, in the order they are offered.</summary>
    public static IReadOnlyList<IEntryColourProvider> All => Catalogue;

    /// <summary>
    /// Adds a provider, after the ones already added.
    /// </summary>
    /// <remarks>
    /// A provider of an identity already offered is ignored rather than replacing it, so that two plugins
    /// claiming one name cannot have a listing marked by one of them and the preference panel show the other.
    /// </remarks>
    /// <param name="provider">The provider to add.</param>
    public static void Register(IEntryColourProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (Catalogue.Any(other => string.Equals(other.Id, provider.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(provider);
        FolderColours.Forget();
    }

    /// <summary>The colour a provider marks an entry with, or nothing when none marks it.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The first provider's answer.</returns>
    internal static Color? Of(Entry entry)
    {
        foreach (var provider in Catalogue)
        {
            if (provider.Colour(entry) is { } colour)
            {
                return colour;
            }
        }

        return null;
    }

    /// <summary>
    /// Says to every provider that what it answered may have changed.
    /// </summary>
    internal static void Forget()
    {
        foreach (var provider in Catalogue)
        {
            provider.Forget();
        }

        FolderColours.Forget();
    }

    /// <summary>Forgets every provider, which the suite that drives this plugin needs.</summary>
    /// <remarks>
    /// The catalogue is the run's, and a run starts a program once: only a suite that starts the plugin over and
    /// over in one process has a reason to clear it, and it has to, or one test's provider would mark the next
    /// test's listing.
    /// </remarks>
    internal static void Clear()
    {
        Catalogue.Clear();
        FolderColours.Forget();
    }
}

/// <summary>
/// What colour an entry is marked with, and what a view draws with it.
/// </summary>
/// <remarks>
/// A directory that a provider marks gives its colour to everything under it, and the nearest marked directory
/// wins, which is the rule a marked folder inside another marked folder follows. A tree draws that as one line
/// beside the marked step and through the height it covers, a list as a strip at a row's left edge, and a grid
/// as a line under a tile's caption. A row that inherited its colour carries a darker shade than the folder it
/// inherited from, as Godot's file system dock draws it, so that the folder itself reads as the place the colour
/// belongs to and the rows under it as being inside that place.
/// <para>
/// Nothing is filled behind a row or a tile: the mark is a line in the colour, and the name and the picture stay
/// on the ground the dock is drawn on.
/// </para>
/// </remarks>
internal static class FolderColours
{
    /// <summary>How much of a marked colour an inherited row keeps.</summary>
    private const double InheritedShare = 0.70;

    /// <summary>What was worked out for an entry, and whether it was its own colour or an inherited one.</summary>
    private readonly record struct Found(Color Colour, bool Own);

    /// <summary>What was worked out for an entry, so that a directory walked for every row is walked once.</summary>
    private static readonly Dictionary<Entry, Found?> Effective = [];

    /// <summary>The colour a marked directory or file is drawn as, at the strength a line wears.</summary>
    /// <remarks>
    /// The answer is the entry's own colour rather than one it inherits, and an inherited row keeps the
    /// darker shade the tree's rule gives it: the line is the mark, so it is the one thing about a marked
    /// row that is fully the colour's own.
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The colour to draw the row's mark in, or nothing when it is not marked.</returns>
    public static Color? Band(Entry entry)
    {
        if (EffectiveOf(entry) is not { } found)
        {
            return null;
        }

        return found.Own ? found.Colour : Darken(found.Colour);
    }

    /// <summary>The colour an entry itself is marked with, and nothing for one that only inherits it.</summary>
    /// <remarks>
    /// A tree draws one line for the step that is marked and lets it run the whole subtree, so a descendant
    /// that only inherits the colour would be a second line beside the same subtree rather than a deeper one.
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The colour of its own mark, or nothing.</returns>
    public static Color? Mark(Entry entry) => EntryColourProviders.Of(entry);

    /// <summary>Lets go of what was worked out, so that the next drawing asks the providers again.</summary>
    internal static void Forget() => Effective.Clear();

    /// <summary>What colour reaches an entry, and whether it was its own.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The answer, or nothing.</returns>
    private static Found? EffectiveOf(Entry entry)
    {
        if (Effective.TryGetValue(entry, out var known))
        {
            return known;
        }

        var found = Find(entry);
        Effective[entry] = found;

        return found;
    }

    /// <summary>What colour reaches an entry, read from the entry and then from the directories above it.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The answer, or nothing.</returns>
    private static Found? Find(Entry entry)
    {
        if (EntryColourProviders.Of(entry) is { } own)
        {
            return new Found(own, true);
        }

        var at = Path.GetDirectoryName(Path.TrimEndingDirectorySeparator(entry.Path));

        while (!string.IsNullOrEmpty(at))
        {
            if (EntryColourProviders.Of(new Entry(at, EntryKind.Directory)) is { } inherited)
            {
                return new Found(inherited, false);
            }

            var parent = Path.GetDirectoryName(at);
            if (string.IsNullOrEmpty(parent) || string.Equals(parent, at, StringComparison.Ordinal))
            {
                break;
            }

            at = parent;
        }

        return null;
    }

    /// <summary>The colour a row inherits is darker than the one the directory it is in was given.</summary>
    /// <param name="colour">The colour a directory was marked with.</param>
    /// <returns>The darkened colour.</returns>
    private static Color Darken(Color colour) =>
        Color.FromRgb(
            (byte)(colour.R * InheritedShare),
            (byte)(colour.G * InheritedShare),
            (byte)(colour.B * InheritedShare)
        );
}

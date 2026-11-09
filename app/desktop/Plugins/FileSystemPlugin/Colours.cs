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
/// wins, which is the rule a marked folder inside another marked folder follows. A tree draws that as one block
/// behind the marked row and every row under it, and a listing draws it as a wash on the row of each entry it
/// reaches. A row that inherited its colour carries a darker wash than the folder it inherited from, as Godot's
/// file system dock draws it, so that the folder itself reads as the place the colour belongs to and the rows
/// under it as being inside that place.
/// <para>
/// The wash is a share of the colour over the row's own ground rather than an opaque fill, so that the choice a
/// row already wears stays readable through it, and a look changed while the program runs changes the ground
/// under a wash that stays the same share.
/// </para>
/// </remarks>
internal static class FolderColours
{
    /// <summary>How much of a marked colour an inherited row's wash keeps.</summary>
    private const double InheritedShare = 0.70;

    /// <summary>How much of a marked colour the block behind a tree's step adds.</summary>
    private const double BlockShare = 0.24;

    /// <summary>How much of a marked colour a listing row's wash adds.</summary>
    private const double WashShare = 0.12;

    /// <summary>How much of a marked colour the ground of a listing inside it adds.</summary>
    private const double SurfaceShare = 0.06;

    /// <summary>What was worked out for an entry, and whether it was its own colour or an inherited one.</summary>
    private readonly record struct Found(Color Colour, bool Own);

    /// <summary>What was worked out for an entry, so that a directory walked for every row is walked once.</summary>
    private static readonly Dictionary<Entry, Found?> Effective = [];

    /// <summary>The block a marked directory or file is drawn as, or nothing when it is not marked itself.</summary>
    /// <remarks>
    /// The answer is the entry's own colour rather than one it inherits: a block behind a marked directory already
    /// covers what is under it, and a second block for an inherited row would be a second fill over the first.
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The colour to lay behind the block, or nothing.</returns>
    public static Color? Block(Entry entry) =>
        EntryColourProviders.Of(entry) is { } own ? WithAlpha(own, BlockShare) : null;

    /// <summary>The wash a row of an entry carries, or nothing when no colour reaches it.</summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>The wash, or nothing.</returns>
    public static Color? Wash(Entry entry)
    {
        if (EffectiveOf(entry) is not { } found)
        {
            return null;
        }

        var colour = found.Own ? found.Colour : Darken(found.Colour);

        return WithAlpha(colour, WashShare);
    }

    /// <summary>The ground a listing of a directory that is inside a marked one adds, or nothing.</summary>
    /// <remarks>
    /// It is a share of the colour and nothing else, laid over the bright surface the dock is drawn on, rather
    /// than an opaque mix of the two: a look changed while the program runs changes that surface, and a colour
    /// that had the old one mixed in would draw the old ground under the new look.
    /// </remarks>
    /// <param name="directory">The directory the listing is of.</param>
    /// <returns>The colour to lay over the ground, or nothing when no colour reaches the directory.</returns>
    public static Color? Surface(string directory)
    {
        if (string.IsNullOrEmpty(directory))
        {
            return null;
        }

        return EffectiveOf(new Entry(directory, EntryKind.Directory)) is { } found
            ? WithAlpha(found.Colour, SurfaceShare)
            : null;
    }

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

    /// <summary>A colour at a share of its strength, for a row wash.</summary>
    /// <param name="colour">The colour.</param>
    /// <param name="share">How much of the row is the colour, from nothing to one.</param>
    /// <returns>The colour with that share as its alpha.</returns>
    private static Color WithAlpha(Color colour, double share) =>
        Color.FromArgb((byte)Math.Round(share * 255), colour.R, colour.G, colour.B);
}

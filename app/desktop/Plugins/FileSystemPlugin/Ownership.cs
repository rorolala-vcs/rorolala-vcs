using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>What ink a contributed mark is drawn in.</summary>
/// <remarks>
/// The colour is the contribution's, the brush is not: a plugin names a role and this plugin picks
/// the theme's resource for it, so a mark follows the user's colours rather than being the one
/// colour the plugin was written against.
/// </remarks>
public enum MarkInk
{
    /// <summary>The list's own metadata ink: nothing is being drawn attention to.</summary>
    Quiet,

    /// <summary>The ordinary foreground, which reads as any other words on the card.</summary>
    Plain,

    /// <summary>The red the look keeps for what is wrong, which marks what must not be recorded.</summary>
    Error,

    /// <summary>The user's own colour, which marks what the work is theirs through.</summary>
    Primary,

    /// <summary>The second colour the user chose, which marks what is neither theirs nor wrong.</summary>
    Accent,

    /// <summary>
    /// The green the look uses for what is coming in, which marks what has been taken for the reader.
    /// </summary>
    /// <remarks>
    /// Beside the accent rather than instead of it: the accent is the colour the user chose, and this is the
    /// look's own answer for "this is yours to change", which reads the same whatever that choice was.
    /// </remarks>
    Add,

    /// <summary>
    /// The one amber, which marks what has to be read twice.
    /// </summary>
    /// <remarks>
    /// Fixed rather than chosen, and the same on either ground: what a warning is coloured is not a
    /// preference, so a state that wants noticing says so in this rather than in the user's own colour.
    /// </remarks>
    Warn,
}

/// <summary>
/// One plugin's answer to who holds an entry.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c> may hand
/// one of these to <see cref="EntryLockProviders.Register"/>. What is contributed is the Ownership column
/// of a directory dock's list, and — through <see cref="IEntryMarkProvider"/>, which this is — the badge a
/// tile wears in its grid.
/// <para>
/// A provider answers about a directory as much as about an entry, because whether an entry has a lock at
/// all is not a fact about the entry: it is a fact about the work the directory sits in, and a listing
/// outside any such work has no column and no badges.
/// </para>
/// </remarks>
public interface IEntryLockProvider : IEntryMarkProvider
{
    /// <summary>An i18n key naming the column this provider heads.</summary>
    string ColumnKey { get; }
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
    /// Says to every provider that what it answered may have changed.
    /// </summary>
    /// <remarks>
    /// Said to all of them rather than to the one a directory is answered by: what is read again is not
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
    /// The catalogue is the run's, and a run starts a program once: only a suite that starts the
    /// plugin over and over in one process has a reason to clear it, and it has to, or one test's
    /// provider would answer for the next test's listing.
    /// </remarks>
    internal static void Clear() => Catalogue.Clear();
}

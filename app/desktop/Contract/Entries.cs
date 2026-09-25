namespace RorolalaDesktop.Contract;

/// <summary>
/// One filesystem item the browser shows: a file or a directory.
/// </summary>
/// <param name="Path">Where the item is, as an absolute path.</param>
/// <param name="Kind">Whether it is a file or a directory.</param>
public sealed record Entry(string Path, EntryKind Kind);

/// <summary>Whether an entry is a file or a directory.</summary>
public enum EntryKind
{
    /// <summary>A file.</summary>
    File,

    /// <summary>A directory.</summary>
    Directory,
}

/// <summary>A badge to add to an entry's icon: where to put it, and what to draw there.</summary>
/// <param name="Position">The offset the badge is placed at, in provider order.</param>
/// <param name="IconKey">An i18n-independent key naming the icon within the icon library.</param>
public sealed record Badge(int Position, string IconKey);

/// <summary>
/// What a plugin contributes to how an entry's icon is drawn.
/// </summary>
/// <remarks>
/// The two steps are separate so that the cheap one can be asked of every entry and cached: a
/// provider says whether it has anything to say about an entry, and only then what badge to add.
/// Both results are cached for the session.
/// </remarks>
public interface IIconBadgeProvider
{
    /// <summary>
    /// Whether this provider has anything to say about an entry.
    /// </summary>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether <see cref="GetBadge"/> should be asked.</returns>
    bool Cares(Entry entry);

    /// <summary>
    /// The badge to add to an entry's icon, or nothing.
    /// </summary>
    /// <param name="entry">The entry to draw a badge for.</param>
    /// <returns>The badge, or nothing when there is none.</returns>
    Badge? GetBadge(Entry entry);
}

/// <summary>Where a plugin contributes icon badges.</summary>
public interface IIconBadgeRegistry
{
    /// <summary>
    /// Adds a provider, after the providers already added.
    /// </summary>
    /// <remarks>
    /// Badges from several providers are placed by offset, in the order the providers were added,
    /// which is plugin load order.
    /// </remarks>
    /// <param name="provider">The provider to add.</param>
    void Add(IIconBadgeProvider provider);
}

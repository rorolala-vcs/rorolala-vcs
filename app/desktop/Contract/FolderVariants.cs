using System.Collections.Generic;

namespace RorolalaDesktop.Contract;

/// <summary>
/// One kind of folder a menu may make, beyond the plain folder the file system makes itself.
/// </summary>
/// <remarks>
/// The folder is made by the file system, because making one and naming it is what a listing knows how to
/// do; what the kind adds is what the folder becomes once it exists. That is why <paramref name="Made"/>
/// runs after the directory is there rather than making it: a tree is a directory holding a
/// <c>.rola-tree</c>, not a second kind of entry.
/// </remarks>
/// <param name="LabelKey">An i18n key for the menu item's label.</param>
/// <param name="IconKey">The key of the picture the item wears.</param>
/// <param name="NameKey">An i18n key for the name a new folder of this kind is given.</param>
/// <param name="Made">What to do with the directory once it has been made.</param>
public sealed record FolderVariant(
    string LabelKey,
    string IconKey,
    string NameKey,
    Action<string> Made
);

/// <summary>Where a plugin offers another kind of folder the file system's menus may make.</summary>
/// <remarks>
/// The file system draws the menu and makes the directory; a plugin that knows what a kind becomes says so
/// here. Without a registration the menu is the one it was before, which is what keeps the file system
/// usable with no other plugin loaded.
/// </remarks>
public interface IFolderVariants
{
    /// <summary>Adds a kind of folder to the menus.</summary>
    /// <param name="variant">The kind.</param>
    void Add(FolderVariant variant);

    /// <summary>Every kind registered, in the order they are offered.</summary>
    /// <returns>The kinds.</returns>
    IReadOnlyList<FolderVariant> All();
}

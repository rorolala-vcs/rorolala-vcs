namespace RorolalaDesktop.Menus;

/// <summary>
/// One entry of a menu, at the end of the path of keys that names it.
/// </summary>
/// <remarks>
/// The path is the entry's identity: the first key is the category it sits under, the last is the entry itself,
/// and the keys between are further categories when there are any. Two plugins that name the same path mean the
/// same entry, whatever each of them calls it, and two plugins that name different paths stay apart even when
/// their words are alike.
/// <para>
/// A leaf is the whole of what a menu needs that does not depend on the file browser: an action, an order, and
/// the two questions a menu asks before it draws one. It is what the Version Graph and the Tree Panel build,
/// since neither opens on a file, and the File System hands its own entries to the same renderer by turning a
/// context-menu item into one.
/// </para>
/// </remarks>
/// <param name="Keys">The path, from the first category to the entry itself, with at least one key.</param>
/// <param name="Order">Where the entry sits among those of its category.</param>
/// <param name="Run">What choosing it does.</param>
/// <param name="IconKey">An optional key naming the entry's icon.</param>
/// <param name="Force">
/// Whether the entry is one of the forced actions, which are shown while the user holds Shift and are drawn
/// as what they are: what an action does past the checks it would otherwise make is not what a plain click
/// should come to.
/// </param>
/// <param name="Applies">What the entry is offered for, or nothing for everything in its context.</param>
/// <param name="Enabled">
/// Whether the entry may be chosen, or nothing for one that always may: an entry that is shown but does
/// nothing says why it is there, while one that is left out says nothing at all.
/// </param>
public sealed record MenuLeaf(
    IReadOnlyList<string> Keys,
    int Order,
    Action Run,
    string? IconKey = null,
    bool Force = false,
    Func<bool>? Applies = null,
    Func<bool>? Enabled = null
)
{
    /// <summary>The key naming the entry itself, which is the last of <see cref="Keys"/>.</summary>
    public string Key => Keys[^1];

    /// <summary>The key naming the category the entry sits under, which is the first of <see cref="Keys"/>.</summary>
    public string Root => Keys[0];
}

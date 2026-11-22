using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The park a rola-tree wears: one tree, drawn in the look's own ground.
/// </summary>
/// <remarks>
/// A rola-tree is a directory whose contents are one version rather than a set of loose entries, and the
/// mark says so wherever a mark is read: in the badge column at a tree row's right, and on the folder's
/// corner in a directory listing. The colour a tree's id names belongs to the rows and the line the tree
/// marks, so the park is drawn in the look's Mid instead: it says what the folder is, and the colour says
/// where the tree reaches.
/// </remarks>
internal sealed class RolaTreeIcon : IEntryMarkProvider
{
    /// <summary>What reads a tree's id and answers with its colour.</summary>
    private readonly RolaTreeColours _colours;

    /// <summary>Makes the provider over the colours a tree is read with.</summary>
    /// <param name="colours">What reads a tree's id and answers with its colour.</param>
    public RolaTreeIcon(RolaTreeColours colours) => _colours = colours;

    /// <inheritdoc />
    public string Id => "rorolala.tree.icon";

    /// <summary>
    /// Every directory may be a tree, so every listing asks; a directory that is not one answers with no mark.
    /// </summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>Whether this provider may have something to say.</returns>
    public bool Applies(string directory) => true;

    /// <inheritdoc />
    public EntryMark Mark(Entry entry) =>
        _colours.Colour(entry) is not null
            ? new EntryMark(
                IconKey: Icons.Park,
                Colour: RorolalaColours.Mid,
                Tagged: true,
                HoverKey: "rorolala_vcs.tree_badge"
            )
            : new EntryMark();

    /// <inheritdoc />
    public void Forget() => _colours.Forget();
}

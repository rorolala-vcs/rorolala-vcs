using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The park a rola-tree wears: one tree, drawn in the colour the tree's own id names.
/// </summary>
/// <remarks>
/// A rola-tree is a directory whose contents are one version rather than a set of loose entries, and the
/// mark says so wherever a mark is read: in the badge column at a tree row's right, and on the folder's
/// corner in a directory listing. It is the same colour as the block the tree's rows are drawn on, so the
/// picture and the place it stands for are one answer rather than two.
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
        _colours.Colour(entry) is { } colour
            ? new EntryMark(
                IconKey: Icons.Park,
                Colour: colour,
                Tagged: true,
                HoverKey: "rorolala_vcs.tree_badge"
            )
            : new EntryMark();

    /// <inheritdoc />
    public void Forget() => _colours.Forget();
}

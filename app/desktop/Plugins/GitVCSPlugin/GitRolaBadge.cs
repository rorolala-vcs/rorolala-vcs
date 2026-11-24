using Avalonia.Media;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace GitVCSPlugin;

/// <summary>
/// The badge a material wears when what it holds is not what the project's lock records.
/// </summary>
/// <remarks>
/// The picture is the one the Rorolala plugin draws for a modified entry, by its key: the icon
/// library is the program's, and a key another plugin contributed is the program's answer for what
/// a change looks like. What differs is the colour: this is not a change waiting to be recorded but
/// a material disagreeing with the lock, which is the program's error red.
/// <para>
/// The badge and the colour are one answer read twice: a material drawn in the error colour and a
/// material wearing this badge are the same material, which is why both ask the same check.
/// </para>
/// </remarks>
internal sealed class GitRolaBadge : IEntryMarkProvider
{
    /// <summary>The picture a change is drawn with, drawn into this plugin's own set.</summary>
    private const string Modified = Icons.NotePencil;

    /// <summary>What the check answered about the work.</summary>
    private readonly GitRolaCheck _check;

    /// <summary>The palette in force, which is where the red is read from.</summary>
    private readonly IPalette _palette;

    /// <summary>The provider, offered to the File System plugin's registry.</summary>
    /// <param name="check">What the project's record says about the work.</param>
    /// <param name="palette">The palette in force.</param>
    public GitRolaBadge(GitRolaCheck check, IPalette palette)
    {
        ArgumentNullException.ThrowIfNull(check);
        ArgumentNullException.ThrowIfNull(palette);

        _check = check;
        _palette = palette;
    }

    /// <inheritdoc />
    public string Id => "rorolala.git.rola.wrong";

    /// <inheritdoc />
    public bool Applies(string directory) => GitRolaToml.ProjectOf(directory) is not null;

    /// <inheritdoc />
    public EntryMark Mark(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return _check.Wrong(entry.Path)
            ? new EntryMark(
                IconKey: Modified,
                Tagged: true,
                HoverKey: "rorolala_git.mark.wrong_hover",
                Colour: _palette.Semantic.Error
            )
            : new EntryMark();
    }

    /// <inheritdoc />
    public void Forget() => _check.Forget();
}

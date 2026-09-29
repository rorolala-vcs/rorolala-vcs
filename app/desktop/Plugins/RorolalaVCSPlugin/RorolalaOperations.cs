using FileSystemPlugin;

namespace RorolalaVCSPlugin;

/// <summary>Rorolala's own file operations: the commands of <c>rola fs-ops</c>, as one preset.</summary>
/// <remarks>
/// The two paths are named in the templates rather than appended, as the extension point requires. The
/// two removals share one command, because <c>rola fs-ops rm</c> reads a file from a directory off the
/// path itself rather than being told which it is.
/// </remarks>
internal sealed class RorolalaOperations : IFileOperationPreset
{
    /// <inheritdoc />
    public string Id => "rorolala";

    /// <inheritdoc />
    public string LabelKey => "rorolala_vcs.setting.preset";

    /// <inheritdoc />
    public FileOperationCommands Commands { get; } =
        new(
            "rola fs-ops cp {{from}} {{to}}",
            "rola fs-ops mv {{from}} {{to}}",
            "rola fs-ops rm {{from}}",
            "rola fs-ops rm {{from}}"
        );
}

using FileSystemPlugin;

namespace RorolalaVCSPlugin;

/// <summary>Rorolala's own file operations: the commands of <c>rola fs-ops</c>, as one preset.</summary>
/// <remarks>
/// The two paths are named in the templates rather than appended, as the extension point requires. The
/// two removals share one command, because <c>rola fs-ops rm</c> reads a file from a directory off the
/// path itself rather than being told which it is.
/// <para>
/// The program is named by the variable the Desktop is started with rather than by the bare word
/// <c>rola</c>: the two programs are exported together, so the one beside this window is the one to run,
/// and a <c>PATH</c> is the machine's answer rather than this program's.
/// </para>
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
            "{{env:ROLA_EXE}} fs-ops cp {{from}} {{to}}",
            "{{env:ROLA_EXE}} fs-ops mv {{from}} {{to}}",
            "{{env:ROLA_EXE}} fs-ops rm {{from}}",
            "{{env:ROLA_EXE}} fs-ops rm {{from}}"
        );
}

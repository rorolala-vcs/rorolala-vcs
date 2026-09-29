using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// Every file operation the plugin performs: a plan over the items, a question about any conflict, and
/// the command run over each.
/// </summary>
/// <remarks>
/// The work is not this program's: an operation is carried out by a command of the user's own — the
/// system's own tools until they say otherwise, or Rorolala's once that plugin is in — and what is
/// settled here is what happens where a name is already taken, which is a question only a person can
/// answer. The command is a template naming where the two paths go as <c>{{from}}</c> and <c>{{to}}</c>.
/// <para>
/// The command runs as a program of its own, because starting it is cheap and the work is the command
/// line's; what was not worth a program of its own is the question, which is a window of this plugin's
/// over the host's main window rather than a program's of its own (see <see cref="ConflictFlow"/>).
/// </para>
/// </remarks>
internal static class FileOps
{
    /// <summary>The setting the copy command is kept under.</summary>
    public const string CopySetting = "Commands/copy";

    /// <summary>The setting the move command is kept under.</summary>
    public const string MoveSetting = "Commands/move";

    /// <summary>The setting the directory-removal command is kept under.</summary>
    public const string RemoveDirsSetting = "Commands/remove_dirs";

    /// <summary>The setting the file-removal command is kept under.</summary>
    public const string RemoveFilesSetting = "Commands/remove_files";

    /// <summary>The setting the preset is chosen under.</summary>
    public const string PresetSetting = "Commands/preset";

    /// <summary>The commands a Windows system carries the operations out with.</summary>
    /// <remarks>
    /// Every one of them is a word the command interpreter owns rather than a program of its own — there
    /// is no <c>copy.exe</c> — so each is reached through <c>cmd /c</c>. Note that <c>copy</c> handles a
    /// file and not a directory, so a default that transfers a directory is the user's to change; the
    /// presets are a starting point, not a guarantee.
    /// </remarks>
    internal static readonly FileOperationCommands Windows = new(
        "cmd /c copy /y {{from}} {{to}}",
        "cmd /c move /y {{from}} {{to}}",
        "cmd /c rmdir /s /q {{from}}",
        "cmd /c del /f /q {{from}}"
    );

    /// <summary>The commands a Unix system carries the operations out with.</summary>
    /// <remarks>
    /// Named by their bare names rather than by the paths they were found at, so that what is written
    /// down is what a reader would type: the word is the same wherever the program is installed, and it is
    /// the path — which differs from one installation to the next — that would be the odd thing to keep.
    /// </remarks>
    internal static readonly FileOperationCommands Unix = new(
        "cp -r {{from}} {{to}}",
        "mv {{from}} {{to}}",
        "rm -r {{from}}",
        "rm {{from}}"
    );

    /// <summary>The commands of the system the program is running on.</summary>
    private static FileOperationCommands Native => OperatingSystem.IsWindows() ? Windows : Unix;

    /// <summary>What the copy command is until the user says otherwise.</summary>
    public static string DefaultCopy => Native.Copy;

    /// <inheritdoc cref="DefaultCopy" />
    public static string DefaultMove => Native.Move;

    /// <inheritdoc cref="DefaultCopy" />
    public static string DefaultRemoveDirs => Native.RemoveDirs;

    /// <inheritdoc cref="DefaultCopy" />
    public static string DefaultRemoveFiles => Native.RemoveFiles;

    /// <summary>
    /// The plugin's own settings, where the commands that carry the operations out are said.
    /// </summary>
    /// <remarks>
    /// Set once, during initialization, and read on every operation rather than kept: a command the user
    /// changes takes effect on the next operation instead of on the next start, which is what a command is
    /// — nothing about it survives the operation it carried out.
    /// </remarks>
    private static IPluginConfig? _config;

    /// <summary>Hands the plugin's settings to the file operations.</summary>
    /// <param name="config">The plugin's own section of the preferences.</param>
    public static void Configure(IPluginConfig config) => _config = config;

    /// <summary>A command the user may have changed, or what it is until they do.</summary>
    /// <param name="id">The setting's identity, written as <c>Group/Key</c>.</param>
    /// <param name="fallback">What it is until it is changed.</param>
    private static string Command(string id, string fallback) =>
        _config?.ReadKeyAs<string>(id) is { Length: > 0 } command ? command : fallback;

    /// <summary>
    /// Strips the trailing separators a path may carry.
    /// </summary>
    /// <remarks>
    /// This exists because of one bug that is worth not having again: a path that came from a drag is read
    /// from a URI, and a directory's URI ends in a separator — so the name taken from it is the empty string,
    /// a "directory" of no name is the directory it sits in, that directory always exists, and the entry was
    /// therefore renamed instead of arriving under its own name. Bare the path before asking anything of it.
    /// </remarks>
    /// <param name="path">The path to bare.</param>
    /// <returns>The path without trailing separators, or the path itself when it is one all the way.</returns>
    public static string Bare(string path)
    {
        var trimmed = path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        return trimmed.Length == 0 ? path : trimmed;
    }

    /// <summary>Copies sources into a directory.</summary>
    /// <param name="sources">What to copy.</param>
    /// <param name="into">The directory to put them in.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was copied.</returns>
    public static Task<bool> Copy(IReadOnlyList<string> sources, string into, Action<string> failed) =>
        Transfer(Operation.Copy, Command(CopySetting, DefaultCopy), sources, into, failed);

    /// <summary>Moves sources into a directory.</summary>
    /// <param name="sources">What to move.</param>
    /// <param name="into">The directory to put them in.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was moved.</returns>
    public static Task<bool> Move(IReadOnlyList<string> sources, string into, Action<string> failed) =>
        Transfer(Operation.Move, Command(MoveSetting, DefaultMove), sources, into, failed);

    /// <summary>
    /// Removes entries.
    /// </summary>
    /// <remarks>
    /// Directories and files are one call each, because the operation names which of the two it is removing
    /// — the command that removes a file does not remove a directory, and the two are told apart here so that
    /// they need not be told apart again once the run is made.
    /// </remarks>
    /// <param name="entries">What to remove.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was removed.</returns>
    public static async Task<bool> Remove(IReadOnlyList<Entry> entries, Action<string> failed)
    {
        var directories = entries.Where(entry => entry.Kind == EntryKind.Directory).Select(entry => entry.Path).ToArray();
        var files = entries.Where(entry => entry.Kind == EntryKind.File).Select(entry => entry.Path).ToArray();
        var removed = false;

        if (directories.Length > 0)
        {
            removed |= await Without(Operation.RemoveDirs, Command(RemoveDirsSetting, DefaultRemoveDirs), directories, failed);
        }

        if (files.Length > 0)
        {
            removed |= await Without(Operation.RemoveFiles, Command(RemoveFilesSetting, DefaultRemoveFiles), files, failed);
        }

        return removed;
    }

    /// <summary>One transfer of a batch into a directory.</summary>
    /// <param name="operation">What is being done to every item.</param>
    /// <param name="command">The command template that carries it out.</param>
    /// <param name="sources">What is being transferred.</param>
    /// <param name="into">The directory they go into.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static async Task<bool> Transfer(
        Operation operation,
        string command,
        IReadOnlyList<string> sources,
        string into,
        Action<string> failed
    )
    {
        if (sources.Count == 0)
        {
            return false;
        }

        // One batch rather than one call per item, so that a question about a taken name is asked once for
        // the whole of it — offering "the same for the rest" — instead of once per item.
        return await Ask(operation, command, [.. sources.Select(source => new Pair(source, into))], failed);
    }

    /// <summary>One removal of a batch.</summary>
    /// <param name="operation">What is being done to every item.</param>
    /// <param name="command">The command template that carries it out.</param>
    /// <param name="paths">What is being removed.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static Task<bool> Without(Operation operation, string command, IReadOnlyList<string> paths, Action<string> failed) =>
        Ask(operation, command, [.. paths.Select(path => new Pair(path, string.Empty))], failed);

    /// <summary>
    /// Plans one call, asks about any conflict, and runs the command over every item.
    /// </summary>
    /// <remarks>
    /// A command the operation cannot be run with — one that names no program, or no path, or a target a
    /// removal has none of — is refused before anything is planned, so that the reason names the command
    /// rather than an item. The plan is made first, so that every conflict is answered before the first
    /// command runs — which is what makes calling the run off leave nothing done at all.
    /// </remarks>
    /// <param name="operation">What is being done to every item.</param>
    /// <param name="command">The command template that carries it out.</param>
    /// <param name="pairs">What is being done, one source and its destination.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was done.</returns>
    private static async Task<bool> Ask(Operation operation, string command, IReadOnlyList<Pair> pairs, Action<string> failed)
    {
        // The command is a template: it names where the two paths go as `{{from}}` and `{{to}}`, and the
        // runner expands it per item. What can be told about it before anything is planned — that it names
        // a program and the paths its operation needs — is told here, so the reason names the command
        // rather than an item.
        if (Executor.Validate(operation, command) is { } problem)
        {
            failed(problem);

            return false;
        }

        try
        {
            var plan = Plan.Build(operation, pairs);

            if (plan.HasConflicts)
            {
                await ConflictFlow.Decide(plan);
            }

            // Off the window's thread: every item starts a program and waits for it, and the window has to
            // go on drawing while that happens.
            var results = await Task.Run(() => Executor.Run(plan, command));

            var done = false;

            foreach (var result in results)
            {
                if (result.Outcome == "failed")
                {
                    failed(result.Note ?? $"{result.From} could not be {result.How}");

                    continue;
                }

                done |= result.Outcome == "done";
            }

            return done;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);

            return false;
        }
    }
}

using System.Text;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What the Tree panel asks the command line for: staging a path, taking it back out, and putting
/// the work back to the version the tree records.
/// </summary>
/// <remarks>
/// The staging area and the description are the tree's own, and the command line is what owns both.
/// These are the words it takes rather than a second reading of the tree's format in this program:
/// `tree add` stages what a path holds, `tree rm` stages its removal, `tree reset` puts it back to
/// the recorded version, and `align --restore-modify` writes the recorded version of one file,
/// which is also what recreates a file the work has lost.
/// <para>
/// The commands are run where the tree is rooted rather than where the panel is, because every one
/// of them locates the tree from the path it is given. What they answer is whether they ended and
/// what they said, not their JSON: this window only needs to know whether to read the tree again.
/// </para>
/// </remarks>
internal static class TreePanelCommands
{
    /// <summary>The description of the next version, as a file of the tree's mark.</summary>
    /// <remarks>
    /// It is the same file `rola tree write-desc` writes and `rola tree cat-desc` reads: the
    /// description is staged work, not a note beside it, and the graph reads its presence to draw a
    /// draft. Writing it here is writing the file the command line would write, which is why the
    /// file's name and encoding are stated in one place.
    /// </remarks>
    internal static class Description
    {
        /// <summary>The encoding the description is written in, which is the command line's.</summary>
        private static readonly UTF8Encoding Utf8 = new(encoderShouldEmitUTF8Identifier: false);

        /// <summary>Where a tree keeps the description of its next version.</summary>
        /// <param name="tree">The tree's root.</param>
        /// <returns>The file's path.</returns>
        public static string PathOf(string tree) =>
            Path.Combine(tree, ".rola-tree", "description");

        /// <summary>The description staged for a tree, or nothing when none is.</summary>
        /// <param name="tree">The tree's root.</param>
        /// <returns>What the file says, or nothing.</returns>
        public static string? Read(string tree)
        {
            ArgumentNullException.ThrowIfNull(tree);

            try
            {
                var path = PathOf(tree);

                return File.Exists(path) ? File.ReadAllText(path) : null;
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                return null;
            }
        }

        /// <summary>
        /// Writes the description of a tree's next version, or removes it when the text says nothing.
        /// </summary>
        /// <remarks>
        /// An empty or whitespace-only text is a cleared description rather than a file holding
        /// spaces: that is what `rola tree write-desc` makes of one, and a file the command line
        /// would have removed is a draft the graph would draw for nothing.
        /// </remarks>
        /// <param name="tree">The tree's root.</param>
        /// <param name="text">What the description says.</param>
        /// <param name="said">Why it could not be written, when it could not.</param>
        /// <returns>Whether the file is as the text says.</returns>
        public static bool Write(string tree, string text, out string said)
        {
            ArgumentNullException.ThrowIfNull(tree);

            try
            {
                var path = PathOf(tree);

                if (string.IsNullOrWhiteSpace(text))
                {
                    if (File.Exists(path))
                    {
                        File.Delete(path);
                    }
                }
                else
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(path)!);
                    File.WriteAllText(path, text, Utf8);
                }

                said = string.Empty;

                return true;
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                said = error.Message;

                return false;
            }
        }
    }

    /// <summary>The arguments `tree add` stages one path with.</summary>
    /// <param name="path">The path inside the tree.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> StageArguments(string path) =>
        Arguments("tree", "add", path);

    /// <summary>The arguments `tree add` stages several paths with.</summary>
    /// <param name="paths">The paths inside the tree.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> StagePathsArguments(IReadOnlyList<string> paths)
    {
        ArgumentNullException.ThrowIfNull(paths);

        var arguments = Arguments("tree", "add");
        arguments.AddRange(paths);

        return arguments;
    }

    /// <summary>The arguments `tree add` stages every change with.</summary>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> StageAllArguments() =>
        Arguments("tree", "add", ".");

    /// <summary>The arguments `tree rm` stages one path's removal with.</summary>
    /// <param name="path">The path inside the tree.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> StageRemovalArguments(string path) =>
        Arguments("tree", "rm", path);

    /// <summary>The arguments `tree rm` stages several paths' removals with.</summary>
    /// <param name="paths">The paths inside the tree.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> StageRemovalsArguments(IReadOnlyList<string> paths)
    {
        ArgumentNullException.ThrowIfNull(paths);

        var arguments = Arguments("tree", "rm");
        arguments.AddRange(paths);

        return arguments;
    }

    /// <summary>The arguments `tree reset` puts paths back to the recorded version with.</summary>
    /// <param name="paths">The paths to unstage.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> UnstageArguments(IReadOnlyList<string> paths)
    {
        ArgumentNullException.ThrowIfNull(paths);

        var arguments = new List<string> { "--theme-choice=" + Sync.Theme, "--json", "tree", "reset" };
        arguments.AddRange(paths);

        return arguments;
    }

    /// <summary>The arguments `tree reset` clears the whole staging area with.</summary>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> ResetArguments() =>
        Arguments("tree", "reset");

    /// <summary>The arguments `align --restore-modify` writes a recorded file back with.</summary>
    /// <remarks>
    /// The same words restore a modified file and recreate a deleted one: the command writes the
    /// entry the recorded structure holds at the path, and a path the work no longer has is one it
    /// creates again. `--restore-delete` is not used here because a path inside a tree is not a
    /// Layout entry: that mode restores the whole tree.
    /// </remarks>
    /// <param name="path">The path inside the tree.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> RestoreArguments(string path) =>
        Arguments("align", "--restore-modify", path);

    /// <summary>How much of a command line paths may take, in characters.</summary>
    /// <remarks>
    /// Windows ends a process whose command line passes about thirty-two thousand characters, and a
    /// tree can hold more paths than that. The budget is half of it, which leaves the program's own
    /// arguments and quoting room and is still a batch worth running.
    /// </remarks>
    private const int ArgumentBudget = 16_000;

    /// <summary>What one command's own words and quoting are given before paths take the rest.</summary>
    private const int ArgumentReserve = 256;

    /// <summary>Stages `path`.</summary>
    /// <param name="tree">The tree's root, where the run is made.</param>
    /// <param name="path">The path inside the tree.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Stage(string tree, string path) =>
        Run(tree, StageArguments(path));

    /// <summary>Stages every change the tree holds.</summary>
    /// <param name="tree">The tree's root, where the run is made.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) StageAll(string tree) =>
        Run(tree, StageAllArguments());

    /// <summary>
    /// Stages the paths named, and the removals named, as one section's bulk action.
    /// </summary>
    /// <remarks>
    /// A conflicted path is not swept up: the section's own checkbox stages what the section shows,
    /// and a conflict is its own section in the panel. The paths are stageable in two runs because
    /// the command line stages a path and stages a removal differently, and a run that has done the
    /// first says so when the second cannot be done.
    /// </remarks>
    /// <param name="tree">The tree's root, where the runs are made.</param>
    /// <param name="adds">The paths whose work is to be staged.</param>
    /// <param name="removals">The paths whose removal is to be staged.</param>
    /// <returns>Whether both runs ended well, and what to say when one did not.</returns>
    public static (bool Done, string Said) StageAll(
        string tree,
        IReadOnlyList<string> adds,
        IReadOnlyList<string> removals
    )
    {
        ArgumentNullException.ThrowIfNull(adds);
        ArgumentNullException.ThrowIfNull(removals);

        foreach (var batch in Batches(adds))
        {
            var (staged, said) = Run(tree, StagePathsArguments(batch));

            if (!staged)
            {
                return (staged, said);
            }
        }

        foreach (var batch in Batches(removals))
        {
            var (removed, said) = Run(tree, StageRemovalsArguments(batch));

            if (!removed)
            {
                return (removed, said);
            }
        }

        return (true, string.Empty);
    }

    /// <summary>Stages the removal of `path`.</summary>
    /// <param name="tree">The tree's root, where the run is made.</param>
    /// <param name="path">The path inside the tree.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) StageRemoval(string tree, string path) =>
        Run(tree, StageRemovalArguments(path));

    /// <summary>Puts the named paths back to the recorded version in the staging area.</summary>
    /// <param name="tree">The tree's root, where the run is made.</param>
    /// <param name="paths">The paths to unstage.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Unstage(
        string tree,
        IReadOnlyList<string> paths
    )
    {
        ArgumentNullException.ThrowIfNull(paths);

        if (paths.Count == 0)
        {
            return (true, string.Empty);
        }

        foreach (var batch in Batches(paths))
        {
            var (unstaged, said) = Run(tree, UnstageArguments(batch));

            if (!unstaged)
            {
                return (unstaged, said);
            }
        }

        return (true, string.Empty);
    }

    /// <summary>Clears the whole staging area, description and conflicts included.</summary>
    /// <param name="tree">The tree's root, where the run is made.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Reset(string tree) =>
        Run(tree, ResetArguments());

    /// <summary>Writes the recorded version of `path` back over the work.</summary>
    /// <param name="tree">The tree's root, where the run is made.</param>
    /// <param name="path">The path inside the tree.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Restore(string tree, string path) =>
        Run(tree, RestoreArguments(path));

    /// <summary>Stages a rename as the two facts it is: the new path, and the old one gone.</summary>
    /// <param name="tree">The tree's root, where the runs are made.</param>
    /// <param name="path">Where the file is now.</param>
    /// <param name="from">Where it was recorded.</param>
    /// <returns>Whether both runs ended well, and what to say when one did not.</returns>
    public static (bool Done, string Said) StageRename(string tree, string path, string from)
    {
        var (staged, said) = Run(tree, StageArguments(path));

        return staged ? Run(tree, StageRemovalArguments(from)) : (staged, said);
    }

    /// <summary>The paths split into command lines the operating system will start.</summary>
    /// <param name="paths">The paths to split.</param>
    /// <returns>The batches, in order.</returns>
    private static IEnumerable<IReadOnlyList<string>> Batches(IReadOnlyList<string> paths)
    {
        var batch = new List<string>();
        var length = ArgumentReserve;

        foreach (var path in paths)
        {
            if (batch.Count > 0 && length + path.Length + 1 > ArgumentBudget)
            {
                yield return batch;
                batch = [];
                length = ArgumentReserve;
            }

            batch.Add(path);
            length += path.Length + 1;
        }

        if (batch.Count > 0)
        {
            yield return batch;
        }
    }

    /// <summary>Runs one command where the tree is rooted.</summary>
    /// <param name="tree">The tree's root.</param>
    /// <param name="arguments">What the command is told, after the program's name.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Run(string tree, IReadOnlyList<string> arguments)    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(arguments);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, arguments, tree);

        return (done, said);
    }

    /// <summary>The arguments one of these commands is run with.</summary>
    /// <param name="words">The command and what it is told.</param>
    /// <returns>The arguments, after the program's name.</returns>
    private static List<string> Arguments(params string[] words)
    {
        var arguments = new List<string> { "--theme-choice=" + Sync.Theme, "--json" };
        arguments.AddRange(words);

        return arguments;
    }
}

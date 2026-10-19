using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running the command line's align: putting a path back to the version the Layout names, and ending a
/// merge.
/// </summary>
/// <remarks>
/// A merge is ended in two runs, because that is the contract `align --delete` states: it ends the merge
/// of a variant file that is no longer there, so the file — which is only how the variant was shown —
/// is removed first. Neither run touches the main file unless a restore is asked for on top of it, which
/// is the difference the two cancel items draw.
/// </remarks>
internal static class AlignCommand
{
    /// <summary>The arguments a restore is run with, after the program's name.</summary>
    /// <param name="path">The path to put back to the version the Layout names.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> RestoreArguments(string path) =>
        ["--theme-choice=" + Sync.Theme, "--json", "align", "--restore", path];

    /// <summary>The arguments the removal of a variant file is run with, after the program's name.</summary>
    /// <param name="variant">The variant file to remove.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> RemoveArguments(string variant) =>
        ["--theme-choice=" + Sync.Theme, "--json", "fs-ops", "rm", variant];

    /// <summary>The arguments that take a merge's record away, after the program's name.</summary>
    /// <param name="variant">The variant file at the place the record names.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> UnmergeArguments(string variant) =>
        ["--theme-choice=" + Sync.Theme, "--json", "align", "--delete", variant];

    /// <summary>Runs a restore, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: the content of the version a file is put back to may not be in the
    /// local store, and the command reaches for a Vault when it is not.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="path">The path to put back.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Restore(string directory, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(path);

        return Run(directory, RestoreArguments(path));
    }

    /// <summary>Ends a merge: the variant file goes, and the record with it.</summary>
    /// <remarks>
    /// The file is removed where the tree holds it, which is not always where the record names it: a
    /// move no command of ours made is found by content and reported beside the record rather than
    /// written into it. What `align --delete` is given is the record's place either way, since that is
    /// the name it ends a merge by, and a file that is gone under that name is a merge no longer
    /// waiting.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Unmerge(
        string directory,
        string variant,
        string recorded
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(variant);
        ArgumentNullException.ThrowIfNull(recorded);

        var (removed, said) = Run(directory, RemoveArguments(variant));

        return removed ? Run(directory, UnmergeArguments(recorded)) : (removed, said);
    }

    /// <summary>Runs one of these commands, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the run is made.</param>
    /// <param name="arguments">What it is run with, after the program's name.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Run(
        string directory,
        IReadOnlyList<string> arguments
    )
    {
        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, arguments, directory);

        return (done, said);
    }
}

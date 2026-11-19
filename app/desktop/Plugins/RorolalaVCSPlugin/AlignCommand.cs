using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Putting a path back to the version the Layout names, and ending a merge, through the writer the
/// command line uses.
/// </summary>
/// <remarks>
/// A merge is ended in two runs, because that is the contract `align --delete` states: it ends the merge
/// of a variant file that is no longer there, so the file, which is only how the variant was shown,
/// is removed first. Neither run touches the main file unless a restore is asked for on top of it, which
/// is the difference the two cancel items draw.
/// <para>
/// The alignment is the writer's. The variant file itself is still taken away by `fs-ops rm`, which no
/// writer carries in this slice, so ending a merge is the one run here that still starts the program.
/// What `align --delete` is given is the record's place either way, since that is the name it ends a
/// merge by, and a file that is gone under that name is a merge no longer waiting.
/// </para>
/// </remarks>
internal static class AlignCommand
{
    /// <summary>The request one alignment is made with.</summary>
    /// <param name="path">The path the run acts on, relative to the Workspace root or absolute.</param>
    /// <param name="mode">What the run does with the path.</param>
    /// <returns>The request.</returns>
    public static AlignRequest Request(string path, AlignMode mode)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(mode);

        return new AlignRequest
        {
            Path = path,
            Mode = mode,
            Account = UserScope.CurrentAccount,
        };
    }

    /// <summary>Runs a restore, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: the content of the version a file is put back to may not be in the
    /// local store, and the run reaches for a Vault when it is not.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="path">The path to put back.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Restore(string directory, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(path);

        return Run(directory, Request(path, AlignMode.Restore));
    }

    /// <summary>Runs a restore of a tree file to the version the tree records.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="path">The file to put back.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) RestoreBaseline(string directory, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(path);

        return Run(directory, Request(path, AlignMode.RestoreModify));
    }

    /// <summary>Runs a restore of a tree file to what the tree has staged.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="path">The file to put back.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) RestoreStaged(string directory, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(path);

        return Run(directory, Request(path, AlignMode.RestoreStaged));
    }

    /// <summary>Ends a merge: the variant file goes, and the record with it.</summary>
    /// <remarks>
    /// The file is removed where the tree holds it, which is not always where the record names it: a
    /// move no command of ours made is found by content and reported beside the record rather than
    /// written into it. The record's own place is what the alignment is given, since that is the name
    /// it ends a merge by, and a file that is gone under that name is a merge no longer waiting.
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

        var (removed, said) = Remove(directory, variant);

        return removed
            ? Run(directory, Request(recorded, AlignMode.Delete))
            : (removed, said);
    }

    /// <summary>Runs one alignment, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="request">What the run is to do.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Run(string directory, AlignRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            new WorkspaceWriter(workspace).Align(request);

            return (true, string.Empty);
        }
        catch (RolaWriteError error)
        {
            return (false, Sync.Told(error));
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.NoWorkspace or RolaReadErrorKind.Unavailable)
        {
            return (false, RolaI18N.Get("rorolala_vcs.not_a_workspace"));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, error.Message);
        }
    }

    /// <summary>Takes the variant file away, which the writer does not carry in this slice.</summary>
    /// <param name="directory">Where the run is made.</param>
    /// <param name="variant">The variant file to remove.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Remove(string directory, string variant)
    {
        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var arguments = new List<string>
        {
            "--theme-choice=" + Sync.Theme,
            "--json",
            "fs-ops",
            "rm",
            variant,
        };

        var (done, said, _) = Sync.Asked(program, arguments, directory);

        return (done, said);
    }
}

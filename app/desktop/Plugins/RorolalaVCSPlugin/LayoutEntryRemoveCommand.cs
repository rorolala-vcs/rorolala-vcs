namespace RorolalaVCSPlugin;

/// <summary>
/// Running <c>rola layout path remove</c> against one Layout: what taking an entry out of a Vault's
/// own Layout is.
/// </summary>
/// <remarks>
/// Removing a path here is not the unbinding a Layout of the Workspace's own does: a Vault's entry is a
/// file the Vault holds, and letting one go is the entry going, path and all. Only the account that holds
/// it, or one that administrates the Vault, is allowed to, which is the command's to say, what this owns
/// is the question, and the answer read back from the copy the command wrote.
/// <para>
/// The Layout is named rather than left to be resolved, because what the place shows is the one the
/// address names: a run without the name would unbind a path of the Layout being worked in, which is a
/// different write to a different file. A Vault's own Layout is named <c>truth@VAULT</c>.
/// </para>
/// </remarks>
internal static class LayoutEntryRemoveCommand
{
    /// <summary>The arguments a removal is run with, after the program's name.</summary>
    /// <param name="layout">The Layout to remove from, as the command line names it, or nothing for the one being worked in.</param>
    /// <param name="path">The path the entry is at.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string? layout, string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        List<string> arguments =
        [
            "--theme-choice=" + Sync.Theme,
            "--json",
            "layout",
            "path",
            "remove",
        ];

        // The Layout being worked in is what a run removes from when it names none: a checkin that takes a local
        // record away is about the work as it stands, and naming a Layout would write one that is not the one on screen.
        if (layout is { Length: > 0 })
        {
            arguments.Add("--layout");
            arguments.Add(layout);
        }

        arguments.Add(path);

        return arguments;
    }

    /// <summary>Runs the removal, saying what it is doing while it goes.</summary>
    /// <param name="program">The program the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="layout">The Layout to remove from, as the command line names it, or nothing for the one being worked in.</param>
    /// <param name="path">The path the entry is at.</param>
    /// <param name="records">What the run's records are read into, which the panel draws.</param>
    /// <param name="progress">What is told that a record was said, which redraws on the window's thread.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string program,
        string directory,
        string? layout,
        string path,
        ProgressRecords records,
        Action progress
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(records);
        ArgumentNullException.ThrowIfNull(progress);

        var (done, said, _) = CommandLine.Asked(
            program,
            Arguments(layout, path),
            directory,
            records,
            progress
        );

        return (done, said);
    }
}

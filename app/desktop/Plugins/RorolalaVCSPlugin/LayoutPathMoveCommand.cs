namespace RorolalaVCSPlugin;

/// <summary>
/// Running <c>rola layout path move</c> against one Layout: what a move within a place is.
/// </summary>
/// <remarks>
/// What a move here changes is the Layout's own record of where its entries are, and nothing else: the content stays
/// where it is, and no file on disk moves. Locating an entry is moving it out of <c>@/new/</c>, and deprecating one is
/// moving it under <c>@/removed/</c>, which is why a place's tree can offer both by dragging.
/// <para>
/// The Layout is named rather than left to be resolved, because what the place shows is the one the address names: a run
/// without the name would move a path of the Layout being worked in, which is a different write to a different file. A
/// Vault's own Layout is named <c>truth@VAULT</c>; one of the Workspace's by its name alone.
/// </para>
/// </remarks>
internal static class LayoutPathMoveCommand
{
    /// <summary>The name a Vault's single Layout is known by.</summary>
    /// <remarks>
    /// It is the command line's <c>VAULT_LAYOUT_NAME</c>, stated here because no header is shared: the name is the
    /// contract across the boundary, and a Vault keeps one Layout, so there is nothing for this side to find out.
    /// </remarks>
    public const string VaultLayout = "truth";

    /// <summary>The name of the Vault's Layout as the command line reads it: <c>truth@VAULT</c>.</summary>
    /// <param name="vault">The Vault, by the name the Workspace bound it under.</param>
    /// <returns>The Layout's name.</returns>
    public static string Named(string vault)
    {
        ArgumentNullException.ThrowIfNull(vault);

        return VaultLayout + "@" + vault;
    }

    /// <summary>The arguments a move is run with, after the program's name.</summary>
    /// <param name="layout">The Layout to move in, as the command line names it.</param>
    /// <param name="from">The path it is at.</param>
    /// <param name="to">The path it is to be at.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string layout, string from, string to)
    {
        ArgumentNullException.ThrowIfNull(layout);
        ArgumentNullException.ThrowIfNull(from);
        ArgumentNullException.ThrowIfNull(to);

        return
        [
            "--theme-choice=" + Sync.Theme,
            "--json",
            "layout",
            "path",
            "move",
            "--layout",
            layout,
            from,
            to,
        ];
    }

    /// <summary>Runs the move, saying what it is doing while it goes.</summary>
    /// <param name="program">The program the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="layout">The Layout to move in, as the command line names it.</param>
    /// <param name="from">The path it is at.</param>
    /// <param name="to">The path it is to be at.</param>
    /// <param name="records">What the run's records are read into, which the panel draws.</param>
    /// <param name="progress">What is told that a record was said, which redraws on the window's thread.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string program,
        string directory,
        string layout,
        string from,
        string to,
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
            Arguments(layout, from, to),
            directory,
            records,
            progress
        );

        return (done, said);
    }
}

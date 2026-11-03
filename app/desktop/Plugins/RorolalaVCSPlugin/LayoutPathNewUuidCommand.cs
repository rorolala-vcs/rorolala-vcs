using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running <c>rola layout path new-uuid</c> against the Layout being worked in: giving a path's entry a
/// new Uuid.
/// </summary>
/// <remarks>
/// The entry the path named is dropped and made again under a new <c>Uuid</c>, holding what it held:
/// what changes is the identity a record is kept under, not the version or the content. That is what
/// makes it a fork — a file may go on being recorded under an entry of its own without moving the
/// entry it was following.
/// </remarks>
internal static class LayoutPathNewUuidCommand
{
    /// <summary>The arguments a fork is run with, after the program's name.</summary>
    /// <param name="path">The path in the Layout being worked in.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return
        [
            "--theme-choice=" + Sync.Theme,
            "--json",
            "layout",
            "path",
            "new-uuid",
            path,
        ];
    }

    /// <summary>Runs the fork, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="path">The path in the Layout being worked in.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string directory, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, Arguments(path), directory);

        return (done, said);
    }
}

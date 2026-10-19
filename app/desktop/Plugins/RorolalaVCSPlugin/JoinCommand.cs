using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running the command line's checkin in its variant mode: joining one variant into one file.
/// </summary>
/// <remarks>
/// The join is the command line's — it reads the variant, writes the variant file beside its target,
/// and records the waiting merge — so this owns only the words that ask for one: the variant and the
/// file it is to be joined into. A checkin in this mode can reach for a Vault when the variant or its
/// content is not held here, which is why it is run off the window's thread.
/// </remarks>
internal static class JoinCommand
{
    /// <summary>The arguments a join is run with, after the program's name.</summary>
    /// <param name="file">The file the variant is to be joined into.</param>
    /// <param name="variant">The variant to join, as the index names it.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string file, string variant) =>
        [
            "--theme-choice=" + Sync.Theme,
            "--json",
            "checkin",
            variant,
            "--join",
            file,
        ];

    /// <summary>Runs a join, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The file the variant is to be joined into.</param>
    /// <param name="variant">The variant to join, as the index names it.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        string file,
        string variant
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);
        ArgumentNullException.ThrowIfNull(variant);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, Arguments(file, variant), directory);

        return (done, said);
    }
}

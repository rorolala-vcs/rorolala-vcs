using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running the command line's retrack: moving the version a file's Layout entry is at.
/// </summary>
/// <remarks>
/// The movement is the command line's — the Layout pointer, the content written back over the file, and the
/// Vault consulted when the content is not here — so this owns only the words that ask for one: the file, the
/// version it goes to, and whether the work on disk is to follow the pointer.
/// <para>
/// The program is the one the window was opened by, named by the environment the run handed over, because the
/// two are exported together: a <c>rola</c> found on the <c>PATH</c> could be another one entirely, of another
/// version, against another Vault.
/// </para>
/// </remarks>
internal static class RetrackCommand
{
    /// <summary>The arguments a retrack is run with, after the program's name.</summary>
    /// <param name="file">The one file to move the record of.</param>
    /// <param name="version">The version the pointer goes to, as the index names it.</param>
    /// <param name="onlyIndex">
    /// Whether the pointer moves and the file on disk is left alone, which is
    /// <c>--only-index</c>.
    /// </param>
    /// <param name="allowJump">
    /// Whether a version that is not on the way back may be gone to, which is <c>--allow-jump</c>.
    /// It is given for a Vault version the work's own chain does not hold.
    /// </param>
    /// <param name="force">
    /// Whether changes that were never recorded are to be discarded, which is <c>--force</c>. It is
    /// given only where the reader has been asked and has agreed, since the content written back is
    /// what replaces them.
    /// </param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(
        string file,
        string version,
        bool onlyIndex,
        bool allowJump,
        bool force
    )
    {
        var arguments = new List<string>
        {
            "--theme-choice=" + Sync.Theme,
            "--json",
            "retrack",
            "--until",
            version,
        };

        if (onlyIndex)
        {
            arguments.Add("--only-index");
        }

        if (allowJump)
        {
            arguments.Add("--allow-jump");
        }

        // `--force` and `--only-index` say opposite things about the file on disk, so the two are
        // never given together.
        if (force && !onlyIndex)
        {
            arguments.Add("--force");
        }

        arguments.Add(file);

        return arguments;
    }

    /// <summary>Runs a retrack, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: a retrack may reach for a Vault when the content of the version it goes to
    /// is not in the local store, which takes as long as a network takes.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The one file to move the record of.</param>
    /// <param name="version">The version the pointer goes to, as the index names it.</param>
    /// <param name="onlyIndex">Whether the pointer moves and the file on disk is left alone.</param>
    /// <param name="allowJump">Whether a version off the way back may be gone to.</param>
    /// <param name="force">Whether unrecorded changes are to be discarded.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        string file,
        string version,
        bool onlyIndex,
        bool allowJump,
        bool force
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);
        ArgumentNullException.ThrowIfNull(version);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(
            program,
            Arguments(file, version, onlyIndex, allowJump, force),
            directory
        );

        return (done, said);
    }
}

using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running the command line's track: recording the working files' content as new versions.
/// </summary>
/// <remarks>
/// The recording is the command line's — reading what changed, writing the versions, and moving the
/// Layout's entries onto them — so this owns only the words that ask for one: the files and the message
/// they carry. The editor is never opened: a window has one line of its own where the message was written,
/// and a run that opened an editor beside it would be asking the same question twice.
/// <para>
/// The program is the one the window was opened by, named by the environment the run handed over, for
/// the reason <see cref="RetrackCommand"/> gives: a <c>rola</c> found on the <c>PATH</c> could be
/// another one entirely.
/// </para>
/// </remarks>
internal static class TrackCommand
{
    /// <summary>The arguments a track is run with, after the program's name.</summary>
    /// <param name="files">The files to record, in the order the listing shows them.</param>
    /// <param name="message">What the new versions say.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(IReadOnlyList<string> files, string message)
    {
        ArgumentNullException.ThrowIfNull(files);
        ArgumentNullException.ThrowIfNull(message);

        var arguments = new List<string>
        {
            "--theme-choice=" + Sync.Theme,
            "--json",
            "track",
            "--message",
            message,
            "--no-editor",
        };

        arguments.AddRange(files);

        return arguments;
    }

    /// <summary>Runs a track, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: reading the files and writing the versions and the Layout is work
    /// over the store, and a dock that stopped drawing for it would stop for every recording.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="files">The files to record, in the order the listing shows them.</param>
    /// <param name="message">What the new versions say.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        IReadOnlyList<string> files,
        string message
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(files);
        ArgumentNullException.ThrowIfNull(message);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, Arguments(files, message), directory);

        return (done, said);
    }
}

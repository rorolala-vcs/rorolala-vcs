using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running the command line's track: recording the working file's content as a new version.
/// </summary>
/// <remarks>
/// The recording is the command line's — reading what changed, writing the version, and moving the
/// Layout's entry onto it — so this owns only the words that ask for one: the file and the message it
/// carries. The editor is never opened: a window has one line of its own where the message was written,
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
    /// <param name="file">The one file to record.</param>
    /// <param name="message">What the new version says.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string file, string message)
    {
        var arguments = new List<string>
        {
            "--theme-choice=" + Sync.Theme,
            "--json",
            "track",
            "--message",
            message,
            "--no-editor",
        };

        arguments.Add(file);

        return arguments;
    }

    /// <summary>Runs a track, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: reading the file and writing the version and the Layout is work
    /// over the store, and a dock that stopped drawing for it would stop for every recording.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The one file to record.</param>
    /// <param name="message">What the new version says.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string directory, string file, string message)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);
        ArgumentNullException.ThrowIfNull(message);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, Arguments(file, message), directory);

        return (done, said);
    }
}

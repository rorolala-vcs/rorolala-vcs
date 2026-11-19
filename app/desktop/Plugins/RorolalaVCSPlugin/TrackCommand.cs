using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Recording the working files' content as new versions, through the writer the command line uses.
/// </summary>
/// <remarks>
/// The recording is the writer's: the plan reads every file and tree that would be recorded, the
/// message the field holds is what every subject carries, and the apply writes the plan that was
/// previewed. What this owns is the words that ask for one: the files and the message they carry.
/// The editor is never opened: a window has one line of its own where the message was written, and a
/// run that opened an editor beside it would be asking the same question twice.
/// </remarks>
internal static class TrackCommand
{
    /// <summary>The request a track is asked with.</summary>
    /// <remarks>
    /// The Workspace is not here: the writer fills it with the root it found. The account is the one
    /// the work acts as, which is what a recording is made as.
    /// </remarks>
    /// <param name="files">The files to record, in the order the listing shows them.</param>
    /// <param name="force">Whether the run records changes the work would otherwise refuse.</param>
    /// <returns>The request.</returns>
    public static TrackRequest Request(IReadOnlyList<string> files, bool force = false)
    {
        ArgumentNullException.ThrowIfNull(files);

        return new TrackRequest
        {
            Files = files,
            Force = force,
            Account = UserScope.CurrentAccount,
        };
    }

    /// <summary>Runs a track, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: reading the files and writing the versions and the Layout is work
    /// over the store, and a dock that stopped drawing for it would stop for every recording.
    /// <para>
    /// The plan is made before any message is put to a subject, and the same plan is what the apply
    /// writes, so what the run previewed is what it records. The message is the field's own and is
    /// given to every subject: this window has no room for one message per file, which is also what
    /// the command line does when it is handed a single `--message`.
    /// </para>
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="files">The files to record, in the order the listing shows them.</param>
    /// <param name="message">What the new versions say.</param>
    /// <param name="force">Whether the run records changes the work would otherwise refuse.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        IReadOnlyList<string> files,
        string message,
        bool force = false
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(files);
        ArgumentNullException.ThrowIfNull(message);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            var writer = new WorkspaceWriter(workspace);
            var request = Request(files, force);
            var plan = writer.PlanTrack(request);

            if (plan.Refused is { } refused)
            {
                return (false, refused);
            }

            var messages = new Dictionary<string, string>(
                plan.Subjects.Count,
                StringComparer.Ordinal
            );
            foreach (var subject in plan.Subjects)
            {
                messages[subject.Path] = message;
            }

            var result = writer.Track(request, plan, messages);

            return result.Refused.Count > 0
                ? (false, result.Refused[0])
                : (true, string.Empty);
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
}

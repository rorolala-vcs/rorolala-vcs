using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Joining one variant into one file, through the writer the command line uses.
/// </summary>
/// <remarks>
/// The merge is the writer's: it reads the variant, writes the variant file beside its target, and
/// records the waiting merge, so this owns only the words that ask for one: the variant and the file
/// it is to be joined into. A join can reach for a Vault when the variant or its content is not held
/// here, which is why it is run off the window's thread.
/// </remarks>
internal static class JoinCommand
{
    /// <summary>Runs a join, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// The Workspace is located from the file's own directory, since that is the one the run is made
    /// beside, and the file is named as the program names it so a path outside the run's directory is
    /// still read as the file it is.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The file the variant is to be joined into.</param>
    /// <param name="variant">The variant to join, as the index names it.</param>
    /// <param name="offline">Whether the Vault could be reached at the last fetch.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        string file,
        string variant,
        bool offline
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);
        ArgumentNullException.ThrowIfNull(variant);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            var request = new BringVariantRequest
            {
                Workspace = directory,
                Account = UserScope.CurrentAccount,
                Target = WriteTarget.Tracked,
                // A variant whose content is stored here is all the merge needs, so a run made while the Vault
                // cannot be reached joins from what is here rather than waiting on it.
                Offline = offline,
                Variant = variant,
                File = file,
            };

            new WorkspaceWriter(workspace).BringVariant(request);

            return (true, string.Empty);
        }
        catch (RolaWriteError error)
        {
            return (false, Told(error, offline));
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

    /// <summary>What a write failure is said as in the window.</summary>
    /// <remarks>
    /// The writer's own message is what a refusal says, since the merge words it where it knows what
    /// was refused. A failure whose kind carries no message is answered with the notice's own words
    /// rather than an empty line, which is the one state a variant merge can reach with no words at
    /// all: a Vault that is needed and not here.
    /// </remarks>
    /// <param name="error">The failure the writer answered with.</param>
    /// <param name="offline">Whether the Vault could be reached at the last fetch.</param>
    /// <returns>What to show.</returns>
    private static string Told(RolaWriteError error, bool offline) =>
        error.Kind switch
        {
            RolaWriteErrorKind.NoWorkspace => RolaI18N.Get("rorolala_vcs.not_a_workspace"),
            RolaWriteErrorKind.NoTrack => RolaI18N.Get("rorolala_vcs.version_graph.join_failed"),
            // Offline, a variant whose content is not here has nowhere to come from: what is missing is said
            // rather than the writer's English about a content the run was never going to fetch.
            RolaWriteErrorKind.NotFound when offline => RolaI18N.Get("rorolala_vcs.offline_content"),
            _ => error.Message is { Length: > 0 } message ? message : RolaI18N.Get("rorolala_vcs.version_graph.join_failed"),
        };
}

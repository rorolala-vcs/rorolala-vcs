using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Giving the entry at a path a new Uuid: what forking a record is, through the writer the command line uses.
/// </summary>
/// <remarks>
/// The entry the path named is dropped and made again under a new <c>Uuid</c>, holding what it held:
/// what changes is the identity a record is kept under, not the version or the content. That is what
/// makes it a fork, a file may go on being recorded under an entry of its own without moving the
/// entry it was following.
/// </remarks>
internal static class LayoutPathNewUuidCommand
{
    /// <summary>Runs the fork, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// The writer opens the Workspace itself, so a fork needs nothing of the command line and nothing of
    /// the program the window was opened by. Run off the window's thread: the Layout is read and written,
    /// which is disk work.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="path">The path in the Layout being worked in.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string directory, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(path);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            var layout = workspace.Layout;
            layout.Read();

            // The data the new entry holds is the data the path names now, read from the Layout rather
            // than asked for again: what a fork changes is the identity, and reading it here keeps the
            // version, the holder and the words the file was last recorded with.
            var entry = layout.Find(path)
                ?? throw new RolaWriteError(
                    RolaWriteErrorKind.NotFound,
                    path,
                    "the Layout names no such path"
                );

            new LayoutWriter(layout).SwitchEntry(
                path,
                Guid.NewGuid(),
                entry.Version,
                entry.Owner,
                entry.Description,
                string.IsNullOrEmpty(entry.Preference) ? null : entry.Preference
            );

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
}

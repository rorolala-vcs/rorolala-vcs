using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Taking an entry out of a Vault's own Layout: the one write a place's tree offers that is a loss
/// rather than a hint about the work.
/// </summary>
/// <remarks>
/// Removing a path here is not the unbinding a Layout of the Workspace's own does: a Vault's entry is a
/// file the Vault holds, and letting one go is the entry going, path and all. Only the account that holds
/// it, or one that administrates the Vault, is allowed to, which is the Vault's to say, what this owns
/// is the question, and the answer read back from the copy the writer followed.
/// <para>
/// The Layout is named rather than left to be resolved, because what the place shows is the one the
/// address names: a run without the name would unbind a path of the Layout being worked in, which is a
/// different write to a different file. A Vault's own Layout is named <c>truth@VAULT</c>.
/// </para>
/// </remarks>
internal static class LayoutEntryRemoveCommand
{
    /// <summary>Removes an entry through the writer, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// The writer opens the Workspace itself, so a removal needs nothing of the command line and nothing of the program
    /// the window was opened by. Run off the window's thread: the Vault is reached and the copy written, which is work
    /// that waits.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="layout">The fetched copy to remove from, as <c>truth@VAULT</c>.</param>
    /// <param name="uuid">The entry, as the copy names it.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string directory, string layout, string uuid)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(layout);
        ArgumentNullException.ThrowIfNull(uuid);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            new LayoutWriter(new LayoutScope(workspace.Root.FullName, layout)).RemoveFetchedEntry(
                Guid.Parse(uuid)
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

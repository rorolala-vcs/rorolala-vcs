using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Archiving a path of a Vault's own Layout: the removal a place's tree offers that keeps what it takes
/// away.
/// </summary>
/// <remarks>
/// It is not the removal <see cref="LayoutEntryRemoveCommand"/> makes: the entry stays, at a path under
/// <c>@/removed/</c> dated in UTC, so what reads the Layout afterwards finds it deprecated rather than
/// gone. Which is why it asks for no holder: a move changes where an entry is, not who has it.
/// <para>
/// The Layout is named rather than left to be resolved, because what the place shows is the one the
/// address names. A Vault's own Layout is named <c>truth@VAULT</c>, and the target is worked out by the
/// writer, so this names the path it has and nothing else.
/// </para>
/// </remarks>
internal static class LayoutPathSoftDeleteCommand
{
    /// <summary>Archives a path through the writer, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// The writer opens the Workspace itself, so an archive needs nothing of the command line and nothing of the
    /// program the window was opened by. Run off the window's thread: the Vault is reached and the copy written,
    /// which is work that waits.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="layout">The fetched copy to archive in, as <c>truth@VAULT</c>.</param>
    /// <param name="path">The path the entry is at.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string directory, string layout, string path)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(layout);
        ArgumentNullException.ThrowIfNull(path);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            new LayoutWriter(new LayoutScope(workspace.Root.FullName, layout)).SoftDeleteFetched(path);

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

using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Undoing the newest operation a Workspace recorded, through the writer the command line uses.
/// </summary>
/// <remarks>
/// The undo is the writer's: it reads the stack, puts back what the newest run changed, and drops that
/// run from the log. What this owns is the one button a header has room for, which is why it always
/// asks for one operation and never for a mode: a partial operation is refused rather than carried
/// halfway, which is the answer a reader who pressed Undo once would expect.
/// </remarks>
internal static class WorkspaceUndo
{
    /// <summary>Undoes the newest operation of a Workspace.</summary>
    /// <param name="directory">Where the Workspace is rooted.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        try
        {
            using var workspace = new WorkspaceScope(directory);
            workspace.Read();

            var writer = new HistoryWriter(workspace);
            writer.Undo(new UndoRequest { Count = 1 });

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

using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Moving a path within one Layout: what a move within a place is, through the writer the command line uses.
/// </summary>
/// <remarks>
/// What a move here changes is the Layout's own record of where its entries are, and nothing else: the content stays
/// where it is, and no file on disk moves. Locating an entry is moving it out of <c>@/new/</c>, and deprecating one is
/// moving it under <c>@/removed/</c>, which is why a place's tree can offer both by dragging.
/// <para>
/// The Layout is named rather than left to be resolved, because what the place shows is the one the address names: a run
/// without the name would move a path of the Layout being worked in, which is a different write to a different file. A
/// Vault's own Layout is named <c>truth@VAULT</c>; one of the Workspace's by its name alone.
/// </para>
/// <para>
/// A Layout of the Workspace's own is moved by <see cref="LayoutWriter.MovePath"/>, over the Workspace and the Layout
/// the caller names. A fetched copy of a Vault's Layout is not a place to work: the Vault itself makes the move, and the
/// copy follows what it agreed to, which is <see cref="LayoutWriter.MoveFetched"/>.
/// </para>
/// </remarks>
internal static class LayoutPathMoveCommand
{
    /// <summary>The name a Vault's single Layout is known by.</summary>
    /// <remarks>
    /// It is the command line's <c>VAULT_LAYOUT_NAME</c>, stated here because no header is shared: the name is the
    /// contract across the boundary, and a Vault keeps one Layout, so there is nothing for this side to find out.
    /// </remarks>
    public const string VaultLayout = "truth";

    /// <summary>The name of the Vault's Layout as the command line reads it: <c>truth@VAULT</c>.</summary>
    /// <param name="vault">The Vault, by the name the Workspace bound it under.</param>
    /// <returns>The Layout's name.</returns>
    public static string Named(string vault)
    {
        ArgumentNullException.ThrowIfNull(vault);

        return VaultLayout + "@" + vault;
    }

    /// <summary>Moves a path through the writer, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// The writer opens the Workspace itself, so a move of the Layout being worked in needs nothing of the command line
    /// and nothing of the program the window was opened by. Run off the window's thread: the Layout is read and written,
    /// which is disk work.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="layout">
    /// The Layout to move in, as the command line names it, or nothing for the one being worked in.
    /// </param>
    /// <param name="from">The path it is at.</param>
    /// <param name="to">The path it is to be at.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Move(
        string directory,
        string? layout,
        string from,
        string to
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(from);
        ArgumentNullException.ThrowIfNull(to);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            // A Layout the caller names is opened by that name; one it does not is the one being worked in, which is
            // what the writer reads from the Workspace's own scope.
            var target = layout is { Length: > 0 }
                ? new LayoutScope(workspace.Root.FullName, layout)
                : (LayoutScope)workspace.Layout;

            // A name carrying `@` is a fetched copy of a Vault's Layout, which the Vault itself is asked to move. Every
            // other name is a Layout of the Workspace's own, moved in place.
            if (layout is { Length: > 0 } && layout.Contains('@', StringComparison.Ordinal))
            {
                new LayoutWriter(target).MoveFetched(from, to);
            }
            else
            {
                new LayoutWriter(target).MovePath(from, to);
            }

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

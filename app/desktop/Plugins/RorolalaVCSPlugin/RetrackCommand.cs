using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Setting what a file's Layout entry records, through the writer the command line uses.
/// </summary>
/// <remarks>
/// The move is the writer's: the Layout pointer, the content written back over the file, and the
/// Vault consulted when the content is not here. What this owns is the words that ask for it: the
/// file, what the path is to record, and whether the work on disk is to follow it.
/// </remarks>
internal static class RetrackCommand
{
    /// <summary>The request a retrack is made with.</summary>
    /// <remarks>
    /// A record-only run and a forced one say opposite things about the file on disk, so force is
    /// left out when the record moves alone. The command line refuses the pair rather than choosing
    /// between them; this window never asks for both, and saying the same here keeps a caller that
    /// did from turning a record-only run into a refusal.
    /// </remarks>
    /// <param name="file">The one file to set the record of.</param>
    /// <param name="target">What the path is to record, as the index names it.</param>
    /// <param name="onlyIndex">Whether the record moves and the file on disk is left alone.</param>
    /// <param name="force">Whether unrecorded changes are to be discarded.</param>
    /// <returns>The request.</returns>
    public static RetrackRequest Request(
        string file,
        string target,
        bool onlyIndex,
        bool force
    )
    {
        ArgumentNullException.ThrowIfNull(file);
        ArgumentNullException.ThrowIfNull(target);

        return new RetrackRequest
        {
            File = file,
            Record = target,
            OnlyIndex = onlyIndex,
            Force = force && !onlyIndex,
            Account = UserScope.CurrentAccount,
        };
    }

    /// <summary>The request a fork of a path's entry is made with.</summary>
    /// <param name="file">The path whose entry is to be given a fresh `Uuid`.</param>
    /// <returns>The request, which records what the path holds now and names no target.</returns>
    public static RetrackRequest ForkRequest(string file)
    {
        ArgumentNullException.ThrowIfNull(file);

        return new RetrackRequest
        {
            File = file,
            Record = string.Empty,
            Fork = true,
            Account = UserScope.CurrentAccount,
        };
    }

    /// <summary>Runs a fork, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The path whose entry is to be given a fresh `Uuid`.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Fork(string directory, string file)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);

        return Run(directory, ForkRequest(file));
    }

    /// <summary>Runs a retrack, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Run off the window's thread: a retrack may reach for a Vault when the content of the version it goes to
    /// is not in the local store, which takes as long as a network takes.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The one file to set the record of.</param>
    /// <param name="target">What the path is to record, as the index names it.</param>
    /// <param name="onlyIndex">Whether the record moves and the file on disk is left alone.</param>
    /// <param name="force">Whether unrecorded changes are to be discarded.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        string file,
        string target,
        bool onlyIndex,
        bool force
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);
        ArgumentNullException.ThrowIfNull(target);

        return Run(directory, Request(file, target, onlyIndex, force));
    }

    /// <summary>Runs one retrack request, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="request">What the run is to do.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Run(string directory, RetrackRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            new WorkspaceWriter(workspace).Retrack(request);

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

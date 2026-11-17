using System.Runtime.InteropServices;
using System.Text.Json;

using RolaSharp.V2;

namespace RolaSharp.V2.Write;

/// <summary>What a tree cache rebuild found.</summary>
/// <param name="Paths">How many paths were written down.</param>
/// <param name="Bytes">How many bytes the paths hold.</param>
public sealed record CacheReport(int Paths, ulong Bytes);

/// <summary>A write over a Workspace a caller has read.</summary>
public sealed class WorkspaceWriter : Writer
{
    private readonly WorkspaceScoop _workspace;

    /// <summary>Makes a writer over a Workspace.</summary>
    /// <param name="workspace">The Workspace, which must have been read.</param>
    public WorkspaceWriter(WorkspaceScoop workspace) =>
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));

    /// <summary>The store the Workspace keeps.</summary>
    public StorageScoop Storage => _workspace.Storage;

    /// <summary>Reads the whole tree and writes down what each path holds.</summary>
    /// <returns>What the rebuild found.</returns>
    public CacheReport RebuildTreeCache()
    {
        var directory = Utf8(_workspace.Root.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_workspace_rebuild_cache(directory);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));
            return new CacheReport(
                (int)(RolaJson.Number(document.RootElement, "paths") ?? 0),
                RolaJson.Number(document.RootElement, "bytes") ?? 0
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }

    /// <summary>Makes the Workspace's Layout track a Vault.</summary>
    /// <param name="track">The Vault's name.</param>
    public void SetTrack(string track)
    {
        ArgumentNullException.ThrowIfNull(track);

        var directory = Utf8(_workspace.Root.FullName);
        var name = Utf8(track);
        try
        {
            Check(RorolalaBinding.rola_write_workspace_set_track(directory, name));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(name);
        }
    }

    /// <summary>Makes the Workspace's Layout track no Vault.</summary>
    public void ClearTrack()
    {
        var directory = Utf8(_workspace.Root.FullName);
        try
        {
            Check(RorolalaBinding.rola_write_workspace_clear_track(directory));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }

    /// <summary>Locks the Workspace for as long as the guard lives.</summary>
    /// <returns>The guard, or nothing when the Workspace was locked already.</returns>
    public WorkspaceLock? Lock()
    {
        var guard = RorolalaBinding.RolaWorkspace_get_locking_guard(_workspace.Handle);

        return guard == 0 ? null : new WorkspaceLock(guard);
    }

    private static void Check(RorolalaResult answer)
    {
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.From(answer.payload);
        }
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

/// <summary>A lock a writer holds, and gives back when it is disposed.</summary>
public sealed class WorkspaceLock : IDisposable
{
    private nint _guard;

    /// <summary>Makes a guard over a native lock.</summary>
    /// <param name="guard">The native guard, which must not be null.</param>
    internal WorkspaceLock(nint guard) => _guard = guard;

    /// <summary>Gives the lock back.</summary>
    public void Dispose()
    {
        if (_guard != 0)
        {
            RorolalaBinding.RolaWorkspace_drop_locking_guard(_guard);
            _guard = 0;
        }

        GC.SuppressFinalize(this);
    }
}

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

    /// <summary>Moves a path's record to a version, or one version back when none is named.</summary>
    /// <param name="path">The path, relative to the Workspace root or absolute.</param>
    /// <param name="target">The version key, or nothing for one version back.</param>
    /// <param name="fork">Give the path a fresh Uuid holding what it records now.</param>
    /// <param name="onlyIndex">Leave the work on disk alone.</param>
    /// <returns>What was written.</returns>
    public Retracked RetrackFile(
        string path,
        string? target = null,
        bool fork = false,
        bool onlyIndex = false
    )
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        var named = Utf8(target ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_retrack_file(
                directory,
                at,
                named,
                fork,
                onlyIndex
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return new Retracked(
                Guid.Parse(RolaJson.Text(document.RootElement, "id") ?? string.Empty),
                RolaJson.Text(document.RootElement, "version") ?? string.Empty
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(named);
        }
    }

    /// <summary>Puts a version's content back at a path and records that the two agree.</summary>
    /// <param name="path">The path, relative to the Workspace root or absolute.</param>
    /// <param name="version">The version key.</param>
    /// <returns>What was written.</returns>
    public Restored Restore(string path, string version)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(version);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        var named = Utf8(version);
        try
        {
            var answer = RorolalaBinding.rola_write_checkout_restore(directory, at, named);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return new Restored(
                Guid.Parse(RolaJson.Text(document.RootElement, "id") ?? string.Empty),
                RolaJson.Text(document.RootElement, "version") ?? string.Empty
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(named);
        }
    }

    /// <summary>Writes down that a path now holds what the Layout names.</summary>
    /// <param name="path">The path, relative to the Workspace root.</param>
    public void Remember(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        try
        {
            Check(RorolalaBinding.rola_write_checkout_remember(directory, at));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
        }
    }

    /// <summary>Records a file's content as a version and names it in the Layout.</summary>
    /// <param name="path">The file's path, relative to the Workspace root or absolute.</param>
    /// <param name="creator">The creator's name.</param>
    /// <param name="message">What the version says was done.</param>
    /// <param name="policy">A chunking policy, or nothing for the store's own choice.</param>
    /// <param name="owner">The account that holds the entry, or nothing to keep the existing owner.</param>
    /// <returns>What was written.</returns>
    public TrackedFile TrackFile(
        string path,
        string creator,
        string message,
        string? policy = null,
        string? owner = null
    )
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(creator);
        ArgumentNullException.ThrowIfNull(message);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        var author = Utf8(creator);
        var words = Utf8(message);
        var chunking = Utf8(policy ?? string.Empty);
        var held = Utf8(owner ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_track_file(
                directory,
                at,
                author,
                words,
                chunking,
                held
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return new TrackedFile(
                Guid.Parse(RolaJson.Text(document.RootElement, "id") ?? string.Empty),
                RolaJson.Text(document.RootElement, "storage") ?? string.Empty,
                RolaJson.Text(document.RootElement, "variant") ?? string.Empty,
                RolaJson.Text(document.RootElement, "version") ?? string.Empty
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(author);
            Marshal.FreeCoTaskMem(words);
            Marshal.FreeCoTaskMem(chunking);
            Marshal.FreeCoTaskMem(held);
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

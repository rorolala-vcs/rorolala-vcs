using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>What a tree cache rebuild found.</summary>
/// <param name="Paths">How many paths were written down.</param>
/// <param name="Bytes">How many bytes the paths hold.</param>
public sealed record CacheReport(int Paths, ulong Bytes);

/// <summary>A write over a Workspace a caller has read.</summary>
public sealed partial class WorkspaceWriter : Writer
{
    private readonly WorkspaceScope _workspace;

    /// <summary>Makes a writer over a Workspace.</summary>
    /// <param name="workspace">The Workspace, which must have been read.</param>
    public WorkspaceWriter(WorkspaceScope workspace) =>
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));

    /// <summary>The store the Workspace keeps.</summary>
    public StorageScope Storage => _workspace.Storage;

    /// <summary>Binds a store to the Workspace.</summary>
    /// <param name="storage">The directory carrying the store's `rolast.toml`.</param>
    /// <returns>What the binding came to.</returns>
    public StorageBound BindStorage(DirectoryInfo storage)
    {
        ArgumentNullException.ThrowIfNull(storage);

        var workspace = Utf8(_workspace.Root.FullName);
        var at = Utf8(storage.FullName);
        try
        {
            var answer = RorolalaBinding.rola_workspace_storage_bind(workspace, at);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return StorageBound.From(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_write_storage_bind(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(workspace);
            Marshal.FreeCoTaskMem(at);
        }
    }

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

            var handle = answer.payload;
            try
            {
                return new CacheReport(
                    (int)RorolalaBinding.rola_write_workspace_cache_paths(handle),
                    RorolalaBinding.rola_write_workspace_cache_bytes(handle)
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_workspace_cache(handle);
            }
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
    /// <param name="force">Go past changes the record does not name.</param>
    /// <param name="denyJump">Refuse a target off the path's chain.</param>
    /// <returns>What was written.</returns>
    public Retracked RetrackFile(
        string path,
        string? target = null,
        bool fork = false,
        bool onlyIndex = false,
        bool force = false,
        bool denyJump = false
    ) {
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
                onlyIndex,
                force,
                denyJump
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new Retracked(
                    Guid.Parse(NativeText.Taken(RorolalaBinding.rola_write_restore_id(handle))),
                    NativeText.Taken(RorolalaBinding.rola_write_restore_version(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_restore(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(named);
        }
    }

    /// <summary>Reads what a collecting run would take away.</summary>
    /// <param name="storage">Whether the store is swept.</param>
    /// <param name="index">Whether the index is swept.</param>
    /// <param name="grace">How long an unreferenced entry is kept, in seconds, or zero for the default.</param>
    /// <returns>What each side would take away.</returns>
    public CollectPlan PlanCollect(bool storage = true, bool index = true, ulong grace = 0) =>
        Collector.Plan(_workspace.Root.FullName, storage, index, grace);

    /// <summary>Carries out a collecting run.</summary>
    /// <param name="storage">Whether the store is swept.</param>
    /// <param name="index">Whether the index is swept.</param>
    /// <param name="grace">How long an unreferenced entry is kept, in seconds, or zero for the default.</param>
    /// <returns>What each side took away.</returns>
    public CollectReport Collect(bool storage = true, bool index = true, ulong grace = 0) =>
        Collector.Run(_workspace.Root.FullName, storage, index, grace);
    /// <summary>Carries a sync run out against a Vault.</summary>
    /// <param name="vault">The Vault the Workspace binds.</param>
    /// <param name="account">The account the run signs as.</param>
    /// <param name="direction">Which way content may move.</param>
    /// <param name="forced">Whether the run may change history rather than refuse.</param>
    /// <param name="storage">Whether the store moves.</param>
    /// <param name="index">Whether the index moves.</param>
    /// <param name="layout">Whether the Vault's Layout is read and written.</param>
    /// <returns>What the run came to.</returns>
    public SyncReport Sync(
        string vault,
        AccountScope account,
        SyncDirection direction = SyncDirection.Both,
        bool forced = false,
        bool storage = true,
        bool index = true,
        bool layout = true
    )
    {
        ArgumentNullException.ThrowIfNull(vault);
        ArgumentNullException.ThrowIfNull(account);

        var binding =
            _workspace.Config?.Vaults.FirstOrDefault(item => item.Name == vault)
            ?? throw new RolaWriteError(
                RolaWriteErrorKind.InvalidPath,
                vault,
                "the Workspace binds no such Vault"
            );

        return SyncAt(
            vault,
            binding.Address,
            account.Name,
            account.KeyPath,
            direction,
            forced,
            storage,
            index,
            layout
        );
    }

    /// <summary>Carries a sync run out against an explicit address.</summary>
    /// <param name="vault">The Vault name the run speaks of.</param>
    /// <param name="address">The Vault's address.</param>
    /// <param name="account">The account's name.</param>
    /// <param name="keyPath">The account's private key file.</param>
    /// <param name="direction">Which way content may move.</param>
    /// <param name="forced">Whether the run may change history rather than refuse.</param>
    /// <param name="storage">Whether the store moves.</param>
    /// <param name="index">Whether the index moves.</param>
    /// <param name="layout">Whether the Vault's Layout is read and written.</param>
    /// <returns>What the run came to.</returns>
    public SyncReport SyncAt(
        string vault,
        string address,
        string account,
        string keyPath,
        SyncDirection direction = SyncDirection.Both,
        bool forced = false,
        bool storage = true,
        bool index = true,
        bool layout = true
    )
    {
        ArgumentNullException.ThrowIfNull(vault);
        ArgumentNullException.ThrowIfNull(address);
        ArgumentNullException.ThrowIfNull(account);
        ArgumentNullException.ThrowIfNull(keyPath);

        var directory = Utf8(_workspace.Root.FullName);
        var place = Utf8(vault);
        var me = Utf8(account);
        var key = Utf8(keyPath);
        var where = Utf8(address);
        var way = Utf8(
            direction switch
            {
                SyncDirection.Up => "up",
                SyncDirection.Down => "down",
                _ => "both",
            }
        );
        try
        {
            var answer = RorolalaBinding.rola_write_sync(
                directory,
                place,
                me,
                key,
                where,
                way,
                forced,
                storage,
                index,
                layout
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return SyncReports.OfRun(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_write_sync(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(place);
            Marshal.FreeCoTaskMem(me);
            Marshal.FreeCoTaskMem(key);
            Marshal.FreeCoTaskMem(where);
            Marshal.FreeCoTaskMem(way);
        }
    }

    /// <summary>Reads what a sync run would do, against what the Vault holds now.</summary>
    /// <remarks>
    /// The Vault's Layout is fetched before the plan is made, unless the request is offline, so the
    /// plan answers from what the Vault holds rather than from a copy that may have gone stale. A
    /// request that leaves the Vault's Layout alone has no plan to make, and is refused here.
    /// </remarks>
    /// <param name="request">The account, the Vault, the direction and the sides the run moves.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the run is to do.</returns>
    /// <exception cref="RolaWriteError">The run failed, and the failure says why.</exception>
    public SyncPlan PlanSync(
        SyncRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(SyncFields(memory, request));

        var answer = Call(
            RorolalaBinding.rola_write_sync_plan,
            native,
            progress,
            cancellation
        );
        try
        {
            return SyncPlans.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_sync_plan(answer);
        }
    }

    /// <summary>Carries out a sync plan a caller agreed to.</summary>
    /// <remarks>
    /// The native side reads the plan again and compares it with the work before writing, and a
    /// plan that no longer matches is refused rather than applied. A file changed here that the
    /// Vault is ahead on is refused before anything is written, unless the request is forced, so a
    /// pull never overwrites what the run does not know about.
    /// </remarks>
    /// <param name="request">The account, the Vault, the direction and the sides the run moves.</param>
    /// <param name="plan">The plan a caller agreed to, which must be one <see cref="PlanSync"/> made.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the run came to.</returns>
    /// <exception cref="RolaWriteError">
    /// The plan did not come from <see cref="PlanSync"/>, the plan no longer matches the work, a
    /// blocked entry was not forced past, or the run failed. The failure says which.
    /// </exception>
    [System.Diagnostics.CodeAnalysis.SuppressMessage(
        "Usage",
        "CA1062:Validate arguments of public methods",
        Justification = "a null progress or cancellation is the caller asking this side to make one, which Invoke does"
    )]
    public SyncReport Sync(
        SyncRequest request,
        SyncPlan plan,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(plan);

        // A plan built by hand holds no entries the native side could read back, and applying an
        // empty one would write what nobody previewed.
        if (!plan.Agreed)
        {
            throw new RolaWriteError(
                RolaWriteErrorKind.Precondition,
                null,
                "the plan was not made by PlanSync"
            );
        }

        using var memory = new NativeRequest();
        var native = memory.Pointer(SyncFields(memory, request));
        var entries = memory.Array(
            plan.Entries
                .Select(entry => new RolaWriteSyncAgreed
                {
                    uuid = memory.Text(entry.Uuid),
                    local_path = memory.Text(entry.LocalPath),
                    remote_path = memory.Text(entry.RemotePath),
                    planned_path = memory.Text(entry.PlannedPath),
                    local_version = memory.Text(entry.LocalVersion),
                    remote_version = memory.Text(entry.RemoteVersion),
                    kind = memory.Text(entry.Kind),
                })
                .ToList()
        );

        var answer = Call(
            RorolalaBinding.rola_write_sync_apply,
            native,
            entries.Items,
            entries.Count,
            progress,
            cancellation
        );
        try
        {
            return SyncReports.OfApply(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_sync_apply(answer);
        }
    }

    /// <summary>The fields a sync sends, which a plan and an apply share.</summary>
    /// <param name="memory">Where the request's strings are held.</param>
    /// <param name="request">The sync.</param>
    /// <returns>The native request.</returns>
    private RolaWriteSyncRequest SyncFields(NativeRequest memory, SyncRequest request) =>
        new()
        {
            common = memory.Common(request, _workspace.Root.FullName),
            direction = request.Direction switch
            {
                SyncDirection.Up => RolaWriteDirection.RolaWriteDirection_Up,
                SyncDirection.Down => RolaWriteDirection.RolaWriteDirection_Down,
                _ => RolaWriteDirection.RolaWriteDirection_Both,
            },
            forced = NativeRequest.Flag(request.Forced),
            storage = NativeRequest.Flag(request.Storage),
            index = NativeRequest.Flag(request.Index),
            layout = NativeRequest.Flag(request.Layout),
        };

    /// <summary>Writes what an entry says, in one call.</summary>
    /// <param name="id">The entry's id.</param>
    /// <param name="version">The version it records.</param>
    /// <param name="owner">The account that holds it, or nothing for none.</param>
    /// <param name="description">What it says about itself.</param>
    /// <param name="preference">The variant it prefers, or nothing for none.</param>
    public void SetAll(
        Guid id,
        string version,
        string? owner = null,
        string description = "",
        string? preference = null
    ) =>
        new LayoutWriter(_workspace.Layout).SetAll(
            id,
            version,
            owner,
            description,
            preference
        );

    /// <summary>Puts a path on one version and writes what the entry says about it.</summary>
    /// <param name="path">The path, relative to the Workspace root or absolute.</param>
    /// <param name="version">The version key.</param>
    /// <param name="owner">The account that holds the entry, or nothing for none.</param>
    /// <param name="description">What the entry says about itself.</param>
    /// <param name="preference">The variant to prefer, or nothing for none.</param>
    /// <returns>The entry and the version the Layout now names.</returns>
    public Restored AlignEntry(
        string path,
        string version,
        string? owner = null,
        string description = "",
        string? preference = null
    )
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(version);
        ArgumentNullException.ThrowIfNull(description);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        var named = Utf8(version);
        var held = Utf8(owner ?? string.Empty);
        var words = Utf8(description);
        var preferred = Utf8(preference ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_align_entry(
                directory,
                at,
                named,
                held,
                words,
                preferred
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new Restored(
                    Guid.Parse(NativeText.Taken(RorolalaBinding.rola_write_restore_id(handle))),
                    NativeText.Taken(RorolalaBinding.rola_write_restore_version(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_restore(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(named);
            Marshal.FreeCoTaskMem(held);
            Marshal.FreeCoTaskMem(words);
            Marshal.FreeCoTaskMem(preferred);
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

            var handle = answer.payload;
            try
            {
                return new Restored(
                    Guid.Parse(NativeText.Taken(RorolalaBinding.rola_write_restore_id(handle))),
                    NativeText.Taken(RorolalaBinding.rola_write_restore_version(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_restore(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(named);
        }
    }

    /// <summary>Removes a Layout entry and the path that names it.</summary>
    /// <param name="id">The entry's id.</param>
    public void RemoveEntry(Guid id)
    {
        var layout = _workspace.Layout;
        string? path = null;
        if (!NoCheck)
        {
            layout.Read();
            var entry =
                layout.Entries.FirstOrDefault(item => item.Uuid == id.ToString())
                ?? throw new RolaWriteError(
                    RolaWriteErrorKind.InvalidPath,
                    id.ToString(),
                    "the Layout names no such entry"
                );
            path = entry.Path == "." ? null : entry.Path;
        }

        new LayoutWriter(layout).RemoveEntry(id);
        if (path is not null)
        {
            Forget(path);
        }
    }

    /// <summary>Removes a path, leaving its entry and its work.</summary>
    /// <param name="path">The path, relative to the Workspace root.</param>
    public void RemovePath(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        new LayoutWriter(_workspace.Layout).RemovePath(path);
        Forget(path);
    }

    /// <summary>Removes a path's record, leaving the work where it is.</summary>
    /// <param name="path">The path, relative to the Workspace root.</param>
    public void Untrack(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var layout = _workspace.Layout;
        layout.Read();
        var entry =
            layout.Find(path)
            ?? throw new RolaWriteError(
                RolaWriteErrorKind.InvalidPath,
                path,
                "the Layout names no such path"
            );

        RemoveEntry(Guid.Parse(entry.Uuid));
    }

    /// <summary>Forgets the reading of a path the run took out.</summary>
    /// <param name="path">The path, relative to the Workspace root.</param>
    public void Forget(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        try
        {
            Check(RorolalaBinding.rola_write_checkout_forget(directory, at));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
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

    /// <summary>Lays the Workspace's store and index out afresh.</summary>
    /// <param name="storage">Whether to pack the store.</param>
    /// <param name="index">Whether to pack the VCS index.</param>
    /// <returns>What changed.</returns>
    public Packed Pack(bool storage = true, bool index = true)
    {
        var directory = Utf8(_workspace.Root.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_pack(directory, storage, index);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new Packed(RorolalaBinding.rola_write_pack_changed(handle));
            }
            finally
            {
                RorolalaBinding.free_rola_write_pack(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }

    /// <summary>Records a file's content as a version and names it in the Layout.</summary>
    /// <param name="path">The file's path, relative to the Workspace root or absolute.</param>
    /// <param name="creator">The creator's name.</param>
    /// <param name="message">What the version says was done.</param>
    /// <param name="policy">A chunking policy, or nothing for the store's own choice.</param>
    /// <param name="owner">The account that holds the entry, or nothing to keep the existing owner.</param>
    /// <param name="description">What the entry says about itself, or nothing.</param>
    /// <returns>What was written.</returns>
    public TrackedFile TrackFile(
        string path,
        string creator,
        string message,
        string? policy = null,
        string? owner = null,
        string? description = null
    )
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(creator);
        ArgumentNullException.ThrowIfNull(message);

        TrackedFile tracked;
        var directory = Utf8(_workspace.Root.FullName);
        var at = Utf8(path);
        var author = Utf8(creator);
        var words = Utf8(message);
        var chunking = Utf8(policy ?? string.Empty);
        var held = Utf8(owner ?? string.Empty);
        var note = Utf8(description ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_track_file(
                directory,
                at,
                author,
                words,
                chunking,
                held,
                note
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                tracked = new TrackedFile(
                    Guid.Parse(NativeText.Taken(RorolalaBinding.rola_write_tracked_file_id(handle))),
                    NativeText.Taken(RorolalaBinding.rola_write_tracked_file_storage(handle)),
                    NativeText.Taken(RorolalaBinding.rola_write_tracked_file_variant(handle)),
                    NativeText.Taken(RorolalaBinding.rola_write_tracked_file_version(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_tracked_file(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(author);
            Marshal.FreeCoTaskMem(words);
            Marshal.FreeCoTaskMem(chunking);
            Marshal.FreeCoTaskMem(held);
            Marshal.FreeCoTaskMem(note);
        }

        return tracked;
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
        var guard = RorolalaBinding.rola_workspace_lock_acquire(_workspace.Handle);

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
            RorolalaBinding.rola_workspace_lock_release(_guard);
            _guard = 0;
        }

        GC.SuppressFinalize(this);
    }
}

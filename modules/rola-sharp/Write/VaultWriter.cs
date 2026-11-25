using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp.Write;

/// <summary>A write over a Vault a caller has read.</summary>
public sealed class VaultWriter : Writer
{
    private readonly VaultScope _vault;

    /// <summary>Makes a writer over a Vault.</summary>
    /// <param name="vault">The Vault, which must have been read.</param>
    public VaultWriter(VaultScope vault) =>
        _vault = vault ?? throw new ArgumentNullException(nameof(vault));

    /// <summary>Binds a store to the Vault.</summary>
    /// <param name="storage">The directory carrying the store's `rolast.toml`.</param>
    /// <returns>What the binding came to.</returns>
    public StorageBound BindStorage(DirectoryInfo storage)
    {
        ArgumentNullException.ThrowIfNull(storage);

        var vault = Marshal.StringToCoTaskMemUTF8(_vault.Root.FullName);
        var at = Marshal.StringToCoTaskMemUTF8(storage.FullName);
        try
        {
            var answer = RorolalaBinding.rola_vault_storage_bind(vault, at);
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
            Marshal.FreeCoTaskMem(vault);
            Marshal.FreeCoTaskMem(at);
        }
    }

    /// <summary>Removes a Layout entry and the path that names it.</summary>
    /// <param name="id">The entry's id.</param>
    public void RemoveEntry(Guid id)
    {
        if (!NoCheck)
        {
            var layout = _vault.Layout;
            layout.Read();
            if (layout.Entries.All(item => item.Uuid != id.ToString()))
            {
                throw new RolaWriteError(
                    RolaWriteErrorKind.InvalidPath,
                    id.ToString(),
                    "the Layout names no such entry"
                );
            }

            new LayoutWriter(layout).RemoveEntry(id);
            return;
        }

        new LayoutWriter(_vault.Layout).RemoveEntry(id);
    }

    /// <summary>Removes a path, leaving its entry and its work.</summary>
    /// <param name="path">The path, relative to the Vault root.</param>
    public void RemovePath(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        new LayoutWriter(_vault.Layout).RemovePath(path);
    }

    /// <summary>Moves a path under the archive, leaving its entry and its work.</summary>
    /// <param name="path">The path, relative to the Vault root.</param>
    public void SoftDelete(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        new LayoutWriter(_vault.Layout).SoftDelete(path);
    }

    /// <summary>Removes a path's record, leaving the work where it is.</summary>
    /// <param name="path">The path, relative to the Vault root.</param>
    public void Untrack(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var layout = _vault.Layout;
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

    /// <summary>Reads what a collecting run would take away.</summary>
    /// <param name="storage">Whether the store is swept.</param>
    /// <param name="index">Whether the index is swept.</param>
    /// <param name="grace">How long an unreferenced entry is kept, in seconds, or zero for the default.</param>
    /// <returns>What each side would take away.</returns>
    public CollectPlan PlanCollect(bool storage = true, bool index = true, ulong grace = 0) =>
        Collector.Plan(_vault.Root.FullName, storage, index, grace);

    /// <summary>Carries out a collecting run.</summary>
    /// <param name="storage">Whether the store is swept.</param>
    /// <param name="index">Whether the index is swept.</param>
    /// <param name="grace">How long an unreferenced entry is kept, in seconds, or zero for the default.</param>
    /// <returns>What each side took away.</returns>
    public CollectReport Collect(bool storage = true, bool index = true, ulong grace = 0) =>
        Collector.Run(_vault.Root.FullName, storage, index, grace);
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
        new LayoutWriter(_vault.Layout).SetAll(
            id,
            version,
            owner,
            description,
            preference
        );

    /// <summary>Puts a path on one version and writes what the entry says about it.</summary>
    /// <param name="path">The path, relative to the Vault root or absolute.</param>
    /// <param name="version">The version key.</param>
    /// <param name="owner">The account that holds the entry, or nothing for none.</param>
    /// <param name="description">What the entry says about itself.</param>
    /// <param name="preference">The variant to prefer, or nothing for none.</param>
    /// <returns>The entry and the version the Layout now names.</returns>
    public Restored Align(
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

        var directory = Utf8(_vault.Root.FullName);
        var at = Utf8(path);
        var named = Utf8(version);
        var held = Utf8(owner ?? string.Empty);
        var words = Utf8(description);
        var preferred = Utf8(preference ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_align_vault(
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

    /// <summary>Moves a path's record to a version in the Vault.</summary>
    /// <param name="path">The path, relative to the Vault root.</param>
    /// <param name="target">The version, or nothing for one version back.</param>
    /// <param name="fork">Whether to give the path a fresh id.</param>
    /// <param name="onlyIndex">Whether to leave the work on disk alone.</param>
    /// <param name="force">Whether to go past changes the record does not name.</param>
    /// <param name="denyJump">Whether to refuse a target off the path's chain.</param>
    /// <returns>The entry and the version the Layout now names.</returns>
    public Retracked RetrackFile(
        string path,
        string? target = null,
        bool fork = false,
        bool onlyIndex = false,
        bool force = false,
        bool denyJump = false
    )
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_vault.Root.FullName);
        var at = Utf8(path);
        var named = Utf8(target ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_retrack_vault_file(
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

    /// <summary>Puts a version's content back at a path in the Vault.</summary>
    /// <param name="path">The path, relative to the Vault root.</param>
    /// <param name="version">The version whose content is written.</param>
    /// <returns>The entry and the version the Layout now names.</returns>
    public Restored Restore(string path, string version)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(version);

        var directory = Utf8(_vault.Root.FullName);
        var at = Utf8(path);
        var named = Utf8(version);
        try
        {
            var answer = RorolalaBinding.rola_write_checkout_restore_vault(directory, at, named);
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

    /// <summary>Records a file's content and names it in the Vault's Layout.</summary>
    /// <param name="path">The file, relative to the Vault root.</param>
    /// <param name="creator">Who recorded the version.</param>
    /// <param name="message">What the version says.</param>
    /// <param name="policy">The cut to use, or nothing for the store's own choice.</param>
    /// <param name="owner">The account that holds the entry, or nothing to keep the existing owner.</param>
    /// <param name="description">What the entry says about itself.</param>
    /// <returns>The entry and the version written.</returns>
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

        var directory = Utf8(_vault.Root.FullName);
        var at = Utf8(path);
        var author = Utf8(creator);
        var words = Utf8(message);
        var chunking = Utf8(policy ?? string.Empty);
        var held = Utf8(owner ?? string.Empty);
        var note = Utf8(description ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_track_vault_file(
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
                return new TrackedFile(
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
    }

    /// <summary>Lays the Vault's store and index out afresh.</summary>
    /// <param name="storage">Whether to pack the store.</param>
    /// <param name="index">Whether to pack the VCS index.</param>
    /// <returns>What changed.</returns>
    public Packed Pack(bool storage = true, bool index = true)
    {
        var directory = Utf8(_vault.Root.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_pack_vault(directory, storage, index);
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

    /// <summary>Locks the Vault for as long as the guard lives.</summary>
    /// <returns>The guard, or nothing when the Vault was locked already.</returns>
    public VaultLock? Lock()
    {
        var guard = RorolalaBinding.rola_vault_lock_acquire(_vault.Handle);

        return guard == 0 ? null : new VaultLock(guard);
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

/// <summary>A Vault lock a writer holds, and gives back when it is disposed.</summary>
public sealed class VaultLock : IDisposable
{
    private nint _guard;

    /// <summary>Makes a guard over a native lock.</summary>
    /// <param name="guard">The native guard, which must not be null.</param>
    internal VaultLock(nint guard) => _guard = guard;

    /// <summary>Gives the lock back.</summary>
    public void Dispose()
    {
        if (_guard != 0)
        {
            RorolalaBinding.rola_vault_lock_release(_guard);
            _guard = 0;
        }

        GC.SuppressFinalize(this);
    }
}

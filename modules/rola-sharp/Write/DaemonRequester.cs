using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>The daemon actions a Workspace runs against a Vault.</summary>
/// <remarks>
/// Each method is one action and fills the request struct that action reads. The requester does not
/// interpret what the Vault answers beyond the handles the header declares: a listing stays the text
/// the action printed, and a typed answer is read through its own getters and released here.
/// </remarks>
public sealed class DaemonRequester
{
    private readonly WorkspaceScope _workspace;
    private readonly AccountScope _account;
    private readonly string _target;

    /// <summary>Makes a requester for one Vault.</summary>
    /// <param name="workspace">The Workspace the exchange is run from, which must have been read.</param>
    /// <param name="account">The account the exchange signs as.</param>
    /// <param name="target">The Vault's address or name, as the action takes it.</param>
    public DaemonRequester(WorkspaceScope workspace, AccountScope account, string target)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _account = account ?? throw new ArgumentNullException(nameof(account));
        _target = target ?? throw new ArgumentNullException(nameof(target));
    }

    /// <summary>Runs the handshake action.</summary>
    /// <param name="name">The name the action takes.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? Handshake(string name)
    {
        var target = Utf8(_target);
        var text = Utf8(name);
        try
        {
            return AnswerText(
                RorolalaBinding.rola_daemon_action_handshake(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    text
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the sync-all action.</summary>
    /// <param name="ask">How much the sending side reads at once, `c:N`, or empty for the default.</param>
    public void SyncAll(string ask)
    {
        var target = Utf8(_target);
        var text = Utf8(ask);
        var request = Pinned(new RolaDaemonSyncAllRequest { ask = text });
        try
        {
            AnswerVoid(
                RorolalaBinding.rola_daemon_action_sync_all(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the sync-index-all action.</summary>
    public void SyncIndexAll()
    {
        var target = Utf8(_target);
        try
        {
            AnswerVoid(
                RorolalaBinding.rola_daemon_action_sync_index_all(
                    _workspace.Handle,
                    _account.Handle,
                    target
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
        }
    }

    /// <summary>Runs the list-remote action.</summary>
    /// <param name="which">`manifests` for the contents the store keeps cut, anything else for its objects.</param>
    /// <returns>The keys, one a line.</returns>
    public string? ListRemote(string which)
    {
        var target = Utf8(_target);
        var text = Utf8(which);
        var request = Pinned(new RolaDaemonListRemoteRequest { which = text });
        try
        {
            return AnswerText(
                RorolalaBinding.rola_daemon_action_list_remote(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the sync-hashes action.</summary>
    /// <param name="units">The units to carry, one a line, and the `c:N` ask.</param>
    /// <returns>The reads granted at once, as decimal text.</returns>
    public string? SyncHashes(string units)
    {
        var target = Utf8(_target);
        var text = Utf8(units);
        var request = Pinned(new RolaDaemonSyncHashesRequest { units = text });
        try
        {
            return AnswerText(
                RorolalaBinding.rola_daemon_action_sync_hashes(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the list-remote-index action.</summary>
    /// <param name="which">`variants`, `versions`, `str`, or anything else for every object.</param>
    /// <returns>The hashes, one a line.</returns>
    public string? ListRemoteIndex(string which)
    {
        var target = Utf8(_target);
        var text = Utf8(which);
        var request = Pinned(new RolaDaemonListRemoteIndexRequest { which = text });
        try
        {
            return AnswerText(
                RorolalaBinding.rola_daemon_action_list_remote_index(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the read-remote-index action.</summary>
    /// <param name="hash">The hash of the object, as hex or named with `blake3:`.</param>
    /// <returns>The index object, which the caller releases with `free_rola_vcs_index_object`.</returns>
    public nint ReadRemoteIndex(string hash)
    {
        var target = Utf8(_target);
        var text = Utf8(hash);
        var request = Pinned(new RolaDaemonReadRemoteIndexRequest { hash = text });
        try
        {
            return AnswerHandle(
                RorolalaBinding.rola_daemon_action_read_remote_index(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the fetch-layout action.</summary>
    /// <param name="name">The name the Workspace keeps the fetched Layout under.</param>
    public void FetchLayout(string name)
    {
        var target = Utf8(_target);
        var text = Utf8(name);
        var request = Pinned(new RolaDaemonFetchLayoutRequest { name = text });
        try
        {
            AnswerVoid(
                RorolalaBinding.rola_daemon_action_fetch_layout(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the request-ownership action.</summary>
    /// <param name="name">The words the bar is drawn under, or empty for none.</param>
    /// <param name="uuids">The `Uuid`s of the entries to take.</param>
    /// <returns>What became of each, in the order asked.</returns>
    public IReadOnlyList<DaemonOwnership> RequestOwnership(
        string name,
        IEnumerable<string> uuids
    ) =>
        Ownership(
            name,
            uuids,
            false,
            RorolalaBinding.rola_daemon_action_request_ownership
        );

    /// <summary>Runs the giveup-ownership action.</summary>
    /// <param name="name">The words the bar is drawn under, or empty for none.</param>
    /// <param name="uuids">The `Uuid`s of the entries to let go of.</param>
    /// <returns>What became of each, in the order asked.</returns>
    public IReadOnlyList<DaemonOwnership> GiveupOwnership(
        string name,
        IEnumerable<string> uuids
    ) =>
        Ownership(
            name,
            uuids,
            true,
            RorolalaBinding.rola_daemon_action_giveup_ownership
        );

    /// <summary>Runs the fetch-ignore-rules action.</summary>
    /// <param name="name">The name the Workspace keeps this Vault's rules under.</param>
    public void FetchIgnoreRules(string name)
    {
        var target = Utf8(_target);
        var text = Utf8(name);
        var request = Pinned(new RolaDaemonFetchIgnoreRulesRequest { name = text });
        try
        {
            AnswerVoid(
                RorolalaBinding.rola_daemon_action_fetch_ignore_rules(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the remove-remote-path action.</summary>
    /// <param name="path">The path to remove, as the Vault's own Layout writes it.</param>
    /// <returns>What became of the removal.</returns>
    public DaemonRemove RemoveRemotePath(string path)
    {
        var target = Utf8(_target);
        var text = Utf8(path);
        var request = Pinned(new RolaDaemonRemoveRemotePathRequest { path = text });
        try
        {
            var handle = AnswerHandle(
                RorolalaBinding.rola_daemon_action_remove_remote_path(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );

            try
            {
                return new DaemonRemove(
                    RemoveKind(RorolalaBinding.rola_daemon_entry_remove_outcome_kind(handle)),
                    NativeText.Taken(RorolalaBinding.rola_daemon_entry_remove_outcome_owner(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_daemon_entry_remove_outcome(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the write-remote-entries action.</summary>
    /// <param name="creates">The entries to create, each a path, a `Uuid`, a version and what it is.</param>
    /// <param name="sets">The versions to set, each a `Uuid` and a version.</param>
    /// <returns>What became of each write, the creates first and then the sets.</returns>
    public IReadOnlyList<DaemonEntryWrite> WriteRemoteEntries(
        IEnumerable<(string Path, string Uuid, string Version, string Kind)> creates,
        IEnumerable<(string Uuid, string Version)> sets
    )
    {
        ArgumentNullException.ThrowIfNull(creates);
        ArgumentNullException.ThrowIfNull(sets);

        var target = Utf8(_target);
        var createRecords = creates.ToArray();
        var setRecords = sets.ToArray();
        var written = new List<nint>();
        var createArray = Records(
            createRecords.Length,
            Marshal.SizeOf<RolaDaemonRemoteCreate>(),
            out var createSize
        );
        var setArray = Records(
            setRecords.Length,
            Marshal.SizeOf<RolaDaemonRemoteSet>(),
            out var setSize
        );

        try
        {
            for (var index = 0; index < createRecords.Length; index++)
            {
                var (path, uuid, version, kind) = createRecords[index];
                var record = new RolaDaemonRemoteCreate
                {
                    path = Remember(written, path),
                    uuid = Remember(written, uuid),
                    version = Remember(written, version),
                    kind = Remember(written, kind),
                };
                Marshal.StructureToPtr(record, createArray + (index * createSize), false);
            }

            for (var index = 0; index < setRecords.Length; index++)
            {
                var (uuid, version) = setRecords[index];
                var record = new RolaDaemonRemoteSet
                {
                    uuid = Remember(written, uuid),
                    version = Remember(written, version),
                };
                Marshal.StructureToPtr(record, setArray + (index * setSize), false);
            }

            var handle = AnswerHandle(
                RorolalaBinding.rola_daemon_action_write_remote_entries(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    createArray,
                    (nuint)createRecords.Length,
                    setArray,
                    (nuint)setRecords.Length
                )
            );

            try
            {
                return ReadEntryWrites(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_daemon_entry_writes(handle);
            }
        }
        finally
        {
            if (createArray != 0)
            {
                Marshal.FreeCoTaskMem(createArray);
            }

            if (setArray != 0)
            {
                Marshal.FreeCoTaskMem(setArray);
            }

            foreach (var text in written)
            {
                Marshal.FreeCoTaskMem(text);
            }

            Marshal.FreeCoTaskMem(target);
        }
    }

    /// <summary>Runs the bring-remote-index action.</summary>
    /// <param name="hash">The hash of the version to start from.</param>
    /// <returns>What became of the walk.</returns>
    public DaemonChain BringRemoteIndex(string hash)
    {
        var target = Utf8(_target);
        var text = Utf8(hash);
        var request = Pinned(new RolaDaemonBringRemoteIndexRequest { hash = text });
        try
        {
            var handle = AnswerHandle(
                RorolalaBinding.rola_daemon_action_bring_remote_index(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    request
                )
            );

            try
            {
                return new DaemonChain(
                    ChainKind(RorolalaBinding.rola_daemon_chain_outcome_kind(handle)),
                    NativeText.Taken(RorolalaBinding.rola_daemon_chain_outcome_key(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_daemon_chain_outcome(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(request);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Runs the move-remote-path action.</summary>
    /// <param name="from">The path it is at.</param>
    /// <param name="to">The path it is to be at.</param>
    /// <returns>What became of the move.</returns>
    public DaemonPathMove MoveRemotePath(string from, string to)
    {
        var target = Utf8(_target);
        var one = Utf8(from);
        var two = Utf8(to);
        try
        {
            var handle = AnswerHandle(
                RorolalaBinding.rola_daemon_action_move_remote_path(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    one,
                    two
                )
            );

            try
            {
                return PathMoveKind(RorolalaBinding.rola_daemon_path_move_outcome_kind(handle));
            }
            finally
            {
                RorolalaBinding.free_rola_daemon_path_move_outcome(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(one);
            Marshal.FreeCoTaskMem(two);
        }
    }

    /// <summary>Runs the create-remote-entry action.</summary>
    /// <param name="path">The path to bind.</param>
    /// <param name="uuid">The entry's Uuid.</param>
    /// <param name="version">The version it holds.</param>
    /// <returns>What became of the write.</returns>
    public DaemonEntryWrite CreateRemoteEntry(string path, string uuid, string version)
    {
        var target = Utf8(_target);
        var one = Utf8(path);
        var two = Utf8(uuid);
        var three = Utf8(version);
        try
        {
            return SingleWrite(
                RorolalaBinding.rola_daemon_action_create_remote_entry(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    one,
                    two,
                    three
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(one);
            Marshal.FreeCoTaskMem(two);
            Marshal.FreeCoTaskMem(three);
        }
    }

    /// <summary>Runs the set-remote-version action.</summary>
    /// <param name="uuid">The entry's Uuid.</param>
    /// <param name="version">The version it is to hold.</param>
    /// <returns>What became of the write.</returns>
    public DaemonEntryWrite SetRemoteVersion(string uuid, string version)
    {
        var target = Utf8(_target);
        var one = Utf8(uuid);
        var two = Utf8(version);
        try
        {
            return SingleWrite(
                RorolalaBinding.rola_daemon_action_set_remote_version(
                    _workspace.Handle,
                    _account.Handle,
                    target,
                    one,
                    two
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(one);
            Marshal.FreeCoTaskMem(two);
        }
    }

    private IReadOnlyList<DaemonOwnership> Ownership(
        string name,
        IEnumerable<string> uuids,
        bool giveup,
        Func<nint, nint, nint, nint, RorolalaResult> action
    )
    {
        var target = Utf8(_target);
        var label = Utf8(name);
        var ids = uuids.ToArray();
        var strings = new List<nint>(ids.Length);
        var array =
            ids.Length == 0 ? 0 : Marshal.AllocCoTaskMem(IntPtr.Size * ids.Length);

        try
        {
            for (var index = 0; index < ids.Length; index++)
            {
                var text = Utf8(ids[index]);
                strings.Add(text);
                Marshal.WriteIntPtr(array, index * IntPtr.Size, text);
            }

            var span = new RolaStrSpan
            {
                items = array,
                count = (nuint)ids.Length,
            };
            var request = giveup
                ? Pinned(
                    new RolaDaemonGiveupOwnershipRequest { name = label, uuids = span }
                )
                : Pinned(
                    new RolaDaemonRequestOwnershipRequest { name = label, uuids = span }
                );

            try
            {
                var handle = AnswerHandle(
                    action(_workspace.Handle, _account.Handle, target, request)
                );

                try
                {
                    return ReadOwnerships(handle);
                }
                finally
                {
                    RorolalaBinding.free_rola_daemon_ownerships(handle);
                }
            }
            finally
            {
                Marshal.FreeCoTaskMem(request);
            }
        }
        finally
        {
            if (array != 0)
            {
                Marshal.FreeCoTaskMem(array);
            }

            foreach (var text in strings)
            {
                Marshal.FreeCoTaskMem(text);
            }

            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(label);
        }
    }

    private static IReadOnlyList<DaemonOwnership> ReadOwnerships(nint handle)
    {
        var count = RorolalaBinding.rola_daemon_ownerships_count(handle);
        var outcomes = new List<DaemonOwnership>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_daemon_ownerships_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                outcomes.Add(
                    new DaemonOwnership(
                        OwnershipKind(RorolalaBinding.rola_daemon_ownership_outcome_kind(item)),
                        NativeText.Taken(RorolalaBinding.rola_daemon_ownership_outcome_owner(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_daemon_ownership_outcome(item);
            }
        }

        return outcomes;
    }

    private static IReadOnlyList<DaemonEntryWrite> ReadEntryWrites(nint handle)
    {
        var count = RorolalaBinding.rola_daemon_entry_writes_count(handle);
        var outcomes = new List<DaemonEntryWrite>((int)count);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_daemon_entry_writes_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                outcomes.Add(
                    new DaemonEntryWrite(
                        EntryWriteKind(RorolalaBinding.rola_daemon_entry_write_outcome_kind(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_daemon_entry_write_outcome(item);
            }
        }

        return outcomes;
    }

    private static DaemonEntryWrite SingleWrite(RorolalaResult answer)
    {
        var handle = AnswerHandle(answer);

        try
        {
            return new DaemonEntryWrite(
                EntryWriteKind(RorolalaBinding.rola_daemon_entry_write_outcome_kind(handle))
            );
        }
        finally
        {
            RorolalaBinding.free_rola_daemon_entry_write_outcome(handle);
        }
    }

    private static DaemonOwnershipKind OwnershipKind(RolaDaemonOwnershipKind kind) =>
        kind switch
        {
            RolaDaemonOwnershipKind.RolaDaemonOwnershipKind_Owner => DaemonOwnershipKind.Owner,
            RolaDaemonOwnershipKind.RolaDaemonOwnershipKind_Missing =>
                DaemonOwnershipKind.Missing,
            RolaDaemonOwnershipKind.RolaDaemonOwnershipKind_HeldBy => DaemonOwnershipKind.HeldBy,
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };

    private static DaemonEntryWriteKind EntryWriteKind(RolaDaemonEntryWriteKind kind) =>
        kind switch
        {
            RolaDaemonEntryWriteKind.RolaDaemonEntryWriteKind_Written =>
                DaemonEntryWriteKind.Written,
            RolaDaemonEntryWriteKind.RolaDaemonEntryWriteKind_Exists => DaemonEntryWriteKind.Exists,
            RolaDaemonEntryWriteKind.RolaDaemonEntryWriteKind_Taken => DaemonEntryWriteKind.Taken,
            RolaDaemonEntryWriteKind.RolaDaemonEntryWriteKind_Missing =>
                DaemonEntryWriteKind.Missing,
            RolaDaemonEntryWriteKind.RolaDaemonEntryWriteKind_Refused =>
                DaemonEntryWriteKind.Refused,
            RolaDaemonEntryWriteKind.RolaDaemonEntryWriteKind_Malformed =>
                DaemonEntryWriteKind.Malformed,
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };

    private static DaemonChainKind ChainKind(RolaDaemonChainKind kind) =>
        kind switch
        {
            RolaDaemonChainKind.RolaDaemonChainKind_Brought => DaemonChainKind.Brought,
            RolaDaemonChainKind.RolaDaemonChainKind_NotAversion => DaemonChainKind.NotAVersion,
            RolaDaemonChainKind.RolaDaemonChainKind_NotAvariant => DaemonChainKind.NotAVariant,
            RolaDaemonChainKind.RolaDaemonChainKind_Missing => DaemonChainKind.Missing,
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };

    private static DaemonRemoveKind RemoveKind(RolaDaemonEntryRemoveKind kind) =>
        kind switch
        {
            RolaDaemonEntryRemoveKind.RolaDaemonEntryRemoveKind_Removed => DaemonRemoveKind.Removed,
            RolaDaemonEntryRemoveKind.RolaDaemonEntryRemoveKind_Missing => DaemonRemoveKind.Missing,
            RolaDaemonEntryRemoveKind.RolaDaemonEntryRemoveKind_Refused => DaemonRemoveKind.Refused,
            RolaDaemonEntryRemoveKind.RolaDaemonEntryRemoveKind_Malformed =>
                DaemonRemoveKind.Malformed,
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };

    private static DaemonPathMove PathMoveKind(RolaDaemonPathMoveKind kind) =>
        kind switch
        {
            RolaDaemonPathMoveKind.RolaDaemonPathMoveKind_Moved => DaemonPathMove.Moved,
            RolaDaemonPathMoveKind.RolaDaemonPathMoveKind_Missing => DaemonPathMove.Missing,
            RolaDaemonPathMoveKind.RolaDaemonPathMoveKind_Taken => DaemonPathMove.Taken,
            RolaDaemonPathMoveKind.RolaDaemonPathMoveKind_Malformed => DaemonPathMove.Malformed,
            _ => throw new ArgumentOutOfRangeException(nameof(kind)),
        };

    private static string? AnswerText(RorolalaResult answer)
    {
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.Action(answer.payload);
        }

        var text = NativeText.Taken(answer.payload);

        return text.Length == 0 ? null : text;
    }

    private static void AnswerVoid(RorolalaResult answer)
    {
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.Action(answer.payload);
        }
    }

    private static nint AnswerHandle(RorolalaResult answer)
    {
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.Action(answer.payload);
        }

        return answer.payload;
    }

    private static nint Pinned<T>(T value)
        where T : struct
    {
        var pointer = Marshal.AllocCoTaskMem(Marshal.SizeOf<T>());
        Marshal.StructureToPtr(value, pointer, false);

        return pointer;
    }

    private static nint Records(int count, int size, out int recordSize)
    {
        recordSize = size;

        return count == 0 ? 0 : Marshal.AllocCoTaskMem(count * size);
    }

    private static nint Remember(List<nint> strings, string text)
    {
        var pointer = Utf8(text);
        strings.Add(pointer);

        return pointer;
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

/// <summary>What became of one ownership request.</summary>
/// <param name="Kind">Which outcome it was.</param>
/// <param name="Owner">The account the outcome names, empty when it names none.</param>
public sealed record DaemonOwnership(DaemonOwnershipKind Kind, string Owner);

/// <summary>Which outcome an ownership request reached.</summary>
public enum DaemonOwnershipKind
{
    /// <summary>The entry is now held by the account, or by nobody when it is empty.</summary>
    Owner,

    /// <summary>The Vault holds no entry by that Uuid.</summary>
    Missing,

    /// <summary>Another account holds it.</summary>
    HeldBy,
}

/// <summary>What became of one write.</summary>
/// <param name="Kind">Which outcome it was.</param>
public sealed record DaemonEntryWrite(DaemonEntryWriteKind Kind);

/// <summary>Which outcome a write reached.</summary>
public enum DaemonEntryWriteKind
{
    /// <summary>The Vault's Layout now holds what was asked for.</summary>
    Written,

    /// <summary>The Vault already holds an entry by that Uuid.</summary>
    Exists,

    /// <summary>The Vault already names something at that path.</summary>
    Taken,

    /// <summary>The Vault holds no entry by that Uuid.</summary>
    Missing,

    /// <summary>The account may not write it.</summary>
    Refused,

    /// <summary>A path, a Uuid or a version could not be read.</summary>
    Malformed,
}

/// <summary>What became of a chain the Vault was asked for.</summary>
/// <param name="Kind">Which outcome the walk reached.</param>
/// <param name="Key">The hash the walk stopped at, empty when the outcome names none.</param>
public sealed record DaemonChain(DaemonChainKind Kind, string Key);

/// <summary>Which outcome a walk of a chain reached.</summary>
public enum DaemonChainKind
{
    /// <summary>The chain was brought, and is written into the index.</summary>
    Brought,

    /// <summary>The Vault holds an object under the hash that is not a version.</summary>
    NotAVersion,

    /// <summary>The object under a hash is not the variant the version above it names.</summary>
    NotAVariant,

    /// <summary>The Vault holds nothing under a hash the chain needs.</summary>
    Missing,
}

/// <summary>What became of a removal.</summary>
/// <param name="Kind">Which outcome it was.</param>
/// <param name="Owner">The account that holds the entry, empty when nobody does.</param>
public sealed record DaemonRemove(DaemonRemoveKind Kind, string Owner);

/// <summary>Which outcome a removal reached.</summary>
public enum DaemonRemoveKind
{
    /// <summary>The Vault's Layout no longer holds the entry.</summary>
    Removed,

    /// <summary>The Vault's Layout names nothing at that path.</summary>
    Missing,

    /// <summary>The account may not remove it.</summary>
    Refused,

    /// <summary>The path is not one a remote Layout may name.</summary>
    Malformed,
}

/// <summary>Which outcome a move reached.</summary>
public enum DaemonPathMove
{
    /// <summary>The entry is now at the path it was moved to.</summary>
    Moved,

    /// <summary>The Vault's Layout names nothing at the path that was to move.</summary>
    Missing,

    /// <summary>The path it was to move to already names something.</summary>
    Taken,

    /// <summary>A path is not one a remote Layout may name.</summary>
    Malformed,
}

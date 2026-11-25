using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>Changes one Layout, one batch at a time.</summary>
/// <remarks>
/// These are the low-level changes: a path is moved, an entry is inserted, and the caller owns the
/// order. The checks that a workflow makes, ownership and the version a path may move to, are in
/// <see cref="WorkspaceWriter"/>'s workflows rather than here.
/// </remarks>
public sealed class LayoutWriter : Writer
{
    private readonly LayoutScope _layout;

    /// <summary>Makes a writer over a Layout a caller has named.</summary>
    /// <param name="layout">The Layout.</param>
    public LayoutWriter(LayoutScope layout) =>
        _layout = layout ?? throw new ArgumentNullException(nameof(layout));

    /// <summary>Changes everything an entry holds, in one write.</summary>
    /// <param name="id">The entry's id.</param>
    /// <param name="version">The version key.</param>
    /// <param name="owner">The account, or nothing.</param>
    /// <param name="description">What it says about itself.</param>
    /// <param name="preference">The cut it prefers, or nothing.</param>
    public void SetAll(
        Guid id,
        string version,
        string? owner,
        string description,
        string? preference
    )
    {
        ArgumentNullException.ThrowIfNull(version);
        ArgumentNullException.ThrowIfNull(description);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var uuid = Utf8(id.ToString());
        var at = Utf8(version);
        var held = Utf8(owner ?? string.Empty);
        var words = Utf8(description);
        var cut = Utf8(preference ?? string.Empty);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_set_all(
                    directory,
                    layout,
                    uuid,
                    at,
                    held,
                    words,
                    cut
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(uuid);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(held);
            Marshal.FreeCoTaskMem(words);
            Marshal.FreeCoTaskMem(cut);
        }
    }

    /// <summary>Changes the version one entry names.</summary>
    /// <param name="id">The entry's id.</param>
    /// <param name="version">The version key.</param>
    public void SetVersion(Guid id, string version)
    {
        ArgumentNullException.ThrowIfNull(version);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var uuid = Utf8(id.ToString());
        var at = Utf8(version);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_set_version(
                    directory,
                    layout,
                    uuid,
                    at
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(uuid);
            Marshal.FreeCoTaskMem(at);
        }
    }

    /// <summary>Changes the cut one entry prefers.</summary>
    /// <param name="id">The entry's id.</param>
    /// <param name="preference">The chunking policy, or nothing to clear it.</param>
    public void SetPreference(Guid id, string? preference)
    {
        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var uuid = Utf8(id.ToString());
        var cut = Utf8(preference ?? string.Empty);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_set_preference(
                    directory,
                    layout,
                    uuid,
                    cut
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(uuid);
            Marshal.FreeCoTaskMem(cut);
        }
    }

    /// <summary>Changes what one entry says about itself.</summary>
    /// <param name="id">The entry's id.</param>
    /// <param name="description">What it is to say.</param>
    public void SetDescription(Guid id, string description)
    {
        ArgumentNullException.ThrowIfNull(description);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var uuid = Utf8(id.ToString());
        var words = Utf8(description);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_set_description(
                    directory,
                    layout,
                    uuid,
                    words
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(uuid);
            Marshal.FreeCoTaskMem(words);
        }
    }

    /// <summary>Changes the account that holds one entry.</summary>
    /// <param name="id">The entry's id.</param>
    /// <param name="owner">The account, or nothing to clear the owner.</param>
    public void SetOwner(Guid id, string? owner)
    {
        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var uuid = Utf8(id.ToString());
        var held = Utf8(owner ?? string.Empty);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_set_owner(directory, layout, uuid, held)
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(uuid);
            Marshal.FreeCoTaskMem(held);
        }
    }

    /// <summary>Applies a batch of changes to the Layout this writer works over.</summary>
    /// <param name="ops">The changes, in the order they are to be applied.</param>
    public void Apply(IReadOnlyList<LayoutChange> ops)
    {
        ArgumentNullException.ThrowIfNull(ops);

        using var memory = new NativeRequest();
        var built = memory.Array(ops.Select(change => Native(memory, change)).ToList());
        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        try
        {
            var answer = RorolalaBinding.rola_write_layout_apply(
                directory,
                layout,
                built.Items,
                built.Count
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
        }
    }

    /// <summary>One change as the native side reads it.</summary>
    /// <param name="memory">Where the change's strings are held.</param>
    /// <param name="change">The change.</param>
    /// <returns>The native change.</returns>
    private static RolaWriteLayoutOp Native(NativeRequest memory, LayoutChange change)
    {
        ArgumentNullException.ThrowIfNull(change);

        return change.Kind switch
        {
            LayoutChangeKind.CreateEntry => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_CreateEntry,
                payload = new RolaWriteLayoutOpPayload
                {
                    CreateEntry = new RolaWriteLayoutOpCreateEntry
                    {
                        uuid = memory.Text(change.Uuid),
                        path = memory.Text(change.Path),
                        version = memory.Text(change.Version),
                        owner = memory.Text(change.Owner),
                        description = memory.Text(change.Description),
                        kind = memory.Text(change.EntryKind),
                    },
                },
            },
            LayoutChangeKind.UpdateEntry => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_UpdateEntry,
                payload = new RolaWriteLayoutOpPayload
                {
                    UpdateEntry = new RolaWriteLayoutOpUpdateEntry
                    {
                        uuid = memory.Text(change.Uuid),
                        version = memory.Text(change.Version),
                        owner = memory.Text(change.Owner),
                        description = memory.Text(change.Description),
                    },
                },
            },
            LayoutChangeKind.RemoveEntry => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_RemoveEntry,
                payload = new RolaWriteLayoutOpPayload
                {
                    RemoveEntry = new RolaWriteLayoutOpRemoveEntry
                    {
                        uuid = memory.Text(change.Uuid),
                    },
                },
            },
            LayoutChangeKind.CreatePath => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_CreatePath,
                payload = new RolaWriteLayoutOpPayload
                {
                    CreatePath = new RolaWriteLayoutOpCreatePath
                    {
                        path = memory.Text(change.Path),
                        uuid = memory.Text(change.Uuid),
                    },
                },
            },
            LayoutChangeKind.MovePath => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_MovePath,
                payload = new RolaWriteLayoutOpPayload
                {
                    MovePath = new RolaWriteLayoutOpMovePath
                    {
                        from = memory.Text(change.From),
                        to = memory.Text(change.To),
                    },
                },
            },
            LayoutChangeKind.RemovePath => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_RemovePath,
                payload = new RolaWriteLayoutOpPayload
                {
                    RemovePath = new RolaWriteLayoutOpRemovePath
                    {
                        path = memory.Text(change.Path),
                    },
                },
            },
            LayoutChangeKind.SoftDeletePath => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_SoftDeletePath,
                payload = new RolaWriteLayoutOpPayload
                {
                    SoftDeletePath = new RolaWriteLayoutOpSoftDeletePath
                    {
                        path = memory.Text(change.Path),
                    },
                },
            },
            LayoutChangeKind.ChangeOwnership => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_ChangeOwnership,
                payload = new RolaWriteLayoutOpPayload
                {
                    ChangeOwnership = new RolaWriteLayoutOpChangeOwnership
                    {
                        uuid = memory.Text(change.Uuid),
                        owner = memory.Text(change.Owner),
                    },
                },
            },
            _ => new RolaWriteLayoutOp
            {
                tag = RolaWriteLayoutOpTag.RolaWriteLayoutOp_ChangeUuid,
                payload = new RolaWriteLayoutOpPayload
                {
                    ChangeUuid = new RolaWriteLayoutOpChangeUuid
                    {
                        from = memory.Text(change.From),
                        to = memory.Text(change.To),
                    },
                },
            },
        };
    }

    /// <summary>Moves the entry at one path to another.</summary>
    /// <param name="from">The path the entry is at.</param>
    /// <param name="to">The path it is to be at.</param>
    public void MovePath(string from, string to)
    {
        ArgumentNullException.ThrowIfNull(from);
        ArgumentNullException.ThrowIfNull(to);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var fromPath = Utf8(from);
        var toPath = Utf8(to);
        try
        {
            Check(RorolalaBinding.rola_write_layout_move_path(directory, layout, fromPath, toPath));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(fromPath);
            Marshal.FreeCoTaskMem(toPath);
        }
    }

    /// <summary>Removes a path, leaving its entry.</summary>
    /// <param name="path">The path to remove.</param>
    public void RemovePath(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var target = Utf8(path);
        try
        {
            Check(RorolalaBinding.rola_write_layout_remove_path(directory, layout, target));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(target);
        }
    }

    /// <summary>Moves a path under the archive, leaving its entry.</summary>
    /// <remarks>
    /// It is the unbinding <see cref="RemovePath"/> is not: the path is moved to
    /// <c>@/removed/&lt;date&gt;/</c> and the entry stays where a reader can find it. The date is the
    /// day the call is made, in UTC, and the target is worked out by the native side, so a caller
    /// names the path it has.
    /// </remarks>
    /// <param name="path">The path to archive.</param>
    public void SoftDelete(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var target = Utf8(path);
        try
        {
            Check(RorolalaBinding.rola_write_layout_soft_delete_path(directory, layout, target));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(target);
        }
    }

    /// <summary>Moves a path in the fetched copy of a Vault's own Layout.</summary>
    /// <remarks>
    /// A copy is read, never worked in: the Vault is asked to make the move, and the copy follows
    /// only what it answered. The Layout must be named <c>truth@VAULT</c>, which is the one name a
    /// fetch leaves a copy under.
    /// </remarks>
    /// <param name="from">The path the entry is at.</param>
    /// <param name="to">The path it is to be at.</param>
    public void MoveFetched(string from, string to)
    {
        ArgumentNullException.ThrowIfNull(from);
        ArgumentNullException.ThrowIfNull(to);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var fromPath = Utf8(from);
        var toPath = Utf8(to);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_move_fetched(
                    directory,
                    layout,
                    fromPath,
                    toPath
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(fromPath);
            Marshal.FreeCoTaskMem(toPath);
        }
    }

    /// <summary>Moves a path of the fetched copy under the archive, by asking the Vault to.</summary>
    /// <remarks>
    /// The Vault is asked to move the path, not to let the entry go, and the copy follows only what
    /// it answered. The target, <c>@/removed/&lt;date&gt;/</c> dated in UTC, is worked out by the
    /// native side. The Layout must be named <c>truth@VAULT</c>.
    /// </remarks>
    /// <param name="path">The path the entry is at now.</param>
    public void SoftDeleteFetched(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var target = Utf8(path);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_soft_delete_fetched(directory, layout, target)
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(target);
        }
    }

    /// <summary>Removes an entry, path and all, from the fetched copy of a Vault's own Layout.</summary>
    /// <remarks>
    /// It is not the unbinding <see cref="RemovePath"/> does: the Vault is asked to let the entry go,
    /// and only the account that holds it, or an administrator of the Vault, may. The copy follows
    /// only what the Vault answered, and it is the copy that names the path the entry is at, so a
    /// copy that was never fetched here is refused rather than reached past.
    /// </remarks>
    /// <param name="uuid">The entry as the copy names it.</param>
    public void RemoveFetchedEntry(Guid uuid)
    {
        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var id = Utf8(uuid.ToString());
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_remove_fetched_entry(directory, layout, id)
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(id);
        }
    }

    /// <summary>Binds a path to an entry.</summary>
    /// <param name="path">The path to bind.</param>
    /// <param name="uuid">The entry it names.</param>
    public void CreatePath(string path, Guid uuid)
    {
        ArgumentNullException.ThrowIfNull(path);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var target = Utf8(path);
        var id = Utf8(uuid.ToString());
        try
        {
            Check(RorolalaBinding.rola_write_layout_create_path(directory, layout, target, id));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(id);
        }
    }

    /// <summary>Makes or changes what an entry holds.</summary>
    /// <param name="uuid">The entry's Uuid.</param>
    /// <param name="version">The version it holds, as a key.</param>
    /// <param name="owner">The account that holds it, or nothing.</param>
    /// <param name="description">What it says about itself, or nothing.</param>
    /// <param name="update">Whether the entry must already be there.</param>
    public void SetEntry(
        Guid uuid,
        string version,
        string? owner = null,
        string? description = null,
        bool update = false
    )
    {
        ArgumentNullException.ThrowIfNull(version);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var id = Utf8(uuid.ToString());
        var at = Utf8(version);
        var holder = Utf8(owner ?? string.Empty);
        var words = Utf8(description ?? string.Empty);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_write_entry(
                    directory,
                    layout,
                    id,
                    at,
                    holder,
                    words,
                    update
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(id);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(holder);
            Marshal.FreeCoTaskMem(words);
        }
    }

    /// <summary>Drops an entry, and the path that named it.</summary>
    /// <param name="uuid">The entry's Uuid.</param>
    public void RemoveEntry(Guid uuid)
    {
        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var id = Utf8(uuid.ToString());
        try
        {
            Check(RorolalaBinding.rola_write_layout_remove_entry(directory, layout, id));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(id);
        }
    }

    /// <summary>Makes an entry, and binds a path to it.</summary>
    /// <param name="uuid">The Uuid the entry is known by.</param>
    /// <param name="path">The path to bind.</param>
    /// <param name="version">The version it is at, as the hex a key is written by.</param>
    /// <param name="owner">The account that holds it, or nothing.</param>
    /// <param name="description">What it says about itself.</param>
    /// <param name="tree">Whether it names a tree rather than a file.</param>
    public void InsertEntry(
        Guid uuid,
        string path,
        string version,
        string? owner,
        string description,
        bool tree = false
    ) =>
        Apply(
            [
                new LayoutChange
                {
                    Kind = LayoutChangeKind.CreateEntry,
                    Uuid = uuid.ToString(),
                    Path = path,
                    Version = version,
                    Owner = owner ?? string.Empty,
                    Description = description,
                    EntryKind = tree ? "tree" : "file",
                },
            ]
        );

    /// <summary>Changes who holds an entry, keeping what it holds.</summary>
    /// <param name="uuid">The entry.</param>
    /// <param name="owner">The account that now holds it, or nothing.</param>
    public void ChangeOwnership(Guid uuid, string? owner) =>
        Apply(
            [
                new LayoutChange
                {
                    Kind = LayoutChangeKind.ChangeOwnership,
                    Uuid = uuid.ToString(),
                    Owner = owner ?? string.Empty,
                },
            ]
        );

    /// <summary>Moves an entry to a new Uuid.</summary>
    /// <param name="from">The Uuid it has.</param>
    /// <param name="to">The Uuid it is to have.</param>
    public void ChangeUuid(Guid from, Guid to) =>
        Apply(
            [
                new LayoutChange
                {
                    Kind = LayoutChangeKind.ChangeUuid,
                    From = from.ToString(),
                    To = to.ToString(),
                },
            ]
        );

    /// <summary>Gives the entry at a path a new Uuid, holding what it held.</summary>
    /// <remarks>
    /// The entry the path named is dropped and one made in its place, in one write, so a reader
    /// sees the path naming one of them and never neither. It is a fork rather than a
    /// <see cref="ChangeUuid"/>: the old entry goes, path and all, and the new one is what the path
    /// names from then on.
    /// </remarks>
    /// <param name="path">The path whose entry is to be renewed.</param>
    /// <param name="uuid">The Uuid the entry is to have.</param>
    /// <param name="version">The version it holds, as a key.</param>
    /// <param name="owner">The account that holds it, or nothing.</param>
    /// <param name="description">What it says about itself.</param>
    /// <param name="preference">The chunking policy it prefers, or nothing.</param>
    public void SwitchEntry(
        string path,
        Guid uuid,
        string version,
        string? owner,
        string description,
        string? preference
    )
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(version);
        ArgumentNullException.ThrowIfNull(description);

        var directory = Utf8(_layout.Root.FullName);
        var layout = Utf8(_layout.Name ?? string.Empty);
        var target = Utf8(path);
        var id = Utf8(uuid.ToString());
        var at = Utf8(version);
        var held = Utf8(owner ?? string.Empty);
        var words = Utf8(description);
        var preferred = Utf8(preference ?? string.Empty);
        try
        {
            Check(
                RorolalaBinding.rola_write_layout_switch_entry(
                    directory,
                    layout,
                    target,
                    id,
                    at,
                    held,
                    words,
                    preferred
                )
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(layout);
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(id);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(held);
            Marshal.FreeCoTaskMem(words);
            Marshal.FreeCoTaskMem(preferred);
        }
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

using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>A Workspace: the directory a run works in, and everything under it.</summary>
public sealed class WorkspaceScope
    : Scope,
        IDisposable,
        IVCSIndexAccessable,
        IStorageAccessable,
        ILayoutAccessable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the Workspace holding a directory.</summary>
    /// <param name="path">Any directory inside the Workspace.</param>
    public WorkspaceScope(string path) => _root = Path.GetFullPath(path);

    /// <summary>Makes a scope over the Workspace holding a directory.</summary>
    /// <param name="directory">Any directory inside the Workspace.</param>
    public WorkspaceScope(DirectoryInfo directory)
        : this(directory?.FullName ?? throw new ArgumentNullException(nameof(directory))) { }

    /// <summary>Finds the Workspace a path is in and makes a scope over its root.</summary>
    /// <param name="start">Any path inside the Workspace.</param>
    /// <returns>The scope over the Workspace.</returns>
    public static WorkspaceScope Open(string start)
    {
        var at = new DirectoryInfo(Path.GetFullPath(start));
        while (at is not null)
        {
            if (Directory.Exists(RolaPaths.WorkspaceData(at.FullName)))
            {
                return new WorkspaceScope(at.FullName);
            }

            at = at.Parent;
        }

        throw new RolaReadError(RolaReadErrorKind.Unavailable, start, "no Workspace is there");
    }

    /// <summary>Finds the Workspace a path is in and makes a scope over its root.</summary>
    /// <param name="start">Any directory inside the Workspace.</param>
    /// <returns>The scope over the Workspace.</returns>
    public static WorkspaceScope Open(DirectoryInfo start) =>
        Open(start?.FullName ?? throw new ArgumentNullException(nameof(start)));

    /// <summary>The Workspace's root.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>Where the Workspace keeps the data that makes it one.</summary>
    public DirectoryInfo Data => new(RolaPaths.WorkspaceData(_root));

    /// <summary>The Workspace configuration file.</summary>
    public FileInfo ConfigFile => new(RolaPaths.WorkspaceConfig(_root));

    /// <summary>Where the Workspace keeps its Layouts.</summary>
    public DirectoryInfo LayoutDirectory => new(RolaPaths.WorkspaceLayout(_root));

    /// <summary>Where the Workspace keeps its store.</summary>
    public DirectoryInfo StorageDirectory => new(RolaPaths.WorkspaceStorage(_root));

    /// <summary>Where the Workspace keeps its VCS index.</summary>
    public DirectoryInfo IndexDirectory => new(RolaPaths.WorkspaceIndex(_root));

    /// <summary>Where the Workspace keeps the accounts it knows.</summary>
    public DirectoryInfo KeysDirectory => new(RolaPaths.WorkspaceKeys(_root));

    /// <summary>Where the Workspace keeps its reading cache.</summary>
    public DirectoryInfo CacheDirectory => new(RolaPaths.WorkspaceCache(_root));

    /// <summary>Where the Workspace keeps fetched Layout copies.</summary>
    public DirectoryInfo ReadonlyLayoutsDirectory => new(RolaPaths.WorkspaceReadonlyLayouts(_root));

    /// <summary>The file a run edits to write the next message.</summary>
    public FileInfo EditingFile => new(RolaPaths.WorkspaceEditing(_root));

    /// <summary>The native Workspace, for the writes that hold it.</summary>
    internal nint Handle
    {
        get
        {
            if (_handle == 0)
            {
                throw new InvalidOperationException("call Read first");
            }

            return _handle;
        }
    }

    /// <summary>Whether the Workspace is locked.</summary>
    public bool Locked
    {
        get
        {
            if (_handle == 0)
            {
                throw new InvalidOperationException("call Read first");
            }

            return RorolalaBinding.rola_workspace_is_locking(_handle);
        }
    }

    /// <summary>What the Workspace's configuration says, once it is read.</summary>
    public WorkspaceConfig? Config { get; private set; }

    /// <summary>The Layout the Workspace works in.</summary>
    public LocalLayoutScope LocalLayout => new(_root);

    /// <summary>The Layout the Workspace works in.</summary>
    public LayoutScope Layout => LocalLayout;

    /// <summary>The copy of a Vault's truth Layout a fetch left here.</summary>
    /// <param name="vault">The Vault whose copy is read.</param>
    /// <returns>The scope over the copy.</returns>
    public TruthLayoutScope TruthLayout(string vault)
    {
        ArgumentNullException.ThrowIfNull(vault);
        if (
            vault.Length == 0
            || vault is "." or ".."
            || vault.Trim() != vault
            || vault.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0
        )
        {
            throw new RolaReadError(RolaReadErrorKind.InvalidArgument, vault, "the Vault name is not one path component");
        }

        return new TruthLayoutScope(_root, vault);
    }

    /// <summary>The stores the Workspace works on.</summary>
    public StorageScope Storage => new(StorageTarget, _root);

    /// <summary>The store a write goes to: the first binding that is there and can be written.</summary>
    private string StorageTarget =>
        Config?.Storages.FirstOrDefault(binding => binding.Present && binding.Writable)?.Root
        ?? RolaPaths.WorkspaceStorage(_root);

    /// <summary>The VCS index the Workspace keeps.</summary>
    public VCSIndexScope VCSIndex => new(_root);

    /// <summary>The inverse index the Workspace keeps.</summary>
    public InverseIndexScope InverseIndex => new(_root);

    /// <summary>The ignore rules the Workspace is managed by.</summary>
    public IgnoreScope Ignore =>
        new(RolaPaths.WorkspaceIgnoreRules(_root), _root);

    /// <summary>The tree the Workspace holds.</summary>
    public TreeScope Tree => new(_root);

    /// <summary>A tree at a path under the Workspace.</summary>
    /// <param name="path">The tree's path, relative to the Workspace root.</param>
    /// <returns>The scope over the tree.</returns>
    public TreeScope TreeAt(string path) => new(Subtree(path));

    /// <summary>A path under the Workspace, refusing one that leaves it.</summary>
    private string Subtree(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var root = Path.GetFullPath(_root);
        var at = Path.GetFullPath(Path.Combine(root, path));
        var relative = Path.GetRelativePath(root, at);
        if (relative == ".." || relative.StartsWith($"..{Path.DirectorySeparatorChar}", StringComparison.Ordinal))
        {
            throw new RolaReadError(RolaReadErrorKind.InvalidArgument, path, "the path leaves the Workspace");
        }

        return at;
    }

    /// <summary>The built-in book, looked for from the Workspace.</summary>
    public BookScope Book => new(_root);

    /// <summary>What holds each entry of the tracked Vault, from the fetched copy.</summary>
    public OwnershipScope Ownership => new(_root);

    /// <summary>The graph around one version.</summary>
    /// <param name="target">The version to draw around.</param>
    /// <returns>The scope over the graph.</returns>
    public VersionGraphScope VersionGraph(string target) => new(_root, target);

    /// <summary>The variant graph the Workspace's entries draw.</summary>
    /// <param name="request">The window to draw, or nothing for the default one.</param>
    /// <returns>The scope over the graph.</returns>
    public VariantGraphScope VariantGraph(VariantGraphRequest? request = null) =>
        new(_root, request);

    /// <summary>How the Workspace's tree stands beside its Layout.</summary>
    public StatusScope Status => new(_root);

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root) || !Directory.Exists(Path.Combine(_root, ".rola")))
        {
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _root,
                "no Workspace holds the directory"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();

        var path = Utf8(_root);
        var handle = RorolalaBinding.rola_workspace_locate(path);
        if (handle == 0)
        {
            Marshal.FreeCoTaskMem(path);
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _root,
                "no Workspace holds the directory"
            );
        }

        _handle = handle;
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            var config = RorolalaBinding.rola_workspace_config_read(path);
            var storages = RorolalaBinding.rola_workspace_storages_new(path);
            try
            {
                Config = config == 0 ? null : ReadConfig(config, storages);
            }
            finally
            {
                if (config != 0)
                {
                    RorolalaBinding.free_rola_workspace_config(config);
                }

                if (storages != 0)
                {
                    RorolalaBinding.free_rola_storage_bindings(storages);
                }
            }
        }
        catch
        {
            Dispose();
            throw;
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Gives the native Workspace back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_workspace(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static WorkspaceConfig ReadConfig(nint config, nint storages)
    {
        var count = RorolalaBinding.rola_workspace_config_vault_count(config);
        var vaults = new List<VaultBinding>();
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_workspace_config_vault_at(config, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                vaults.Add(
                    new VaultBinding(
                        NativeText.Taken(RorolalaBinding.rola_workspace_vault_name(item)),
                        NativeText.Taken(RorolalaBinding.rola_workspace_vault_address(item)),
                        RorolalaBinding.rola_workspace_vault_is_default(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_workspace_vault(item);
            }
        }

        var defaultVault = NativeText.Taken(
            RorolalaBinding.rola_workspace_config_default_vault(config)
        );

        GcConfig? gc = null;
        if (RorolalaBinding.rola_workspace_config_gc_present(config))
        {
            var grace = RorolalaBinding.rola_workspace_config_gc_grace(config);
            gc = new GcConfig(grace < 0 ? null : checked((ulong)grace));
        }

        return new WorkspaceConfig(
            vaults,
            string.IsNullOrEmpty(defaultVault) ? null : defaultVault,
            gc,
            ReadStorages(storages)
        );
    }

    private static IReadOnlyList<StorageBinding> ReadStorages(nint handle)
    {
        var storages = new List<StorageBinding>();
        if (handle == 0)
        {
            return storages;
        }

        var count = RorolalaBinding.rola_storage_bindings_count(handle);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_storage_bindings_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                var reason = NativeText.Taken(RorolalaBinding.rola_storage_binding_reason(item));
                storages.Add(
                    new StorageBinding(
                        NativeText.Taken(RorolalaBinding.rola_storage_binding_path(item)),
                        NativeText.Taken(RorolalaBinding.rola_storage_binding_root(item)),
                        RorolalaBinding.rola_storage_binding_own(item),
                        RorolalaBinding.rola_storage_binding_present(item),
                        RorolalaBinding.rola_storage_binding_writable(item),
                        string.IsNullOrEmpty(reason) ? null : reason
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_storage_binding(item);
            }
        }

        return storages;
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

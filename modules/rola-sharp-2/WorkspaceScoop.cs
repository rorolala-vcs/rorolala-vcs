using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp.V2;

/// <summary>A Workspace: the directory a run works in, and everything under it.</summary>
public sealed class WorkspaceScoop
    : Scoop,
        IDisposable,
        IVCSIndexAccessable,
        IStorageAccessable,
        ILayoutAccessable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the Workspace holding a directory.</summary>
    /// <param name="path">Any directory inside the Workspace.</param>
    public WorkspaceScoop(string path) => _root = Path.GetFullPath(path);

    /// <summary>Makes a scope over the Workspace holding a directory.</summary>
    /// <param name="directory">Any directory inside the Workspace.</param>
    public WorkspaceScoop(DirectoryInfo directory)
        : this(directory?.FullName ?? throw new ArgumentNullException(nameof(directory))) { }

    /// <summary>The Workspace's root.</summary>
    public DirectoryInfo Root => new(_root);

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

            return RorolalaBinding.RolaWorkspace_is_locking(_handle);
        }
    }

    /// <summary>What the Workspace's configuration says, once it is read.</summary>
    public WorkspaceConfig? Config { get; private set; }

    /// <summary>The Layout the Workspace works in.</summary>
    public LocalLayoutScoop LocalLayout => new(_root);

    /// <summary>The Layout the Workspace works in.</summary>
    public LayoutScoop Layout => LocalLayout;

    /// <summary>The copy of a Vault's truth Layout a fetch left here.</summary>
    /// <param name="vault">The Vault whose copy is read.</param>
    /// <returns>The scope over the copy.</returns>
    public TruthLayoutScoop TruthLayout(string vault) => new(_root, vault);

    /// <summary>The store the Workspace keeps.</summary>
    public StorageScoop Storage => new(RolaPaths.WorkspaceStorage(_root));

    /// <summary>The VCS index the Workspace keeps.</summary>
    public VCSIndexScoop VCSIndex => new(_root);

    /// <summary>The inverse index the Workspace keeps.</summary>
    public InverseIndexScoop InverseIndex => new(_root);

    /// <summary>The ignore rules the Workspace is managed by.</summary>
    public IgnoreScoop Ignore =>
        new(RolaPaths.WorkspaceIgnoreRules(_root), _root);

    /// <summary>The tree the Workspace holds.</summary>
    public TreeScoop Tree => new(_root);

    /// <summary>A tree at a path under the Workspace.</summary>
    /// <param name="path">The tree's path, relative to the Workspace root.</param>
    /// <returns>The scope over the tree.</returns>
    public TreeScoop TreeAt(string path) => new(Path.Combine(_root, path));

    /// <summary>The built-in book, looked for from the Workspace.</summary>
    public BookScoop Book => new(_root);

    /// <summary>What holds each entry of the tracked Vault, from the fetched copy.</summary>
    public OwnershipScoop Ownership => new(_root);

    /// <summary>The graph around one version.</summary>
    /// <param name="target">The version to draw around.</param>
    /// <returns>The scope over the graph.</returns>
    public VersionGraphScoop VersionGraph(string target) => new(_root, target);

    /// <summary>The variant graph the Workspace's entries draw.</summary>
    /// <param name="request">The window to draw, or nothing for the default one.</param>
    /// <returns>The scope over the graph.</returns>
    public VariantGraphScoop VariantGraph(VariantGraphRequest? request = null) =>
        new(_root, request);

    /// <summary>How the Workspace's tree stands beside its Layout.</summary>
    public StatusScoop Status => new(_root);

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
        var handle = RorolalaBinding.locate_rola_workspace(path);
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
            var json = NativeText.Taken(RorolalaBinding.rola_workspace_config_json(path));
            Config = ParseConfig(json);
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

    private static WorkspaceConfig? ParseConfig(string json)
    {
        if (string.IsNullOrEmpty(json) || json == "null")
        {
            return null;
        }

        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        var vaults = RolaJson
            .Items(root, "vaults")
            ?.EnumerateArray()
            .Select(item => new VaultBinding(
                RolaJson.Text(item, "name") ?? string.Empty,
                RolaJson.Text(item, "address") ?? string.Empty,
                RolaJson.Flag(item, "is_default")
            ))
            .ToArray()
            ?? [];
        var defaultVault = RolaJson.Text(root, "default");
        ulong? grace = null;
        if (
            root.ValueKind is JsonValueKind.Object
            && root.TryGetProperty("gc", out var gc)
            && gc.ValueKind is JsonValueKind.Object
        )
        {
            grace = RolaJson.Number(gc, "grace");
        }

        return new WorkspaceConfig(
            vaults,
            defaultVault,
            grace is null ? null : new GcConfig(grace)
        );
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

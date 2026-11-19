using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>A Vault: the directory a server-side Vault keeps, and everything under it.</summary>
public sealed class VaultScope
    : Scope,
        IDisposable,
        IVCSIndexAccessable,
        IStorageAccessable,
        ILayoutAccessable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the Vault holding a directory.</summary>
    /// <param name="path">Any directory inside the Vault.</param>
    public VaultScope(string path) => _root = Path.GetFullPath(path);

    /// <summary>Makes a scope over the Vault holding a directory.</summary>
    /// <param name="directory">Any directory inside the Vault.</param>
    public VaultScope(DirectoryInfo directory)
        : this(directory?.FullName ?? throw new ArgumentNullException(nameof(directory))) { }

    /// <summary>Finds the Vault a path is in and makes a scope over its root.</summary>
    /// <param name="start">Any path inside the Vault.</param>
    /// <returns>The scope over the Vault.</returns>
    public static VaultScope Open(string start)
    {
        var at = new DirectoryInfo(Path.GetFullPath(start));
        while (at is not null)
        {
            if (File.Exists(RolaPaths.VaultConfig(at.FullName)))
            {
                return new VaultScope(at.FullName);
            }

            at = at.Parent;
        }

        throw new RolaReadError(RolaReadErrorKind.Unavailable, start, "no Vault is there");
    }

    /// <summary>Finds the Vault a path is in and makes a scope over its root.</summary>
    /// <param name="start">Any directory inside the Vault.</param>
    /// <returns>The scope over the Vault.</returns>
    public static VaultScope Open(DirectoryInfo start) =>
        Open(start?.FullName ?? throw new ArgumentNullException(nameof(start)));

    /// <summary>The Vault's root.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>The Vault configuration file.</summary>
    public FileInfo ConfigFile => new(RolaPaths.VaultConfig(_root));

    /// <summary>Where the Vault keeps its store.</summary>
    public DirectoryInfo StorageDirectory => new(RolaPaths.VaultStorage(_root));

    /// <summary>Where the Vault keeps its VCS index.</summary>
    public DirectoryInfo IndexDirectory => new(RolaPaths.VaultIndex(_root));

    /// <summary>Where the Vault keeps its Layout.</summary>
    public DirectoryInfo LayoutDirectory => new(RolaPaths.VaultLayout(_root));

    /// <summary>Where the Vault keeps the accounts it knows.</summary>
    public DirectoryInfo KeysDirectory => new(RolaPaths.VaultKeys(_root));

    /// <summary>Where the Vault keeps the Vaults inside it.</summary>
    public DirectoryInfo VaultsDirectory => new(RolaPaths.VaultVaults(_root));

    /// <summary>The native Vault, for the writes that hold it.</summary>
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

    /// <summary>Whether the Vault is locked.</summary>
    public bool Locked
    {
        get
        {
            if (_handle == 0)
            {
                throw new InvalidOperationException("call Read first");
            }

            return RorolalaBinding.RolaVault_is_locking(_handle);
        }
    }

    /// <summary>What the Vault says about itself, once it is read.</summary>
    public VaultConfig? Config { get; private set; }

    /// <summary>The truth Layout the Vault keeps.</summary>
    public TruthLayoutScope TruthLayout => new(_root);

    /// <summary>The Layout the Vault works from.</summary>
    public LayoutScope Layout => TruthLayout;

    /// <summary>The store the Vault keeps.</summary>
    public StorageScope Storage => new(RolaPaths.VaultStorage(_root));

    /// <summary>The VCS index the Vault keeps.</summary>
    public VCSIndexScope VCSIndex => new(_root);

    /// <summary>The inverse index the Vault keeps.</summary>
    public InverseIndexScope InverseIndex => new(_root);

    /// <summary>The ignore rules the Vault hands down.</summary>
    public IgnoreScope Ignore => new(RolaPaths.VaultIgnoreRules(_root), _root);

    /// <summary>The tree the Vault holds.</summary>
    public TreeScope Tree => new(_root);

    /// <summary>A tree at a path under the Vault.</summary>
    /// <param name="path">The tree's path, relative to the Vault root.</param>
    /// <returns>The scope over the tree.</returns>
    public TreeScope TreeAt(string path) => new(Subtree(path));

    /// <summary>A path under the Vault, refusing one that leaves it.</summary>
    private string Subtree(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var root = Path.GetFullPath(_root);
        var at = Path.GetFullPath(Path.Combine(root, path));
        var relative = Path.GetRelativePath(root, at);
        if (relative == ".." || relative.StartsWith($"..{Path.DirectorySeparatorChar}", StringComparison.Ordinal))
        {
            throw new RolaReadError(RolaReadErrorKind.InvalidArgument, path, "the path leaves the Vault");
        }

        return at;
    }

    /// <summary>The graph around one version.</summary>
    /// <param name="target">The version to draw around.</param>
    /// <returns>The scope over the graph.</returns>
    public VersionGraphScope VersionGraph(string target) => new(_root, target);

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root) || !File.Exists(Path.Combine(_root, "vault.toml")))
        {
            throw new RolaReadError(RolaReadErrorKind.NoVault, _root, "no Vault holds the directory");
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();

        var path = Utf8(_root);
        var handle = RorolalaBinding.locate_rola_vault(path);
        if (handle == 0)
        {
            Marshal.FreeCoTaskMem(path);
            throw new RolaReadError(RolaReadErrorKind.NoVault, _root, "no Vault holds the directory");
        }

        _handle = handle;
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            var json = NativeText.Taken(RorolalaBinding.rola_vault_config_json(path));
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

    /// <summary>Gives the native Vault back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_vault(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static VaultConfig? ParseConfig(string json)
    {
        if (string.IsNullOrEmpty(json) || json == "null")
        {
            return null;
        }

        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        var admins = RolaJson
            .Items(root, "admins")
            ?.EnumerateArray()
            .Select(item => item.GetString() ?? string.Empty)
            .ToArray()
            ?? [];

        return new VaultConfig(
            RolaJson.Text(root, "name") ?? string.Empty,
            RolaJson.Text(root, "description") ?? string.Empty,
            admins,
            (ushort)(RolaJson.Number(root, "max_concurrency") ?? 0)
        );
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

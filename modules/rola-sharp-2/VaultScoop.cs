using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp.V2;

/// <summary>A Vault: the directory a server-side Vault keeps, and everything under it.</summary>
public sealed class VaultScoop
    : Scoop,
        IDisposable,
        IVCSIndexAccessable,
        IStorageAccessable,
        ILayoutAccessable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the Vault holding a directory.</summary>
    /// <param name="path">Any directory inside the Vault.</param>
    public VaultScoop(string path) => _root = Path.GetFullPath(path);

    /// <summary>Makes a scope over the Vault holding a directory.</summary>
    /// <param name="directory">Any directory inside the Vault.</param>
    public VaultScoop(DirectoryInfo directory)
        : this(directory?.FullName ?? throw new ArgumentNullException(nameof(directory))) { }

    /// <summary>The Vault's root.</summary>
    public DirectoryInfo Root => new(_root);

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
    public TruthLayoutScoop TruthLayout => new(_root);

    /// <summary>The Layout the Vault works from.</summary>
    public LayoutScoop Layout => TruthLayout;

    /// <summary>The store the Vault keeps.</summary>
    public StorageScoop Storage => new(RolaPaths.VaultStorage(_root));

    /// <summary>The VCS index the Vault keeps.</summary>
    public VCSIndexScoop VCSIndex => new(_root);

    /// <summary>The inverse index the Vault keeps.</summary>
    public InverseIndexScoop InverseIndex => new(_root);

    /// <summary>The graph around one version.</summary>
    /// <param name="target">The version to draw around.</param>
    /// <returns>The scope over the graph.</returns>
    public VersionGraphScoop VersionGraph(string target) => new(_root, target);

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

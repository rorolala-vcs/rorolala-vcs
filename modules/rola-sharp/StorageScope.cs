using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>The content-addressed store a Workspace or a Vault keeps.</summary>
public sealed class StorageScope : Scope
{
    private readonly string _root;
    private readonly string _scope_root;
    private IReadOnlyList<string> _keys = [];
    private IReadOnlyList<string> _manifests = [];

    /// <summary>Makes a scope over the store rooted at a Workspace or a Vault.</summary>
    /// <param name="root">The directory the store is rooted in.</param>
    public StorageScope(string root)
        : this(root, root) { }

    /// <summary>Makes a scope over the stores a place works on.</summary>
    /// <param name="root">The store a write goes to.</param>
    /// <param name="scopeRoot">The Workspace or Vault root the stores are read from.</param>
    public StorageScope(string root, string scopeRoot)
    {
        _root = Path.GetFullPath(root);
        _scope_root = Path.GetFullPath(scopeRoot);
    }

    /// <summary>The directory the store a write goes to is rooted in.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>The place whose stores this scope reads.</summary>
    public DirectoryInfo ScopeRoot => new(_scope_root);

    /// <summary>Every object key the store holds.</summary>
    public IReadOnlyList<string> Keys
    {
        get
        {
            EnsureLoaded();
            return _keys;
        }
    }

    /// <summary>Every content key the store keeps as a manifest.</summary>
    public IReadOnlyList<string> Manifests
    {
        get
        {
            EnsureLoaded();
            return _manifests;
        }
    }

    /// <summary>Reads the chunks a chunked object is put back together from.</summary>
    /// <param name="key">The content key.</param>
    /// <returns>The manifest, or nothing when the content is kept whole.</returns>
    public Manifest? ReadManifest(string key)
    {
        ArgumentNullException.ThrowIfNull(key);

        var directory = Utf8(_scope_root);
        var hash = Utf8(key);
        try
        {
            var answer = RorolalaBinding.rola_storage_manifest(directory, hash);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Storage(answer.payload);
            }

            var handle = answer.payload;
            if (handle == 0)
            {
                return null;
            }

            try
            {
                var count = RorolalaBinding.rola_storage_manifest_chunk_count(handle);
                if (count == 0)
                {
                    return null;
                }

                var chunks = new List<Chunk>();
                for (nuint index = 0; index < count; index++)
                {
                    var chunk = RorolalaBinding.rola_storage_manifest_chunk_at(handle, index);
                    if (chunk == 0)
                    {
                        continue;
                    }

                    try
                    {
                        chunks.Add(
                            new Chunk(
                                NativeText.Taken(RorolalaBinding.rola_storage_chunk_key(chunk)),
                                RorolalaBinding.rola_storage_chunk_len(chunk)
                            )
                        );
                    }
                    finally
                    {
                        RorolalaBinding.free_rola_storage_chunk(chunk);
                    }
                }

                return new Manifest(chunks);
            }
            finally
            {
                RorolalaBinding.free_rola_storage_manifest(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(hash);
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_scope_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _scope_root,
                "the store's root is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        _keys = ReadKeys(RorolalaBinding.rola_storage_objects, cancellationToken);
        _manifests = ReadKeys(RorolalaBinding.rola_storage_manifests, cancellationToken);
    }

    private IReadOnlyList<string> ReadKeys(
        Func<nint, RorolalaResult> read,
        CancellationToken cancellationToken
    )
    {
        cancellationToken.ThrowIfCancellationRequested();

        var directory = Utf8(_scope_root);
        try
        {
            var answer = read(directory);

            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Storage(answer.payload);
            }

            var handle = answer.payload;
            if (handle == 0)
            {
                return [];
            }

            try
            {
                var count = RorolalaBinding.rola_storage_keys_count(handle);
                var keys = new List<string>();
                for (nuint index = 0; index < count; index++)
                {
                    var key = NativeText.Taken(
                        RorolalaBinding.rola_storage_keys_at(handle, index)
                    );
                    if (!string.IsNullOrEmpty(key))
                    {
                        keys.Add(key);
                    }
                }

                return keys;
            }
            finally
            {
                RorolalaBinding.free_rola_storage_keys(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }

    private void EnsureLoaded()
    {
        if (!Loaded)
        {
            throw new InvalidOperationException("call Read first");
        }
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

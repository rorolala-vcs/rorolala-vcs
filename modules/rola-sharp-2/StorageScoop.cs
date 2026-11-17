using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp.V2;

/// <summary>The content-addressed store a Workspace or a Vault keeps.</summary>
public sealed class StorageScoop : Scoop
{
    private readonly string _root;
    private IReadOnlyList<string> _keys = [];
    private IReadOnlyList<string> _manifests = [];

    /// <summary>Makes a scope over the store rooted at a Workspace or a Vault.</summary>
    /// <param name="root">The directory the store is rooted in.</param>
    public StorageScoop(string root) => _root = Path.GetFullPath(root);

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

        var directory = Utf8(_root);
        var hash = Utf8(key);
        try
        {
            var answer = RorolalaBinding.rola_storage_manifest_json(directory, hash);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Storage(answer.payload);
            }

            var json = NativeText.Taken(answer.payload);
            if (string.IsNullOrEmpty(json) || json == "null")
            {
                return null;
            }

            using var document = JsonDocument.Parse(json);
            var chunks = RolaJson
                .Items(document.RootElement, "chunks")
                ?.EnumerateArray()
                .Select(item => new Chunk(
                    RolaJson.Text(item, "key") ?? string.Empty,
                    RolaJson.Number(item, "len") ?? 0
                ))
                .ToArray()
                ?? [];

            return new Manifest(chunks);
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
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _root,
                "the store's root is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        _keys = ReadList(RorolalaBinding.rola_storage_objects_json, cancellationToken);
        _manifests = ReadList(RorolalaBinding.rola_storage_manifests_json, cancellationToken);
    }

    private IReadOnlyList<string> ReadList(
        Func<nint, RorolalaResult> read,
        CancellationToken cancellationToken
    )
    {
        cancellationToken.ThrowIfCancellationRequested();

        var directory = Utf8(_root);
        try
        {
            var answer = read(directory);

            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Storage(answer.payload);
            }

            var json = NativeText.Taken(answer.payload);
            if (string.IsNullOrEmpty(json) || json == "null")
            {
                return [];
            }

            using var document = JsonDocument.Parse(json);
            return RolaJson
                    .Items(document.RootElement, "keys")
                    ?.EnumerateArray()
                    .Select(item => item.GetString() ?? string.Empty)
                    .Where(key => key.Length > 0)
                    .ToArray()
                ?? [];
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

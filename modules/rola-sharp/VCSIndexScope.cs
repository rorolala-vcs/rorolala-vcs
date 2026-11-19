using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>The VCS index a Workspace or a Vault keeps: versions, variants and text objects.</summary>
public sealed class VCSIndexScope : Scope, IDisposable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the index a directory is in.</summary>
    /// <param name="directory">Any directory inside the Workspace or the Vault.</param>
    public VCSIndexScope(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The index's root.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>The root version's key, or an empty string when the index holds no root.</summary>
    public string RootVersion { get; private set; } = string.Empty;

    /// <summary>Everything the index holds, once it is read.</summary>
    public VcsIndexListing? Listing { get; private set; }

    /// <summary>One object as JSON, as the command line writes it.</summary>
    /// <param name="hash">The object's key.</param>
    /// <returns>The document, or nothing when the index holds no such object.</returns>
    public string? ObjectJson(string hash)
    {
        ArgumentNullException.ThrowIfNull(hash);
        EnsureHandle();

        var key = Marshal.StringToCoTaskMemUTF8(hash);
        try
        {
            var answer = RorolalaBinding.rola_vcsindex_object_json(_handle, key);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.VcsIndex(answer.payload);
            }

            var json = NativeText.Taken(answer.payload);

            return json is "" or "null" ? null : json;
        }
        finally
        {
            Marshal.FreeCoTaskMem(key);
        }
    }

    /// <summary>Reads one version's number, traced through the chain.</summary>
    /// <param name="hash">The version's key.</param>
    /// <returns>The number, or nothing when the chain does not say.</returns>
    public ulong? VersionNumber(string hash)
    {
        ArgumentNullException.ThrowIfNull(hash);

        var json = ObjectJson(hash);
        if (json is null)
        {
            return null;
        }

        using var document = JsonDocument.Parse(json);

        return RolaJson.Number(document.RootElement, "number");
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _root,
                "the index's directory is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        var path = Marshal.StringToCoTaskMemUTF8(_root);
        try
        {
            var handle = RorolalaBinding.locate_rola_vcs_index(path);
            if (handle == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.NoVault,
                    _root,
                    "no VCS index is there"
                );
            }

            _handle = handle;
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }

        var root = NativeText.Taken(RorolalaBinding.rola_vcsindex_rootver(_handle));
        RootVersion = root == "null" ? string.Empty : root;

        var json = NativeText.Taken(RorolalaBinding.rola_vcsindex_list_json(_handle));
        if (string.IsNullOrEmpty(json) || json == "null")
        {
            throw new RolaReadError(
                RolaReadErrorKind.Unreadable,
                _root,
                "the index could not be listed"
            );
        }

        using var document = JsonDocument.Parse(json);
        Listing = Parse(document.RootElement);
    }

    /// <summary>Gives the native index back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_vcsindex(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private void EnsureHandle()
    {
        if (_handle == 0)
        {
            throw new InvalidOperationException("call Read first");
        }
    }

    private static VcsIndexListing Parse(JsonElement root)
    {
        var versions = new List<VcsVersionItem>();
        var variants = new List<VcsVariantItem>();
        var strings = new List<string>();

        if (RolaJson.Items(root, "versions") is { } versionItems)
        {
            foreach (var item in versionItems.EnumerateArray())
            {
                var version = VersionOf(item, "version");
                if (version is null)
                {
                    continue;
                }

                var variant = item.TryGetProperty("variant", out var one) ? Variant(one) : null;
                versions.Add(
                    new VcsVersionItem(RolaJson.Number(item, "version_num"), version, variant)
                );
            }
        }

        if (RolaJson.Items(root, "variants") is { } variantItems)
        {
            foreach (var item in variantItems.EnumerateArray())
            {
                if (!item.TryGetProperty("variant", out var one) || Variant(one) is not { } variant)
                {
                    continue;
                }

                variants.Add(new VcsVariantItem(variant, VersionOf(item, "version")));
            }
        }

        if (RolaJson.Items(root, "string_hashes") is { } hashes)
        {
            strings.AddRange(
                hashes
                    .EnumerateArray()
                    .Select(item => Key(item.GetString() ?? string.Empty))
                    .Where(key => key.Length > 0)
            );
        }

        return new VcsIndexListing(versions, variants, strings);
    }

    private static VcsVersion? VersionOf(JsonElement item, string name)
    {
        if (
            !item.TryGetProperty(name, out var version)
            || version.ValueKind is not JsonValueKind.Object
            || RolaJson.Text(version, "variant") is not { Length: > 0 } variant
        )
        {
            return null;
        }

        return new VcsVersion(Key(variant));
    }

    private static VcsVariant? Variant(JsonElement item)
    {
        var storage = RolaJson.Text(item, "storage_hash") ?? RolaJson.Text(item, "storage");
        var baseVersion = RolaJson.Text(item, "base_version");
        var creator = RolaJson.Text(item, "creator");
        var message = RolaJson.Text(item, "message");
        if (
            string.IsNullOrEmpty(storage)
            || string.IsNullOrEmpty(baseVersion)
            || string.IsNullOrEmpty(creator)
            || string.IsNullOrEmpty(message)
        )
        {
            return null;
        }

        var join = RolaJson.Text(item, "join");

        return new VcsVariant(
            Key(storage),
            Key(baseVersion),
            string.IsNullOrEmpty(join) ? null : Key(join),
            Key(creator),
            Key(message)
        );
    }

    private static string Key(string text) =>
        text.Length == 0 || text.StartsWith("blake3:", StringComparison.Ordinal)
            ? text
            : $"blake3:{text}";
}

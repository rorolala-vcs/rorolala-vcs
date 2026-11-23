using System.Runtime.InteropServices;

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

    /// <summary>Reads one index object as the kind it turned out to be.</summary>
    /// <param name="hash">The object's key.</param>
    /// <returns>The object, or nothing when the index holds no such object.</returns>
    public VcsIndexObjectView? ObjectView(string hash)
    {
        ArgumentNullException.ThrowIfNull(hash);
        EnsureHandle();

        var key = Marshal.StringToCoTaskMemUTF8(hash);
        try
        {
            var answer = RorolalaBinding.rola_vcs_index_object_view(_handle, key);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.VcsIndex(answer.payload);
            }

            var handle = answer.payload;
            if (handle == 0)
            {
                return null;
            }

            try
            {
                return ViewOf(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_vcs_index_object_view(handle);
            }
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

        return ObjectView(hash) is VcsIndexVersionView version ? version.Number : null;
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
            var handle = RorolalaBinding.rola_vcs_index_locate(path);
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

        var root = NativeText.Taken(RorolalaBinding.rola_vcs_index_root_version(_handle));
        RootVersion = root == "null" ? string.Empty : root;

        var listing = RorolalaBinding.rola_vcs_index_list(_handle);
        if (listing == 0)
        {
            throw new RolaReadError(
                RolaReadErrorKind.Unreadable,
                _root,
                "the index could not be listed"
            );
        }

        try
        {
            Listing = ReadListing(listing);
        }
        finally
        {
            RorolalaBinding.free_rola_vcs_index_listing(listing);
        }
    }

    /// <summary>Gives the native index back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_vcs_index(_handle);
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

    private static VcsIndexObjectView ViewOf(nint handle)
    {
        var view = Marshal.PtrToStructure<RolaVCSIndexObjectView>(handle);

        switch (view.tag)
        {
            case RolaVCSIndexObjectViewTag.RolaVCSIndexObjectView_Creator:
                return new VcsIndexCreatorView(NativeText.Taken(view.payload.Creator.text));

            case RolaVCSIndexObjectViewTag.RolaVCSIndexObjectView_Message:
                return new VcsIndexMessageView(NativeText.Taken(view.payload.Message.text));

            case RolaVCSIndexObjectViewTag.RolaVCSIndexObjectView_Variant:
                var variant = view.payload.Variant;

                return new VcsIndexVariantView(
                    Key(NativeText.Taken(variant.hash)),
                    Key(NativeText.Taken(variant.base_version)),
                    Key(NativeText.Taken(variant.storage)),
                    Optional(Key(NativeText.Taken(variant.join)))
                );

            case RolaVCSIndexObjectViewTag.RolaVCSIndexObjectView_Version:
                var version = view.payload.Version;

                return new VcsIndexVersionView(
                    version.number_present != 0 ? version.number : null,
                    Key(NativeText.Taken(version.hash)),
                    Key(NativeText.Taken(version.variant))
                );

            default:
                throw new RolaReadError(
                    RolaReadErrorKind.Unreadable,
                    null,
                    "the index object is of a kind this build does not read"
                );
        }
    }

    private static VcsIndexListing ReadListing(nint handle)
    {
        return new VcsIndexListing(
            ReadVersions(handle),
            ReadVariants(handle),
            ReadStrings(handle)
        );
    }

    private static IReadOnlyList<VcsVersionItem> ReadVersions(nint handle)
    {
        var versions = new List<VcsVersionItem>();
        var count = RorolalaBinding.rola_vcs_index_listing_version_count(handle);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_vcs_index_listing_version_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                var version = Key(
                    NativeText.Taken(RorolalaBinding.rola_vcs_index_version_item_version(item))
                );
                if (version.Length == 0)
                {
                    continue;
                }

                var variant = Optional(
                    Key(
                        NativeText.Taken(
                            RorolalaBinding.rola_vcs_index_version_item_variant(item)
                        )
                    )
                );
                ulong? number = RorolalaBinding.rola_vcs_index_version_item_version_num_present(
                    item
                )
                    ? RorolalaBinding.rola_vcs_index_version_item_version_num(item)
                    : null;

                versions.Add(new VcsVersionItem(number, version, variant));
            }
            finally
            {
                RorolalaBinding.free_rola_vcs_index_version_item(item);
            }
        }

        return versions;
    }

    private static IReadOnlyList<VcsVariantItem> ReadVariants(nint handle)
    {
        var variants = new List<VcsVariantItem>();
        var count = RorolalaBinding.rola_vcs_index_listing_variant_count(handle);

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_vcs_index_listing_variant_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                var variant = Key(
                    NativeText.Taken(RorolalaBinding.rola_vcs_index_variant_item_variant(item))
                );
                if (variant.Length == 0)
                {
                    continue;
                }

                var version = Optional(
                    Key(
                        NativeText.Taken(
                            RorolalaBinding.rola_vcs_index_variant_item_version(item)
                        )
                    )
                );

                variants.Add(new VcsVariantItem(variant, version));
            }
            finally
            {
                RorolalaBinding.free_rola_vcs_index_variant_item(item);
            }
        }

        return variants;
    }

    private static IReadOnlyList<string> ReadStrings(nint handle)
    {
        var strings = new List<string>();
        var count = RorolalaBinding.rola_vcs_index_listing_string_count(handle);

        for (nuint index = 0; index < count; index++)
        {
            var hash = Key(
                NativeText.Taken(RorolalaBinding.rola_vcs_index_listing_string_at(handle, index))
            );
            if (hash.Length > 0)
            {
                strings.Add(hash);
            }
        }

        return strings;
    }

    private static string? Optional(string text) => text.Length == 0 ? null : text;

    private static string Key(string text) =>
        text.Length == 0 || text.StartsWith("blake3:", StringComparison.Ordinal)
            ? text
            : $"blake3:{text}";
}

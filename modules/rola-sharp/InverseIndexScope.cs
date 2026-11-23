using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>What building an inverse index found.</summary>
/// <param name="Mode">The mode it was built in.</param>
/// <param name="Objects">How many objects it read.</param>
/// <param name="ObjectsRead">How many of those it read whole.</param>
/// <param name="Variants">How many variants it saw.</param>
/// <param name="Versions">How many versions it saw.</param>
/// <param name="Creators">How many creators it saw.</param>
/// <param name="Messages">How many messages it saw.</param>
/// <param name="VersionsNumbered">How many versions it numbered.</param>
/// <param name="Entries">How many edges it wrote.</param>
/// <param name="Dependents">How many dependents it wrote.</param>
/// <param name="Fingerprint">The fingerprint it wrote.</param>
public sealed record InverseIndexReport(
    string Mode,
    ulong Objects,
    ulong ObjectsRead,
    ulong Variants,
    ulong Versions,
    ulong Creators,
    ulong Messages,
    ulong VersionsNumbered,
    ulong Entries,
    ulong Dependents,
    string Fingerprint
);

/// <summary>The inverse index a Workspace or a Vault keeps: what each key is used by.</summary>
public sealed class InverseIndexScope : Scope, IDisposable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the inverse index a directory is in.</summary>
    /// <param name="directory">Any directory inside the Workspace or the Vault.</param>
    public InverseIndexScope(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The inverse index's root.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>The versions that hold the variant named by a key.</summary>
    /// <param name="hash">The variant's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> VariantDependents(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_variant_dependents);

    /// <summary>The versions that name the content stored under a key.</summary>
    /// <param name="hash">The content key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> StoreDependents(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_store_dependents);

    /// <summary>The versions based on a version.</summary>
    /// <param name="hash">The version's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> VersionDependents(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_version_dependents);

    /// <summary>The versions a version's chain holds.</summary>
    /// <param name="hash">The version's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> ChildVersions(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_child_versions);

    /// <summary>The variants that name a creator.</summary>
    /// <param name="hash">The creator's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> CreatorDependents(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_creator_dependents);

    /// <summary>The variants that name a message.</summary>
    /// <param name="hash">The message's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> MessageDependents(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_message_dependents);

    /// <summary>The variants that merge a variant in.</summary>
    /// <param name="hash">The variant's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> JoinDependents(string hash) =>
        List(hash, RorolalaBinding.rola_inverse_index_join_dependents);

    /// <summary>The number a version's chain gives it.</summary>
    /// <param name="hash">The version's key.</param>
    /// <returns>The number, or nothing when the chain does not give one.</returns>
    public ulong? VersionNumber(string hash)
    {
        ArgumentNullException.ThrowIfNull(hash);
        EnsureHandle();

        var key = Utf8(Wire(hash));
        try
        {
            var answer = RorolalaBinding.rola_inverse_index_version_number(_handle, key);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Inverse(answer.payload);
            }

            try
            {
                return RorolalaBinding.rola_version_number_value(answer.payload);
            }
            finally
            {
                RorolalaBinding.free_rola_version_number(answer.payload);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(key);
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _root,
                "the inverse index's directory is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        var path = Utf8(_root);
        try
        {
            var handle = RorolalaBinding.rola_inverse_index_locate(path);
            if (handle == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.NoVault,
                    _root,
                    "no inverse index is there"
                );
            }

            _handle = handle;
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Builds the inverse index from the VCS index.</summary>
    /// <param name="full">Whether to read every object rather than only what changed.</param>
    /// <returns>What the build found.</returns>
    public InverseIndexReport Rebuild(bool full = false)
    {
        EnsureHandle();

        var answer = full
            ? RorolalaBinding.rola_inverse_index_full_rebuild(_handle)
            : RorolalaBinding.rola_inverse_index_rebuild(_handle);
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw ReadResults.InverseBuild(answer.payload);
        }

        var report = answer.payload;
        try
        {
            return new InverseIndexReport(
                NativeText.Taken(RorolalaBinding.rola_inverse_index_report_mode(report)),
                RorolalaBinding.rola_inverse_index_report_objects(report),
                RorolalaBinding.rola_inverse_index_report_objects_read(report),
                RorolalaBinding.rola_inverse_index_report_variants(report),
                RorolalaBinding.rola_inverse_index_report_versions(report),
                RorolalaBinding.rola_inverse_index_report_creators(report),
                RorolalaBinding.rola_inverse_index_report_messages(report),
                RorolalaBinding.rola_inverse_index_report_versions_numbered(report),
                RorolalaBinding.rola_inverse_index_report_entries(report),
                RorolalaBinding.rola_inverse_index_report_dependents(report),
                NativeText.Taken(RorolalaBinding.rola_inverse_index_report_fingerprint(report))
            );
        }
        finally
        {
            RorolalaBinding.free_rola_inverse_index_report(report);
        }
    }

    /// <summary>Gives the native inverse index back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_inverse_index(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private IReadOnlyList<string> List(string hash, Func<nint, nint, RorolalaResult> read)
    {
        ArgumentNullException.ThrowIfNull(hash);
        EnsureHandle();

        var key = Utf8(Wire(hash));
        try
        {
            var answer = read(_handle, key);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Inverse(answer.payload);
            }

            var list = answer.payload;
            try
            {
                var count = (int)RorolalaBinding.rola_hash_list_count(list);
                var keys = new List<string>(count);
                for (var index = 0; index < count; index++)
                {
                    keys.Add(
                        Key(
                            NativeText.Taken(
                                RorolalaBinding.rola_hash_list_at(list, (nuint)index)
                            )
                        )
                    );
                }

                return keys;
            }
            finally
            {
                RorolalaBinding.free_rola_hash_list(list);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(key);
        }
    }

    private void EnsureHandle()
    {
        if (_handle == 0)
        {
            throw new InvalidOperationException("call Read first");
        }
    }

    private static string Wire(string hash) =>
        hash.StartsWith("blake3:", StringComparison.Ordinal) ? hash["blake3:".Length..] : hash;

    private static string Key(string text) =>
        text.Length == 0 || text.StartsWith("blake3:", StringComparison.Ordinal)
            ? text
            : $"blake3:{text}";

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

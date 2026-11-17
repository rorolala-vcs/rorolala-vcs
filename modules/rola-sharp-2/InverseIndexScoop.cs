using System.Runtime.InteropServices;

namespace RolaSharp.V2;

/// <summary>The inverse index a Workspace or a Vault keeps: what each key is used by.</summary>
public sealed class InverseIndexScoop : Scoop, IDisposable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the inverse index a directory is in.</summary>
    /// <param name="directory">Any directory inside the Workspace or the Vault.</param>
    public InverseIndexScoop(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The inverse index's root.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>The versions that hold the variant named by a key.</summary>
    /// <param name="hash">The variant's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> VariantDependents(string hash) =>
        List(hash, RorolalaBinding.get_variant_dependents);

    /// <summary>The versions that name the content stored under a key.</summary>
    /// <param name="hash">The content key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> StoreDependents(string hash) =>
        List(hash, RorolalaBinding.get_store_dependents);

    /// <summary>The versions based on a version.</summary>
    /// <param name="hash">The version's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> VersionDependents(string hash) =>
        List(hash, RorolalaBinding.get_version_dependents);

    /// <summary>The versions a version's chain holds.</summary>
    /// <param name="hash">The version's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> ChildVersions(string hash) =>
        List(hash, RorolalaBinding.get_child_versions);

    /// <summary>The variants that name a creator.</summary>
    /// <param name="hash">The creator's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> CreatorDependents(string hash) =>
        List(hash, RorolalaBinding.get_creator_dependents);

    /// <summary>The variants that name a message.</summary>
    /// <param name="hash">The message's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> MessageDependents(string hash) =>
        List(hash, RorolalaBinding.get_message_dependents);

    /// <summary>The variants that merge a variant in.</summary>
    /// <param name="hash">The variant's key.</param>
    /// <returns>The keys, in the index's order.</returns>
    public IReadOnlyList<string> JoinDependents(string hash) =>
        List(hash, RorolalaBinding.get_join_dependents);

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
            var answer = RorolalaBinding.get_version_number(_handle, key);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Inverse(answer.payload);
            }

            try
            {
                return RorolalaBinding.version_number_value(answer.payload);
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
            var handle = RorolalaBinding.locate_rola_inverse_index(path);
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
                var count = (int)RorolalaBinding.rola_hash_list_len(list);
                var keys = new List<string>(count);
                for (var index = 0; index < count; index++)
                {
                    keys.Add(
                        Key(
                            NativeText.Taken(
                                RorolalaBinding.read_rola_hash_list_by_index(list, (nuint)index)
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

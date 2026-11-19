using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>One version in a version graph.</summary>
/// <param name="Level">How far it is from the target.</param>
/// <param name="Hash">The version's key.</param>
/// <param name="Creator">The creator's key, when it is known.</param>
/// <param name="Message">The message's key, when it is known.</param>
public sealed record VersionNode(ulong Level, string Hash, string Creator, string Message);

/// <summary>One variant in a version graph.</summary>
/// <param name="Label">The label drawn beside it.</param>
/// <param name="Hash">The variant's key.</param>
/// <param name="Level">How far it is from the target.</param>
/// <param name="Spine">Whether it is on the chain the target stands on.</param>
public sealed record VariantNode(string Label, string Hash, ulong Level, bool Spine);

/// <summary>The versions and variants around one version.</summary>
public sealed class VersionGraphScope : Scope, IDisposable
{
    private readonly string _root;
    private readonly string _target;
    private nint _handle;

    /// <summary>Makes a scope over the graph around a version.</summary>
    /// <param name="directory">Any directory inside the Workspace or the Vault.</param>
    /// <param name="target">The version to draw around.</param>
    public VersionGraphScope(string directory, string target)
    {
        _root = Path.GetFullPath(directory);
        _target = target;
    }

    /// <summary>The version the graph is drawn around.</summary>
    public string Target => _target;

    /// <summary>The versions, in the graph's order.</summary>
    public IReadOnlyList<VersionNode> Versions { get; private set; } = [];

    /// <summary>The variants, in the graph's order.</summary>
    public IReadOnlyList<VariantNode> Variants { get; private set; } = [];

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _root,
                "the graph's directory is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        var directory = Utf8(_root);
        var target = Utf8(Wire(_target));
        try
        {
            var handle = RorolalaBinding.locate_rola_version_graph(directory, target);
            if (handle == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.NotFound,
                    _target,
                    "no version graph could be made"
                );
            }

            _handle = handle;
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(target);
        }

        var versions = new List<VersionNode>();
        var versionCount = (int)RorolalaBinding.rola_version_graph_version_count(_handle);
        for (var index = 0; index < versionCount; index++)
        {
            versions.Add(
                new VersionNode(
                    RorolalaBinding.rola_version_graph_version_level(_handle, (nuint)index),
                    Key(
                        NativeText.Taken(
                            RorolalaBinding.rola_version_graph_version_hash(_handle, (nuint)index)
                        )
                    ),
                    Key(
                        NativeText.Taken(
                            RorolalaBinding.rola_version_graph_version_creator(
                                _handle,
                                (nuint)index
                            )
                        )
                    ),
                    Key(
                        NativeText.Taken(
                            RorolalaBinding.rola_version_graph_version_message(
                                _handle,
                                (nuint)index
                            )
                        )
                    )
                )
            );
        }

        var variants = new List<VariantNode>();
        var variantCount = (int)RorolalaBinding.rola_version_graph_variant_count(_handle);
        for (var index = 0; index < variantCount; index++)
        {
            variants.Add(
                new VariantNode(
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_variant_label(_handle, (nuint)index)
                    ),
                    Key(
                        NativeText.Taken(
                            RorolalaBinding.rola_version_graph_variant_hash(_handle, (nuint)index)
                        )
                    ),
                    RorolalaBinding.rola_version_graph_variant_level(_handle, (nuint)index),
                    RorolalaBinding.rola_version_graph_variant_spine(_handle, (nuint)index)
                )
            );
        }

        Versions = versions;
        Variants = variants;
    }

    /// <summary>Gives the native graph back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_version_graph(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static string Wire(string hash) =>
        hash.StartsWith("blake3:", StringComparison.Ordinal) ? hash["blake3:".Length..] : hash;

    private static string Key(string text) =>
        text.Length == 0 || text.StartsWith("blake3:", StringComparison.Ordinal)
            ? text
            : $"blake3:{text}";

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

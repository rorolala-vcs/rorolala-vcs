using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>One version the Version Graph can draw.</summary>
/// <param name="Level">The version's number, zero for the oldest.</param>
/// <param name="Hash">The full hash the index names the version by, as hex.</param>
/// <param name="Creator">Who made the variant that produced the version, or an empty string.</param>
/// <param name="Message">What that variant says, or an empty string.</param>
public sealed record GraphVersion(ulong Level, string Hash, string Creator, string Message);

/// <summary>One variant the Version Graph can draw.</summary>
/// <param name="Label">The label it is drawn by: its hash's leading characters.</param>
/// <param name="Hash">
/// The full hash the index names the variant by, as hex; empty for a placeholder the graph draws
/// but the index does not hold.
/// </param>
/// <param name="Level">The version it is based on.</param>
/// <param name="Spine">Whether it is the variant the version above points at.</param>
/// <param name="Join">
/// The index of the variant it merges into, in <see cref="RolaVersionGraph.Variants"/>, or nothing
/// when it merges none.
/// </param>
/// <param name="Creator">Who made it, or an empty string.</param>
/// <param name="Message">What it says, or an empty string.</param>
public sealed record GraphVariant(
    string Label,
    string Hash,
    ulong Level,
    bool Spine,
    int? Join,
    string Creator,
    string Message
);

/// <summary>
/// The version chain of one file, as the Desktop's Version Graph lays it out.
/// </summary>
/// <remarks>
/// What is read here is the structure rather than the command line's drawing: the versions, the
/// variants that produced them, and the joins between variants. The Desktop is what turns that into
/// geometry. A file with no version history is answered as <see langword="null"/> rather than as an
/// empty chain: the two are different things to say, and an empty chain would be a second way of
/// saying what nothing says.
/// <para>
/// The reading is a snapshot, so asking it again cannot answer anything newer; the versions and
/// variants are read out in full when it is made, and the native handle is given back on
/// <see cref="Dispose"/>.
/// </para>
/// </remarks>
public sealed class RolaVersionGraph : IDisposable
{
    /// <summary>The native reading, until it is given back.</summary>
    private nint _handle;

    /// <summary>Makes a reading from a native graph, which is read out and kept.</summary>
    /// <param name="handle">The native graph, which must not be null.</param>
    private RolaVersionGraph(nint handle)
    {
        _handle = handle;

        try
        {
            Versions = VersionsOf(handle);
            Variants = VariantsOf(handle);
            Upstream = UpstreamOf(handle);
            UpstreamHead = NativeText.Taken(
                RorolalaBinding.rola_version_graph_upstream_head(handle)
            );
            UpstreamVault = NativeText.Taken(
                RorolalaBinding.rola_version_graph_upstream_vault(handle)
            );

            var origin = RorolalaBinding.rola_version_graph_origin_level(handle);
            OriginLevel = origin < 0 ? null : (ulong)origin;
            OriginVault = NativeText.Taken(RorolalaBinding.rola_version_graph_origin_vault(handle));
        }
        catch
        {
            // A reading that could not be taken out whole is not one to keep the native side of: what
            // is left would be a handle nobody can reach.
            RorolalaBinding.free_rola_version_graph(handle);
            _handle = 0;

            throw;
        }
    }

    /// <summary>The versions, the highest first.</summary>
    public IReadOnlyList<GraphVersion> Versions { get; }

    /// <summary>
    /// The tracked Vault's versions the index here holds, the highest first, or none when it holds none.
    /// </summary>
    /// <remarks>
    /// A fetch brings the Vault's Layout and never its index, so this is empty in the ordinary state of
    /// being behind upstream: what the Vault records is known from <see cref="UpstreamHead"/> alone. When
    /// it is not empty, the levels are numbered from the same root as <see cref="Versions"/>, so a version
    /// both chains hold stands at the same level in each.
    /// </remarks>
    public IReadOnlyList<GraphVersion> Upstream { get; }

    /// <summary>
    /// The version the tracked Vault records for the file, as hex, or an empty string when none is tracked.
    /// </summary>
    public string UpstreamHead { get; }

    /// <summary>The Vault whose version <see cref="UpstreamHead"/> is, or an empty string when none is tracked.</summary>
    public string UpstreamVault { get; }

    /// <summary>
    /// The variants in the chain's order: level by level, the spine before the variants merged in
    /// beside it.
    /// </summary>
    public IReadOnlyList<GraphVariant> Variants { get; }

    /// <summary>
    /// The number of the version the tracked Vault records, when the chain marks one.
    /// </summary>
    public ulong? OriginLevel { get; }

    /// <summary>
    /// The name of the Vault whose version is marked, or an empty string when none is.
    /// </summary>
    public string OriginVault { get; }

    /// <summary>
    /// Reads the chain of the file <paramref name="target"/>.
    /// </summary>
    /// <param name="directory">
    /// The directory the Workspace is searched upwards from, which is the directory a listing is of.
    /// </param>
    /// <param name="target">The file whose chain is wanted, as the program names it.</param>
    /// <returns>
    /// The chain, or nothing when the file has no version history: no Workspace holds the directory,
    /// the Workspace works in no Layout, the Layout does not name the file, nothing was ever recorded
    /// for it, or the chain cannot be read.
    /// </returns>
    public static RolaVersionGraph? Locate(string directory, string target)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(target);

        var at = Marshal.StringToCoTaskMemUTF8(directory);
        var wanted = Marshal.StringToCoTaskMemUTF8(target);

        try
        {
            var handle = RorolalaBinding.locate_rola_version_graph(at, wanted);

            return handle == 0 ? null : new RolaVersionGraph(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(wanted);
        }
    }

    /// <inheritdoc />
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_version_graph(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    /// <summary>Reads the versions out of the native reading.</summary>
    /// <param name="handle">The native graph.</param>
    /// <returns>The versions, the highest first.</returns>
    private static List<GraphVersion> VersionsOf(nint handle)
    {
        var count = (int)RorolalaBinding.rola_version_graph_version_count(handle);
        var versions = new List<GraphVersion>(count);

        for (var at = 0; at < count; at++)
        {
            var index = (nuint)at;

            versions.Add(
                new GraphVersion(
                    RorolalaBinding.rola_version_graph_version_level(handle, index),
                    NativeText.Taken(RorolalaBinding.rola_version_graph_version_hash(handle, index)),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_version_creator(handle, index)
                    ),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_version_message(handle, index)
                    )
                )
            );
        }

        return versions;
    }

    /// <summary>Reads the upstream versions out of the native reading.</summary>
    /// <param name="handle">The native graph.</param>
    /// <returns>The versions, the highest first.</returns>
    private static List<GraphVersion> UpstreamOf(nint handle)
    {
        var count = (int)RorolalaBinding.rola_version_graph_upstream_count(handle);
        var versions = new List<GraphVersion>(count);

        for (var at = 0; at < count; at++)
        {
            var index = (nuint)at;

            versions.Add(
                new GraphVersion(
                    RorolalaBinding.rola_version_graph_upstream_level(handle, index),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_upstream_hash(handle, index)
                    ),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_upstream_creator(handle, index)
                    ),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_upstream_message(handle, index)
                    )
                )
            );
        }

        return versions;
    }

    /// <summary>Reads the variants out of the native reading.</summary>
    /// <param name="handle">The native graph.</param>
    /// <returns>The variants in the chain's order.</returns>
    private static List<GraphVariant> VariantsOf(nint handle)
    {
        var count = (int)RorolalaBinding.rola_version_graph_variant_count(handle);
        var variants = new List<GraphVariant>(count);

        for (var at = 0; at < count; at++)
        {
            var index = (nuint)at;
            var join = RorolalaBinding.rola_version_graph_variant_join(handle, index);

            variants.Add(
                new GraphVariant(
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_variant_label(handle, index)
                    ),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_variant_hash(handle, index)
                    ),
                    RorolalaBinding.rola_version_graph_variant_level(handle, index),
                    RorolalaBinding.rola_version_graph_variant_spine(handle, index),
                    join < 0 || join > int.MaxValue ? null : (int)join,
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_variant_creator(handle, index)
                    ),
                    NativeText.Taken(
                        RorolalaBinding.rola_version_graph_variant_message(handle, index)
                    )
                )
            );
        }

        return variants;
    }
}

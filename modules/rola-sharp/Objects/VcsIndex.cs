namespace RolaSharp;

/// <summary>One variant: the stored entry it points at and what made it.</summary>
/// <param name="Storage">The content key the variant points at.</param>
/// <param name="BaseVersion">The version it is based on.</param>
/// <param name="Join">The variant it merges in, when it merges one.</param>
/// <param name="Creator">The creator's key.</param>
/// <param name="Message">The message's key.</param>
public sealed record VcsVariant(
    string Storage,
    string BaseVersion,
    string? Join,
    string Creator,
    string Message
);

/// <summary>One version: the variant it points at.</summary>
/// <param name="Variant">The variant's key.</param>
public sealed record VcsVersion(string Variant);

/// <summary>One version in a listing, with its traced number and its variant.</summary>
/// <param name="Number">The number the chain traced, when it could.</param>
/// <param name="Version">The version.</param>
/// <param name="Variant">The variant it points at, when one is stored.</param>
public sealed record VcsVersionItem(ulong? Number, VcsVersion Version, VcsVariant? Variant);

/// <summary>One variant in a listing, with the version it is based on.</summary>
/// <param name="Variant">The variant.</param>
/// <param name="Version">The version it is based on, when one is stored.</param>
public sealed record VcsVariantItem(VcsVariant Variant, VcsVersion? Version);

/// <summary>Everything a VCS index holds, as its three listings name it.</summary>
/// <param name="Versions">The versions.</param>
/// <param name="Variants">The variants.</param>
/// <param name="StringHashes">The hashes of the text objects.</param>
public sealed record VcsIndexListing(
    IReadOnlyList<VcsVersionItem> Versions,
    IReadOnlyList<VcsVariantItem> Variants,
    IReadOnlyList<string> StringHashes
);

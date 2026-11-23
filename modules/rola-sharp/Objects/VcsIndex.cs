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

/// <summary>One version in a listing, with its traced number and the variant it points at.</summary>
/// <param name="Number">The number the chain traced, when it could.</param>
/// <param name="Version">The version's key.</param>
/// <param name="Variant">The variant's key, or nothing when none is stored.</param>
public sealed record VcsVersionItem(ulong? Number, string Version, string? Variant);

/// <summary>One variant in a listing, with the version it is based on.</summary>
/// <param name="Variant">The variant's key.</param>
/// <param name="Version">The version it is based on, or nothing when none is stored.</param>
public sealed record VcsVariantItem(string Variant, string? Version);

/// <summary>Everything a VCS index holds, as its three listings name it.</summary>
/// <param name="Versions">The versions.</param>
/// <param name="Variants">The variants.</param>
/// <param name="StringHashes">The hashes of the text objects.</param>
public sealed record VcsIndexListing(
    IReadOnlyList<VcsVersionItem> Versions,
    IReadOnlyList<VcsVariantItem> Variants,
    IReadOnlyList<string> StringHashes
);

/// <summary>One index object as the ABI reads it: which kind it is, and what it holds.</summary>
public abstract record VcsIndexObjectView;

/// <summary>A Creator: the name it is.</summary>
/// <param name="Text">The name.</param>
public sealed record VcsIndexCreatorView(string Text) : VcsIndexObjectView;

/// <summary>A Message: the text it is.</summary>
/// <param name="Text">The text.</param>
public sealed record VcsIndexMessageView(string Text) : VcsIndexObjectView;

/// <summary>A Variant: the hashes it carries.</summary>
/// <param name="Hash">The variant's key.</param>
/// <param name="BaseVersion">The key of the version it is based on.</param>
/// <param name="Storage">The key of the storage entry it points at.</param>
/// <param name="Join">The key of the variant it merges in, or nothing when it merges none.</param>
public sealed record VcsIndexVariantView(
    string Hash,
    string BaseVersion,
    string Storage,
    string? Join
) : VcsIndexObjectView;

/// <summary>A Version: its key, the variant it points at, and the number traced for it.</summary>
/// <param name="Number">The number the chain traced, when it could.</param>
/// <param name="Hash">The version's key.</param>
/// <param name="Variant">The key of the variant it points at.</param>
public sealed record VcsIndexVersionView(ulong? Number, string Hash, string Variant)
    : VcsIndexObjectView;

namespace RolaSharp.V2.Write;

/// <summary>What a store answered when content went in.</summary>
/// <param name="Key">The key the content is kept under.</param>
/// <param name="Chunked">Whether the content is kept as a manifest of chunks.</param>
public sealed record Stored(string Key, bool Chunked);

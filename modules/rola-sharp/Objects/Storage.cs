namespace RolaSharp;

/// <summary>One piece of content a store keeps, and how many bytes of it the chunk holds.</summary>
/// <param name="Key">The key the chunk is stored under.</param>
/// <param name="Length">How many bytes the chunk holds.</param>
public sealed record Chunk(string Key, ulong Length);

/// <summary>A chunked object: the chunks it is put back together from, in order.</summary>
/// <param name="Chunks">The chunks.</param>
public sealed record Manifest(IReadOnlyList<Chunk> Chunks);

namespace RolaSharp.Write;

/// <summary>What packing a store or an index changed.</summary>
/// <param name="Changed">Whether anything was rewritten.</param>
public sealed record Packed(bool Changed);

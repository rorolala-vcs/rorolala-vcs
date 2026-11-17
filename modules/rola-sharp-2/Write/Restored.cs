namespace RolaSharp.V2.Write;

/// <summary>What restoring one path wrote.</summary>
/// <param name="Id">The Layout entry the path names.</param>
/// <param name="Version">The version the Layout now records.</param>
public sealed record Restored(Guid Id, string Version);

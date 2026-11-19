namespace RolaSharp.Write;

/// <summary>What retracking one path wrote.</summary>
/// <param name="Id">The Layout entry the path names.</param>
/// <param name="Version">The version the Layout now records.</param>
public sealed record Retracked(Guid Id, string Version);

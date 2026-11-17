namespace RolaSharp.V2.Write;

/// <summary>What tracking one file wrote.</summary>
/// <param name="Id">The Layout entry the path names.</param>
/// <param name="Storage">The content's key.</param>
/// <param name="Variant">The variant the version points at.</param>
/// <param name="Version">The version the Layout records.</param>
public sealed record TrackedFile(Guid Id, string Storage, string Variant, string Version);

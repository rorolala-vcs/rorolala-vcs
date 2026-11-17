namespace RolaSharp.V2.Write;

/// <summary>The variant and version a file's content was recorded as.</summary>
/// <param name="Variant">The variant the version points at.</param>
/// <param name="Version">The version the Layout records.</param>
public sealed record FileVersion(string Variant, string Version);

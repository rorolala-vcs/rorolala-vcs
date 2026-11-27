namespace RolaSharp.Write;

/// <summary>What applying a sheet to a path's chain wrote.</summary>
/// <param name="File">The path whose chain was adjusted, as the Layout names it.</param>
/// <param name="Version">The version the entry records now, as hex.</param>
/// <param name="Adjusted">How many versions the run wrote.</param>
public sealed record Tweaked(string File, string Version, ulong Adjusted);

namespace RolaSharp.Write;

/// <summary>The operations an undo reversed.</summary>
/// <param name="Commands">The commands whose operations were undone, newest first.</param>
public sealed record Undone(IReadOnlyList<string> Commands);

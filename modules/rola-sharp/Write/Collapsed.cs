namespace RolaSharp.Write;

/// <summary>What folding a chain's local versions into one wrote.</summary>
/// <param name="File">The path whose chain was folded, as the Layout names it.</param>
/// <param name="From">The version the entry recorded before the fold, as hex.</param>
/// <param name="To">The version it records after the fold, as hex.</param>
/// <param name="Folded">How many versions the fold removed.</param>
public sealed record Collapsed(string File, string From, string To, ulong Folded);

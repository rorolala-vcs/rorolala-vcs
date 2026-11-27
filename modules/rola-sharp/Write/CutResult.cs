namespace RolaSharp.Write;

/// <summary>What cutting a file's old history away wrote.</summary>
/// <param name="File">The path whose history was cut, as the Layout names it.</param>
/// <param name="From">The version the entry recorded before the cut, as hex.</param>
/// <param name="To">The version it records after the cut, as hex.</param>
/// <param name="Point">The first version the new chain keeps, as a name for a person.</param>
/// <param name="Uuid">The `Uuid` the path records after the cut.</param>
/// <param name="Kept">How many versions the new chain keeps.</param>
public sealed record CutResult(
    string File,
    string From,
    string To,
    string Point,
    string Uuid,
    ulong Kept
);

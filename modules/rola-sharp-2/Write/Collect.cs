using System.Text.Json;

namespace RolaSharp.V2.Write;

/// <summary>What one side of a collecting run would take away, or took.</summary>
/// <param name="Removed">How many entries the side takes away.</param>
/// <param name="Bytes">How many bytes those entries occupy.</param>
/// <param name="Kept">How many entries the grace keeps.</param>
/// <param name="Changed">Whether the side changed, which a plan never says.</param>
public sealed record CollectSide(ulong Removed, ulong Bytes, ulong Kept, bool Changed);

/// <summary>What a collecting run would take away, by the side it sweeps.</summary>
/// <param name="Index">The index side, when it is swept.</param>
/// <param name="Storage">The store side, when it is swept.</param>
public sealed record CollectPlan(CollectSide? Index, CollectSide? Storage);

/// <summary>What a collecting run took away, by the side it swept.</summary>
/// <param name="Index">The index side, when it was swept.</param>
/// <param name="Storage">The store side, when it was swept.</param>
public sealed record CollectReport(CollectSide? Index, CollectSide? Storage);

/// <summary>Reads the numbers a collecting run answers with.</summary>
internal static class Collects
{
    /// <summary>Reads one side's numbers.</summary>
    /// <param name="root">The answer.</param>
    /// <param name="name">The side's name.</param>
    /// <returns>The numbers, or nothing when the side was not swept.</returns>
    public static CollectSide? Side(JsonElement root, string name)
    {
        if (
            !root.TryGetProperty(name, out var side)
            || side.ValueKind is JsonValueKind.Null
        )
        {
            return null;
        }

        return new CollectSide(
            RolaJson.Number(side, "removed") ?? 0,
            RolaJson.Number(side, "bytes") ?? 0,
            RolaJson.Number(side, "kept") ?? 0,
            RolaJson.Flag(side, "changed")
        );
    }
}

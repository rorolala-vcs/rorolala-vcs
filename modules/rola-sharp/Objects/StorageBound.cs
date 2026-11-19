using System.Text.Json;

namespace RolaSharp;

/// <summary>What binding a store came to.</summary>
/// <param name="BoundWorkspace">Whether a Workspace was bound.</param>
/// <param name="BoundVault">Whether a Vault was bound.</param>
/// <param name="ReverseRecorded">Whether the store recorded the binding back.</param>
/// <param name="ReadOnly">Whether the store can only be read.</param>
/// <param name="Warnings">What the caller should be told, one line each.</param>
public sealed record StorageBound(
    bool BoundWorkspace,
    bool BoundVault,
    bool ReverseRecorded,
    bool ReadOnly,
    IReadOnlyList<string> Warnings
)
{
    /// <summary>Reads the answer the native side writes.</summary>
    internal static StorageBound From(JsonElement root) =>
        new(
            RolaJson.Flag(root, "bound_workspace"),
            RolaJson.Flag(root, "bound_vault"),
            RolaJson.Flag(root, "reverse_recorded"),
            RolaJson.Flag(root, "read_only"),
            RolaJson
                .Items(root, "warnings")
                ?.EnumerateArray()
                .Select(item => item.GetString() ?? string.Empty)
                .Where(line => line.Length > 0)
                .ToArray()
            ?? []
        );
}

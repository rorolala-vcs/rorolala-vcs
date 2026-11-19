using System.Text.Json;

namespace RolaSharp;

/// <summary>
/// Reading the JSON the native side writes.
/// </summary>
/// <remarks>
/// The shapes are the command line's own: a wrapper that asks the library for a listing is asking for
/// the JSON `rola` writes under `--json`, so the members are read by the names the command writes and
/// nothing here renames them. A member that is not there is answered as missing rather than raised,
/// because the native side leaves out what a place does not have.
/// </remarks>
internal static class RolaJson
{
    /// <summary>The array under a member of the answer, or nothing when the answer holds none.</summary>
    public static JsonElement? Items(JsonElement root, string name) =>
        root.ValueKind is JsonValueKind.Object
        && root.TryGetProperty(name, out var items)
        && items.ValueKind is JsonValueKind.Array
            ? items
            : null;

    /// <summary>The string a member of an object holds, or nothing when it holds none.</summary>
    public static string? Text(JsonElement item, string name) =>
        item.ValueKind is JsonValueKind.Object
        && item.TryGetProperty(name, out var value)
        && value.ValueKind is JsonValueKind.String
            ? value.GetString()
            : null;

    /// <summary>Whether a member of an object is the JSON `true`.</summary>
    public static bool Flag(JsonElement item, string name) =>
        item.ValueKind is JsonValueKind.Object
        && item.TryGetProperty(name, out var value)
        && value.ValueKind is JsonValueKind.True;

    /// <summary>The number a member of an object holds, or nothing when it holds none.</summary>
    public static ulong? Number(JsonElement item, string name) =>
        item.ValueKind is JsonValueKind.Object
        && item.TryGetProperty(name, out var value)
        && value.ValueKind is JsonValueKind.Number
        && value.TryGetUInt64(out var number)
            ? number
            : null;
}

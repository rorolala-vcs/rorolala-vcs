using System.Text.Json;

namespace RolaSharp.V2.Write;

/// <summary>Which way a sync may move content.</summary>
public enum SyncDirection
{
    /// <summary>Only what this Layout holds goes up.</summary>
    Up,

    /// <summary>Only what the Vault holds comes down.</summary>
    Down,

    /// <summary>Both directions.</summary>
    Both,
}

/// <summary>One entry a sync plan speaks about.</summary>
/// <param name="Uuid">The Uuid it is known by.</param>
/// <param name="LocalPath">The path this Layout names it by, when it names one.</param>
/// <param name="RemotePath">The path the Vault's Layout names it by, when it names one.</param>
/// <param name="PlannedPath">The name the Vault's Layout is to give it, when this creates one.</param>
/// <param name="LocalVersion">The version this Layout is at, as hex, when it is here.</param>
/// <param name="RemoteVersion">The version the Vault is at, as hex, when it is there.</param>
/// <param name="Owner">The account the Vault names as its holder.</param>
/// <param name="Deprecated">Whether the Vault moved it under its removed prefix.</param>
/// <param name="ItemKind">What the entry is, a file or a tree.</param>
/// <param name="Modified">Whether the file changed in the tree.</param>
/// <param name="Kind">What the plan means to do with it.</param>
public sealed record SyncEntry(
    string Uuid,
    string? LocalPath,
    string? RemotePath,
    string? PlannedPath,
    string? LocalVersion,
    string? RemoteVersion,
    string? Owner,
    bool Deprecated,
    string? ItemKind,
    bool Modified,
    string Kind
);

/// <summary>What a sync run is to do.</summary>
/// <param name="Layout">The Layout being worked in.</param>
/// <param name="Vault">The Vault it tracks.</param>
/// <param name="Forced">Whether the run may change history rather than refuse.</param>
/// <param name="Entries">What is to move, in path order.</param>
/// <param name="UpToDate">How many entries both sides already hold at the same version.</param>
public sealed record SyncPlan(
    string Layout,
    string Vault,
    bool Forced,
    IReadOnlyList<SyncEntry> Entries,
    ulong UpToDate
);

/// <summary>What a sync run came to.</summary>
/// <param name="Applied">How many entries were written to the Vault.</param>
/// <param name="Received">How many versions were brought down.</param>
/// <param name="Failed">What was not done, one line each.</param>
/// <param name="IndexObjects">How many stored objects the index named.</param>
/// <param name="StorageObjects">How many units of content were sent.</param>
/// <param name="MissingLayout">Whether the Vault's Layout was not read.</param>
/// <param name="Took">What each Vault handed over.</param>
public sealed record SyncReport(
    ulong Applied,
    ulong Received,
    IReadOnlyList<string> Failed,
    ulong IndexObjects,
    ulong StorageObjects,
    bool MissingLayout,
    IReadOnlyList<(string Vault, ulong Count)> Took
);

/// <summary>Reads the plan the library answers with.</summary>
internal static class SyncPlans
{
    /// <summary>Reads a plan from its JSON.</summary>
    /// <param name="root">The plan's JSON.</param>
    /// <returns>The plan.</returns>
    public static SyncPlan Parse(JsonElement root)
    {
        var entries = new List<SyncEntry>();
        var items = RolaJson.Items(root, "entries");
        if (items is { } array)
        {
            foreach (var item in array.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Object)
                {
                    continue;
                }

                entries.Add(
                    new SyncEntry(
                        RolaJson.Text(item, "uuid") ?? string.Empty,
                        RolaJson.Text(item, "local_path"),
                        RolaJson.Text(item, "remote_path"),
                        RolaJson.Text(item, "planned_path"),
                        RolaJson.Text(item, "local_version"),
                        RolaJson.Text(item, "remote_version"),
                        RolaJson.Text(item, "owner"),
                        RolaJson.Flag(item, "deprecated"),
                        RolaJson.Text(item, "item_kind"),
                        RolaJson.Flag(item, "modified"),
                        RolaJson.Text(item, "kind") ?? string.Empty
                    )
                );
            }
        }

        return new SyncPlan(
            RolaJson.Text(root, "layout") ?? string.Empty,
            RolaJson.Text(root, "vault") ?? string.Empty,
            RolaJson.Flag(root, "forced"),
            entries,
            RolaJson.Number(root, "up_to_date") ?? 0
        );
    }
}

/// <summary>Reads what a sync run came to.</summary>
internal static class SyncReports
{
    /// <summary>Reads a report from its JSON.</summary>
    /// <param name="root">The report's JSON.</param>
    /// <returns>The report.</returns>
    public static SyncReport Parse(JsonElement root)
    {
        var failed = new List<string>();
        if (RolaJson.Items(root, "failed") is { } failures)
        {
            foreach (var item in failures.EnumerateArray())
            {
                if (item.ValueKind is JsonValueKind.String)
                {
                    failed.Add(item.GetString() ?? string.Empty);
                }
            }
        }

        var took = new List<(string Vault, ulong Count)>();
        if (RolaJson.Items(root, "took") is { } taken)
        {
            foreach (var item in taken.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Array)
                {
                    continue;
                }

                var parts = item.EnumerateArray().ToArray();
                if (parts.Length == 2 && parts[0].ValueKind is JsonValueKind.String)
                {
                    took.Add(
                        (
                            parts[0].GetString() ?? string.Empty,
                            parts[1].GetUInt64()
                        )
                    );
                }
            }
        }

        return new SyncReport(
            RolaJson.Number(root, "applied") ?? 0,
            RolaJson.Number(root, "received") ?? 0,
            failed,
            RolaJson.Number(root, "index_objects") ?? 0,
            RolaJson.Number(root, "storage_objects") ?? 0,
            RolaJson.Flag(root, "missing_layout"),
            took
        );
    }
}

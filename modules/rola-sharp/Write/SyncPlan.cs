namespace RolaSharp.Write;

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
)
{
    /// <summary>Whether the plan came from the native side, which an apply requires.</summary>
    internal bool Agreed { get; init; }

    /// <summary>Whether the Vault's Layout was read, which a plan from the native side always was.</summary>
    public bool LayoutRead { get; init; } = true;

    /// <summary>Whether the index moves in the run the plan was made for.</summary>
    public bool Index { get; init; } = true;

    /// <summary>Whether the store moves in the run the plan was made for.</summary>
    public bool Storage { get; init; } = true;
}

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
)
{
    /// <summary>The Layout the run was about.</summary>
    public string Layout { get; init; } = string.Empty;

    /// <summary>The Vault it tracks.</summary>
    public string Vault { get; init; } = string.Empty;

    /// <summary>How many entries both sides already held at the same version.</summary>
    public ulong UpToDate { get; init; }

    /// <summary>Whether the Vault's Layout was read at all.</summary>
    public bool LayoutRead { get; init; }
}

/// <summary>Reads the plan the library hands over.</summary>
internal static class SyncPlans
{
    /// <summary>Reads a plan from its handle.</summary>
    /// <param name="handle">The plan, which this does not release.</param>
    /// <returns>The plan.</returns>
    public static SyncPlan Read(nint handle)
    {
        var entries = new List<SyncEntry>();
        var count = RorolalaBinding.rola_write_sync_plan_entry_count(handle);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_write_sync_plan_entry_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                entries.Add(
                    new SyncEntry(
                        NativeText.Taken(RorolalaBinding.rola_write_sync_entry_uuid(item)),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_local_path(item))),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_remote_path(item))),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_planned_path(item))),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_local_version(item))),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_remote_version(item))),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_owner(item))),
                        RorolalaBinding.rola_write_sync_entry_deprecated(item),
                        Optional(NativeText.Taken(RorolalaBinding.rola_write_sync_entry_item_kind(item))),
                        RorolalaBinding.rola_write_sync_entry_modified(item),
                        NativeText.Taken(RorolalaBinding.rola_write_sync_entry_kind(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_sync_entry(item);
            }
        }

        var plan = new SyncPlan(
            NativeText.Taken(RorolalaBinding.rola_write_sync_plan_layout(handle)),
            NativeText.Taken(RorolalaBinding.rola_write_sync_plan_vault(handle)),
            RorolalaBinding.rola_write_sync_plan_forced(handle),
            entries,
            RorolalaBinding.rola_write_sync_plan_up_to_date(handle)
        )
        {
            Agreed = true,
            LayoutRead = RorolalaBinding.rola_write_sync_plan_layout_read(handle),
            Index = RorolalaBinding.rola_write_sync_plan_index(handle),
            Storage = RorolalaBinding.rola_write_sync_plan_storage(handle),
        };

        return plan;
    }

    private static string? Optional(string text) => text.Length == 0 ? null : text;
}

/// <summary>Reads what a sync run came to.</summary>
internal static class SyncReports
{
    /// <summary>Reads the one-shot run's answer.</summary>
    /// <param name="handle">The answer, which this does not release.</param>
    /// <returns>The report.</returns>
    public static SyncReport OfRun(nint handle)
    {
        var failed = new List<string>();
        var failedCount = RorolalaBinding.rola_write_sync_failed_count(handle);
        for (nuint index = 0; index < failedCount; index++)
        {
            failed.Add(
                NativeText.Taken(RorolalaBinding.rola_write_sync_failed_at(handle, index))
            );
        }

        var took = new List<(string Vault, ulong Count)>();
        var tookCount = RorolalaBinding.rola_write_sync_took_count(handle);
        for (nuint index = 0; index < tookCount; index++)
        {
            var item = RorolalaBinding.rola_write_sync_took_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                took.Add(
                    (
                        NativeText.Taken(RorolalaBinding.rola_write_sync_took_vault(item)),
                        RorolalaBinding.rola_write_sync_took_objects(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_sync_took(item);
            }
        }

        var missing = RorolalaBinding.rola_write_sync_missing_layout(handle);

        return new SyncReport(
            RorolalaBinding.rola_write_sync_applied(handle),
            RorolalaBinding.rola_write_sync_received(handle),
            failed,
            RorolalaBinding.rola_write_sync_index_objects(handle),
            RorolalaBinding.rola_write_sync_storage_objects(handle),
            missing,
            took
        )
        {
            LayoutRead = !missing,
        };
    }

    /// <summary>Reads the answer of a plan carried out.</summary>
    /// <param name="handle">The answer, which this does not release.</param>
    /// <returns>The report.</returns>
    public static SyncReport OfApply(nint handle)
    {
        var failed = new List<string>();
        var failedCount = RorolalaBinding.rola_write_sync_apply_failed_count(handle);
        for (nuint index = 0; index < failedCount; index++)
        {
            failed.Add(
                NativeText.Taken(
                    RorolalaBinding.rola_write_sync_apply_failed_at(handle, index)
                )
            );
        }

        var layoutRead = RorolalaBinding.rola_write_sync_apply_layout_read(handle);

        return new SyncReport(
            RorolalaBinding.rola_write_sync_apply_applied(handle),
            RorolalaBinding.rola_write_sync_apply_received(handle),
            failed,
            RorolalaBinding.rola_write_sync_apply_index_objects(handle),
            RorolalaBinding.rola_write_sync_apply_storage_objects(handle),
            !layoutRead,
            []
        )
        {
            Layout = NativeText.Taken(RorolalaBinding.rola_write_sync_apply_layout(handle)),
            Vault = NativeText.Taken(RorolalaBinding.rola_write_sync_apply_vault(handle)),
            UpToDate = RorolalaBinding.rola_write_sync_apply_up_to_date(handle),
            LayoutRead = layoutRead,
        };
    }
}

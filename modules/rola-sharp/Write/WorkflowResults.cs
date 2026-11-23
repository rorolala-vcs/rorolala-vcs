namespace RolaSharp.Write;

/// <summary>One entry a bring put in.</summary>
/// <param name="Uuid">The Uuid it is known by.</param>
/// <param name="RemotePath">The path the Vault's Layout names it by, when it names one.</param>
/// <param name="LocalPath">The path this Layout now names it by.</param>
public sealed record BringEntry(string Uuid, string? RemotePath, string LocalPath);

/// <summary>What a bring came to.</summary>
/// <param name="Entries">The entries that came in, in the order they landed.</param>
/// <param name="Failed">What could not be brought in, one line each.</param>
public sealed record BringResult(IReadOnlyList<BringEntry> Entries, IReadOnlyList<string> Failed);

/// <summary>What a variant merge came to.</summary>
/// <param name="Target">The file the variant waits to be joined into, as the request named it.</param>
/// <param name="Variant">The variant that came in, as the request named it.</param>
/// <param name="Path">The path the variant's file was written to.</param>
public sealed record BringVariantResult(string Target, string Variant, string Path);

/// <summary>What a retrack moved.</summary>
public enum RetrackKind
{
    /// <summary>The record of one path, and its work with it.</summary>
    Path = 0,

    /// <summary>The record of a tree, and its work put back from the version.</summary>
    Tree = 1,
}

/// <summary>What a retrack came to.</summary>
/// <param name="Kind">Whether the record that moved belongs to one path or to a tree.</param>
/// <param name="Path">Where the entry sits in the Layout.</param>
/// <param name="Version">The version it records now, as hex.</param>
/// <param name="Number">That version's number in its chain, when it is known.</param>
/// <param name="Changed">Whether what the path records is not what it recorded before.</param>
/// <param name="Switched">Whether the version's content was written back over the file.</param>
/// <param name="From">The Uuid the path recorded before, when the run recorded an entry.</param>
/// <param name="To">The Uuid the path records now, when the run recorded an entry.</param>
/// <param name="Forked">Whether the run gave the entry a fresh Uuid.</param>
/// <param name="Took">What each Vault handed over while the run went on.</param>
public sealed record RetrackResult(
    RetrackKind Kind,
    string Path,
    string Version,
    int? Number,
    bool Changed,
    bool Switched,
    string? From,
    string? To,
    bool Forked,
    IReadOnlyList<(string Vault, ulong Count)> Took
);

/// <summary>Which shape an alignment answered with.</summary>
public enum AlignKind
{
    /// <summary>What was done to the paths named, in the order it was done.</summary>
    Paths = 0,

    /// <summary>One tree's work was put back.</summary>
    Tree = 1,

    /// <summary>One file inside a tree was put back.</summary>
    TreeFile = 2,
}

/// <summary>One path an alignment acted on.</summary>
/// <param name="Did">What was done with it, as the native side names it, such as `deleted` or `moved`.</param>
/// <param name="What">What it was done to.</param>
public sealed record AlignChange(string Did, string What);

/// <summary>What an alignment came to.</summary>
/// <param name="Kind">Which of the three answers the run gave.</param>
/// <param name="Paths">What was done to the paths named, in the order it was done.</param>
/// <param name="Path">The tree or tree file the run put back, when that is the answer.</param>
/// <param name="Staged">Whether a tree file came from what the tree has staged.</param>
public sealed record AlignResult(
    AlignKind Kind,
    IReadOnlyList<AlignChange> Paths,
    string? Path,
    bool Staged
);

/// <summary>One subject a track plan asks a message for.</summary>
/// <param name="Tree">Whether the subject is a tree's structure rather than a file's content.</param>
/// <param name="Key">The key the editing file names the subject's section by.</param>
/// <param name="Path">The path as a person knows it.</param>
/// <param name="Digest">The content's digest in lowercase hex, or empty for a tree.</param>
public sealed record TrackSubject(bool Tree, string Key, string Path, string Digest);

/// <summary>What a track plan would record, and what stops it before anything is written.</summary>
/// <param name="Subjects">The subjects a message is asked for, in the order the run named them.</param>
/// <param name="Unchanged">How many files hold what the Layout already names.</param>
/// <param name="Prefill">What each subject is to be asked with, one per subject.</param>
/// <param name="Refused">The refusal that stops the run, described in words, or nothing when the plan goes ahead.</param>
public sealed record TrackPlan(
    IReadOnlyList<TrackSubject> Subjects,
    int Unchanged,
    IReadOnlyList<string> Prefill,
    string? Refused
)
{
    /// <summary>Whether the plan came from the native side, which an apply requires.</summary>
    internal bool Agreed { get; init; }
}

/// <summary>What happened to one subject a track applied.</summary>
public enum TrackAction
{
    /// <summary>It was not recorded before, and now is.</summary>
    Added = 0,

    /// <summary>It was recorded, and its content changed.</summary>
    Updated = 1,

    /// <summary>The version it was at was replaced by a new one.</summary>
    Replaced = 2,
}

/// <summary>One subject a track recorded.</summary>
/// <param name="Path">Where it sits in the Layout.</param>
/// <param name="Action">What happened to it.</param>
public sealed record TrackItem(string Path, TrackAction Action);

/// <summary>What a track came to.</summary>
/// <param name="Items">The subjects recorded, in the order the run named them.</param>
/// <param name="Refused">What stopped the run after the plan was made, described in words.</param>
public sealed record TrackResult(IReadOnlyList<TrackItem> Items, IReadOnlyList<string> Refused);

/// <summary>One entry an ownership plan means to move.</summary>
/// <param name="Path">Where the entry sits in the Layout.</param>
/// <param name="Uuid">The entry's Uuid.</param>
public sealed record OwnershipWanted(string Path, string Uuid);

/// <summary>Why an entry's ownership may not be moved.</summary>
/// <param name="Kind">The kind of refusal, such as `path`, `taken` or `failed`.</param>
/// <param name="Path">The path the refusal names, or an empty string when it names none.</param>
/// <param name="Owner">The account that holds the entry, or an empty string when none does.</param>
/// <param name="Cause">The failure's words, or an empty string when the refusal carries none.</param>
public sealed record OwnershipRefusal(string Kind, string Path, string Owner, string Cause);

/// <summary>What an ownership plan would move, what it refused, and what it skipped.</summary>
/// <param name="Wanted">The entries a move would act on, in path order.</param>
/// <param name="Refused">Why the gates refuse an entry.</param>
/// <param name="Skipped">How many files a directory brought in that the Layout does not name.</param>
public sealed record OwnershipPlan(
    IReadOnlyList<OwnershipWanted> Wanted,
    IReadOnlyList<OwnershipRefusal> Refused,
    ulong Skipped
);

/// <summary>One entry whose ownership the Vault agreed to move.</summary>
/// <param name="Path">The path the entry is named at.</param>
/// <param name="Uuid">The entry's Uuid.</param>
/// <param name="Owner">The owner the Vault answered with, or nothing when nobody holds it.</param>
public sealed record OwnershipEntry(string Path, string Uuid, string? Owner);

/// <summary>What an ownership move came to.</summary>
/// <param name="Entries">The entries the Vault moved.</param>
/// <param name="Failed">What was refused.</param>
public sealed record OwnershipResult(
    IReadOnlyList<OwnershipEntry> Entries,
    IReadOnlyList<OwnershipRefusal> Failed
);

/// <summary>Reads what a bring answered with.</summary>
internal static class Brings
{
    /// <summary>Reads a bring of entries.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>What came in, and what could not.</returns>
    public static BringResult Read(nint answer)
    {
        var entries = new List<BringEntry>();
        var count = RorolalaBinding.rola_write_bring_entry_count(answer);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_write_bring_entry_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                entries.Add(
                    new BringEntry(
                        NativeText.Taken(RorolalaBinding.rola_write_bring_entry_uuid(item)),
                        Optional(
                            NativeText.Taken(RorolalaBinding.rola_write_bring_entry_remote_path(item))
                        ),
                        NativeText.Taken(RorolalaBinding.rola_write_bring_entry_local_path(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_bring_entry(item);
            }
        }

        var failed = new List<string>();
        var failedCount = RorolalaBinding.rola_write_bring_failed_count(answer);
        for (nuint index = 0; index < failedCount; index++)
        {
            failed.Add(
                NativeText.Taken(RorolalaBinding.rola_write_bring_failed_at(answer, index))
            );
        }

        return new BringResult(entries, failed);
    }

    /// <summary>Reads a variant merge.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>The file it waits to be joined into, the variant, and where its file was written.</returns>
    public static BringVariantResult ReadVariant(nint answer) =>
        new(
            NativeText.Taken(RorolalaBinding.rola_write_bring_target(answer)),
            NativeText.Taken(RorolalaBinding.rola_write_bring_variant(answer)),
            NativeText.Taken(RorolalaBinding.rola_write_bring_path(answer))
        );

    private static string? Optional(string text) => text.Length == 0 ? null : text;
}

/// <summary>Reads what a retrack answered with.</summary>
internal static class Retracks
{
    /// <summary>Reads a retrack's answer.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>What the record moved to.</returns>
    /// <exception cref="RolaWriteError">The answer names a kind this build does not know.</exception>
    public static RetrackResult Read(nint answer)
    {
        var kind = NativeText.Taken(RorolalaBinding.rola_write_retrack_kind(answer));
        var took = new List<(string Vault, ulong Count)>();
        var tookCount = RorolalaBinding.rola_write_retrack_took_count(answer);
        for (nuint index = 0; index < tookCount; index++)
        {
            var item = RorolalaBinding.rola_write_retrack_took_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                took.Add(
                    (
                        NativeText.Taken(RorolalaBinding.rola_write_retrack_took_vault(item)),
                        RorolalaBinding.rola_write_retrack_took_objects(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_retrack_took(item);
            }
        }

        return kind switch
        {
            "path" => new RetrackResult(
                RetrackKind.Path,
                NativeText.Taken(RorolalaBinding.rola_write_retrack_path(answer)),
                NativeText.Taken(RorolalaBinding.rola_write_retrack_version(answer)),
                Number(answer),
                RorolalaBinding.rola_write_retrack_changed(answer),
                RorolalaBinding.rola_write_retrack_switched(answer),
                Optional(NativeText.Taken(RorolalaBinding.rola_write_retrack_from(answer))),
                Optional(NativeText.Taken(RorolalaBinding.rola_write_retrack_to(answer))),
                RorolalaBinding.rola_write_retrack_forked(answer),
                took
            ),
            "tree" => new RetrackResult(
                RetrackKind.Tree,
                NativeText.Taken(RorolalaBinding.rola_write_retrack_path(answer)),
                NativeText.Taken(RorolalaBinding.rola_write_retrack_version(answer)),
                null,
                false,
                false,
                null,
                null,
                false,
                took
            ),
            _ => throw new RolaWriteError(
                RolaWriteErrorKind.Internal,
                null,
                "the retrack answered with no kind this build knows"
            ),
        };
    }

    /// <summary>The version's number in its chain, when the run knows it.</summary>
    private static int? Number(nint answer) =>
        RorolalaBinding.rola_write_retrack_number_present(answer)
        && RorolalaBinding.rola_write_retrack_number(answer) <= int.MaxValue
            ? (int)RorolalaBinding.rola_write_retrack_number(answer)
            : null;

    private static string? Optional(string text) => text.Length == 0 ? null : text;
}

/// <summary>Reads what an alignment answered with.</summary>
internal static class Aligns
{
    /// <summary>Reads an alignment's answer.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>What the run did.</returns>
    /// <exception cref="RolaWriteError">
    /// The run refused to act, or answered with no kind this build knows.
    /// </exception>
    public static AlignResult Read(nint answer)
    {
        switch (NativeText.Taken(RorolalaBinding.rola_write_align_kind(answer)))
        {
            case "paths":
            {
                var paths = new List<AlignChange>();
                var count = RorolalaBinding.rola_write_align_paths_count(answer);
                for (nuint index = 0; index < count; index++)
                {
                    var item = RorolalaBinding.rola_write_align_paths_at(answer, index);
                    if (item == 0)
                    {
                        continue;
                    }

                    try
                    {
                        paths.Add(
                            new AlignChange(
                                NativeText.Taken(RorolalaBinding.rola_write_align_entry_did(item)),
                                NativeText.Taken(RorolalaBinding.rola_write_align_entry_what(item))
                            )
                        );
                    }
                    finally
                    {
                        RorolalaBinding.free_rola_write_align_entry(item);
                    }
                }

                return new AlignResult(AlignKind.Paths, paths, null, false);
            }
            case "tree":
                return new AlignResult(
                    AlignKind.Tree,
                    [],
                    NativeText.Taken(RorolalaBinding.rola_write_align_path(answer)),
                    false
                );
            case "tree_file":
                return new AlignResult(
                    AlignKind.TreeFile,
                    [],
                    NativeText.Taken(RorolalaBinding.rola_write_align_path(answer)),
                    RorolalaBinding.rola_write_align_staged(answer)
                );
            case "refused":
                throw Refused(answer);
            default:
                throw new RolaWriteError(
                    RolaWriteErrorKind.Internal,
                    null,
                    "the align answered with no kind this build knows"
                );
        }
    }

    /// <summary>The failure a refused alignment is reported as.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>The failure, whose reason is the refusal's words.</returns>
    private static RolaWriteError Refused(nint answer)
    {
        var refusal = RorolalaBinding.rola_write_align_refusal(answer);
        if (refusal == 0)
        {
            return new RolaWriteError(
                RolaWriteErrorKind.Internal,
                null,
                "the align refused without a refusal"
            );
        }

        try
        {
            var kind = NativeText.Taken(RorolalaBinding.rola_write_align_refusal_kind(refusal));
            var path = NativeText.Taken(RorolalaBinding.rola_write_align_refusal_path(refusal));
            if (path.Length == 0)
            {
                path = NativeText.Taken(RorolalaBinding.rola_write_align_refusal_from(refusal));
            }

            if (path.Length == 0)
            {
                path = NativeText.Taken(RorolalaBinding.rola_write_align_refusal_to(refusal));
            }

            var cause = NativeText.Taken(RorolalaBinding.rola_write_align_refusal_cause(refusal));
            var move = NativeText.Taken(
                RorolalaBinding.rola_write_align_refusal_move_kind(refusal)
            );
            var reason = cause.Length > 0 && move.Length > 0
                ? $"{kind}: {cause} ({move})"
                : cause.Length > 0
                    ? $"{kind}: {cause}"
                    : move.Length > 0
                        ? $"{kind}: {move}"
                        : kind;

            return new RolaWriteError(
                RolaWriteErrorKind.Precondition,
                path.Length == 0 ? null : path,
                reason
            );
        }
        finally
        {
            RorolalaBinding.free_rola_write_align_refusal(refusal);
        }
    }
}

/// <summary>Reads what the track workflows answered with.</summary>
internal static class Tracks
{
    /// <summary>Reads a plan's answer.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>What a run would record, and what stops it.</returns>
    public static TrackPlan ReadPlan(nint answer)
    {
        var subjects = new List<TrackSubject>();
        var count = RorolalaBinding.rola_write_track_plan_subject_count(answer);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_write_track_plan_subject_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                subjects.Add(
                    new TrackSubject(
                        RorolalaBinding.rola_write_track_subject_tree(item),
                        NativeText.Taken(RorolalaBinding.rola_write_track_subject_key(item)),
                        NativeText.Taken(RorolalaBinding.rola_write_track_subject_path(item)),
                        NativeText.Taken(RorolalaBinding.rola_write_track_subject_digest(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_track_subject(item);
            }
        }

        var prefill = new List<string>();
        var prefillCount = RorolalaBinding.rola_write_track_plan_prefill_count(answer);
        for (nuint index = 0; index < prefillCount; index++)
        {
            prefill.Add(
                NativeText.Taken(RorolalaBinding.rola_write_track_plan_prefill_at(answer, index))
            );
        }

        var refused = Refusal(RorolalaBinding.rola_write_track_plan_refused(answer));
        var plan = new TrackPlan(
            subjects,
            (int)RorolalaBinding.rola_write_track_plan_unchanged(answer),
            prefill,
            refused
        );

        // The apply reads back only the subjects the plan promised to record, so what this side
        // keeps is each subject's path and the digest the plan read.
        return plan with { Agreed = true };
    }

    /// <summary>Reads an apply's answer.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>The subjects recorded, and what was refused.</returns>
    /// <exception cref="RolaWriteError">The answer names an action this build does not know.</exception>
    public static TrackResult ReadApply(nint answer)
    {
        var items = new List<TrackItem>();
        var count = RorolalaBinding.rola_write_track_apply_item_count(answer);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_write_track_apply_item_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                items.Add(
                    new TrackItem(
                        NativeText.Taken(RorolalaBinding.rola_write_track_item_path(item)),
                        ActionOf(NativeText.Taken(RorolalaBinding.rola_write_track_item_action(item)))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_track_item(item);
            }
        }

        var refused = new List<string>();
        var refusedCount = RorolalaBinding.rola_write_track_apply_refused_count(answer);
        for (nuint index = 0; index < refusedCount; index++)
        {
            var item = RorolalaBinding.rola_write_track_apply_refused_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                refused.Add(Describe(item));
            }
            finally
            {
                RorolalaBinding.free_rola_write_track_refusal(item);
            }
        }

        return new TrackResult(items, refused);
    }

    /// <summary>The refusal a plan carries, in words, or nothing when it goes ahead.</summary>
    private static string? Refusal(nint refusal)
    {
        if (refusal == 0)
        {
            return null;
        }

        try
        {
            return Describe(refusal);
        }
        finally
        {
            RorolalaBinding.free_rola_write_track_refusal(refusal);
        }
    }

    /// <summary>One refusal, in the words its own fields say.</summary>
    private static string Describe(nint refusal)
    {
        var kind = NativeText.Taken(RorolalaBinding.rola_write_track_refusal_kind(refusal));
        var path = NativeText.Taken(RorolalaBinding.rola_write_track_refusal_path(refusal));
        if (path.Length == 0)
        {
            path = NativeText.Taken(RorolalaBinding.rola_write_track_refusal_name(refusal));
        }

        if (path.Length == 0)
        {
            var count = RorolalaBinding.rola_write_track_refusal_paths_count(refusal);
            if (count > 0)
            {
                path = NativeText.Taken(
                    RorolalaBinding.rola_write_track_refusal_paths_at(refusal, 0)
                );
            }
        }

        if (path.Length == 0)
        {
            path = NativeText.Taken(RorolalaBinding.rola_write_track_refusal_cause(refusal));
        }

        return path.Length == 0 ? kind : $"{kind}: {path}";
    }

    /// <summary>What an action name means.</summary>
    /// <param name="text">The action as the native side names it.</param>
    /// <returns>The action.</returns>
    /// <exception cref="RolaWriteError">The name is not one this build knows.</exception>
    private static TrackAction ActionOf(string text) =>
        text switch
        {
            "added" => TrackAction.Added,
            "updated" => TrackAction.Updated,
            "replaced" => TrackAction.Replaced,
            _ => throw new RolaWriteError(
                RolaWriteErrorKind.Internal,
                null,
                "the track apply answered with no action this build knows"
            ),
        };
}

/// <summary>Reads what the ownership workflows answered with.</summary>
internal static class Ownerships
{
    /// <summary>Reads a plan's answer.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>What a move would act on, what it refused, and what it skipped.</returns>
    public static OwnershipPlan ReadPlan(nint answer)
    {
        var wanted = new List<OwnershipWanted>();
        var count = RorolalaBinding.rola_write_ownership_plan_wanted_count(answer);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_write_ownership_plan_wanted_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                wanted.Add(
                    new OwnershipWanted(
                        NativeText.Taken(RorolalaBinding.rola_write_ownership_wanted_path(item)),
                        NativeText.Taken(RorolalaBinding.rola_write_ownership_wanted_uuid(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_ownership_wanted(item);
            }
        }

        return new OwnershipPlan(
            wanted,
            Refusals(
                answer,
                RorolalaBinding.rola_write_ownership_plan_refused_count,
                RorolalaBinding.rola_write_ownership_plan_refused_at
            ),
            RorolalaBinding.rola_write_ownership_plan_skipped(answer)
        );
    }

    /// <summary>Reads an apply's answer.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <returns>The entries moved, and what was refused.</returns>
    public static OwnershipResult ReadApply(nint answer)
    {
        var entries = new List<OwnershipEntry>();
        var count = RorolalaBinding.rola_write_ownership_apply_entry_count(answer);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_write_ownership_apply_entry_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                entries.Add(
                    new OwnershipEntry(
                        NativeText.Taken(RorolalaBinding.rola_write_ownership_entry_path(item)),
                        NativeText.Taken(RorolalaBinding.rola_write_ownership_entry_uuid(item)),
                        Optional(
                            NativeText.Taken(RorolalaBinding.rola_write_ownership_entry_owner(item))
                        )
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_ownership_entry(item);
            }
        }

        return new OwnershipResult(
            entries,
            Refusals(
                answer,
                RorolalaBinding.rola_write_ownership_apply_failed_count,
                RorolalaBinding.rola_write_ownership_apply_failed_at
            )
        );
    }

    /// <summary>The refusals an answer lists.</summary>
    /// <param name="answer">The answer, which this does not release.</param>
    /// <param name="count">How many refusals there are.</param>
    /// <param name="at">One refusal, or zero past the end.</param>
    /// <returns>The refusals.</returns>
    private static List<OwnershipRefusal> Refusals(
        nint answer,
        Func<nint, nuint> count,
        Func<nint, nuint, nint> at
    )
    {
        var refusals = new List<OwnershipRefusal>();
        var total = count(answer);
        for (nuint index = 0; index < total; index++)
        {
            var item = at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                refusals.Add(
                    new OwnershipRefusal(
                        NativeText.Taken(
                            RorolalaBinding.rola_write_ownership_refusal_kind(item)
                        ),
                        NativeText.Taken(
                            RorolalaBinding.rola_write_ownership_refusal_path(item)
                        ),
                        NativeText.Taken(
                            RorolalaBinding.rola_write_ownership_refusal_owner(item)
                        ),
                        NativeText.Taken(
                            RorolalaBinding.rola_write_ownership_refusal_cause(item)
                        )
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_ownership_refusal(item);
            }
        }

        return refusals;
    }

    private static string? Optional(string text) => text.Length == 0 ? null : text;
}

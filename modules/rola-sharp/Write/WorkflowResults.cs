using System.Text.Json;
using System.Text.Json.Serialization;

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
/// <param name="Refused">The refusal that stops the run, as its raw JSON, or nothing when the plan goes ahead.</param>
public sealed record TrackPlan(
    IReadOnlyList<TrackSubject> Subjects,
    int Unchanged,
    IReadOnlyList<string> Prefill,
    string? Refused
)
{
    /// <summary>The plan's own JSON, which an apply sends back unchanged.</summary>
    internal string Json { get; init; } = string.Empty;
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
/// <param name="Refused">What stopped the run after the plan was made, one raw JSON object each.</param>
public sealed record TrackResult(IReadOnlyList<TrackItem> Items, IReadOnlyList<string> Refused);

/// <summary>One entry an ownership plan means to move.</summary>
/// <param name="Path">Where the entry sits in the Layout.</param>
/// <param name="Uuid">The entry's Uuid.</param>
public sealed record OwnershipWanted(
    [property: JsonPropertyName("path")] string Path,
    [property: JsonPropertyName("uuid")] string Uuid
);

/// <summary>What an ownership plan would move, what it refused, and what it skipped.</summary>
/// <param name="Wanted">The entries a move would act on, in path order.</param>
/// <param name="Refused">Why the gates refuse an entry, one raw JSON object each.</param>
/// <param name="Skipped">How many files a directory brought in that the Layout does not name.</param>
public sealed record OwnershipPlan(
    IReadOnlyList<OwnershipWanted> Wanted,
    IReadOnlyList<string> Refused,
    ulong Skipped
);

/// <summary>One entry whose ownership the Vault agreed to move.</summary>
/// <param name="Path">The path the entry is named at.</param>
/// <param name="Uuid">The entry's Uuid.</param>
/// <param name="Owner">The owner the Vault answered with, or nothing when nobody holds it.</param>
public sealed record OwnershipEntry(string Path, string Uuid, string? Owner);

/// <summary>What an ownership move came to.</summary>
/// <param name="Entries">The entries the Vault moved.</param>
/// <param name="Failed">What was refused, one raw JSON object each.</param>
public sealed record OwnershipResult(
    IReadOnlyList<OwnershipEntry> Entries,
    IReadOnlyList<string> Failed
);

/// <summary>Reads shapes the workflows share out of the JSON the native side writes.</summary>
internal static class WorkflowJson
{
    /// <summary>The strings under an array member, in the order they came.</summary>
    /// <param name="root">The answer.</param>
    /// <param name="name">The member holding the array.</param>
    /// <returns>The strings, or nothing when the member holds none.</returns>
    public static List<string> Texts(JsonElement root, string name)
    {
        var texts = new List<string>();
        if (RolaJson.Items(root, name) is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is JsonValueKind.String)
                {
                    texts.Add(item.GetString() ?? string.Empty);
                }
            }
        }

        return texts;
    }

    /// <summary>The raw JSON of every entry of an array member, in the order they came.</summary>
    /// <param name="root">The answer.</param>
    /// <param name="name">The member holding the array.</param>
    /// <returns>The raw JSON, or nothing when the member holds none.</returns>
    public static List<string> Raw(JsonElement root, string name)
    {
        var texts = new List<string>();
        if (RolaJson.Items(root, name) is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                texts.Add(item.GetRawText());
            }
        }

        return texts;
    }

    /// <summary>The Vaults and counts under the `took` member, in the order they came.</summary>
    /// <param name="root">The answer.</param>
    /// <returns>What each Vault handed over.</returns>
    public static List<(string Vault, ulong Count)> Took(JsonElement root)
    {
        var took = new List<(string Vault, ulong Count)>();
        if (RolaJson.Items(root, "took") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Array)
                {
                    continue;
                }

                var parts = item.EnumerateArray().ToArray();
                if (parts.Length == 2 && parts[0].ValueKind is JsonValueKind.String)
                {
                    took.Add((parts[0].GetString() ?? string.Empty, parts[1].GetUInt64()));
                }
            }
        }

        return took;
    }
}

/// <summary>Reads what a bring answered with.</summary>
internal static class Brings
{
    /// <summary>Reads a bring's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>What came in, and what could not.</returns>
    public static BringResult Parse(JsonElement root)
    {
        var entries = new List<BringEntry>();
        if (RolaJson.Items(root, "entries") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Object)
                {
                    continue;
                }

                entries.Add(
                    new BringEntry(
                        RolaJson.Text(item, "uuid") ?? string.Empty,
                        RolaJson.Text(item, "remote_path"),
                        RolaJson.Text(item, "local_path") ?? string.Empty
                    )
                );
            }
        }

        return new BringResult(entries, WorkflowJson.Texts(root, "failed"));
    }

    /// <summary>Reads a variant merge's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>The file it waits to be joined into, the variant, and where its file was written.</returns>
    public static BringVariantResult ParseVariant(JsonElement root) =>
        new(
            RolaJson.Text(root, "target") ?? string.Empty,
            RolaJson.Text(root, "variant") ?? string.Empty,
            RolaJson.Text(root, "path") ?? string.Empty
        );
}

/// <summary>Reads what a retrack answered with.</summary>
internal static class Retracks
{
    /// <summary>Reads a retrack's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>What the record moved to.</returns>
    public static RetrackResult Parse(JsonElement root)
    {
        var path = RolaJson.Text(root, "path") ?? string.Empty;
        var version = RolaJson.Text(root, "version") ?? string.Empty;
        var took = WorkflowJson.Took(root);

        return RolaJson.Text(root, "kind") switch
        {
            "path" => new RetrackResult(
                RetrackKind.Path,
                path,
                version,
                Number(root),
                RolaJson.Flag(root, "changed"),
                RolaJson.Flag(root, "switched"),
                RolaJson.Text(root, "from"),
                RolaJson.Text(root, "to"),
                RolaJson.Flag(root, "forked"),
                took
            ),
            "tree" => new RetrackResult(
                RetrackKind.Tree,
                path,
                version,
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

    /// <summary>The version's number in its chain, when it is known.</summary>
    /// <param name="root">The answer.</param>
    /// <returns>The number, or nothing.</returns>
    private static int? Number(JsonElement root) =>
        RolaJson.Number(root, "number") is { } number && number <= int.MaxValue
            ? (int)number
            : null;
}

/// <summary>Reads what an alignment answered with.</summary>
internal static class Aligns
{
    /// <summary>Reads an alignment's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>What the run did.</returns>
    /// <exception cref="RolaWriteError">
    /// The run refused to act, or answered with no kind this build knows.
    /// </exception>
    public static AlignResult Parse(JsonElement root)
    {
        switch (RolaJson.Text(root, "kind"))
        {
            case "paths":
            {
                var paths = new List<AlignChange>();
                if (RolaJson.Items(root, "paths") is { } items)
                {
                    foreach (var item in items.EnumerateArray())
                    {
                        if (item.ValueKind is not JsonValueKind.Object)
                        {
                            continue;
                        }

                        paths.Add(
                            new AlignChange(
                                RolaJson.Text(item, "did") ?? string.Empty,
                                RolaJson.Text(item, "what") ?? string.Empty
                            )
                        );
                    }
                }

                return new AlignResult(AlignKind.Paths, paths, null, false);
            }
            case "tree":
                return new AlignResult(
                    AlignKind.Tree,
                    [],
                    RolaJson.Text(root, "path"),
                    false
                );
            case "tree_file":
                return new AlignResult(
                    AlignKind.TreeFile,
                    [],
                    RolaJson.Text(root, "path"),
                    RolaJson.Flag(root, "staged")
                );
            case "refused":
                throw Refused(root);
            default:
                throw new RolaWriteError(
                    RolaWriteErrorKind.Internal,
                    null,
                    "the align answered with no kind this build knows"
                );
        }
    }

    /// <summary>The failure a refused alignment is reported as.</summary>
    /// <param name="root">The answer.</param>
    /// <returns>The failure, whose reason is the refusal's own JSON.</returns>
    private static RolaWriteError Refused(JsonElement root)
    {
        if (!root.TryGetProperty("refusal", out var refusal))
        {
            return new RolaWriteError(
                RolaWriteErrorKind.Internal,
                null,
                "the align refused without a refusal"
            );
        }

        var path =
            RolaJson.Text(refusal, "path")
            ?? RolaJson.Text(refusal, "from")
            ?? RolaJson.Text(refusal, "to");

        return new RolaWriteError(
            RolaWriteErrorKind.Precondition,
            path,
            refusal.GetRawText()
        );
    }
}

/// <summary>Reads what the track workflows answered with.</summary>
internal static class Tracks
{
    /// <summary>Reads a plan's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>What a run would record, and what stops it.</returns>
    public static TrackPlan ParsePlan(JsonElement root)
    {
        var subjects = new List<TrackSubject>();
        if (RolaJson.Items(root, "subjects") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Object)
                {
                    continue;
                }

                subjects.Add(
                    new TrackSubject(
                        RolaJson.Flag(item, "tree"),
                        RolaJson.Text(item, "key") ?? string.Empty,
                        RolaJson.Text(item, "path") ?? string.Empty,
                        RolaJson.Text(item, "digest") ?? string.Empty
                    )
                );
            }
        }

        return new TrackPlan(
            subjects,
            (int)(RolaJson.Number(root, "unchanged") ?? 0),
            WorkflowJson.Texts(root, "prefill"),
            Refusal(root)
        );
    }

    /// <summary>Reads an apply's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>The subjects recorded, and what was refused.</returns>
    /// <exception cref="RolaWriteError">The answer names an action this build does not know.</exception>
    public static TrackResult ParseApply(JsonElement root)
    {
        var items = new List<TrackItem>();
        if (RolaJson.Items(root, "items") is { } entries)
        {
            foreach (var item in entries.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Object)
                {
                    continue;
                }

                items.Add(
                    new TrackItem(
                        RolaJson.Text(item, "path") ?? string.Empty,
                        ActionOf(RolaJson.Text(item, "action"))
                    )
                );
            }
        }

        return new TrackResult(items, WorkflowJson.Raw(root, "refused"));
    }

    /// <summary>The refusal a plan carries, as its raw JSON, or nothing when it goes ahead.</summary>
    /// <param name="root">The answer.</param>
    /// <returns>The refusal's JSON, or nothing.</returns>
    private static string? Refusal(JsonElement root) =>
        root.ValueKind is JsonValueKind.Object
        && root.TryGetProperty("refused", out var refused)
        && refused.ValueKind is JsonValueKind.Object
            ? refused.GetRawText()
            : null;

    /// <summary>What an action name means.</summary>
    /// <param name="text">The action as the native side names it.</param>
    /// <returns>The action.</returns>
    /// <exception cref="RolaWriteError">The name is not one this build knows.</exception>
    private static TrackAction ActionOf(string? text) =>
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
    /// <param name="root">The answer's JSON.</param>
    /// <returns>What a move would act on, what it refused, and what it skipped.</returns>
    public static OwnershipPlan ParsePlan(JsonElement root)
    {
        var wanted = new List<OwnershipWanted>();
        if (RolaJson.Items(root, "wanted") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Object)
                {
                    continue;
                }

                wanted.Add(
                    new OwnershipWanted(
                        RolaJson.Text(item, "path") ?? string.Empty,
                        RolaJson.Text(item, "uuid") ?? string.Empty
                    )
                );
            }
        }

        return new OwnershipPlan(
            wanted,
            WorkflowJson.Raw(root, "refused"),
            RolaJson.Number(root, "skipped") ?? 0
        );
    }

    /// <summary>Reads an apply's answer.</summary>
    /// <param name="root">The answer's JSON.</param>
    /// <returns>The entries moved, and what was refused.</returns>
    public static OwnershipResult ParseApply(JsonElement root)
    {
        var entries = new List<OwnershipEntry>();
        if (RolaJson.Items(root, "entries") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is not JsonValueKind.Object)
                {
                    continue;
                }

                entries.Add(
                    new OwnershipEntry(
                        RolaJson.Text(item, "path") ?? string.Empty,
                        RolaJson.Text(item, "uuid") ?? string.Empty,
                        RolaJson.Text(item, "owner")
                    )
                );
            }
        }

        return new OwnershipResult(entries, WorkflowJson.Raw(root, "failed"));
    }
}

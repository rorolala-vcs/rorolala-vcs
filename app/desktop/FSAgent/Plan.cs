namespace RorolalaFSAgent;

/// <summary>What was decided for a conflicting item, and what happens to one that does not.</summary>
internal enum Resolution
{
    /// <summary>A conflict nobody has answered yet.</summary>
    Undecided,

    /// <summary>Run the command against the target as it stands.</summary>
    AsIs,

    /// <summary>Run nothing.</summary>
    Skip,

    /// <summary>Remove the existing target, then run.</summary>
    Replace,

    /// <summary>Run against a free name beside the existing target.</summary>
    Rename,
}

/// <summary>One thing to be done: a source, its resolved target, and how it stands.</summary>
internal sealed class Item
{
    /// <summary>The source path, as it was given.</summary>
    public required string From { get; init; }

    /// <summary>
    /// The target path, which is <c>&lt;to&gt;/&lt;file name of from&gt;</c> for a transfer.
    /// </summary>
    /// <remarks>
    /// Empty until the item is known to be runnable, so that a failure before a target exists is
    /// reported without one — the same shape the JSON example has. Every removal leaves it empty,
    /// since a removal has no target.
    /// </remarks>
    public string To { get; set; } = string.Empty;

    /// <summary>Why the item cannot run at all, or nothing when it can.</summary>
    public string? Problem { get; set; }

    /// <summary>Whether the target already exists and has to be answered for.</summary>
    public bool Conflict { get; set; }

    /// <summary>What has been decided for the item.</summary>
    public Resolution Resolution { get; set; } = Resolution.Undecided;
}

/// <summary>The items a run is made of, and whether the run was called off.</summary>
internal sealed class Plan
{
    /// <summary>The items, in the order they were given.</summary>
    public required IReadOnlyList<Item> Items { get; init; }

    /// <summary>Whether the conflict window was dismissed, which calls off the whole run.</summary>
    public bool Cancelled { get; set; }

    /// <summary>Whether any item has a conflict to answer.</summary>
    public bool HasConflicts => Items.Any(item => item.Conflict);

    /// <summary>
    /// Works out every item's target and problem, and which targets already exist.
    /// </summary>
    /// <remarks>
    /// Everything that can be known before anything is touched is worked out here, so that the
    /// conflict window can be answered for every conflict before the first command runs — which is
    /// what makes calling the run off leave nothing done at all.
    /// </remarks>
    /// <param name="parsed">The run the command line named.</param>
    public static Plan Build(ParsedCommand parsed)
    {
        var items = new List<Item>(parsed.Pairs.Count);

        foreach (var pair in parsed.Pairs)
        {
            var item = new Item { From = pair.From };

            if (parsed.Operation is Operation.RemoveDirs or Operation.RemoveFiles)
            {
                if (!Pathing.Exists(pair.From))
                {
                    item.Problem = $"the source `{pair.From}` does not exist";
                }
                else
                {
                    item.Resolution = Resolution.AsIs;
                }
            }
            else
            {
                PlanTransfer(item, pair);
            }

            items.Add(item);
        }

        return new Plan { Items = items };
    }

    /// <summary>Works out one transfer's target, or why it cannot be made.</summary>
    private static void PlanTransfer(Item item, Pair pair)
    {
        if (!Pathing.Exists(pair.From))
        {
            item.Problem = $"the source `{pair.From}` does not exist";
            return;
        }

        if (string.IsNullOrEmpty(pair.To))
        {
            item.Problem = $"the pair for `{pair.From}` names no destination directory";
            return;
        }

        if (!Directory.Exists(pair.To))
        {
            item.Problem = $"the destination `{pair.To}` is not a directory";
            return;
        }

        // A source named with a trailing separator has no file name to take, so the separator is
        // dropped first: `to/a/` transfers as `a`.
        var name = Path.GetFileName(Path.TrimEndingDirectorySeparator(pair.From));

        if (name.Length == 0)
        {
            item.Problem = $"the source `{pair.From}` has no file name to transfer";
            return;
        }

        var target = Path.Combine(pair.To, name);

        string why;

        try
        {
            why = Reject(pair.From, target);
        }
        catch (Exception error)
        {
            item.Problem = $"the paths of `{pair.From}` could not be resolved: {error.Message}";
            return;
        }

        if (why.Length > 0)
        {
            item.Problem = why;
            return;
        }

        item.To = target;
        item.Conflict = Pathing.Exists(target);

        if (!item.Conflict)
        {
            item.Resolution = Resolution.AsIs;
        }
    }

    /// <summary>The reason a transfer is illegal, or empty when it is not.</summary>
    private static string Reject(string from, string target)
    {
        if (Pathing.Same(from, target))
        {
            return "the source and the target are the same path";
        }

        // A directory copied or moved into its own subtree is never a copy — the destination is
        // being written from inside itself — so it is refused before anything is touched.
        if (Directory.Exists(from) && Pathing.Inside(target, from))
        {
            return $"the directory `{from}` cannot be transferred into itself";
        }

        return string.Empty;
    }
}

/// <summary>
/// Path facts the planner and the runner both need, answered the same way for both.
/// </summary>
internal static class Pathing
{
    /// <summary>Whether anything at all is at a path.</summary>
    public static bool Exists(string path) => File.Exists(path) || Directory.Exists(path);

    /// <summary>Whether two paths name the same place.</summary>
    public static bool Same(string one, string other) =>
        string.Equals(Full(one), Full(other), Comparison);

    /// <summary>Whether a path sits inside a directory, at any depth.</summary>
    public static bool Inside(string path, string directory) =>
        Full(path).StartsWith(Full(directory) + Path.DirectorySeparatorChar, Comparison);

    /// <summary>
    /// The comparison the platform names paths with, so two spellings of one path compare equal
    /// where the file system says they are.
    /// </summary>
    private static StringComparison Comparison =>
        OperatingSystem.IsWindows() ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;

    /// <summary>A path made absolute and stripped of a trailing separator, for comparison.</summary>
    private static string Full(string path) =>
        Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
}

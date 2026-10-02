namespace FileSystemPlugin;

/// <summary>What is being done to each item.</summary>
internal enum Operation
{
    /// <summary>Copy a source to a destination directory.</summary>
    Copy,

    /// <summary>Move a source to a destination directory.</summary>
    Move,

    /// <summary>
    /// Move a source to the whole path it is to have, rather than into a directory.
    /// </summary>
    /// <remarks>
    /// A rename is a move and nothing else — the same command carries it out — and what tells it apart is
    /// that the target is named outright: a new name beside the old one, rather than a directory the old
    /// name is taken into.
    /// </remarks>
    Rename,

    /// <summary>Remove directories.</summary>
    RemoveDirs,

    /// <summary>Remove files.</summary>
    RemoveFiles,
}

/// <summary>One source and, for a transfer, the destination it goes to.</summary>
/// <param name="From">The source path, as it was given.</param>
/// <param name="To">
/// The destination directory, or the whole path it is to have for a rename; empty when the item names
/// none, which is every removal.
/// </param>
internal readonly record struct Pair(string From, string To);

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
    /// reported without one. Every removal leaves it empty, since a removal has no target.
    /// </remarks>
    public string To { get; set; } = string.Empty;

    /// <summary>Why the item cannot run at all, or nothing when it can.</summary>
    public string? Problem { get; set; }

    /// <summary>Whether the target already exists and has to be answered for.</summary>
    public bool Conflict { get; set; }

    /// <summary>
    /// Whether the target is the source itself.
    /// </summary>
    /// <remarks>
    /// What an in-place copy is: copying an entry into the directory it already sits in means putting a second
    /// of it beside the first, so the name it would take is taken — by itself. Replacing is no answer to that,
    /// since the target is the very thing being copied: a replace would take it away and leave nothing to copy.
    /// </remarks>
    public bool OntoItself { get; set; }

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
    /// <param name="operation">What is being done to every item.</param>
    /// <param name="pairs">The items, in the order they were given.</param>
    public static Plan Build(Operation operation, IReadOnlyList<Pair> pairs)
    {
        var items = new List<Item>(pairs.Count);
        var copying = operation is Operation.Copy;
        var renaming = operation is Operation.Rename;

        foreach (var pair in pairs)
        {
            var item = new Item { From = pair.From };

            if (operation is Operation.RemoveDirs or Operation.RemoveFiles)
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
                PlanTransfer(item, pair, copying, renaming);
            }

            items.Add(item);
        }

        return new Plan { Items = items };
    }

    /// <summary>Works out one transfer's target, or why it cannot be made.</summary>
    /// <param name="item">The item to plan.</param>
    /// <param name="pair">The source and where it goes.</param>
    /// <param name="copying">Whether this run copies rather than moves.</param>
    /// <param name="renaming">Whether the destination is the whole path rather than a directory.</param>
    private static void PlanTransfer(Item item, Pair pair, bool copying, bool renaming)
    {
        if (!Pathing.Exists(pair.From))
        {
            item.Problem = $"the source `{pair.From}` does not exist";
            return;
        }

        if (string.IsNullOrEmpty(pair.To))
        {
            item.Problem = $"the pair for `{pair.From}` names no destination";
            return;
        }

        string target;

        if (renaming)
        {
            target = pair.To;
        }
        else
        {
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

            target = Path.Combine(pair.To, name);
        }

        if (Pathing.Same(pair.From, target))
        {
            // An in-place copy wants a second of the same name beside the first, so the name it would take is
            // taken — by itself. That is a conflict to answer rather than a transfer to refuse: the answer of
            // putting a second one beside it is exactly what a copy in place means. A move onto itself is
            // another matter and stays refused: there is nowhere to move it to.
            if (!copying)
            {
                item.Problem = "the source and the target are the same path";
                return;
            }

            item.To = target;
            item.Conflict = true;
            item.OntoItself = true;
            return;
        }

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

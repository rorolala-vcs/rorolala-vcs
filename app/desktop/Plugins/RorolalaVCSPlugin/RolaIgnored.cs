using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// Hides what Rorolala's ignore rules govern: the entries a Workspace's rules leave out.
/// </summary>
/// <remarks>
/// The rules are Rorolala's to read and are not reimplemented here: the syntax is gitignore's, the
/// order the rule files apply in — deeper weaker than shallower, and a later name over an earlier one —
/// is the loader's, and a second implementation would be a second answer to what the command line
/// ignores by. So the set is loaded through the C ABI, once per Workspace rather than once per
/// directory, and kept until <see cref="Forget"/> says the files may have changed.
/// <para>
/// What is answered is the rules and only the rules: an entry they match is hidden whether or not the
/// Layout already records it. The reading `rola status` makes is finer — it keeps what the Layout names
/// out of the rules' jurisdiction — so a rule matching a recorded path hides it here and shows it there.
/// That is the one place the two can disagree, and it is stated rather than papered over: telling a
/// recorded path from an unrecorded one needs the reading, which is a walk of the tree this provider is
/// asked about one row at a time.
/// </para>
/// </remarks>
public sealed class RolaIgnored : IEntryHideProvider
{
    /// <summary>What this provider is switched by, which is written into the user's preference file.</summary>
    public const string Value = "rola_ignored";

    /// <summary>
    /// The label is stated by this plugin rather than by the File System plugin, though its key says
    /// otherwise.
    /// </summary>
    /// <remarks>
    /// The key is namespaced as this plugin's, and the words are in this plugin's translation directory:
    /// the hide setting is declared by the File System plugin, but an option of it is a picture and a
    /// label any registered provider may supply.
    /// </remarks>
    public const string Label = "rorolala_vcs.hides.rola_ignored";

    /// <inheritdoc />
    public string Id => Value;

    /// <inheritdoc />
    public string LabelKey => Label;

    /// <inheritdoc />
    public bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return RolaIgnores.Hides(entry);
    }

    /// <inheritdoc />
    public bool Hides(Entry entry, string root)
    {
        ArgumentNullException.ThrowIfNull(entry);
        ArgumentNullException.ThrowIfNull(root);

        return RolaIgnores.Hides(entry, root);
    }

    /// <inheritdoc />
    public bool Ready(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        return RolaIgnores.Ready(root);
    }

    /// <inheritdoc />
    public void Prepare(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        RolaIgnores.Prepare(root);
    }

    /// <inheritdoc />
    public void Forget() => RolaIgnores.Forget();
}

/// <summary>
/// What a Workspace's ignore rules govern, loaded once per Workspace and kept.
/// </summary>
/// <remarks>
/// One set per Workspace rather than one per directory, because the rules are the Workspace's: a view
/// that reads a tree a step at a time asks about one directory per step, and one load per directory
/// would be one walk of the rules per step. The load itself is cheap against the walk a reading makes —
/// a handful of small files — but it is kept all the same, since the rule files do not change between
/// two rows of one drawing.
/// </remarks>
internal static class RolaIgnores
{
    /// <summary>The Workspace each root was found to be in, so the walk up is made once per root.</summary>
    /// <remarks>
    /// Nothing is kept for the Workspace's own sake: what it is for is the answer a root outside every
    /// one gives, which is that nothing is hidden — and that answer is asked once per root rather than
    /// once per row.
    /// </remarks>
    private static readonly Dictionary<string, string?> Workspaces = new(StringComparer.Ordinal);

    /// <summary>
    /// The rules of each Workspace already loaded, by the Workspace's own directory.
    /// </summary>
    /// <remarks>
    /// Kept as a <see cref="Lazy{T}"/> rather than as the set itself, because loading is a native call
    /// that reads files and two threads may ask at once — a view priming a Workspace off the window's
    /// thread while the window's thread asks about a row. One of them loads and the other waits for that
    /// load rather than starting a second of its own.
    /// </remarks>
    private static readonly Dictionary<string, Lazy<RolaIgnoreRules?>> Rules = new(StringComparer.Ordinal);

    /// <summary>What both dictionaries are guarded by, and nothing else.</summary>
    private static readonly object Gate = new();

    /// <summary>
    /// The directory, inside a Workspace's data directory, the rules are kept in.
    /// </summary>
    /// <remarks>
    /// It is spelled here rather than read from the native side's constants because it is a fact about
    /// where a file sits — the same kind of fact as the data directory's own name — and a listing must be
    /// able to find it without a call per root.
    /// </remarks>
    private static readonly string[] RulesTail = ["info", "ignore"];

    /// <summary>
    /// Lets go of every set, so the next one is read from the files as they are now.
    /// </summary>
    /// <remarks>
    /// Said when the files may have changed, and the rules are files: one fetched with a Vault, one
    /// edited by hand, one a run wrote. A kept set outliving that is a listing hiding by rules nobody
    /// states any more — and the cache is what makes loading once per Workspace worth it, so it is
    /// cleared rather than never kept.
    /// </remarks>
    public static void Forget()
    {
        RolaIgnoreRules?[] sets;

        lock (Gate)
        {
            sets = [.. Rules.Values.Where(reading => reading.IsValueCreated).Select(reading => reading.Value)];
            Rules.Clear();
            Workspaces.Clear();
        }

        foreach (var set in sets)
        {
            set?.Dispose();
        }
    }

    /// <summary>
    /// Whether what this provider answers with is already at hand for a root.
    /// </summary>
    /// <param name="root">The directory the view reading the entries is rooted at.</param>
    /// <returns>Whether the root's Workspace rules are held.</returns>
    public static bool Ready(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        lock (Gate)
        {
            return Workspace(root) is not { } workspace
                || (Rules.TryGetValue(workspace, out var loaded) && loaded.IsValueCreated);
        }
    }

    /// <summary>
    /// Loads what this provider answers with for a root, unless it has been loaded already.
    /// </summary>
    /// <param name="root">The directory the view reading the entries is rooted at.</param>
    public static void Prepare(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        _ = Answer(root);
    }

    /// <summary>
    /// Whether an entry is one the rules of the Workspace holding it leave out.
    /// </summary>
    /// <remarks>
    /// Read against the directory holding it, which is where a rule about one entry is stated — the
    /// answer a listing asks for about what it is showing.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether it is hidden.</returns>
    public static bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        // UNWRAP: an entry is a path the system named, and a path has a directory — the root itself is the
        // one that answers with its own path.
        var holder = Path.GetDirectoryName(entry.Path) is { Length: > 0 } parent ? parent : entry.Path;

        return Hides(entry, holder);
    }

    /// <summary>
    /// Whether an entry is one the rules leave out, read against the place a view is rooted at.
    /// </summary>
    /// <remarks>
    /// The Workspace is looked for upwards from <c>root</c> and not from the entry: a tree is rooted at
    /// a base, and a base no Workspace holds is a view under which nothing is hidden — whatever the
    /// directories under it happen to hold. The other way, a Workspace nested under the base would
    /// answer for itself, and one tree would be read by two sets of rules.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>Whether it is hidden.</returns>
    public static bool Hides(Entry entry, string root)
    {
        ArgumentNullException.ThrowIfNull(entry);
        ArgumentNullException.ThrowIfNull(root);

        return Answer(root) is { } rules
            && rules.IsIgnored(entry.Path, entry.Kind == EntryKind.Directory);
    }

    /// <summary>
    /// The rules a root's Workspace is managed by, loading them if they have not been loaded.
    /// </summary>
    /// <param name="root">The directory the view reading the entries is rooted at.</param>
    /// <returns>The rules, or nothing when the root is inside no Workspace or its rules cannot be read.</returns>
    private static RolaIgnoreRules? Answer(string root)
    {
        Lazy<RolaIgnoreRules?> loading;

        lock (Gate)
        {
            if (Workspace(root) is not { } workspace)
            {
                return null;
            }

            if (!Rules.TryGetValue(workspace, out var kept))
            {
                kept = new Lazy<RolaIgnoreRules?>(
                    () => Load(workspace),
                    LazyThreadSafetyMode.ExecutionAndPublication
                );
                Rules[workspace] = kept;
            }

            loading = kept;
        }

        // The load is waited for outside the lock: it reads files, and a view asking about a row while
        // another is loading must not hold this class against everything else that asks.
        return loading.Value;
    }

    /// <summary>Loads the rules of a Workspace.</summary>
    /// <param name="workspace">The Workspace's own directory, which is what the rules are written from.</param>
    /// <returns>The rules, or nothing when they cannot be read.</returns>
    private static RolaIgnoreRules? Load(string workspace)
    {
        var rules = Path.Combine([workspace, ".rola", .. RulesTail]);

        try
        {
            return RolaIgnoreRules.Load(rules, workspace);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            // A rules directory that cannot be read is a listing that hides by nothing, which is the
            // same answer as no rules at all: hiding by a half-read set would hide by rules nobody
            // stated.
            return null;
        }
    }

    /// <summary>
    /// The Workspace a directory belongs to, or nothing when it belongs to none.
    /// </summary>
    /// <remarks>
    /// Walking up from the directory rather than asking the native side, for the reason the Git provider
    /// walks up for a repository: this is asked of every root a view is read from, and a file check the
    /// platform answers immediately is cheaper than a call that has to search the same directories again.
    /// What is found is where the rules are read from, which is all this needs.
    /// </remarks>
    /// <param name="root">The directory to start from.</param>
    /// <returns>The Workspace's own directory, or nothing when it belongs to none.</returns>
    private static string? Workspace(string root)
    {
        if (Workspaces.TryGetValue(root, out var known))
        {
            return known;
        }

        var at = new DirectoryInfo(root);

        while (at is not null)
        {
            if (Directory.Exists(Path.Combine(at.FullName, ".rola")))
            {
                Workspaces[root] = at.FullName;

                return at.FullName;
            }

            at = at.Parent;
        }

        Workspaces[root] = null;

        return null;
    }
}

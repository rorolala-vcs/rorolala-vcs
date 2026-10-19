namespace RorolalaVCSPlugin;

/// <summary>One node of the tree a Layout's paths come to: a path segment, which may also be an entry.</summary>
/// <remarks>
/// A Layout is a set of paths and has no directories, so every folder in this tree is made of the segments its paths
/// share. A node is two things at once when a Layout names both `Assets` and `Assets/hero.psd`: the folder the second
/// is under, and an entry of its own.
/// </remarks>
internal sealed class UpstreamNode
{
    /// <summary>The path it is under, which is empty for a node at the top.</summary>
    private readonly string _prefix;

    /// <summary>What is under it, in the order it was made.</summary>
    private readonly List<UpstreamNode> _children = [];

    /// <summary>Makes a node.</summary>
    /// <param name="prefix">The path it is under, empty for a node at the top.</param>
    /// <param name="name">The segment it is named by.</param>
    private UpstreamNode(string prefix, string name)
    {
        _prefix = prefix;
        Name = name;
    }

    /// <summary>The segment it is named by.</summary>
    public string Name { get; }

    /// <summary>The entry it is, or nothing when it is only a folder.</summary>
    public LayoutEntry? Entry { get; private set; }

    /// <summary>What is under it.</summary>
    public IReadOnlyList<UpstreamNode> Children => _children;

    /// <summary>Its whole path, as a Layout names one.</summary>
    public string Path => _prefix.Length == 0 ? Name : _prefix + "/" + Name;

    /// <summary>Whether anything stands under it, which is what makes it a folder to open.</summary>
    public bool HasChildren => _children.Count > 0;

    /// <summary>Makes the node for one segment of a path.</summary>
    /// <param name="prefix">The path it is under.</param>
    /// <param name="name">The segment it is named by.</param>
    /// <returns>The node.</returns>
    public static UpstreamNode At(string prefix, string name) => new(prefix, name);

    /// <summary>Makes it stand for an entry, which a path may do while folders stand under it too.</summary>
    /// <param name="entry">The entry.</param>
    public void Adopt(LayoutEntry entry) => Entry = entry;

    /// <summary>Puts something under it.</summary>
    /// <param name="child">What to put there.</param>
    public void Add(UpstreamNode child) => _children.Add(child);
}

/// <summary>
/// The tree a Layout's paths come to: one node a path segment, and what this side holds beside it.
/// </summary>
/// <remarks>
/// A place is read a level at a time, so what this answers is what stands under one prefix: the folders its paths make,
/// and the entries that are at it. A Layout has no directories — every folder here is the segments two paths share —
/// which is why the tree is made here rather than asked of the copy.
/// <para>
/// This is the pure half of the place on purpose: it reads nothing and starts nothing, so what a level holds and
/// where an entry would land are questions a test asks without a Workspace or a Vault.
/// </para>
/// </remarks>
internal sealed class UpstreamEntriesTree
{
    /// <summary>Every node by the path it stands for, so a path is looked up rather than walked to.</summary>
    private readonly Dictionary<string, UpstreamNode> _nodes = new(StringComparer.Ordinal);

    /// <summary>The nodes at the top, in the order they were made.</summary>
    private readonly List<UpstreamNode> _roots = [];

    /// <summary>What this Layout holds, by uuid, with the path it names each at.</summary>
    private readonly Dictionary<string, string> _local;

    /// <summary>Makes a tree over what the two sides said.</summary>
    /// <param name="local">What this Layout holds, by uuid, with the path it names each at.</param>
    private UpstreamEntriesTree(Dictionary<string, string> local) => _local = local;

    /// <summary>The nodes at the top.</summary>
    public IReadOnlyList<UpstreamNode> Roots => _roots;

    /// <summary>
    /// Builds the tree the copy of a Layout describes.
    /// </summary>
    /// <remarks>
    /// An entry the Layout names at no path is left out: it is something the Layout holds and cannot lend, since a place
    /// shows what is at a path and there is none to name. A path no tree can be drawn of — empty, starting or ending
    /// with a separator, or with an empty segment — is left out too, because the command line would refuse it as a
    /// name rather than write it.
    /// </remarks>
    /// <param name="upstream">What the copy holds.</param>
    /// <param name="local">What this side holds, which is empty for one of the Workspace's own Layouts.</param>
    /// <returns>The tree.</returns>
    public static UpstreamEntriesTree Build(
        IReadOnlyList<LayoutEntry> upstream,
        IReadOnlyList<LayoutEntry> local
    )
    {
        ArgumentNullException.ThrowIfNull(upstream);
        ArgumentNullException.ThrowIfNull(local);

        var held = new Dictionary<string, string>(StringComparer.Ordinal);

        foreach (var entry in local)
        {
            held[entry.Uuid] = entry.Path;
        }

        var tree = new UpstreamEntriesTree(held);

        foreach (var entry in upstream)
        {
            if (Drawable(entry.Path))
            {
                tree.Insert(entry);
            }
        }

        return tree;
    }

    /// <summary>Whether a path is one a tree can be drawn of, which is one a Layout may name.</summary>
    /// <param name="path">The path.</param>
    /// <returns>Whether it is one.</returns>
    public static bool Drawable(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return path.Length > 0
            && path[0] != '/'
            && path[^1] != '/'
            && !path.Contains("//", StringComparison.Ordinal);
    }

    /// <summary>The node standing for a path, or nothing when nothing does.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The node, or nothing.</returns>
    public UpstreamNode? Find(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return _nodes.GetValueOrDefault(path);
    }

    /// <summary>
    /// What stands directly under a prefix, in the order a listing reads it.
    /// </summary>
    /// <remarks>
    /// Folders first, then the entries: a place is read for where things are, and a folder standing between two entries
    /// of the same place is a folder to hunt for. Each of the two is read by name, by case first so that two names
    /// differing only in case do not swap about with the culture.
    /// </remarks>
    /// <param name="prefix">The prefix, empty for the top.</param>
    /// <returns>What is under it, which is nothing for a prefix nothing stands at.</returns>
    public IReadOnlyList<UpstreamNode> At(string prefix)
    {
        ArgumentNullException.ThrowIfNull(prefix);

        var level = prefix.Length == 0 ? _roots : Find(prefix)?.Children ?? [];

        return
        [
            .. level
                .OrderByDescending(node => node.HasChildren)
                .ThenBy(node => node.Name, StringComparer.OrdinalIgnoreCase)
                .ThenBy(node => node.Name, StringComparer.Ordinal),
        ];
    }

    /// <summary>The prefix a path sits under: the top for a path with no segment above it, and nothing for no path.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The prefix, or nothing where there is no path at all.</returns>
    public static string? Parent(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (path.Length == 0)
        {
            return null;
        }

        var at = path.LastIndexOf('/');

        return at < 0 ? string.Empty : path[..at];
    }

    /// <summary>Whether an entry is already this Layout's, wherever it names it.</summary>
    /// <param name="entry">The entry.</param>
    /// <returns>Whether it is.</returns>
    public bool IsHeld(LayoutEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return _local.ContainsKey(entry.Uuid);
    }

    /// <summary>The path this Layout names an entry at, when it holds it.</summary>
    /// <param name="uuid">The uuid the entry is known by.</param>
    /// <returns>The path, which is empty for an entry this Layout holds and names nowhere.</returns>
    public string? LocalPath(string uuid)
    {
        ArgumentNullException.ThrowIfNull(uuid);

        return _local.GetValueOrDefault(uuid);
    }

    /// <summary>Puts one entry in the tree, making the folders its path runs through.</summary>
    /// <param name="entry">The entry.</param>
    private void Insert(LayoutEntry entry)
    {
        var segments = entry.Path.Split('/');
        var prefix = string.Empty;
        UpstreamNode? parent = null;

        for (var at = 0; at < segments.Length; at++)
        {
            var name = segments[at];
            var segment = prefix.Length == 0 ? name : prefix + "/" + name;

            if (Find(segment) is not { } node)
            {
                node = UpstreamNode.At(prefix, name);
                _nodes[segment] = node;

                if (parent is null)
                {
                    _roots.Add(node);
                }
                else
                {
                    parent.Add(node);
                }
            }

            if (at == segments.Length - 1)
            {
                node.Adopt(entry);
            }

            parent = node;
            prefix = segment;
        }
    }
}

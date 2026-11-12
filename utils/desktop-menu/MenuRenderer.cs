using Avalonia.Controls;

namespace RorolalaDesktop.Menus;

/// <summary>
/// Draws a list of menu leaves as a context menu, folding them into categories by their paths.
/// </summary>
/// <remarks>
/// One renderer for every menu the program opens: a file context, a dock's own tab, a graph's hovered part and
/// a tree's row all say what they offer as leaves, and what a menu looks like, which categories are drawn as
/// submenus, which single entries are flattened into one line, and how a key no file translates reads, is
/// answered here once rather than by each of them.
/// <para>
/// What it does not know is where its words or pictures come from: a label function and an icon function are
/// handed in, so the renderer is shared by the host and by plugins without either of them having to be the one
/// that reads the translations.
/// </para>
/// </remarks>
public sealed class MenuRenderer
{
    /// <summary>The class a forced entry wears, which the look draws as what it is.</summary>
    public const string Forced = "force";

    /// <summary>What a key is written as, in the language the program speaks.</summary>
    private readonly Func<string, string> _label;

    /// <summary>What an icon key is drawn as, or nothing for an entry without a picture.</summary>
    private readonly Func<string, Control?>? _icon;

    /// <summary>Where a duplicate path is reported, or nothing where nothing listens.</summary>
    private readonly Action<string>? _warn;

    /// <summary>Makes a renderer over one way of reading words and one way of drawing pictures.</summary>
    /// <param name="label">What a key reads as, in the language the program speaks.</param>
    /// <param name="icon">What an icon key is drawn as, or nothing for an entry without a picture.</param>
    /// <param name="warn">Where a duplicate path is reported, or nothing where nothing listens.</param>
    public MenuRenderer(
        Func<string, string> label,
        Func<string, Control?>? icon = null,
        Action<string>? warn = null
    )
    {
        ArgumentNullException.ThrowIfNull(label);

        _label = label;
        _icon = icon;
        _warn = warn;
    }

    /// <summary>Empties a menu and puts the leaves in it.</summary>
    /// <param name="menu">The menu to fill.</param>
    /// <param name="leaves">What the menu is to offer, in the order they were registered.</param>
    /// <param name="force">Whether the forced entries are shown to begin with, which Shift is what decides.</param>
    public void Fill(ContextMenu menu, IReadOnlyList<MenuLeaf> leaves, bool force)
    {
        ArgumentNullException.ThrowIfNull(menu);
        ArgumentNullException.ThrowIfNull(leaves);

        menu.Items.Clear();

        foreach (var entry in Build(leaves, force))
        {
            menu.Items.Add(entry);
        }
    }

    /// <summary>
    /// The entries of a menu, as controls, without a menu to put them in.
    /// </summary>
    /// <remarks>
    /// Returned rather than only filled, because the Tree Panel and the Version Graph build a menu of their own
    /// and put it where their own menu belongs: the entries are still this renderer's, and what draws them is
    /// the same.
    /// </remarks>
    /// <param name="leaves">What the menu is to offer, in the order they were registered.</param>
    /// <param name="force">Whether the forced entries are shown to begin with.</param>
    /// <returns>The entries, a separator between each pair of categories.</returns>
    public IReadOnlyList<Control> Build(IReadOnlyList<MenuLeaf> leaves, bool force)
    {
        ArgumentNullException.ThrowIfNull(leaves);

        var root = new Node(null);
        var seen = new HashSet<string>(StringComparer.Ordinal);

        for (var at = 0; at < leaves.Count; at++)
        {
            var leaf = leaves[at];

            if (leaf.Keys.Count == 0)
            {
                _warn?.Invoke("menu: an entry with no keys was left out");

                continue;
            }

            if (leaf.Applies?.Invoke() == false)
            {
                continue;
            }

            if (!seen.Add(string.Join('\u001F', leaf.Keys)))
            {
                _warn?.Invoke($"menu: `{string.Join('/', leaf.Keys)}` was registered twice, the first one is kept");

                continue;
            }

            root.Add(leaf, at);
        }

        var built = new List<Control>();
        Slot? previous = null;

        foreach (var slot in Ordered(root, atRoot: true))
        {
            var entry = slot.Group is { } group
                ? Group(group, [], force)
                : Leaf(slot.Leaf!, [], force);

            // A rule between categories, and not between entries that stand on their own: a dock's menu is
            // two plain words, and a rule between them would be a seam where nothing is being separated.
            if (previous is not null && (previous.Group is not null || slot.Group is not null))
            {
                built.Add(new Separator());
            }

            built.Add(entry);
            previous = slot;
        }

        return built;
    }

    /// <summary>A category, as a submenu or as the one entry it holds.</summary>
    /// <param name="group">The category.</param>
    /// <param name="prefix">The categories above it, which a flattened entry is named by.</param>
    /// <param name="force">Whether the forced entries are shown to begin with.</param>
    /// <returns>The entry or submenu.</returns>
    private Control Group(Node group, IReadOnlyList<string> prefix, bool force)
    {
        // One entry under a category is that entry with the category's word in front of it, rather than a
        // submenu holding one line: a menu opened on a file should not make the reader open a category to find
        // the only thing in it.
        if (Leaves(group) == 1 && group.Slots.Count == 1)
        {
            var only = group.Slots[0];
            string[] named = [.. prefix, group.Key!];

            return only.Leaf is { } leaf ? Leaf(leaf, named, force) : Group(only.Group!, named, force);
        }

        var menu = new MenuItem { Header = Text(group.Key!) };

        if (GroupIcon(group) is { } picture)
        {
            menu.Icon = picture;
        }

        var children = new List<Control>(group.Slots.Count);

        foreach (var slot in Ordered(group, atRoot: false))
        {
            if (slot.Leaf is { } leaf)
            {
                children.Add(Leaf(leaf, [], force));
            }
            else if (slot.Group is { } child)
            {
                children.Add(Group(child, [], force));
            }
        }

        foreach (var child in children)
        {
            menu.Items.Add(child);
        }

        return menu;
    }

    /// <summary>One entry, named by the categories above it and by its own key.</summary>
    /// <param name="leaf">The entry.</param>
    /// <param name="prefix">The categories it is flattened under.</param>
    /// <param name="force">Whether the forced entries are shown to begin with.</param>
    /// <returns>The entry.</returns>
    private MenuItem Leaf(MenuLeaf leaf, IReadOnlyList<string> prefix, bool force)
    {
        var name = string.Join(' ', prefix.Append(leaf.Key).Select(Text));

        var menu = new MenuItem
        {
            Header = leaf.Detail is { Length: > 0 } detail ? name + detail : name,
            IsEnabled = leaf.Enabled?.Invoke() ?? true,
        };

        if (Picture(leaf, prefix) is { } picture)
        {
            menu.Icon = picture;
        }

        if (leaf.Force)
        {
            menu.Classes.Add(Forced);
            menu.IsVisible = force;
        }

        menu.Click += (_, _) => leaf.Run();

        return menu;
    }

    /// <summary>What an entry wears, or nothing when neither it nor the category it is folded under has one.</summary>
    /// <remarks>
    /// A category folded into one line is the one action it holds, so when that action has no picture of its own
    /// the category's stands for it: `Restore` over one `Baseline` reads as the restore it is, and a line without
    /// a mark beside its neighbours would read as an entry that has none.
    /// </remarks>
    /// <param name="leaf">The entry.</param>
    /// <param name="prefix">The categories it is folded under, or nothing when it is drawn inside its category.</param>
    /// <returns>The picture, or nothing.</returns>
    private Control? Picture(MenuLeaf leaf, IReadOnlyList<string> prefix)
    {
        if (leaf.IconKey is { } key && _icon?.Invoke(key) is { } picture)
        {
            return picture;
        }

        return prefix.Count > 0 ? _icon?.Invoke(leaf.Root) : null;
    }

    /// <summary>What a category is drawn with, or nothing when neither it nor its entries have a picture.</summary>
    /// <remarks>
    /// The category's own key is asked for first, which is what lets a plugin draw the plus for new things and
    /// the three beads for a menu of views rather than reusing the picture of one of the things inside. A key
    /// nothing answers for falls back to the first entry that has a picture, in the order the entries are drawn.
    /// </remarks>
    /// <param name="group">The category.</param>
    /// <returns>The picture, or nothing.</returns>
    private Control? GroupIcon(Node group)
    {
        if (_icon is null)
        {
            return null;
        }

        if (_icon(group.Key!) is { } picture)
        {
            return picture;
        }

        string[] folded = [group.Key!];

        foreach (var slot in Ordered(group, atRoot: false))
        {
            if (slot.Leaf is { } leaf && Picture(leaf, folded) is { } entry)
            {
                return entry;
            }

            if (slot.Group is { } child && GroupIcon(child) is { } nested)
            {
                return nested;
            }
        }

        return null;
    }

    /// <summary>
    /// The entries of a category, in the order they are drawn.
    /// </summary>
    /// <remarks>
    /// A category keeps the place where it was first registered, which is what lets one plugin open a category
    /// and another add to it without the category moving. Inside a category the entries are ordered by their own
    /// order values, since that is what a plugin sets when it adds more than one.
    /// </remarks>
    /// <param name="node">The category.</param>
    /// <param name="atRoot">Whether the entries are the menu's own, where categories are ordered by first use.</param>
    /// <returns>What it holds, in the order it is drawn.</returns>
    private static IReadOnlyList<Slot> Ordered(Node node, bool atRoot)
    {
        var slots = new List<Slot>(node.Slots);

        slots.Sort((left, right) =>
        {
            if (!atRoot && left.Leaf is { } one && right.Leaf is { } other)
            {
                var order = one.Order.CompareTo(other.Order);

                return order != 0 ? order : left.First.CompareTo(right.First);
            }

            return left.First.CompareTo(right.First);
        });

        return slots;
    }

    /// <summary>How many entries a category holds, at every depth.</summary>
    /// <param name="node">The category.</param>
    /// <returns>The number of leaves under it.</returns>
    private static int Leaves(Node node) =>
        node.Slots.Sum(slot => slot.Leaf is not null ? 1 : Leaves(slot.Group!));

    /// <summary>What a key reads as, falling back to its own last part when no file translates it.</summary>
    /// <param name="key">The i18n key.</param>
    /// <returns>What to draw.</returns>
    private string Text(string key)
    {
        var text = _label(key);

        if (!string.Equals(text, key, StringComparison.Ordinal))
        {
            return text;
        }

        var last = key[(key.LastIndexOf('.') + 1)..];
        var words = last.Replace('_', ' ');

        return words.Length == 0 ? last : words[..1].ToUpperInvariant() + words[1..];
    }

    /// <summary>One thing a category holds: an entry, or another category.</summary>
    private sealed class Slot
    {
        /// <summary>The entry, or nothing when this is a category.</summary>
        public MenuLeaf? Leaf { get; init; }

        /// <summary>The category, or nothing when this is an entry.</summary>
        public Node? Group { get; init; }

        /// <summary>Where the thing was first registered, which is what decides a category's place.</summary>
        public int First { get; init; }
    }

    /// <summary>A category, or the root of the whole menu.</summary>
    private sealed class Node
    {
        /// <summary>The key the category is named by, or nothing at the root.</summary>
        public string? Key { get; }

        /// <summary>What it holds, in the order it was met.</summary>
        public List<Slot> Slots { get; } = [];

        /// <summary>Makes a category or the root.</summary>
        /// <param name="key">The category's key, or nothing at the root.</param>
        public Node(string? key) => Key = key;

        /// <summary>Puts a leaf under the path its keys name, making the categories on the way.</summary>
        /// <param name="leaf">The leaf.</param>
        /// <param name="at">Where the leaf was in the list it came from.</param>
        public void Add(MenuLeaf leaf, int at)
        {
            var node = this;

            for (var step = 0; step < leaf.Keys.Count - 1; step++)
            {
                var key = leaf.Keys[step];
                var found = node.Slots.Find(slot =>
                    slot.Group is { } group && string.Equals(group.Key, key, StringComparison.Ordinal)
                );

                if (found?.Group is { } group)
                {
                    node = group;

                    continue;
                }

                var made = new Node(key);

                node.Slots.Add(new Slot { Group = made, First = at });
                node = made;
            }

            node.Slots.Add(new Slot { Leaf = leaf, First = at });
        }
    }
}

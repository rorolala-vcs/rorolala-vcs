using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Docking;

/// <summary>
/// A place a dock is asked to go: into the tabs of a group, or beside that group.
/// </summary>
/// <remarks>
/// What a drop resolves to, and what a registration's default side is applied against. The group id is the
/// group a person was over; it is empty only when the main window holds no docks at all, which is the one
/// case in which there is no group to be over and the drop makes the first one.
/// </remarks>
/// <param name="GroupId">The group the dock was dropped on, or empty to make the root group.</param>
/// <param name="Side">Whether to tab into that group or split it on that side.</param>
/// <param name="Index">Where among the tabs to insert, used only for <see cref="DockSide.Center"/>.</param>
internal readonly record struct DockTarget(string GroupId, DockSide Side, int Index);

/// <summary>
/// The operations on the layout tree, as pure functions over a <see cref="DockLayout"/>.
/// </summary>
/// <remarks>
/// Kept out of the area and the manager so that the shape of the layout can be exercised without a
/// toolkit: which group a dock went into, what a removal collapsed, what a loaded file was made into. The
/// tree is the truth about where a dock is, and these are the only things that write it.
/// <para>
/// The invariants every operation leaves behind: a group holds at least one dock; every group id is unique
/// and of the form <c>g-&lt;number&gt;</c>; a split's ratio is in <c>(0, 1)</c>; a dock appears at most once;
/// a split has two children; and a split has no split of the same axis as its <em>first</em> child. The last
/// is what "same-axis chains are flattened" can mean for a binary tree: a chain is rewritten to nest to the
/// right, which is the one canonical form there is, and is the one the formula in
/// <see cref="Flatten(LayoutNode)"/> produces.
/// </para>
/// </remarks>
internal static class DockTree
{
    /// <summary>How wide a splitter is, which is the room a split loses to the divider between its children.</summary>
    /// <remarks>
    /// Published here rather than in the area because the least a split may be is measured with it, and a
    /// theme draws a line through the middle of the grab: the two have to be one number. The area re-states
    /// it as <c>DockArea.SplitterSize</c> for the look, which is where a theme reads it.
    /// </remarks>
    public const double SplitterSize = 5;

    /// <summary>The narrowest a column group may be, in pixels.</summary>
    public const double MinColumn = 120;

    /// <summary>The shortest a row group may be, in pixels.</summary>
    public const double MinRow = 56;

    /// <summary>The smallest share a split may give either child.</summary>
    /// <remarks>
    /// A split at exactly zero or one has a child of no size at all, which is a child a splitter can never
    /// bring back: the grip is inside the room the split divides, so there is nothing to grab. The guard is
    /// wider than a pixel in any window this program draws in, which is what keeps the ratio read from a drag
    /// from landing on it.
    /// </remarks>
    public const double MinShare = 0.02;

    /// <summary>The largest share a split may give its first child.</summary>
    public const double MaxShare = 0.98;

    /// <summary>Finds a group by its id, or nothing when the tree holds none.</summary>
    /// <param name="layout">The layout to look in.</param>
    /// <param name="groupId">The group id.</param>
    /// <returns>The group, or nothing.</returns>
    public static LayoutGroup? Find(DockLayout layout, string groupId)
    {
        ArgumentNullException.ThrowIfNull(layout);
        ArgumentNullException.ThrowIfNull(groupId);

        return Groups(layout).FirstOrDefault(group =>
            string.Equals(group.Id, groupId, StringComparison.Ordinal)
        );
    }

    /// <summary>The group a dock sits in, or nothing when the tree does not hold it.</summary>
    /// <param name="layout">The layout to look in.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <param name="ordinal">Which instance of that name.</param>
    /// <returns>The group, or nothing.</returns>
    public static LayoutGroup? GroupOf(DockLayout layout, string nameId, int ordinal)
    {
        ArgumentNullException.ThrowIfNull(layout);

        return Groups(layout).FirstOrDefault(group =>
            group.Docks.Exists(dock => Is(dock, nameId, ordinal))
        );
    }

    /// <summary>
    /// Every group of the tree, in the order the tree states them.
    /// </summary>
    /// <remarks>
    /// Document order — first before second, depth first — rather than any order the runtime made the groups
    /// in. It is what a snapshot writes in, and what a comparison of two trees is about: the same
    /// arrangement read back is the same groups in the same places.
    /// </remarks>
    /// <param name="layout">The layout to walk.</param>
    /// <returns>The groups.</returns>
    public static IEnumerable<LayoutGroup> Groups(DockLayout layout)
    {
        ArgumentNullException.ThrowIfNull(layout);

        return Walk(layout.Root);

        static IEnumerable<LayoutGroup> Walk(LayoutNode? node)
        {
            switch (node)
            {
                case LayoutGroup group:
                    yield return group;
                    break;

                case LayoutSplit split:
                    foreach (var found in Walk(split.First))
                    {
                        yield return found;
                    }

                    foreach (var found in Walk(split.Second))
                    {
                        yield return found;
                    }

                    break;

                default:
                    break;
            }
        }
    }

    /// <summary>
    /// Puts a dock where a target says, answering the group it ended up in.
    /// </summary>
    /// <remarks>
    /// A <see cref="DockSide.Center"/> target inserts among the target group's tabs; an edge makes a new
    /// group holding only this dock and splits the target group beside itself with it. A target that names no
    /// group — the tree is empty, or the group was pruned while the pointer was in flight — falls back to the
    /// active group, and then to the first one, rather than losing the dock.
    /// </remarks>
    /// <param name="layout">The layout to change.</param>
    /// <param name="target">Where to put the dock.</param>
    /// <param name="dock">The dock to put there.</param>
    /// <returns>The id of the group the dock is now in.</returns>
    public static string Insert(DockLayout layout, DockTarget target, LayoutDock dock)
    {
        ArgumentNullException.ThrowIfNull(layout);
        ArgumentNullException.ThrowIfNull(dock);

        var root = layout.Root;

        if (root is null)
        {
            var made = new LayoutGroup { Id = NextGroupId(layout) };

            made.Docks.Add(dock);
            layout.Root = made;
            layout.ActiveGroup = made.Id;

            return made.Id;
        }

        var group = target.GroupId.Length > 0 ? Find(layout, target.GroupId) : null;

        group ??= layout.ActiveGroup is { } active ? Find(layout, active) : null;
        group ??= Groups(layout).First();

        if (target.Side == DockSide.Center)
        {
            var index = Math.Clamp(target.Index, 0, group.Docks.Count);

            group.Docks.Insert(index, dock);
            group.Active = index;

            return group.Id;
        }

        var added = new LayoutGroup { Id = NextGroupId(layout) };

        added.Docks.Add(dock);

        // Left and top put the dock before the group it was dropped on, right and bottom after it, which is
        // the order the axis draws its two children in.
        var before = target.Side is DockSide.Left or DockSide.Top;
        var split = new LayoutSplit
        {
            Axis = target.Side is DockSide.Left or DockSide.Right ? DockAxis.Columns : DockAxis.Rows,
            Ratio = 0.5,
            First = before ? added : group,
            Second = before ? group : added,
        };

        if (ReferenceEquals(root, group))
        {
            root = split;
        }
        else
        {
            Splice(root, group, split);
        }

        layout.Root = Flatten(root);

        return added.Id;
    }

    /// <summary>
    /// Takes a dock out of the tree, collapsing whatever its going left empty.
    /// </summary>
    /// <remarks>
    /// Empty groups do not exist: the last dock of a group takes the group with it, and a split left with one
    /// child is replaced by that child. A root group left empty leaves no tree at all, which is what the main
    /// window holding no docks is.
    /// </remarks>
    /// <param name="layout">The layout to change.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <param name="ordinal">Which instance of that name.</param>
    /// <returns>Whether the tree held that dock.</returns>
    public static bool Remove(DockLayout layout, string nameId, int ordinal)
    {
        ArgumentNullException.ThrowIfNull(layout);

        var root = layout.Root;

        if (root is null)
        {
            return false;
        }

        var (node, removed) = Prune(root, nameId, ordinal);

        if (!removed)
        {
            return false;
        }

        layout.Root = node is null ? null : Flatten(node);

        if (layout.Root is null)
        {
            layout.ActiveGroup = null;

            return true;
        }

        if (layout.ActiveGroup is not { } active || Find(layout, active) is null)
        {
            layout.ActiveGroup = Groups(layout).First().Id;
        }

        return true;
    }

    /// <summary>The narrowest a node may become, in pixels.</summary>
    /// <param name="node">The node to measure.</param>
    /// <returns>The least width it may be given.</returns>
    public static double MinimumWidth(LayoutNode node)
    {
        ArgumentNullException.ThrowIfNull(node);

        if (node is not LayoutSplit split)
        {
            return MinColumn;
        }

        return split.Axis == DockAxis.Columns
            ? MinimumWidth(split.First) + MinimumWidth(split.Second) + SplitterSize
            : Math.Max(MinimumWidth(split.First), MinimumWidth(split.Second));
    }

    /// <summary>The shortest a node may become, in pixels.</summary>
    /// <param name="node">The node to measure.</param>
    /// <returns>The least height it may be given.</returns>
    public static double MinimumHeight(LayoutNode node)
    {
        ArgumentNullException.ThrowIfNull(node);

        if (node is not LayoutSplit split)
        {
            return MinRow;
        }

        return split.Axis == DockAxis.Rows
            ? MinimumHeight(split.First) + MinimumHeight(split.Second) + SplitterSize
            : Math.Max(MinimumHeight(split.First), MinimumHeight(split.Second));
    }

    /// <summary>
    /// Brings a layout read from somewhere into a shape the rest of the program may trust.
    /// </summary>
    /// <remarks>
    /// A file is a file: it may have been edited by hand, written by an older build, or left half-written by
    /// a machine that stopped. What can be repaired is repaired rather than refused — a dock of a name or an
    /// ordinal that is not a dock is dropped, so is a second copy of one, an empty group is collapsed, a ratio
    /// is brought in from the ends, a chain is flattened, an id that is not one is renamed, and the active tab
    /// and group are pointed at what is there. What cannot be repaired never reaches here: it failed to read.
    /// </remarks>
    /// <param name="layout">The layout to make whole.</param>
    public static void Normalize(DockLayout layout)
    {
        ArgumentNullException.ThrowIfNull(layout);

        if (layout.Root is null)
        {
            layout.ActiveGroup = null;

            return;
        }

        var seen = new HashSet<(string NameId, int Ordinal)>();

        foreach (var group in Groups(layout))
        {
            group.Docks.RemoveAll(dock =>
                string.IsNullOrEmpty(dock.DockNameId)
                || dock.Ordinal < 0
                || !seen.Add((dock.DockNameId, dock.Ordinal))
            );
        }

        var root = Collapse(layout.Root);

        if (root is null)
        {
            layout.Root = null;
            layout.ActiveGroup = null;

            return;
        }

        layout.Root = Flatten(root);

        var ids = new HashSet<string>(StringComparer.Ordinal);

        foreach (var group in Groups(layout))
        {
            if (!IsGroupId(group.Id) || !ids.Add(group.Id))
            {
                group.Id = NextGroupId(layout);
                ids.Add(group.Id);
            }

            group.Active = Math.Clamp(group.Active, 0, Math.Max(0, group.Docks.Count - 1));
        }

        if (layout.ActiveGroup is not { } active || Find(layout, active) is null)
        {
            layout.ActiveGroup = null;
        }
    }

    /// <summary>
    /// The next group id to hand out: one past the largest number the tree or this run has used.
    /// </summary>
    /// <remarks>
    /// The run's own high-water mark is read as well as the tree, because the largest id may name a group
    /// that has just been pruned while a dock in a window of its own still remembers it. Ids are therefore
    /// not reused within a run, which is what makes a remembered id mean one group and not two.
    /// </remarks>
    /// <param name="layout">The layout to read the ids from.</param>
    /// <returns>A group id nothing in the tree holds.</returns>
    public static string NextGroupId(DockLayout layout)
    {
        ArgumentNullException.ThrowIfNull(layout);

        var largest = layout.GroupIdHighWater;

        foreach (var group in Groups(layout))
        {
            if (NumberOf(group.Id) is { } number && number > largest)
            {
                largest = number;
            }
        }

        var next = largest + 1;

        while (Find(layout, $"g-{next}") is not null)
        {
            next++;
        }

        layout.GroupIdHighWater = next;

        return $"g-{next}";
    }

    /// <summary>Whether an id is a group id of the form <c>g-&lt;number&gt;</c>.</summary>
    /// <param name="id">The id to read.</param>
    /// <returns>Whether it is one.</returns>
    public static bool IsGroupId(string id)
    {
        ArgumentNullException.ThrowIfNull(id);

        return NumberOf(id) is not null;
    }

    /// <summary>Brings a share of a split into the range a split may state.</summary>
    /// <param name="ratio">The ratio as measured or as read.</param>
    /// <returns>The ratio to store, rounded to six places.</returns>
    public static double Share(double ratio) =>
        Math.Clamp(Math.Round(ratio, 6), MinShare, MaxShare);

    /// <summary>
    /// A copy of a node and everything under it, over the docks as a caller makes them.
    /// </summary>
    /// <remarks>
    /// What a snapshot needs: a saved layout is a copy of the live tree with each dock written as the live
    /// instance says it is, because the live tree holds no metadata and the file does.
    /// </remarks>
    /// <param name="node">The node to copy.</param>
    /// <param name="dock">What to write for one dock.</param>
    /// <returns>The copy.</returns>
    public static LayoutNode Copy(LayoutNode node, Func<LayoutDock, LayoutDock> dock)
    {
        ArgumentNullException.ThrowIfNull(node);
        ArgumentNullException.ThrowIfNull(dock);

        if (node is LayoutSplit split)
        {
            return new LayoutSplit
            {
                Axis = split.Axis,
                Ratio = split.Ratio,
                First = Copy(split.First, dock),
                Second = Copy(split.Second, dock),
            };
        }

        var group = (LayoutGroup)node;

        return new LayoutGroup
        {
            Id = group.Id,
            Active = group.Active,
            Docks = [.. group.Docks.Select(dock)],
        };
    }

    /// <summary>The number of a <c>g-&lt;number&gt;</c> id, or nothing when the id is not one.</summary>
    /// <param name="id">The id to read.</param>
    /// <returns>The number, or nothing.</returns>
    private static int? NumberOf(string id)
    {
        if (!id.StartsWith("g-", StringComparison.Ordinal))
        {
            return null;
        }

        return int.TryParse(id.AsSpan(2), out var number) && number >= 0 ? number : null;
    }

    /// <summary>Whether a recorded dock is the one a name id and ordinal name.</summary>
    /// <param name="dock">The recorded dock.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <param name="ordinal">Which instance of that name.</param>
    /// <returns>Whether they are the same dock.</returns>
    private static bool Is(LayoutDock dock, string nameId, int ordinal) =>
        dock.Ordinal == ordinal
        && string.Equals(dock.DockNameId, nameId, StringComparison.Ordinal);

    /// <summary>Puts a split where a node was, below the node given.</summary>
    /// <param name="parent">The node to search under.</param>
    /// <param name="node">The node to replace.</param>
    /// <param name="split">What to put in its place.</param>
    /// <returns>Whether the node was found.</returns>
    private static bool Splice(LayoutNode parent, LayoutNode node, LayoutNode split)
    {
        if (parent is not LayoutSplit at)
        {
            return false;
        }

        if (ReferenceEquals(at.First, node))
        {
            at.First = split;

            return true;
        }

        if (ReferenceEquals(at.Second, node))
        {
            at.Second = split;

            return true;
        }

        return Splice(at.First, node, split) || Splice(at.Second, node, split);
    }

    /// <summary>The tree with a dock taken out, and with what its going left empty collapsed.</summary>
    /// <param name="node">The node to take it out of.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <param name="ordinal">Which instance of that name.</param>
    /// <returns>The node that stands in its place, or nothing when it is gone.</returns>
    private static (LayoutNode? Node, bool Removed) Prune(LayoutNode node, string nameId, int ordinal)
    {
        if (node is LayoutGroup group)
        {
            var at = group.Docks.FindIndex(dock => Is(dock, nameId, ordinal));

            if (at < 0)
            {
                return (group, false);
            }

            group.Docks.RemoveAt(at);
            group.Active = Math.Clamp(group.Active, 0, Math.Max(0, group.Docks.Count - 1));

            return group.Docks.Count == 0 ? (null, true) : (group, true);
        }

        var split = (LayoutSplit)node;
        var (first, found) = Prune(split.First, nameId, ordinal);

        if (found)
        {
            if (first is null)
            {
                return (split.Second, true);
            }

            split.First = first;

            return (split, true);
        }

        var (second, other) = Prune(split.Second, nameId, ordinal);

        if (!other)
        {
            return (split, false);
        }

        if (second is null)
        {
            return (split.First, true);
        }

        split.Second = second;

        return (split, true);
    }

    /// <summary>The tree with every empty group taken out and every split left with one child collapsed.</summary>
    /// <param name="node">The node to collapse.</param>
    /// <returns>The node that stands in its place, or nothing when it is gone.</returns>
    private static LayoutNode? Collapse(LayoutNode node)
    {
        if (node is LayoutGroup group)
        {
            return group.Docks.Count == 0 ? null : group;
        }

        var split = (LayoutSplit)node;
        var first = Collapse(split.First);
        var second = Collapse(split.Second);

        if (first is null)
        {
            return second;
        }

        if (second is null)
        {
            return first;
        }

        split.Ratio = Share(split.Ratio);
        split.First = first;
        split.Second = second;

        return split;
    }

    /// <summary>
    /// Rewrites a split whose first child is a split of the same axis so that the chain nests to the right.
    /// </summary>
    /// <remarks>
    /// The rewrite is <c>split(p, split(q, A, B), C)</c> into <c>split(p·q, A, split(s, B, C))</c>, with
    /// <c>s = p(1−q)/(1−p·q)</c>: A and B keep the shares they had, and the seam between them moves into the
    /// right-hand split. A chain of three columns cannot be one split — a split has two children — so nesting
    /// is the only form there is; what the rewrite buys is one canonical form, so a tree built by hand and a
    /// tree built by dragging come out the same shape.
    /// </remarks>
    /// <param name="node">The node to flatten.</param>
    /// <returns>The flattened node.</returns>
    private static LayoutNode Flatten(LayoutNode node)
    {
        if (node is not LayoutSplit split)
        {
            return node;
        }

        split.First = Flatten(split.First);
        split.Second = Flatten(split.Second);

        if (split.First is not LayoutSplit first || first.Axis != split.Axis)
        {
            return split;
        }

        var outer = Share(split.Ratio);
        var inner = Share(first.Ratio);
        var ratio = Share(outer * inner);
        var rest = 1 - ratio;
        var next = rest <= 0 ? 0.5 : Share(outer * (1 - inner) / rest);

        return Flatten(
            new LayoutSplit
            {
                Axis = split.Axis,
                Ratio = ratio,
                First = first.First,
                Second = new LayoutSplit
                {
                    Axis = split.Axis,
                    Ratio = next,
                    First = first.Second,
                    Second = split.Second,
                },
            }
        );
    }
}

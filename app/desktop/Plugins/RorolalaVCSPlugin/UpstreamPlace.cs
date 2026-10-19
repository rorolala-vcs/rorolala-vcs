using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using MenuItem = RorolalaDesktop.Contract.MenuItem;

namespace RorolalaVCSPlugin;

/// <summary>Which Layout a level of the place shows: the domain it is listed under, and its name.</summary>
/// <remarks>
/// The two domains are two ways of naming one thing: <c>vaults/origin</c> is the Layout the Vault <c>origin</c> keeps,
/// and <c>layouts/work</c> is a Layout of the Workspace's own. What differs is what a run is told, since the command line
/// writes a Vault's Layout as <c>truth@VAULT</c> and the Workspace's as its name alone, and whether an entry of it can be
/// brought into the work, which is only ever from the Vault the work tracks.
/// </remarks>
/// <param name="Domain">The domain it is listed under: <c>vaults</c> or <c>layouts</c>.</param>
/// <param name="Name">The Vault's name, or the Layout's.</param>
internal sealed record UpstreamSource(string Domain, string Name)
{
    /// <summary>What a run names the Layout by: <c>truth@VAULT</c> for a Vault's own, the name alone for the Workspace's.</summary>
    public string Layout => Domain == UpstreamPlace.Vaults ? LayoutPathMoveCommand.Named(Name) : Name;

    /// <summary>The Vault whose copy it is, or empty for a Layout of the Workspace's own.</summary>
    public string Vault => Domain == UpstreamPlace.Vaults ? Name : string.Empty;

    /// <summary>The key what was read of it is kept under, which is two names that may collide told apart.</summary>
    public string Key => Domain + "/" + Name;
}

/// <summary>What a drag of one entry of the place carries: which Layout's copy it is of, and the path in it.</summary>
/// <remarks>
/// A Layout path and not a file: the entry the drag took hold of is a thing a Layout holds, and what a drop does with it
/// is the place's own answer. The type is what tells this drag from a file's, which carries paths that are on disk and
/// means a copy or a move of them.
/// </remarks>
/// <param name="Source">The Layout the entry is of.</param>
/// <param name="Path">The path the entry is at, as that Layout names it.</param>
/// <param name="Entry">What the Layout holds there, or nothing when nothing is named there.</param>
internal sealed record UpstreamCarry(UpstreamSource Source, string Path, LayoutEntry? Entry = null);

/// <summary>One row of a level of a place, before it is drawn: what it is, and where it leads.</summary>
/// <param name="Path">The path it stands for.</param>
/// <param name="Name">What it is called.</param>
/// <param name="Folder">Whether it is a folder to open.</param>
/// <param name="Entry">The entry it is, when the Layout names one at the path.</param>
internal sealed record UpstreamSpot(string Path, string Name, bool Folder, LayoutEntry? Entry);

/// <summary>The rows one level of a Layout's copy comes to: the way up, the folders, and the entries.</summary>
/// <remarks>
/// A level is read from the tree the two sides make, and it is the one thing about a place that can be worked out
/// without a Workspace: what is at a prefix is a question about paths, and whether an entry is already held is a
/// question about what this side names.
/// </remarks>
internal static class UpstreamSpots
{
    /// <summary>The rows standing directly under a prefix.</summary>
    /// <param name="tree">The tree the two sides make.</param>
    /// <param name="prefix">The prefix, empty for the top.</param>
    /// <returns>The rows, in the order a listing reads them.</returns>
    public static IReadOnlyList<UpstreamSpot> Of(UpstreamEntriesTree tree, string prefix)
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(prefix);

        var spots = new List<UpstreamSpot>();

        // The way up is a row of the listing rather than a control of a dock's: it is walked, chosen and scrolled with
        // everything else, and a place opened in a window of its own has nowhere else to offer it.
        if (UpstreamEntriesTree.Parent(prefix) is { } up)
        {
            spots.Add(new UpstreamSpot(up, "..", Folder: true, Entry: null));
        }

        foreach (var node in tree.At(prefix))
        {
            spots.Add(new UpstreamSpot(node.Path, node.Name, node.HasChildren, node.Entry));
        }

        return spots;
    }
}

/// <summary>What a drop of entries comes to: what is to be brought in, and how much of it is already here.</summary>
/// <param name="References">What a checkin is to name, in the order it was carried.</param>
/// <param name="Held">How many of the entries carried this Layout already holds.</param>
internal sealed record UpstreamPlan(IReadOnlyList<string> References, int Held);

/// <summary>The references a drop of entries comes to: every entry under what was carried, in the order it was carried.</summary>
/// <remarks>
/// A folder is not an entry, so what a drop of one brings in is what stands under it — the folder's own entry included,
/// since a path may be both. A path the copy no longer names is dropped rather than guessed at: the copy moved under the
/// drag, and what is not there is not something a checkin can name.
/// </remarks>
internal static class UpstreamBringing
{
    /// <summary>The references a checkin is given for a drag of these entries.</summary>
    /// <param name="tree">The tree the Vault's copy was read to.</param>
    /// <param name="carrying">What the drag carried, in the order it carried it.</param>
    /// <returns>The references, deduplicated and in order, and how many were already held.</returns>
    public static UpstreamPlan Plan(UpstreamEntriesTree tree, IReadOnlyList<UpstreamCarry> carrying)
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(carrying);

        var references = new List<string>();
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var held = 0;

        foreach (var carry in carrying)
        {
            foreach (var entry in Under(tree, carry.Path))
            {
                // A folder and an entry under it dragged together are one reference: the command line would name the
                // entry twice, and a run that says the same thing twice is a run refused as a duplicate.
                if (!seen.Add(entry.Uuid))
                {
                    continue;
                }

                if (tree.IsHeld(entry))
                {
                    held++;

                    continue;
                }

                references.Add(CheckinCommand.Reference(entry));
            }
        }

        return new UpstreamPlan(references, held);
    }

    /// <summary>Every entry at a path and under it, in the order a listing reads them.</summary>
    /// <param name="tree">The tree.</param>
    /// <param name="path">The path.</param>
    /// <returns>The entries.</returns>
    private static IEnumerable<LayoutEntry> Under(UpstreamEntriesTree tree, string path) =>
        tree.Find(path) is { } node ? Nest(tree, node) : [];

    /// <summary>The entry a node is, and every entry under it.</summary>
    /// <param name="tree">The tree.</param>
    /// <param name="node">The node.</param>
    /// <returns>The entries.</returns>
    private static IEnumerable<LayoutEntry> Nest(UpstreamEntriesTree tree, UpstreamNode node)
    {
        if (node.Entry is { } entry)
        {
            yield return entry;
        }

        foreach (var child in tree.At(node.Path))
        {
            foreach (var deeper in Nest(tree, child))
            {
                yield return deeper;
            }
        }
    }
}

/// <summary>
/// The work's upstream, offered to the browser as a place: <c>rola@DOMAIN/NAME/…</c>.
/// </summary>
/// <remarks>
/// The address is a path through what the work knows, and the first segment says which: <c>vaults</c> lists the Vaults
/// the Workspace has bound and <c>layouts</c> the Layouts it works in. Under a Vault is that Vault's own Layout — what it
/// is the upstream of is why the place exists — which is read from the copy the Workspace fetched, with nothing fetched
/// for it. So <c>rola@vaults/origin/@/new</c> is the unplaced entries of the Vault called <c>origin</c>. Under
/// <c>layouts</c> is one of the Workspace's own Layouts, read from the Layout itself, which is a different kind of level:
/// its entries are this side's own record rather than a copy, so there is nothing to bring into it and nothing to fetch
/// for it.
/// <para>
/// The base is read on every answer rather than carried in the address, so a place opened in a window of its own follows
/// the work the browser is rooted in — the same base every other reading of the work is made from.
/// </para>
/// <para>
/// What is read is kept until the files are said to have changed: a provider is asked on the window's thread and asked
/// often, and a run of three programs per answer would be a window that stopped. All of it is read on threads of its own
/// for the same reason — the two name lists included, since a <c>vault ls</c> behind a hover is a window that stalls while
/// the pointer moves.
/// </para>
/// <para>
/// The actions are the command line's, as everywhere else in this plugin: bringing one entry in is a checkin of that
/// one path, and moving one is a <c>layout path move</c> against the Layout the level is of, made when the entry is
/// dropped on a folder's row. One entry at a time, because that is what the command line takes — and what a menu opened
/// on one row is about.
/// </para>
/// </remarks>
internal sealed class UpstreamPlace : ILogicalViewProvider, IDropTarget, ILogicalRefusal
{
    /// <summary>The prefix the place's addresses begin with.</summary>
    public const string Scheme = "rola";

    /// <summary>What every Vault the Workspace knows is listed under.</summary>
    public const string Vaults = "vaults";

    /// <summary>What every Layout the Workspace works in is listed under.</summary>
    public const string Layouts = "layouts";

    /// <summary>The key this plugin's own menu is named by, which is also the path its items are added under.</summary>
    private const string WorkspaceMenu = "rorolala_vcs.menu.workspace";

    /// <summary>The two lists the place answers with at its top, for a row that could not read one of them.</summary>
    /// <remarks>
    /// Kept apart from a Layout's failure because the two are let go of differently: a list is one answer kept beside the
    /// others, and a Layout is one answer in a table keyed by Layout.
    /// </remarks>
    private enum UpstreamList
    {
        /// <summary>The Vaults the Workspace knows.</summary>
        Vaults,

        /// <summary>The Layouts the Workspace works in.</summary>
        Layouts,
    }

    /// <summary>The host, for what a run reports itself to and what a failure is said through.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a base that is not a Workspace is judged.</summary>
    private readonly ILog _log;

    /// <summary>What every command this plugin runs reports itself to.</summary>
    private readonly ProgressHub _hub;

    /// <summary>The one fetch of the Vault's Layout.</summary>
    private readonly VaultFetch _fetch;

    /// <summary>What was read of each Layout, by key, kept until the files are said to have changed.</summary>
    private readonly Dictionary<string, UpstreamReading> _readings = new(StringComparer.Ordinal);

    /// <summary>The trees the readings came to, by key, which is what a level is read out of.</summary>
    private readonly Dictionary<string, UpstreamEntriesTree> _trees = new(StringComparer.Ordinal);

    /// <summary>Whether a directory is inside a Workspace, by directory, so a hover is not a native walk each time.</summary>
    /// <remarks>
    /// Asked for every drag over a directory and for every drop, and answered by the C ABI: a walk of the directories
    /// above a path is cheap but not free, and a pointer moving over a listing asks it per move.
    /// </remarks>
    private readonly Dictionary<string, bool> _holds = new(StringComparer.Ordinal);

    /// <summary>The Vaults the Workspace knows, once they have been read.</summary>
    private IReadOnlyList<VaultListing>? _vaultNames;

    /// <summary>Whether a read of the Vaults is going, so that one read is asked for once.</summary>
    private bool _vaultsAsking;

    /// <summary>Whether the Vaults have been asked for, so a run that failed is not asked again on every draw.</summary>
    private bool _vaultsTried;

    /// <summary>What the command line said where it could not list them.</summary>
    private string _vaultsSaid = string.Empty;

    /// <summary>
    /// Whether the work's own upstream was asked for while the two lists were being read, and is to be opened when
    /// they are in.
    /// </summary>
    /// <remarks>
    /// What the root opens is a Vault, and which Vault it is is the answer of a program: a menu item cannot wait for one,
    /// and a root that opened the list of them instead would be a button that never does what it is named for. So the ask
    /// is kept, and the drawing that follows a landing is what opens it.
    /// </remarks>
    private bool _rootAsked;

    /// <summary>The Layouts the Workspace works in, once they have been read.</summary>
    private IReadOnlyList<LayoutListing>? _layoutNames;

    /// <summary>Whether a read of the Layouts is going, so that one read is asked for once.</summary>
    private bool _layoutsAsking;

    /// <summary>Whether the Layouts have been asked for, so a run that failed is not asked again on every draw.</summary>
    private bool _layoutsTried;

    /// <summary>What the command line said where it could not list them.</summary>
    private string _layoutsSaid = string.Empty;

    /// <summary>The Layouts whose copy is being read, so that one read is asked for once.</summary>
    private readonly HashSet<string> _pending = new(StringComparer.Ordinal);

    /// <summary>Which reading of the work the kept answers are of, which is what a read that landed is judged by.</summary>
    private int _version;

    /// <summary>The base the kept answers were read at.</summary>
    private string _readAt = string.Empty;

    /// <summary>Makes the provider.</summary>
    /// <param name="host">The host, for the reports and for what a failure is shown through.</param>
    /// <param name="log">Where a base that is not a Workspace is judged.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout.</param>
    public UpstreamPlace(IPluginHost host, ILog log, ProgressHub hub, VaultFetch fetch)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(fetch);

        _host = host;
        _log = log;
        _hub = hub;
        _fetch = fetch;

        // What a place answers is a snapshot of the copies, so it is let go of when they may have moved: the files having
        // changed is every command of this plugin's own answer that they have, and the base moving is a different work.
        // A reading that landed is not one of those — it says that an answer is at hand, and letting go of the answers
        // for it would make every reader ask for what it has just been given (see `Drawn`).
        host.Files.Changed += Forget;
        BaseDirectory.Changed += Forget;
    }

    /// <summary>
    /// Offers the place to the browser, and the way into it from the work's own menus.
    /// </summary>
    /// <remarks>
    /// A place is opened by its address, and an address is what the address bar takes — but a reader who has never typed
    /// one has no way to know it exists. So the work's menu offers the three levels of it, and a directory's menu offers
    /// the whole of it: each opens the browser's own listing at an address, out of step with what it was following.
    /// </remarks>
    /// <param name="host">The host, whose registry the place is offered to and whose windows it is opened through.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout.</param>
    public static void Declare(IPluginHost host, ProgressHub hub, VaultFetch fetch)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(fetch);

        var place = new UpstreamPlace(host, host.Log, hub, fetch);

        host.LogicalViews.Register(Scheme, place);

        // And the same answer for the other direction: what a directory does with a drag of this place's entries, which is
        // the work's own bringing them in.
        host.Drops.Add(place);

        // One menu for the work: the root of the place, and the two lists that stand under it.
        host.Menu.AddTopLevel(WorkspaceMenu, 60);

        // The root is the work's own upstream rather than the list of what there is: what a reader asking for upstream
        // wants is what this work is the upstream of, which is the Layout of the Vault it tracks. The two lists stay a
        // step away, so a work with nothing tracked still has somewhere to go.
        host.Menu.AddItem(
            WorkspaceMenu,
            new MenuItem("rorolala_vcs.menu.root", 10, place.OpenRoot, Icons.Download)
        );
        host.Menu.AddItem(
            WorkspaceMenu,
            new MenuItem("rorolala_vcs.menu.layouts", 20, () => Open(host, [Layouts]), Icons.Merge)
        );
        host.Menu.AddItem(
            WorkspaceMenu,
            new MenuItem("rorolala_vcs.menu.vaults", 30, () => Open(host, [Vaults]), Icons.CloudDownload)
        );

        // And the same place from a directory of the work, which is where a reader stands when they wonder what upstream
        // holds.
        host.ContextMenus.Add(
            ContextMenuTarget.Directory,
            new ContextMenuItem(
                "rorolala_vcs.menu.upstream",
                60,
                _ => place.OpenRoot(),
                Icons.Download,
                Applies: target => place.Holds(target.Directory)
            )
        );
    }

    /// <inheritdoc />
    public string Title(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        var segments = Segments(address);

        return segments switch
        {
            [] => RolaI18N.Get("rorolala_vcs.place.title"),
            [Vaults] => RolaI18N.Get("rorolala_vcs.place.vaults_title"),
            [Layouts] => RolaI18N.Get("rorolala_vcs.place.layouts_title"),
            [Vaults, var vault] => RolaI18N.Get("rorolala_vcs.place.vault_title", vault),
            [Layouts, var layout] => RolaI18N.Get("rorolala_vcs.place.layout_title", layout),
            [Vaults, var vault, .. var rest] => RolaI18N.Get(
                "rorolala_vcs.place.inside",
                vault,
                string.Join('/', rest)
            ),
            [Layouts, var layout, .. var rest] => RolaI18N.Get(
                "rorolala_vcs.place.inside_layout",
                layout,
                string.Join('/', rest)
            ),
            _ => RolaI18N.Get("rorolala_vcs.place.title"),
        };
    }

    /// <inheritdoc />
    public IReadOnlyList<LogicalEntry> Entries(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        var segments = Segments(address);

        return segments switch
        {
            // The two domains the work holds: what it is upstream of, and the Layouts it works in.
            [] => [Domain(Vaults), Domain(Layouts)],

            [Vaults] => VaultRows(),
            [Layouts] => LayoutRows(),

            // Under either domain is one Layout, a level at a time: a Vault's own, and one of the Workspace's.
            [Vaults, var vault] => Level(new UpstreamSource(Vaults, vault), string.Empty),
            [Vaults, var vault, .. var rest] => Level(
                new UpstreamSource(Vaults, vault),
                string.Join('/', rest)
            ),
            [Layouts, var layout] => Level(new UpstreamSource(Layouts, layout), string.Empty),
            [Layouts, var layout, .. var rest] => Level(
                new UpstreamSource(Layouts, layout),
                string.Join('/', rest)
            ),

            [var unknown] => [Unanswered(unknown)],
            _ => [Unanswered(string.Join('/', segments))],
        };
    }

    /// <inheritdoc />
    public LogicalAddress? Parent(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        var segments = Segments(address);

        return segments.Length <= 1 ? null : Of(segments[..^1]);
    }

    /// <summary>
    /// Whether a drag is one this place can take: an entry of one Layout put in another folder of the same one.
    /// </summary>
    /// <remarks>
    /// A drag of files is refused rather than guessed at: what it carries is where the files are on disk, and a place's
    /// rows are paths a Layout names — the two are different questions, and a checkin of a file's content is not what
    /// dropping one on a row means. A drag from another Layout is refused too, since a move is one Layout's own.
    /// </remarks>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>Whether it can be taken.</returns>
    public bool Accepts(LogicalAddress address, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(address);
        ArgumentNullException.ThrowIfNull(drag);

        if (Carried(drag.Data) is not { Count: > 0 } carrying || Target(address) is not { } target)
        {
            return false;
        }

        var (source, prefix) = target;
        var tree = Tree(source);

        // Several at once, because a drop of a choice is what a listing's drag is for: each entry is moved by a run of its
        // own, and the ones the Layout refuses are said beside the ones that landed.
        return tree is not null
            && carrying.All(carry =>
                string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal)
                && Editable(tree, carry.Path, prefix)
            );
    }

    /// <summary>
    /// Why this place would refuse a drag, which is the answer a drop that moved nothing is owed.
    /// </summary>
    /// <remarks>
    /// The reasons are this place's own and none of them is something the browser could work out: the entry is already
    /// under the folder it was dropped on, something else is named where it would land, the copy a level is read from is
    /// still being read or was never fetched, or the drag is of another Layout altogether. Said in the words the place
    /// says everything else in, so that what a drop did not do is read as plainly as what it did.
    /// </remarks>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>Why it would not be taken, or nothing when the place has nothing to add.</returns>
    public string? Refusal(LogicalAddress address, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(address);
        ArgumentNullException.ThrowIfNull(drag);

        if (Carried(drag.Data) is not { Count: > 0 } carrying)
        {
            return RolaI18N.Get("rorolala_vcs.place.refused_not_a_drag");
        }

        if (Target(address) is not { } target)
        {
            return RolaI18N.Get("rorolala_vcs.place.refused_not_a_folder");
        }

        var (source, prefix) = target;

        if (carrying.Any(carry => !string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal)))
        {
            return RolaI18N.Get("rorolala_vcs.place.refused_another_layout");
        }

        if (Tree(source) is not { } tree)
        {
            return RolaI18N.Get("rorolala_vcs.place.refused_reading");
        }

        foreach (var carry in carrying)
        {
            if (tree.Find(carry.Path)?.Entry is null)
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_gone");
            }

            if (Landing(carry.Path, prefix) is not { } to)
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_same", carry.Path);
            }

            if (tree.Find(to) is not null)
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_taken", to);
            }
        }

        return RolaI18N.Get("rorolala_vcs.place.refused_generic");
    }

    /// <summary>Takes the drag: the entry is moved to the path the place it was dropped on names.</summary>
    /// <param name="address">The place it was dropped on.</param>
    /// <param name="drag">What was dragged.</param>
    public void Drop(LogicalAddress address, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(address);
        ArgumentNullException.ThrowIfNull(drag);

        if (Carried(drag.Data) is not { Count: > 0 } carrying || Target(address) is not { } target)
        {
            return;
        }

        var (source, prefix) = target;
        var tree = Tree(source);

        if (tree is null)
        {
            return;
        }

        List<(string From, string To, string? Holder)> moves = [];

        foreach (var carry in carrying)
        {
            if (
                !string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal)
                || Landing(carry.Path, prefix) is not { } to
                || !Editable(tree, carry.Path, prefix)
            )
            {
                continue;
            }

            moves.Add((carry.Path, to, tree.Find(carry.Path)?.Entry?.Owner));
        }

        if (moves.Count > 0)
        {
            Move(source, moves);
        }
    }

    /// <summary>
    /// What a place says about each of its entries: the uuid the Vault knows it by, and the account that holds it.
    /// </summary>
    /// <remarks>
    /// Two columns and no colours: what an entry is held by is a fact about it rather than a state of the work, and a
    /// colour would say that it is one to be acted on. A folder has neither, and neither is drawn — a column that repeated
    /// the row's emptiness is a column saying nothing.
    /// </remarks>
    /// <param name="address">The place being read.</param>
    /// <returns>The columns, in the order they are shown.</returns>
    public IReadOnlyList<LogicalColumn> Columns(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        return
        [
            new LogicalColumn(
                "rorolala_vcs.place.column_uuid",
                120,
                entry => Facts(entry)?.Uuid ?? string.Empty,
                BesideName: true
            ),
            new LogicalColumn(
                "rorolala_vcs.place.column_owner",
                140,
                entry => Facts(entry)?.Owner ?? string.Empty,
                Avalonia.Media.TextAlignment.Right
            ),
        ];
    }

    /// <summary>
    /// Whether a directory takes a drag of entries of this work's upstream.
    /// </summary>
    /// <remarks>
    /// A checkin brings what the Layout being worked in tracks, so a drag from another Vault's copy — or from a Layout of
    /// the Workspace's own, which is not an upstream at all — is not one a directory can take: there is no run that would
    /// name it. What is left is the work's own upstream, dropped on a directory of the work.
    /// <para>
    /// The Vault that is tracked is read in the background, so it may not be known yet: a drag judged before it lands is
    /// refused rather than waited for, and the next one — the place having been drawn again — is judged with the answer.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory it is being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>Whether it can be taken.</returns>
    public bool Accepts(string directory, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(drag);

        if (Carried(drag.Data) is not { Count: > 0 } carrying)
        {
            return false;
        }

        var source = carrying[0].Source;

        // Something already here is still a drop worth taking, because what it is owed is a word rather than nothing: a
        // drag that cannot be checked in is a drag the user is told about, not one that quietly did nothing.
        return source.Vault is { Length: > 0 } vault
            && carrying.All(carry => string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal))
            && string.Equals(vault, Tracked(), StringComparison.Ordinal)
            && Holds(directory)
            && Plan(carrying, source) is { References.Count: > 0 } or { Held: > 0 };
    }

    /// <summary>Takes the drag: what it carried is brought in under the directory it was dropped on.</summary>
    /// <param name="directory">The directory it was dropped on.</param>
    /// <param name="drag">What was dragged.</param>
    /// <returns>Whether it was taken.</returns>
    public bool Drop(string directory, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(drag);

        if (Carried(drag.Data) is not { Count: > 0 } carrying)
        {
            return false;
        }

        var source = carrying[0].Source;

        if (
            source.Vault is not { Length: > 0 } vault
            || !carrying.All(carry => string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal))
            || Plan(carrying, source) is not { } plan
            || !Holds(directory)
        )
        {
            return false;
        }

        // What this Layout already holds is left out of the run rather than handed to it: the command line refuses the
        // whole run for one entry it knows, which would be a drag of three files refused because one of them was already
        // here. What was left out is said, since a drop that brought in two of three and said nothing is a drop that lied.
        if (plan.References.Count > 0)
        {
            Bring(vault, plan.References, Path.GetFullPath(directory));
        }

        if (plan.Held > 0)
        {
            Report(
                "rorolala_vcs.place.already_here_title",
                RolaI18N.Get("rorolala_vcs.place.already_here", plan.Held)
            );
        }

        return plan.References.Count > 0 || plan.Held > 0;
    }

    /// <summary>
    /// Opens the work's own upstream, which is the Layout of the Vault the Layout being worked in tracks.
    /// </summary>
    /// <remarks>
    /// The work's own upstream is the Layout of the Vault the Layout being worked in tracks, and the Workspace's default
    /// Vault where it tracks none: a bound Workspace names one Vault it works against even before a Layout was fetched
    /// from it. What is asked before either list is read is opened when it lands, because a menu item cannot wait for a
    /// program; a Workspace with neither says so instead of opening a list that is not what the item names.
    /// </remarks>
    public void OpenRoot()
    {
        // Asked here rather than left to a drawing: what the root opens is one of these two lists' answers, and the
        // click is the only thing that may have asked for them.
        _ = LayoutNames();
        _ = VaultNames();

        if ((_layoutNames is null && !_layoutsTried) || (_vaultNames is null && !_vaultsTried))
        {
            _rootAsked = true;

            return;
        }

        OpenRootNow();
    }

    /// <summary>Opens the Vault the work's upstream is, or says that there is none to open.</summary>
    private void OpenRootNow()
    {
        // The tracked Vault is read again here so that a list that landed since the click is used: what the root opens is
        // the answer this method is for, and the answer may be the one that has just arrived.
        _ = LayoutNames();

        if (RootOf(_layoutNames, _vaultNames) is { Length: > 0 } vault)
        {
            Open(_host, [Vaults, vault]);

            return;
        }

        _host.Dialogs.Notify(
            new Notification(
                NotificationLevel.Warning,
                RolaI18N.Get("rorolala_vcs.place.no_root_title"),
                () =>
                    new TextBlock
                    {
                        Text = RolaI18N.Get("rorolala_vcs.place.no_root"),
                        TextWrapping = TextWrapping.Wrap,
                        MaxWidth = 420,
                    }
            )
        );
    }

    /// <summary>
    /// Which Vault the work's own upstream is, from what the two lists answered.
    /// </summary>
    /// <remarks>
    /// The Vault the Layout being worked in tracks is the work's upstream, and the Workspace's default Vault is the answer
    /// where it tracks none: a bound Workspace names one Vault it works against before any Layout has been fetched from
    /// it. Nothing is the answer for a Workspace that has neither, which is what the root says rather than opening a list
    /// that is not what it is named for.
    /// </remarks>
    /// <param name="layouts">The Layouts, or nothing while they are being read.</param>
    /// <param name="vaults">The Vaults, or nothing while they are being read.</param>
    /// <returns>The Vault's name, or nothing when there is no Vault to open.</returns>
    internal static string? RootOf(
        IReadOnlyList<LayoutListing>? layouts,
        IReadOnlyList<VaultListing>? vaults
    ) =>
        layouts?.FirstOrDefault(layout => layout.Current)?.Tracking is { Length: > 0 } tracked
            ? tracked
            : vaults?.FirstOrDefault(vault => vault.Default)?.Name;

    /// <summary>What a drag of entries comes to: what is to be brought in, and what is already here.</summary>
    /// <param name="carrying">What the drag carried.</param>
    /// <param name="source">The Layout whose copy they are of.</param>
    /// <returns>The plan, or nothing when nothing of the copy is left to name.</returns>
    private UpstreamPlan? Plan(IReadOnlyList<UpstreamCarry> carrying, UpstreamSource source)
    {
        ArgumentNullException.ThrowIfNull(carrying);
        ArgumentNullException.ThrowIfNull(source);

        return Tree(source) is { } tree ? UpstreamBringing.Plan(tree, carrying) : null;
    }

    /// <summary>The entry a row stands for, when it stands for one.</summary>
    /// <param name="entry">The row.</param>
    /// <returns>The entry, or nothing.</returns>
    private static LayoutEntry? Facts(LogicalEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return (entry.Data as UpstreamCarry)?.Entry;
    }

    /// <summary>Every entry a drag carries, in the order it was carried.</summary>
    /// <param name="data">What the drag carries.</param>
    /// <returns>The entries, which is nothing when the drag is not a place's own.</returns>
    private static IReadOnlyList<UpstreamCarry> Carried(object? data) =>
        data switch
        {
            UpstreamCarry one => [one],
            object[] many => [.. many.OfType<UpstreamCarry>()],
            IReadOnlyList<object> many => [.. many.OfType<UpstreamCarry>()],
            _ => [],
        };

    /// <summary>The segments an address names, which is what the place answers by.</summary>
    /// <param name="address">The address.</param>
    /// <returns>The segments, with the empty ones a path is written with left out.</returns>
    internal static string[] Segments(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        return
        [
            .. address.Payload.Split('/', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries),
        ];
    }

    /// <summary>The address of a place a level of the work is at, which is its domain, its name, and a prefix in it.</summary>
    /// <param name="segments">The segments, in the order the address writes them.</param>
    /// <returns>The address.</returns>
    internal static LogicalAddress Of(params string[] segments)
    {
        ArgumentNullException.ThrowIfNull(segments);

        return new LogicalAddress(Scheme, string.Join('/', segments));
    }

    /// <summary>The Layout and the prefix an address names, when it names a level of one.</summary>
    /// <param name="address">The address.</param>
    /// <returns>The pair, or nothing when the address names no Layout.</returns>
    internal static (UpstreamSource Source, string Prefix)? Target(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        var segments = Segments(address);

        return segments switch
        {
            [Vaults, var vault, .. var inVault] => (
                new UpstreamSource(Vaults, vault),
                string.Join('/', inVault)
            ),
            [Layouts, var layout, .. var inLayout] => (
                new UpstreamSource(Layouts, layout),
                string.Join('/', inLayout)
            ),
            _ => null,
        };
    }

    /// <summary>Whether an entry may be moved to a prefix: it is one, and nothing is where it would land.</summary>
    /// <param name="tree">The tree the two sides make.</param>
    /// <param name="path">The path the entry is at.</param>
    /// <param name="prefix">The prefix it is to be put in.</param>
    /// <returns>Whether the move means something.</returns>
    private static bool Editable(UpstreamEntriesTree tree, string path, string prefix) =>
        tree.Find(path)?.Entry is not null
        && Landing(path, prefix) is { } to
        && tree.Find(to) is null;

    /// <summary>Where an entry would be after being put in a prefix, or nothing where that changes nothing.</summary>
    /// <param name="path">The path the entry is at.</param>
    /// <param name="prefix">The prefix it is dropped on.</param>
    /// <returns>The path it would be at, or nothing.</returns>
    internal static string? Landing(string path, string prefix)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(prefix);

        // Let go where it already is: the drop means nothing, and the platform is told so rather than the Vault.
        if (string.Equals(UpstreamEntriesTree.Parent(path) ?? string.Empty, prefix, StringComparison.Ordinal))
        {
            return null;
        }

        var leaf = path[(path.LastIndexOf('/') + 1)..];

        return prefix.Length == 0 ? leaf : prefix + "/" + leaf;
    }

    /// <summary>Opens the browser's own listing at a place, out of step with what it was following.</summary>
    /// <param name="host">The host, whose windows the browser dock is reached through.</param>
    /// <param name="segments">The segments of the place to open.</param>
    private static void Open(IPluginHost host, string[] segments)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(segments);

        // The dock is the File System plugin's own, and the address is read by this plugin's provider: what is asked is
        // that the browser show a place, and which dock draws it is that plugin's answer.
        host.Windows.Show(
            global::FileSystemPlugin.FileSystemPlugin.DirectoryDockNameId,
            Of(segments).ToString()
        );
    }

    /// <summary>One of the two domains the work holds.</summary>
    /// <param name="name">Its name, which is also its segment of the address.</param>
    /// <returns>The row.</returns>
    private static LogicalEntry Domain(string name) =>
        new(name, Draggable: false, Opens: Of(name), Picture: EntryIcons.Folder);

    /// <summary>The Vaults the Workspace knows, each opening that Vault's copy.</summary>
    /// <returns>The rows.</returns>
    private IReadOnlyList<LogicalEntry> VaultRows()
    {
        if (VaultNames() is not { } names)
        {
            // Nothing yet rather than a row: the read is going, and the draw it makes when it lands is what brings the
            // names in. A read that failed has a row, and one that cannot be made at all — a window with no base — has the
            // same one, since there is nothing to wait for either way.
            return _vaultsTried || BaseDirectory.Current is not { Length: > 0 }
                ? [Failed(RolaI18N.Get("rorolala_vcs.place.vaults_failed"), _vaultsSaid, list: UpstreamList.Vaults)]
                : [];
        }

        return
        [
            Up(Of()),
            .. names.Select(vault =>
                new LogicalEntry(vault.Name, Draggable: false, Opens: Of(Vaults, vault.Name), Picture: EntryIcons.Folder)
            ),
        ];
    }

    /// <summary>The Layouts the Workspace works in, each opening its entries.</summary>
    /// <returns>The rows.</returns>
    private IReadOnlyList<LogicalEntry> LayoutRows()
    {
        if (LayoutNames() is not { } layouts)
        {
            return _layoutsTried || BaseDirectory.Current is not { Length: > 0 }
                ? [Failed(RolaI18N.Get("rorolala_vcs.place.layouts_failed"), _layoutsSaid, list: UpstreamList.Layouts)]
                : [];
        }

        return
        [
            Up(Of()),
            .. layouts.Select(layout =>
                new LogicalEntry(layout.Name, Draggable: false, Opens: Of(Layouts, layout.Name), Picture: EntryIcons.Folder)
            ),
        ];
    }

    /// <summary>One level of one Layout.</summary>
    /// <param name="source">The Layout.</param>
    /// <param name="prefix">The prefix, empty for the top of its Layout.</param>
    /// <returns>The rows.</returns>
    private IReadOnlyList<LogicalEntry> Level(UpstreamSource source, string prefix)
    {
        var reading = Reading(source);

        // Nothing yet rather than a row: a place being read is a place with nothing to say about itself, and the wait is
        // what the window draws for it by being drawn again.
        if (reading.State is UpstreamState.Reading)
        {
            return [];
        }

        if (reading.State is not UpstreamState.Ok || Tree(source) is not { } tree)
        {
            return [Up(Of(source.Domain)), State(reading, source)];
        }

        // The way up from the top of a Layout is the list it is one of, which is not a prefix of the copy: what is over the
        // copy is what it is one of, so the row is made here rather than read out of the tree.
        return
        [
            .. (prefix.Length == 0 ? new[] { Up(Of(source.Domain)) } : []),
            .. UpstreamSpots.Of(tree, prefix).Select(spot => Spot(source, spot)),
        ];
    }

    /// <summary>The row that leads out of a level, to the place over it.</summary>
    /// <param name="over">The place it leads to.</param>
    /// <returns>The row.</returns>
    private static LogicalEntry Up(LogicalAddress over) =>
        new("..", Draggable: false, Opens: over, Picture: EntryIcons.Folder);

    /// <summary>One row of a level, as the listing draws it.</summary>
    /// <param name="source">The Layout it is of.</param>
    /// <param name="spot">The row.</param>
    /// <returns>The entry to draw.</returns>
    private LogicalEntry Spot(UpstreamSource source, UpstreamSpot spot)
    {
        ArgumentNullException.ThrowIfNull(spot);

        // The way up leads out of the level and is not a thing of the copy: at the top of a Layout it leads to the list it
        // is one of, and under it to the prefix above.
        if (spot.Name == ".." && spot.Entry is null)
        {
            return new LogicalEntry(
                spot.Name,
                Draggable: false,
                Opens: Of(source.Domain, source.Name, spot.Path)
            );
        }

        return new LogicalEntry(
            spot.Name,
            Draggable: spot.Entry is not null,
            Opens: spot.Folder ? Of(source.Domain, source.Name, spot.Path) : null,
            Menu: Menu(source, spot),
            Data: spot.Entry is not null ? new UpstreamCarry(source, spot.Path, spot.Entry) : null,
            Picture: spot.Folder
                ? size => EntryIcons.For(spot.Path, EntryKind.Directory, size)
                : size => EntryIcons.For(spot.Path, EntryKind.File, size)
        );
    }

    /// <summary>What a row offers, which is what may be done with the entry it is.</summary>
    /// <param name="source">The Layout it is of.</param>
    /// <param name="spot">The row.</param>
    /// <returns>The items, or nothing when there is nothing to do.</returns>
    private IReadOnlyList<ContextMenuItem>? Menu(UpstreamSource source, UpstreamSpot spot)
    {
        if (spot.Entry is not { } entry)
        {
            return null;
        }

        if (Tree(source)?.IsHeld(entry) == true)
        {
            // One entry this side already holds is not one to bring in: what it is, is a path this work names, and that
            // is the one thing worth saying about it.
            return
            [
                new ContextMenuItem(
                    "rorolala_vcs.place.held",
                    10,
                    _ => Report(
                        "rorolala_vcs.place.held_title",
                        RolaI18N.Get(
                            "rorolala_vcs.place.held_at",
                            Tree(source)?.LocalPath(entry.Uuid) is { Length: > 0 } at ? at : "?"
                        )
                    ),
                    Icons.Lock
                ),
            ];
        }

        return
        [
            // A checkin brings what the Layout being worked in *tracks*, so an entry of another Vault's copy — or of a
            // Layout of the Workspace's own — is not something a checkin can name: what there is to do with it is read it,
            // and move it where it belongs.
            .. (source.Vault is { Length: > 0 } vault && string.Equals(vault, Tracked(), StringComparison.Ordinal)
                ? new ContextMenuItem[]
                {
                    new("rorolala_vcs.place.checkin", 10, _ => CheckIn(vault, entry), Icons.Download),
                }
                : []),
            new ContextMenuItem(
                source.Vault.Length > 0 ? "rorolala_vcs.place.copy_path" : "rorolala_vcs.place.copy_path_layout",
                20,
                _ => Report(
                    source.Vault.Length > 0 ? "rorolala_vcs.place.path_title" : "rorolala_vcs.place.path_title_layout",
                    entry.Path
                ),
                Icons.ContentCopy
            ),
        ];
    }

    /// <summary>One row for a state a Layout's copy could not be read in.</summary>
    /// <param name="reading">The reading.</param>
    /// <param name="source">The Layout it was read for, which reading it again is about.</param>
    /// <returns>The entry to draw.</returns>
    private LogicalEntry State(UpstreamReading reading, UpstreamSource source)
    {
        ArgumentNullException.ThrowIfNull(reading);
        ArgumentNullException.ThrowIfNull(source);

        return reading.State switch
        {
            UpstreamState.NotFetched => new LogicalEntry(
                RolaI18N.Get("rorolala_vcs.place.not_fetched", reading.Name),
                Draggable: false,
                Menu:
                [
                    new ContextMenuItem(
                        "rorolala_vcs.place.fetch",
                        10,
                        _ => Fetch(),
                        Icons.Download
                    ),
                ]
            ),
            UpstreamState.NotWorkspace => new LogicalEntry(
                RolaI18N.Get("rorolala_vcs.place.not_workspace"),
                Draggable: false
            ),
            _ => Failed(RolaI18N.Get("rorolala_vcs.place.failed"), reading.Said, source),
        };
    }

    /// <summary>A row that could not be read, with reading it again in its menu.</summary>
    /// <remarks>
    /// One item does both things a reader can want from a row like this: it says what the command line said, and it lets
    /// go of the answer so that the drawing that follows asks again. Two items — one to read and one to say why — would
    /// be a choice between two halves of one question.
    /// </remarks>
    /// <param name="said">What it says.</param>
    /// <param name="why">What the command line said, which is what the report shows.</param>
    /// <param name="source">The Layout it was read for, when the failure is about one Layout.</param>
    /// <param name="list">Which list failed, when the failure is about one of the two lists.</param>
    /// <returns>The row.</returns>
    private LogicalEntry Failed(
        string said,
        string why,
        UpstreamSource? source = null,
        UpstreamList? list = null
    )
    {
        ArgumentNullException.ThrowIfNull(said);

        return new LogicalEntry(
            said,
            Draggable: false,
            Menu:
            [
                new ContextMenuItem(
                    "rorolala_vcs.place.read_again",
                    10,
                    _ => ReadAgain(source, list, why),
                    Icons.Refresh
                ),
            ]
        );
    }

    /// <summary>A segment nothing answers for.</summary>
    /// <param name="segment">What was written.</param>
    /// <returns>The row.</returns>
    private static LogicalEntry Unanswered(string segment) =>
        new(RolaI18N.Get("rorolala_vcs.place.unknown", segment), Draggable: false);

    /// <summary>
    /// Lets go of a failed answer, says what the command line said, and draws the place again so the read is made again.
    /// </summary>
    /// <remarks>
    /// A failure is kept like any other answer, because a place that asked again on every drawing would be a program run
    /// per frame. Asking again is therefore the reader's, and what it lets go of is only the answer it was looking at:
    /// the files having changed lets go of every answer (see <see cref="Forget"/>).
    /// </remarks>
    /// <param name="source">The Layout the failure was of, when it was of one.</param>
    /// <param name="list">Which list failed, when the failure was about one of them.</param>
    /// <param name="why">What the command line said.</param>
    private void ReadAgain(UpstreamSource? source, UpstreamList? list, string why)
    {
        if (source is { } read)
        {
            _readings.Remove(read.Key);
            _trees.Remove(read.Key);
        }
        else if (list is UpstreamList.Vaults)
        {
            _vaultsTried = false;
            _vaultsSaid = string.Empty;
            _vaultNames = null;
        }
        else
        {
            _layoutsTried = false;
            _layoutsSaid = string.Empty;
            _layoutNames = null;
        }

        Report("rorolala_vcs.place.failed_title", why);
        _host.Files.Redraw();
    }

    /// <summary>What the two sides say of one Layout, read again when the base moved or nothing is kept.</summary>
    /// <param name="source">The Layout.</param>
    /// <returns>The reading.</returns>
    private UpstreamReading Reading(UpstreamSource source)
    {
        ArgumentNullException.ThrowIfNull(source);

        var directory = BaseDirectory.Current ?? string.Empty;

        if (!string.Equals(_readAt, directory, StringComparison.Ordinal))
        {
            Forget();
        }

        if (_readings.TryGetValue(source.Key, out var known))
        {
            return known;
        }

        // Read on a thread of its own, because reading the copy may reach the Vault: a run of `rola` on the window's
        // thread is a window that does not answer until the Vault does, and one that never answers cannot be closed.
        if (_pending.Add(source.Key))
        {
            var version = _version;

            _ = Task.Run(() =>
            {
                var reading =
                    directory.Length > 0
                        ? UpstreamEntriesReading.Read(directory, source, _log)
                        : UpstreamReading.Nothing(UpstreamState.NotWorkspace, source.Name);

                Dispatcher.UIThread.Post(() => Landed(source, directory, version, reading));
            });
        }

        return UpstreamReading.Nothing(UpstreamState.Reading, source.Name);
    }

    /// <summary>Keeps what a read that landed came to, and has the place drawn again.</summary>
    /// <remarks>
    /// A read of the work as it was is dropped rather than kept: the base moved under it, so its paths are another
    /// work's, and keeping it would serve the previous work's tree for the new one until something else let the answers go.
    /// What is dropped is read again by the draw that follows, which is the draw of the base it was not of.
    /// </remarks>
    /// <param name="source">The Layout the reading is of.</param>
    /// <param name="directory">Where it was read, which is what it is judged by.</param>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="reading">What it came to.</param>
    private void Landed(UpstreamSource source, string directory, int version, UpstreamReading reading)
    {
        _pending.Remove(source.Key);

        if (version != _version || !string.Equals(_readAt, directory, StringComparison.Ordinal))
        {
            _host.Log.Info($"what was read of `{source.Key}` is of another work, and was dropped");

            // The read that gets the answer for the work as it is now has to be asked for, and the drawing is what asks.
            Drawn();

            return;
        }

        _readings[source.Key] = reading;

        if (reading.State is UpstreamState.Ok)
        {
            _trees[source.Key] = UpstreamEntriesTree.Build(reading.Upstream, reading.Local);
        }

        Drawn();
    }

    /// <summary>The tree one Layout was read to, when it was read.</summary>
    /// <param name="source">The Layout.</param>
    /// <returns>The tree, or nothing when nothing was read.</returns>
    private UpstreamEntriesTree? Tree(UpstreamSource source)
    {
        ArgumentNullException.ThrowIfNull(source);

        // The reading is what makes the tree, so asking it first is what makes a tree that was never made.
        _ = Reading(source);

        return _trees.GetValueOrDefault(source.Key);
    }

    /// <summary>
    /// The Vault the Layout being worked in tracks, which is the one a checkin brings entries from.
    /// </summary>
    /// <remarks>
    /// The list of Layouts is read in the background, so this is empty until it lands: a drop judged in that moment is
    /// refused rather than waited for, because waiting for it on the window's thread is the stall this is here to end.
    /// Nothing is lost by it — the read is asked for as soon as the place is first drawn, and a touch draws it again.
    /// </remarks>
    /// <returns>The Vault, or an empty string when nothing says.</returns>
    private string Tracked()
    {
        _ = LayoutNames();

        return _layoutNames?.FirstOrDefault(layout => layout.Current)?.Tracking ?? string.Empty;
    }

    /// <summary>
    /// The Vaults the Workspace knows, asking for them in the background when they are not here yet.
    /// </summary>
    /// <remarks>
    /// As one background answer rather than a run behind the answer, for the reason the copy is read that way: a provider
    /// is asked on the window's thread, and a name list that took a run to make would take the window with it. The read
    /// is asked for once and the answer is kept until the files are said to have changed.
    /// </remarks>
    /// <returns>The Vaults, or nothing while the read is going or when it failed.</returns>
    private IReadOnlyList<VaultListing>? VaultNames()
    {
        var directory = BaseDirectory.Current ?? string.Empty;

        if (!Refresh())
        {
            return null;
        }

        // A read that landed and failed is not asked again on every draw: what it said is kept and drawn, and a new
        // touch is what asks again.
        if (_vaultsTried || _vaultsAsking)
        {
            return _vaultNames;
        }

        _vaultsAsking = true;

        var version = _version;

        _ = Task.Run(() =>
        {
            var vaults = LayoutEntriesCommand.Vaults(directory);

            Dispatcher.UIThread.Post(() => LandedVaults(directory, version, vaults));
        });

        return null;
    }

    /// <summary>Keeps what a read of the Vaults that landed came to, and has the place drawn again.</summary>
    /// <param name="directory">Where it was read, which is what it is judged by.</param>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="vaults">What it came to.</param>
    private void LandedVaults(string directory, int version, VaultsAnswer vaults)
    {
        _vaultsAsking = false;

        // A name list is kept per place rather than per directory, so one of a base that has moved would be the names of
        // another work drawn under this one's rows: it is dropped, and the draw that made it stale asks again.
        if (version != _version || !string.Equals(_readAt, directory, StringComparison.Ordinal))
        {
            Drawn();

            return;
        }

        _vaultsTried = true;

        if (vaults.Done)
        {
            _vaultNames = vaults.Vaults;
        }
        else
        {
            _vaultsSaid = vaults.Said;
        }

        Drawn();
    }

    /// <summary>
    /// The Layouts the Workspace works in, asking for them in the background when they are not here yet.
    /// </summary>
    /// <remarks>
    /// The tracked Vault is read from it, which is what a drop on a directory and a row's checkin item are judged by, so
    /// the read is asked for as soon as anything wants either — not only when the list of Layouts is opened.
    /// </remarks>
    /// <returns>The Layouts, or nothing while the read is going or when it failed.</returns>
    private IReadOnlyList<LayoutListing>? LayoutNames()
    {
        var directory = BaseDirectory.Current ?? string.Empty;

        if (!Refresh())
        {
            return null;
        }

        if (_layoutsTried || _layoutsAsking)
        {
            return _layoutNames;
        }

        _layoutsAsking = true;

        var version = _version;

        _ = Task.Run(() =>
        {
            var layouts = LayoutEntriesCommand.Layouts(directory);

            Dispatcher.UIThread.Post(() => LandedLayouts(directory, version, layouts));
        });

        return null;
    }

    /// <summary>Keeps what a read of the Layouts that landed came to, and has the place drawn again.</summary>
    /// <param name="directory">Where it was read, which is what it is judged by.</param>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="layouts">What it came to.</param>
    private void LandedLayouts(string directory, int version, LayoutsAnswer layouts)
    {
        _layoutsAsking = false;

        if (version != _version || !string.Equals(_readAt, directory, StringComparison.Ordinal))
        {
            Drawn();

            return;
        }

        _layoutsTried = true;

        if (layouts.Done)
        {
            _layoutNames = layouts.Layouts;
        }
        else
        {
            _layoutsSaid = layouts.Said;
        }

        Drawn();
    }

    /// <summary>
    /// Has the place drawn again, because the rows it just read are at hand.
    /// </summary>
    /// <remarks>
    /// A drawing and not a change: saying the files changed would have every other reader of the work give its answers
    /// back, and one of those is what this place reads a level from — so the next drawing would read it again, and its
    /// landing would say the same thing to this one. Two readers taking turns emptying each other's answers is one
    /// `rola` and two walks of the whole tree per turn, for as long as the window is open.
    /// <para>
    /// A root that was asked for while the two lists were being read is opened here, on the drawing in which both are in:
    /// the click could not wait for a program, and this is the turn after it that can.
    /// </para>
    /// </remarks>
    private void Drawn()
    {
        if (_rootAsked && (_layoutsTried || _layoutNames is not null) && (_vaultsTried || _vaultNames is not null))
        {
            _rootAsked = false;
            OpenRootNow();
        }

        _host.Files.Redraw();
    }

    /// <summary>Lets go of what was read where the base moved, and whether there is a base to read at all.</summary>
    /// <returns>Whether the kept answers are about the base the browser is rooted at now.</returns>
    private bool Refresh()
    {
        var directory = BaseDirectory.Current ?? string.Empty;

        if (!string.Equals(_readAt, directory, StringComparison.Ordinal))
        {
            Forget();
        }

        return directory.Length > 0;
    }

    /// <summary>Whether a directory is inside a Workspace, kept for as long as the answers are.</summary>
    /// <param name="directory">The directory to ask about.</param>
    /// <returns>Whether it is inside one.</returns>
    private bool Holds(string directory)
    {
        if (!_holds.TryGetValue(directory, out var holds))
        {
            holds = WorkspaceNotice.Holds(directory, _log);
            _holds[directory] = holds;
        }

        return holds;
    }

    /// <summary>Lets go of what was read, so the next answer is read from the files as they are now.</summary>
    private void Forget()
    {
        _version++;
        _readAt = BaseDirectory.Current ?? string.Empty;
        _readings.Clear();
        _trees.Clear();
        _holds.Clear();

        // The reads still in flight are of the work as it was. Their answers are dropped where they land, by the version
        // they were asked for, and the flags are cleared here so that the draw after this one asks again.
        _vaultNames = null;
        _vaultsAsking = false;
        _vaultsTried = false;
        _vaultsSaid = string.Empty;
        _layoutNames = null;
        _layoutsAsking = false;
        _layoutsTried = false;
        _layoutsSaid = string.Empty;
        _rootAsked = false;
    }

    /// <summary>Brings one entry of the tracked Vault in, which is a checkin of the one path.</summary>
    /// <param name="vault">The Vault the entry is of.</param>
    /// <param name="entry">The entry.</param>
    private void CheckIn(string vault, LayoutEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        Bring(vault, [CheckinCommand.Reference(entry)], to: null);
    }

    /// <summary>
    /// Runs a checkin of these references, landing them at the paths the Layout names them by or under a directory.
    /// </summary>
    /// <remarks>
    /// One run for everything a drag carried, because the command line is atomic: a reference it refuses is the whole run
    /// refused, and a batch that half landed would be a work with no record of which half. The run materialises the copy
    /// again before it resolves anything, so the place is read again whatever became of it.
    /// </remarks>
    /// <param name="vault">The Vault the entries are of, which is what a failure is said about.</param>
    /// <param name="references">What is to be brought in.</param>
    /// <param name="to">The directory to land them under, or nothing to land them at their upstream paths.</param>
    private void Bring(string vault, IReadOnlyList<string> references, string? to)
    {
        ArgumentNullException.ThrowIfNull(references);

        if (
            references.Count == 0
            || Program() is not { } program
            || BaseDirectory.Current is not { Length: > 0 } directory
        )
        {
            // A drop that did nothing and said nothing is the one thing a reader cannot tell from one that has not landed
            // yet, so it is said.
            FailureNotice.Show(
                _host.Dialogs,
                _host.Icons,
                RolaI18N.Get("rorolala_vcs.place.no_program"),
                RolaI18N.Get("rorolala_vcs.place.no_program_why")
            );

            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.checkin");

        _ = Task.Run(() =>
        {
            var answer = CheckinCommand.Run(program, directory, references, run.Records, _hub.Touch, to);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);

                if (!answer.Done || answer.Failed.Count > 0)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.place.checkin_failed", vault),
                        answer.Done ? string.Join('\n', answer.Failed) : answer.Said
                    );
                }

                // The run fetches the Vault's Layout before it resolves anything, so the copy this place was read from
                // may be newer whatever became of the work.
                _host.Files.Touch();
            });
        });
    }

    /// <summary>Moves paths within one Layout, one run each.</summary>
    /// <remarks>
    /// The words a refusal is shown with are the command's own, and the entry it is about is named beside them: what the
    /// Vault refuses a move for is usually that the entry is held by another account, and a reader who is told only that
    /// something failed has nothing to act on. What the command agreed to is written into the copy, so a move the copy
    /// still names at its old path is read as a refusal whatever the run said about itself — a status of zero is not the
    /// whole of the answer, and a drop that moved nothing cannot be left to say nothing.
    /// </remarks>
    /// <param name="source">The Layout they are moved in.</param>
    /// <param name="moves">Every move, in the order the drop made them, with the account holding the entry.</param>
    private void Move(
        UpstreamSource source,
        IReadOnlyList<(string From, string To, string? Holder)> moves
    )
    {
        ArgumentNullException.ThrowIfNull(source);
        ArgumentNullException.ThrowIfNull(moves);

        if (Program() is not { } program || BaseDirectory.Current is not { Length: > 0 } directory)
        {
            // Nothing was run, and a drag that did nothing and said nothing is the one thing a reader cannot tell from a
            // drag that has not landed yet.
            FailureNotice.Show(
                _host.Dialogs,
                _host.Icons,
                RolaI18N.Get("rorolala_vcs.place.no_program"),
                RolaI18N.Get("rorolala_vcs.place.no_program_why")
            );

            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.path_move");

        _ = Task.Run(() =>
        {
            List<(string From, string To, string? Holder, string Said, bool Done)> outcomes = [];

            // One run each, and not one for the lot: the command moves one Layout path at a time, and an entry the Vault
            // refuses — a locked one, say — must not take the rest of the drop down with it.
            foreach (var (from, to, holder) in moves)
            {
                var (done, said) = LayoutPathMoveCommand.Run(
                    program,
                    directory,
                    source.Layout,
                    from,
                    to,
                    run.Records,
                    _hub.Touch
                );

                outcomes.Add((from, to, holder, said, done));
            }

            // What the command agreed to is written into the copy as well, so the copy is what says whether a move
            // happened: asking it costs one run, and not asking it would be a refusal that left a zero status behind it
            // and so looked like a success. Only when every run said it ended well is there anything to ask about.
            var after = outcomes.TrueForAll(outcome => outcome.Done) ? Reads(source, directory) : null;

            var refused = outcomes
                .Where(outcome =>
                    !outcome.Done
                    || (
                        after is { Done: true } copy
                        && copy.Entries.Any(entry =>
                            string.Equals(entry.Path, outcome.From, StringComparison.Ordinal)
                        )
                    )
                )
                .ToList();

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);

                foreach (var (from, to, holder, said, _) in refused)
                {
                    var leaf = from[(from.LastIndexOf('/') + 1)..];

                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        holder is { Length: > 0 }
                            ? RolaI18N.Get("rorolala_vcs.place.move_refused_held", leaf, holder)
                            : RolaI18N.Get("rorolala_vcs.place.move_refused", leaf),
                        said.Trim() is { Length: > 0 } words
                            ? words
                            : RolaI18N.Get("rorolala_vcs.place.move_refused_why", to)
                    );
                }

                // The command writes what the Vault agreed to into the copy as well, so the place is read again from what
                // is here rather than from the Vault.
                _host.Files.Touch();
            });
        });
    }

    /// <summary>
    /// What a Layout's copy says, read once, for a caller that has to know whether a write landed.
    /// </summary>
    /// <param name="source">The Layout to read.</param>
    /// <param name="directory">Where the work is rooted.</param>
    /// <returns>What the copy says.</returns>
    private static EntriesAnswer Reads(UpstreamSource source, string directory) =>
        source.Vault.Length > 0
            ? LayoutEntriesCommand.Upstream(directory, source.Vault)
            : LayoutEntriesCommand.Layout(directory, source.Name);

    /// <summary>Fetches the Vault's Layout, which is what a copy that was never brought here needs.</summary>
    private void Fetch()
    {
        if (BaseDirectory.Current is not { Length: > 0 } directory)
        {
            return;
        }

        _fetch.Fetch(directory, () =>
        {
            Forget();
            _host.Files.Touch();
        });
    }

    /// <summary>Shows what a row says, since a menu item is a word and what it has to say is a path.</summary>
    /// <param name="title">The key of what the report is called.</param>
    /// <param name="said">What it says.</param>
    private void Report(string title, string said) =>
        _host.Dialogs.Report(new Report(RolaI18N.Get(title), said));

    /// <summary>The program the window was opened by, or nothing when it was opened on its own.</summary>
    /// <returns>The program, or nothing.</returns>
    private static string? Program() =>
        Environment.GetEnvironmentVariable(Sync.Program) is { Length: > 0 } program ? program : null;
}

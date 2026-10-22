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

/// <summary>What a drop of entries comes to: every entry under what was carried, in the order it was carried.</summary>
/// <remarks>
/// A folder is not an entry, so what a drop of one brings is what stands under it — the folder's own entry included,
/// since a path may be both. A path the copy no longer names is dropped rather than guessed at: the copy moved under the
/// drag, and what is not there is not something a checkin can name.
/// </remarks>
internal static class UpstreamBringing
{
    /// <summary>Every entry a drag of these carries brings, deduplicated and in the order it was carried.</summary>
    /// <remarks>
    /// The walk a folder makes, and the one a checkin is planned from: a folder is not an entry, so what it brings is what
    /// stands under it — its own entry included, since a path may be both. A uuid one carry already brought is not brought
    /// again, because the command line would be naming it twice.
    /// </remarks>
    /// <param name="tree">The tree the Vault's copy was read to.</param>
    /// <param name="carrying">What the drag carried, in the order it carried it.</param>
    /// <returns>The entries.</returns>
    public static IReadOnlyList<LayoutEntry> Entries(
        UpstreamEntriesTree tree,
        IReadOnlyList<UpstreamCarry> carrying
    )
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(carrying);

        var found = new List<LayoutEntry>();
        var seen = new HashSet<string>(StringComparer.Ordinal);

        foreach (var carry in carrying)
        {
            foreach (var entry in Under(tree, carry.Path))
            {
                // A folder and an entry under it dragged together are one entry: the command line would name it twice,
                // and a run that says the same thing twice is a run refused as a duplicate.
                if (seen.Add(entry.Uuid))
                {
                    found.Add(entry);
                }
            }
        }

        return found;
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
/// The actions are the command line's, as everywhere else in this plugin: bringing entries in is a checkin of their
/// paths, and moving one is a <c>layout path move</c> against the Layout the level is of, made when the entry is
/// dropped on a folder's row. A checkin is planned before it is run — see [`CheckinFlow`] — because the command line
/// refuses a run whole for the first reference it will not take: a folder dragged in, a choice of rows cut out and
/// pasted into a directory, and a row's own menu item all come to the same plan, and the same question where the plan
/// does not fit. Moving one entry is still one run, because that is what a folder's row does.
/// </para>
/// </remarks>
internal sealed class UpstreamPlace
    : ILogicalViewProvider, IDropTarget, ILogicalRefusal, ILogicalFolderEditor
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

    /// <summary>The account the work acts as, which is who a row has to be held by to be this side's to name.</summary>
    /// <remarks>
    /// An account is a private key and which one is in use is Rorolala's own answer, given by `rola account` — a run. A
    /// provider's answer is asked for on the window's thread, so the answer is read in the background and kept, the way the
    /// lists of Vaults and Layouts are.
    /// </remarks>
    private string _account = string.Empty;

    /// <summary>Whether a read of the account is going, so that one read is asked for once.</summary>
    private bool _accountAsking;

    /// <summary>Whether the account has been asked for, so a run that failed is not asked again on every draw.</summary>
    private bool _accountTried;

    /// <summary>What a checkin of a drag comes to, and what the user is asked about one that does not fit.</summary>
    private readonly CheckinFlow _checkin;

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
        _checkin = new CheckinFlow(host, log, hub, Tree);

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

    /// <summary>
    /// The empty folders this place holds, which are the work's drafts rather than anything read.
    /// </summary>
    /// <remarks>
    /// A path is all a folder is: one that has children has them because their paths begin with its own, which is what
    /// makes a folder made inside another, and a folder moved to another place, carry what it holds. Nothing here is
    /// written down: the place reads a copy of the Vault's Layout, so a folder this side made is a suggestion about what
    /// to put in it, and a suggestion that outlived the window would be a claim the Vault never made.
    /// </remarks>
    private readonly HashSet<string> _drafts = new(StringComparer.Ordinal);

    /// <summary>The empty folders this place holds, which is what the checks read.</summary>
    /// <remarks>
    /// A name is the place's own — the one it gave a folder, and the one a rename gave it — so a check that read it
    /// from a translation would be checking the words rather than the folders.
    /// </remarks>
    internal IReadOnlyCollection<string> DraftPaths => _drafts;

    /// <summary>The folder this place just made, which is the one its listing puts a name box on.</summary>
    private string? _made;

    /// <summary>Whether a folder was made, and the path of the one to name.</summary>
    /// <param name="address">The place it was made in.</param>
    /// <returns>The path, or nothing when nothing was just made.</returns>
    public string? Pending(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        var made = _made;
        _made = null;

        return made;
    }

    /// <inheritdoc />
    /// <remarks>
    /// Only a Vault's upstream holds folders of this side's: the Layouts under `layouts` are read from the work
    /// itself, and a folder there would be a path this side made up about the work. The lists over them — a Vault's, a
    /// Layout's — are read from the Vault rather than being a folder of it, so they hold none either.
    /// </remarks>
    public bool MakesFolders(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        return Target(address) is { } target
            && string.Equals(target.Source.Domain, Vaults, StringComparison.Ordinal)
            && target.Source.Vault is { Length: > 0 };
    }

    /// <inheritdoc />
    public bool NewFolder(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        if (Target(address) is not { } target || !MakesFolders(address))
        {
            return false;
        }

        var (source, prefix) = target;

        _made = Join(prefix, Free(source, prefix));
        _drafts.Add(_made);

        // Said as a warning because it is what it is: a folder with nothing in it is a plan, and a plan is gone when the
        // window is. A message would read as done work.
        Notify(
            NotificationLevel.Warning,
            RolaI18N.Get("rorolala_vcs.place.draft_folder_title"),
            RolaI18N.Get("rorolala_vcs.place.draft_folder")
        );

        _host.Files.Redraw();

        return true;
    }

    /// <summary>Whether a carry is a folder this side made, which is one no entry of the work stands at.</summary>
    /// <param name="carry">What a drag carried, or what a row stands for.</param>
    /// <returns>Whether it is one.</returns>
    private bool Draft(UpstreamCarry carry) =>
        carry.Entry is null && _drafts.Contains(carry.Path);

    /// <inheritdoc />
    /// <remarks>
    /// A folder this side made is its own to name. A row of the work's own upstream is the account's that holds it — all of
    /// it, since a folder is only what stands under it — and a Layout of the Workspace's own is not offered: what this names
    /// is the Vault's upstream, and the paths of the work's own Layout are the work itself.
    /// </remarks>
    public bool Renamable(object data)
    {
        if (data is not UpstreamCarry carry)
        {
            return false;
        }

        if (Draft(carry))
        {
            return true;
        }

        return string.Equals(carry.Source.Domain, Vaults, StringComparison.Ordinal)
            && Tree(carry.Source) is { } tree
            && Held(tree, carry, Account());
    }

    /// <inheritdoc />
    /// <remarks>
    /// A rename of something the work names is a move within its own folder, one run per entry against the Layout the level
    /// is of, which is what the Vault takes: the copy is written by those runs, and the rows follow them.
    /// </remarks>
    public bool Rename(LogicalAddress address, object data, string name)
    {
        ArgumentNullException.ThrowIfNull(address);
        ArgumentNullException.ThrowIfNull(name);

        if (data is not UpstreamCarry carry || !Renamable(carry))
        {
            return false;
        }

        var wanted = name.Trim();

        // A name is one path component, and one that names the folder's own place or the way over it is no name.
        if (
            wanted.Length == 0
            || wanted.Contains('/', StringComparison.Ordinal)
            || wanted is "." or ".."
        )
        {
            return false;
        }

        var to = Join(Above(carry.Path), wanted);

        if (string.Equals(to, carry.Path, StringComparison.Ordinal))
        {
            return true;
        }

        if (Occupied(carry.Source, to))
        {
            // Said rather than dropped: the box the name was typed in is gone once the name is taken, so a refusal that said
            // nothing would be a name that went nowhere and a reader with no way to tell it from a rename that happened.
            Notify(
                NotificationLevel.Warning,
                RolaI18N.Get("rorolala_vcs.place.rename_refused_title"),
                RolaI18N.Get("rorolala_vcs.place.refused_taken", to)
            );

            return false;
        }

        // A folder this side made is moved by rewriting its path, and what is under it comes with it: nothing is run,
        // because there is nothing of the work to run it against.
        if (Draft(carry))
        {
            Rebase(carry.Path, to);
            _host.Files.Redraw();

            return true;
        }

        if (Tree(carry.Source) is not { } tree)
        {
            return false;
        }

        // A folder this side made inside it is a path under this one, and it goes with it: the suggestion is about where the
        // work's things are, and one left at the old prefix would be a folder under a path the work no longer has.
        Rebase(carry.Path, to);

        var moves = Renaming(tree, carry, to);

        if (moves.Count > 0)
        {
            Move(carry.Source, moves);
        }

        return true;
    }

    /// <inheritdoc />
    /// <remarks>
    /// Only a folder this side made is the place's to take away: what the work names is the Vault's, and a row of it is
    /// removed by the command line, not by a suggestion about the work.
    /// </remarks>
    public bool Removable(object data) => data is UpstreamCarry carry && Draft(carry);

    /// <inheritdoc />
    public bool Remove(LogicalAddress address, object data)
    {
        ArgumentNullException.ThrowIfNull(address);

        if (data is not UpstreamCarry carry || !Draft(carry))
        {
            return false;
        }

        // What is under it goes with it: a folder taken away with its children left standing would be children of
        // nothing, and a listing of them would be a folder that is not there.
        if (
            _drafts.RemoveWhere(path =>
                string.Equals(path, carry.Path, StringComparison.Ordinal)
                || path.StartsWith(carry.Path + "/", StringComparison.Ordinal)
            ) == 0
        )
        {
            return false;
        }

        _host.Files.Redraw();

        return true;
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
        // own, and the ones the Layout refuses are said beside the ones that landed. A folder moves as the tree it is, and
        // one dropped inside itself — or back where it was — is not a move at all.
        return tree is not null
            && carrying.All(carry =>
                string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal)
                && !Inside(prefix, carry.Path)
                && !string.Equals(prefix, carry.Path, StringComparison.Ordinal)
            )
            && Moving(tree, carrying, prefix) is { Count: > 0 } moves
            && moves.All(move => tree.Find(move.To) is null);
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

        // A folder this side made is its own reason to be refused or taken, and none of the copy's reasons apply: it is
        // not in the copy at all.
        if (carrying is [{ Entry: null } draft] && _drafts.Contains(draft.Path))
        {
            var to = Join(prefix, Name(draft.Path));

            if (string.Equals(Above(draft.Path), prefix, StringComparison.Ordinal))
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_same", draft.Path);
            }

            return Occupied(source, to)
                ? RolaI18N.Get("rorolala_vcs.place.refused_taken", to)
                : RolaI18N.Get("rorolala_vcs.place.refused_generic");
        }

        foreach (var carry in carrying)
        {
            if (tree.Find(carry.Path) is not { } node)
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_gone");
            }

            // A path with nothing at it and nothing under it is not something to move.
            if (node.Entry is null && !node.HasChildren)
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_gone");
            }

            // A folder dropped on itself, or back where it was, is not a move: it would name its own parent.
            if (
                string.Equals(prefix, carry.Path, StringComparison.Ordinal)
                || Inside(prefix, carry.Path)
            )
            {
                return RolaI18N.Get("rorolala_vcs.place.refused_same", carry.Path);
            }

            var above = Above(carry.Path);
            var root = above.Length == 0 ? string.Empty : above + "/";
            var to = Join(prefix, carry.Path[root.Length..]);

            if (string.Equals(to, carry.Path, StringComparison.Ordinal))
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

        // A folder this side made is moved by rewriting its path, and what is under it comes with it: nothing is run,
        // because there is nothing of the work to run it against.
        if (carrying is [{ Entry: null } draft] && _drafts.Contains(draft.Path))
        {
            var to = Join(prefix, Name(draft.Path));

            if (
                !string.Equals(Above(draft.Path), prefix, StringComparison.Ordinal)
                && !Occupied(source, to)
            )
            {
                Rebase(draft.Path, to);
                _host.Files.Redraw();
            }

            return;
        }

        // A folder is moved as the tree it is: what stands at its path and under it keeps the paths it has below the
        // folder, which is what a folder cut and pasted in the file browser comes to. One run per entry, because that is
        // what the command line takes, and what landed is checked against the copy afterwards.
        var moves = Moving(tree, carrying, prefix)
            .Where(move =>
                string.Equals(
                    carrying[0].Source.Key,
                    source.Key,
                    StringComparison.Ordinal
                )
                && tree.Find(move.To) is null
            )
            .ToList();

        if (moves.Count > 0)
        {
            Move(source, moves);
        }
    }

    /// <summary>What moving these carries to a prefix comes to, one entry at a time.</summary>
    /// <remarks>
    /// A folder is not an entry, so a move of one is a move of everything at its path and under it, each keeping the
    /// path it has below the folder: a folder and an entry of the same path are both moved, which is what makes a
    /// folder that names an entry movable at all. A landing that is where the entry already is is not a move.
    /// </remarks>
    /// <param name="tree">The copy the drag is of.</param>
    /// <param name="carrying">What the drag carried.</param>
    /// <param name="prefix">The prefix they are dropped on.</param>
    /// <returns>The moves, each entry once and in the order the carries were made.</returns>
    internal static List<(string From, string To, string? Holder)> Moving(
        UpstreamEntriesTree tree,
        IReadOnlyList<UpstreamCarry> carrying,
        string prefix
    )
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(carrying);

        List<(string From, string To, string? Holder)> moves = [];
        var seen = new HashSet<string>(StringComparer.Ordinal);

        foreach (var carry in carrying)
        {
            if (tree.Find(carry.Path) is not { } node)
            {
                continue;
            }

            // The path a moved entry has is the one it has below the folder, under the prefix it was dropped on.
            var above = Above(carry.Path);
            var root = above.Length == 0 ? string.Empty : above + "/";

            foreach (var path in node.HasChildren ? Every(tree, carry.Path) : [carry.Path])
            {
                var to = Join(prefix, path[root.Length..]);

                if (!seen.Add(path) || string.Equals(to, path, StringComparison.Ordinal))
                {
                    continue;
                }

                moves.Add((path, to, tree.Find(path)?.Entry?.Owner));
            }
        }

        return moves;
    }

    /// <summary>Every entry at a path and under it, in the order a listing reads them.</summary>
    /// <param name="tree">The copy.</param>
    /// <param name="path">The folder.</param>
    /// <returns>The paths, which is the folder's own only when an entry stands there.</returns>
    private static IEnumerable<string> Every(UpstreamEntriesTree tree, string path)
    {
        if (tree.Find(path)?.Entry is not null)
        {
            yield return path;
        }

        foreach (var child in tree.At(path))
        {
            foreach (var deeper in Every(tree, child.Path))
            {
                yield return deeper;
            }
        }
    }

    /// <summary>Whether every entry at a path and under it is held by one account, which is what makes it that account's.</summary>
    /// <remarks>
    /// A path is only the entries that stand at it and under it, so a folder is this side's exactly when all of them are: one
    /// held by another account is a folder that account would have to move as well, and the Vault refuses the whole of it.
    /// An entry nobody holds has no owner to match, and what nobody holds is not this side's to name.
    /// </remarks>
    /// <param name="tree">The copy.</param>
    /// <param name="carry">The row being judged.</param>
    /// <param name="account">The account the work acts as, empty when it is not known.</param>
    /// <returns>Whether it is held by that account.</returns>
    internal static bool Held(UpstreamEntriesTree tree, UpstreamCarry carry, string account)
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(carry);
        ArgumentNullException.ThrowIfNull(account);

        if (account.Length == 0)
        {
            return false;
        }

        var owners = Every(tree, carry.Path)
            .Select(path => tree.Find(path)?.Entry?.Owner ?? string.Empty)
            .ToList();

        return owners.Count > 0
            && owners.TrueForAll(owner => string.Equals(owner, account, StringComparison.Ordinal));
    }

    /// <summary>What renaming a path comes to, one entry at a time.</summary>
    /// <remarks>
    /// A rename is a move within the path's own folder: the entries under a folder keep what they have below it, because an
    /// entry is named by its whole path and one left at the old prefix would be a child of nothing.
    /// </remarks>
    /// <param name="tree">The copy.</param>
    /// <param name="carry">What is being renamed.</param>
    /// <param name="to">The path it is to have.</param>
    /// <returns>The moves, each entry once, in the order a listing reads them.</returns>
    internal static List<(string From, string To, string? Holder)> Renaming(
        UpstreamEntriesTree tree,
        UpstreamCarry carry,
        string to
    )
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(carry);

        var root = carry.Path + "/";
        List<(string From, string To, string? Holder)> moves = [];

        foreach (var path in Every(tree, carry.Path))
        {
            var landed = path.Length == carry.Path.Length ? to : Join(to, path[root.Length..]);

            if (string.Equals(landed, path, StringComparison.Ordinal))
            {
                continue;
            }

            moves.Add((path, landed, tree.Find(path)?.Entry?.Owner));
        }

        return moves;
    }

    /// <summary>Whether a path is inside another, which is what a folder dropped in itself would be.</summary>
    /// <param name="path">The path.</param>
    /// <param name="of">The folder it may be inside.</param>
    /// <returns>Whether it is.</returns>
    private static bool Inside(string path, string of) =>
        path.Length > of.Length
        && path.StartsWith(of, StringComparison.Ordinal)
        && path[of.Length] == '/';

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
            && Tree(source) is { } tree
            && UpstreamBringing.Entries(tree, carrying).Count > 0;
    }

    /// <summary>Takes the drag: what it carried is planned into a checkin, which is what the user is asked about.</summary>
    /// <remarks>
    /// What is planned is every entry the drag comes to — a folder's rows included — and the planning is what leaves the
    /// ones that cannot come in for a question rather than for a run that refuses the lot.
    /// </remarks>
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
            source.Vault is not { Length: > 0 }
            || !carrying.All(carry => string.Equals(carry.Source.Key, source.Key, StringComparison.Ordinal))
            || Tree(source) is not { } tree
            || UpstreamBringing.Entries(tree, carrying).Count == 0
            || !Holds(directory)
        )
        {
            return false;
        }

        _checkin.Start(source, carrying, Path.GetFullPath(directory));

        return true;
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

        // What may be named here is what the account in use holds, and which one that is is a run: it is asked for on the way
        // to the rows, so that it is here by the time a name is asked to be edited rather than when the press happens.
        _ = Account();

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
        List<LogicalEntry> rows = [];

        if (prefix.Length == 0)
        {
            rows.Add(Up(Of(source.Domain)));
        }

        rows.AddRange(UpstreamSpots.Of(tree, prefix).Select(spot => Spot(source, spot)));
        rows.AddRange(Drafts(source, prefix, tree));

        // Folders before things, each by name, which is the order a level of the copy reads in: a folder this side made is
        // a folder like any other, and one drawn after the files would be one a reader has to find.
        return
        [
            .. rows.OrderBy(row => row.Opens is null ? 1 : 0)
                .ThenBy(row => row.Name, StringComparer.Ordinal),
        ];
    }

    /// <summary>The empty folders this place holds at one level, which nothing read from the Vault has.</summary>
    /// <remarks>
    /// A folder this side made is let go of as soon as the copy names it: a checkin or a move into its path is the work
    /// arriving, and what was a suggestion is then a folder of the Vault's own. It comes back if the work leaves again,
    /// because nothing was ever written down about it.
    /// </remarks>
    /// <param name="source">The Layout the level is of.</param>
    /// <param name="prefix">The prefix the level is at.</param>
    /// <param name="tree">The copy, which says whether the path is the work's own.</param>
    /// <returns>The rows, one per folder, in name order.</returns>
    private IEnumerable<LogicalEntry> Drafts(
        UpstreamSource source,
        string prefix,
        UpstreamEntriesTree tree
    )
    {
        if (!string.Equals(source.Domain, Vaults, StringComparison.Ordinal))
        {
            yield break;
        }

        foreach (var path in _drafts.OrderBy(path => path, StringComparer.Ordinal))
        {
            if (
                !string.Equals(Above(path), prefix, StringComparison.Ordinal)
                || Covered(tree, path)
            )
            {
                continue;
            }

            yield return new LogicalEntry(
                Name(path),
                Draggable: true,
                Opens: Of(source.Domain, source.Name, path),
                Menu: DraftMenu(source, path),
                Data: new UpstreamCarry(source, path),
                Picture: size => EntryIcons.For(path, EntryKind.Directory, size)
            );
        }
    }

    /// <summary>Whether the work already names a path, which is what a folder this side made gives way to.</summary>
    /// <param name="tree">The copy.</param>
    /// <param name="path">The path.</param>
    /// <returns>Whether something is there.</returns>
    private static bool Covered(UpstreamEntriesTree tree, string path) =>
        tree.Find(path) is { Entry: not null } or { HasChildren: true };

    /// <summary>What a folder this side made offers, which is what may be done to it.</summary>
    /// <param name="source">The Layout it is of.</param>
    /// <param name="path">The folder's path.</param>
    /// <returns>The items.</returns>
    private IReadOnlyList<ContextMenuItem> DraftMenu(UpstreamSource source, string path) =>
    [
        new ContextMenuItem(
            "rorolala_vcs.place.draft_remove",
            10,
            _ => Remove(Of(source.Domain, source.Name, path), new UpstreamCarry(source, path)),
            Icons.Error
        ),
    ];

    /// <summary>Makes a folder, naming it so that no name is taken and none is empty.</summary>
    /// <param name="source">The Layout it is made in.</param>
    /// <param name="prefix">The prefix it is made under.</param>
    /// <returns>The name it is given.</returns>
    private string Free(UpstreamSource source, string prefix)
    {
        var stem = RolaI18N.Get("rorolala_vcs.place.new_folder_name");
        var name = stem;

        for (var at = 2; Occupied(source, Join(prefix, name)); at++)
        {
            name = stem + " " + at;
        }

        return name;
    }

    /// <summary>Whether a path is already something here, either the work's or this side's.</summary>
    /// <param name="source">The Layout the path is in.</param>
    /// <param name="path">The path.</param>
    /// <returns>Whether it is taken.</returns>
    private bool Occupied(UpstreamSource source, string path) =>
        _drafts.Contains(path) || Tree(source)?.Find(path) is not null;

    /// <summary>Moves a folder this side made, and everything under it, to another path.</summary>
    /// <remarks>
    /// What a folder holds is the paths that begin with its own, so moving the folder is rewriting those prefixes: a child
    /// left at the old prefix would be a child of nothing.
    /// </remarks>
    /// <param name="from">The path it is at.</param>
    /// <param name="to">The path it is to be at.</param>
    private void Rebase(string from, string to)
    {
        var moving = _drafts
            .Where(path =>
                string.Equals(path, from, StringComparison.Ordinal)
                || path.StartsWith(from + "/", StringComparison.Ordinal)
            )
            .ToArray();

        foreach (var path in moving)
        {
            _ = _drafts.Remove(path);
        }

        foreach (var path in moving)
        {
            _ = _drafts.Add(to + path[from.Length..]);
        }
    }

    /// <summary>The name a path's last component is, which is what a folder row is called.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The name.</returns>
    private static string Name(string path) => path[(path.LastIndexOf('/') + 1)..];

    /// <summary>The path a path is under, which is empty at the top of a Layout.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The path above it.</returns>
    private static string Above(string path) =>
        path.LastIndexOf('/') is var at && at >= 0 ? path[..at] : string.Empty;

    /// <summary>A path under a prefix, which is the path itself when the prefix is the top of a Layout.</summary>
    /// <param name="prefix">The prefix.</param>
    /// <param name="name">The name under it.</param>
    /// <returns>The path.</returns>
    private static string Join(string prefix, string name) =>
        prefix.Length == 0 ? name : prefix + "/" + name;

    /// <summary>Puts a notification in the host's stack, which is what a folder with nothing in it is said with.</summary>
    /// <param name="level">How serious it is.</param>
    /// <param name="title">What it is called.</param>
    /// <param name="said">What it says.</param>
    private void Notify(NotificationLevel level, string title, string said) =>
        _host.Dialogs.Notify(
            new Notification(
                level,
                title,
                () =>
                    new Avalonia.Controls.TextBlock
                    {
                        Text = said,
                        TextWrapping = Avalonia.Media.TextWrapping.Wrap,
                        MaxWidth = 420,
                    }
            )
        );

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
            Draggable: true,
            Opens: spot.Folder ? Of(source.Domain, source.Name, spot.Path) : null,
            Menu: Menu(source, spot),
            Data: new UpstreamCarry(source, spot.Path, spot.Entry),
            Picture: spot.Folder
                ? size => EntryIcons.For(spot.Path, EntryKind.Directory, size)
                : size => EntryIcons.For(spot.Path, EntryKind.File, size)
        );
    }

    /// <summary>What a row offers, which is what may be done with the entry it is.</summary>
    /// <remarks>
    /// A folder is a row with something to do even when no entry stands at its path: what is under it is checked in at
    /// once, which is the same plan a drag of the folder makes. A folder of another Vault's copy is offered nothing,
    /// since a checkin brings what the Layout being worked in tracks.
    /// </remarks>
    /// <param name="source">The Layout it is of.</param>
    /// <param name="spot">The row.</param>
    /// <returns>The items, or nothing when there is nothing to do.</returns>
    private IReadOnlyList<ContextMenuItem>? Menu(UpstreamSource source, UpstreamSpot spot)
    {
        var tracked = source.Vault is { Length: > 0 } vault
            && string.Equals(vault, Tracked(), StringComparison.Ordinal);

        if (spot.Entry is not { } entry)
        {
            // A folder with nothing under it is nothing to check in, and nothing else about it is worth a menu.
            if (!spot.Folder)
            {
                return null;
            }

            return
            [
                // What a folder of a Vault's copy can always have is a folder of this side's in it, which is the one
                // thing that can be arranged before the work is.
                .. (source.Vault is { Length: > 0 }
                    ? new ContextMenuItem[]
                    {
                        new(
                            "rorolala_vcs.place.new_folder",
                            5,
                            _ => NewFolder(Of(source.Domain, source.Name, spot.Path)),
                            Icons.Add
                        ),
                    }
                    : []),
                .. (tracked
                    ? new ContextMenuItem[]
                    {
                        new(
                            "rorolala_vcs.place.checkin",
                            10,
                            _ => _checkin.Start(
                                source,
                                [new UpstreamCarry(source, spot.Path)],
                                destination: null
                            ),
                            Icons.Download
                        ),
                    }
                    : []),
            ];
        }

        var held = Tree(source)?.IsHeld(entry) == true;

        return
        [
            // A folder of this side's can be made in a row that names a folder, whether or not an entry stands there.
            .. (spot.Folder && source.Vault is { Length: > 0 }
                ? new ContextMenuItem[]
                {
                    new(
                        "rorolala_vcs.place.new_folder",
                        5,
                        _ => NewFolder(Of(source.Domain, source.Name, spot.Path)),
                        Icons.Add
                    ),
                }
                : []),
            // A checkin brings what the Layout being worked in *tracks*, so an entry of another Vault's copy — or of a
            // Layout of the Workspace's own — is not something a checkin can name: what there is to do with it is read it,
            // and move it where it belongs.
            //
            // An entry this side already holds is offered the same item: the checkin is planned against what is here, and
            // one held under another name is exactly what the plan's move answer is for.
            .. (tracked && source.Vault is { Length: > 0 }
                ? new ContextMenuItem[]
                {
                    new(
                        "rorolala_vcs.place.checkin",
                        10,
                        _ => _checkin.Start(source, entry, spot.Path),
                        Icons.Download
                    ),
                }
                : []),
            // What a held entry is owed is where this work names it, since that is the one thing about it a checkin
            // cannot say; what it is not held at is worth copying out instead.
            held
                ? new ContextMenuItem(
                    "rorolala_vcs.place.held",
                    20,
                    _ => Report(
                        "rorolala_vcs.place.held_title",
                        RolaI18N.Get(
                            "rorolala_vcs.place.held_at",
                            Tree(source)?.LocalPath(entry.Uuid) is { Length: > 0 } at ? at : "?"
                        )
                    ),
                    Icons.Lock
                )
                : new ContextMenuItem(
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
    /// The account the work acts as, asking for it in the background when it is not here yet.
    /// </summary>
    /// <remarks>
    /// Which account is in use is the command line's answer, so reading it is a run, and a provider is asked on the window's
    /// thread: an answer that took a run to make would take the window with it. The read is asked for once and kept until the
    /// files are said to have changed; until it lands, nothing is known to be held by anyone, which refuses a rename rather
    /// than waiting for it — and the press after it is answered.
    /// </remarks>
    /// <returns>The name, or an empty string while it is being read or when nothing says.</returns>
    private string Account()
    {
        var directory = BaseDirectory.Current ?? string.Empty;

        if (directory.Length == 0 || _accountTried || _accountAsking)
        {
            return _account;
        }

        _accountAsking = true;

        var version = _version;

        _ = Task.Run(() =>
        {
            var (done, accounts, _) = AccountActions.List(directory);
            var current = done
                ? accounts.FirstOrDefault(account => account.Current)?.Name ?? string.Empty
                : string.Empty;

            Dispatcher.UIThread.Post(() => LandedAccount(version, current));
        });

        return _account;
    }

    /// <summary>Keeps the account a read that landed came to.</summary>
    /// <remarks>
    /// Nothing is drawn again for it, because nothing drawn reads it: a name box is put up by a press, and the press reads
    /// the place directly. An answer of a reading that is no longer the one being read is dropped, and the mark that it was
    /// asked for goes with it, so that the next draw asks again.
    /// </remarks>
    /// <param name="version">Which reading of the work it was asked for in.</param>
    /// <param name="current">The account the work acts as, or an empty string when nothing says.</param>
    private void LandedAccount(int version, string current)
    {
        _accountAsking = false;

        if (version != _version)
        {
            _accountTried = false;

            return;
        }

        _accountTried = true;
        _account = current;
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
        _account = string.Empty;
        _accountAsking = false;
        _accountTried = false;
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

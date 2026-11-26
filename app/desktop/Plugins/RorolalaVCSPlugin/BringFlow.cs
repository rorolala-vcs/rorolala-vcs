using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>What is in the way of one entry of a bring.</summary>
internal enum BringBlocker
{
    /// <summary>Nothing: the entry lands where the Vault names it, or under the directory it was dropped on.</summary>
    None,

    /// <summary>The Layout being worked in already holds it at the path it would land at, so there is nothing to do.</summary>
    Here,

    /// <summary>
    /// The Layout being worked in holds it at another path, which can be moved to the landing one.
    /// </summary>
    /// <remarks>
    /// It is the one case a bring has a third way out of: what the Vault now names `A/b.fbx` is what this side brought
    /// in as `imported.fbx`, an earlier name of the same entry, so moving the local file and its record over is what
    /// letting the bring go ahead would have done, without fetching the content again.
    /// </remarks>
    Elsewhere,

    /// <summary>Something else is already at the landing path, another entry's file, or a path the Layout names.</summary>
    Taken,
}

/// <summary>One entry of a bring: where it lands, and what is in the way there.</summary>
/// <param name="Entry">What the Vault's copy holds.</param>
/// <param name="Landing">The path it would be named at here, relative to the Workspace's root.</param>
/// <param name="Local">The path this side names it at, when it holds it; empty for one it holds at no path.</param>
/// <param name="Blocker">What is in the way.</param>
/// <param name="Kind">Whether it lands as a file or a directory, as the Vault's tree says it.</param>
/// <param name="Carried">The path the drag carried that brought this entry, which is what a drop brings in whole.</param>
internal sealed record BringStep(
    LayoutEntry Entry,
    string Landing,
    string? Local,
    BringBlocker Blocker,
    EntryKind Kind,
    string Carried
);

/// <summary>One thing a bring is going to put on disk: where it lands, and what kind of thing it is.</summary>
/// <remarks>
/// It is the carry rather than every entry under it: a folder is brought in as the folder, and a reader
/// watching the directory it is dropped on sees the one row that is waiting rather than a row per file that
/// is not there yet.
/// </remarks>
/// <param name="Path">The path it lands at, relative to the Workspace's root.</param>
/// <param name="Kind">Whether it lands as a file or a directory.</param>
/// <param name="Carried">The path the drag carried that brought it, which is what a drop brings in whole.</param>
internal sealed record BringPlace(string Path, EntryKind Kind, string Carried);

/// <summary>
/// A bring worked out before anything is done: every entry it would bring, and what is in the way of each.
/// </summary>
/// <remarks>
/// The command line refuses a run whole for the first reference it will not take, so a batch that half fits has to be
/// known about before it is run: what this answers is which entries can come in as they stand, which are already here,
/// which are here under another name, and which cannot land at all.
/// </remarks>
/// <param name="Vault">The Vault the entries are of, by the name the Workspace bound it under.</param>
/// <param name="Root">The Workspace's root, which Layout paths are relative to.</param>
/// <param name="Destination">The directory they are to land under, or nothing for the paths the Vault names.</param>
/// <param name="Steps">What the bring comes to, in the order the entries were carried.</param>
/// <param name="Places">What the run will put on disk, one for each thing the drag carried.</param>
/// <param name="Carried">What the drag carried, in the order it carried it.</param>
internal sealed record BringPlan(
    string Vault,
    string Root,
    string? Destination,
    IReadOnlyList<BringStep> Steps,
    IReadOnlyList<BringPlace> Places,
    IReadOnlyList<UpstreamCarry> Carried
)
{
    /// <summary>The things this bring will put on disk, in the order the drag carried them.</summary>
    /// <remarks>
    /// A drop names what it carried, so a drop shows one row for the folder or file that was let go, and only
    /// where that whole carry comes in: one entry of it left out is the whole item left out. A run that names
    /// the entries themselves, a button or a menu item, shows one row for each entry that will actually land,
    /// since that is what the run will write.
    /// </remarks>
    public IReadOnlyList<BringPlace> Pending =>
        Destination is { Length: > 0 }
            ? [.. Places.Where(place => Whole(place.Carried))]
            : [.. Clean.Select(step => new BringPlace(step.Landing, step.Kind, step.Carried))];

    /// <summary>Whether every entry a drag carried comes in as it stands.</summary>
    /// <param name="carried">The path the drag carried.</param>
    /// <returns>Whether the carry brings at least one entry and nothing of it is in the way.</returns>
    private bool Whole(string carried)
    {
        var steps = Steps
            .Where(step => string.Equals(step.Carried, carried, StringComparison.Ordinal))
            .ToArray();

        return steps.Length > 0 && steps.All(step => step.Blocker is BringBlocker.None);
    }

    /// <summary>The entries that can be brought in as they stand.</summary>
    public IReadOnlyList<BringStep> Clean =>
        [.. Steps.Where(step => step.Blocker is BringBlocker.None)];

    /// <summary>The entries this side holds at another path, which can be moved to where they would land.</summary>
    public IReadOnlyList<BringStep> Elsewhere =>
        [.. Steps.Where(step => step.Blocker is BringBlocker.Elsewhere)];

    /// <summary>The entries that cannot land where they would, whatever is done about it.</summary>
    public IReadOnlyList<BringStep> Taken =>
        [.. Steps.Where(step => step.Blocker is BringBlocker.Taken)];

    /// <summary>How many entries this side already holds where they would land.</summary>
    public int Here => Steps.Count(step => step.Blocker is BringBlocker.Here);

    /// <summary>How many entries a run would leave out: everything that is not clean.</summary>
    public int Left => Steps.Count - Clean.Count;

    /// <summary>Whether there is anything the user is owed a question about.</summary>
    public bool Blocks => Elsewhere.Count > 0 || Taken.Count > 0;

    /// <summary>The references a run of the clean entries is given.</summary>
    /// <remarks>
    /// A drag or a paste lands where it was put, so what it names is what it carried rather than every entry under it: a
    /// folder is one reference, and the run keeps the tree it is, the command line rebases each on its own name. The
    /// button names the entries themselves, since each is to land at the path the Vault gives it.
    /// </remarks>
    /// <returns>What a bring is to name, in the order the entries were carried.</returns>
    public IReadOnlyList<string> References()
    {
        if (Destination is not { Length: > 0 })
        {
            return [.. Clean.Select(step => BringCommand.Reference(step.Entry))];
        }

        // What is carried comes in whole or not at all: one entry that cannot land refuses the run that names what it
        // stands under, so an item with any such entry is left out rather than half brought in.
        List<string> references = [];

        foreach (var carry in Carried)
        {
            if (Steps.All(step =>
                !string.Equals(step.Carried, carry.Path, StringComparison.Ordinal)
                || step.Blocker is BringBlocker.None
            ))
            {
                references.Add(BringCommand.Reference(carry));
            }
        }

        return references;
    }
}

/// <summary>Working out what a bring of a drag would come to, without running anything.</summary>
internal static class BringPlanning
{
    /// <summary>The mark the Vault puts in front of a path it holds no place for yet.</summary>
    internal const string UnplacedMark = "@/";

    /// <summary>Works out a bring of what a drag carried, landing it under a directory or at the Vault's paths.</summary>
    /// <remarks>
    /// What this side holds is read from the entries as a run lists them rather than from the reading the place drew from:
    /// the two are the same Layout, and only one of them is certain to be the Layout as it is now.
    /// </remarks>
    /// <param name="tree">The tree the Vault's copy was read to.</param>
    /// <param name="local">What the Layout being worked in holds, which is what says a landing path is taken.</param>
    /// <param name="root">The Workspace's root, which Layout paths are relative to.</param>
    /// <param name="destination">The directory they are to land under, or nothing for the paths the Vault names.</param>
    /// <param name="vault">The Vault the entries are of.</param>
    /// <param name="carrying">What the drag carried.</param>
    /// <returns>The plan.</returns>
    public static BringPlan Of(
        UpstreamEntriesTree tree,
        IReadOnlyList<LayoutEntry> local,
        string root,
        string? destination,
        string vault,
        IReadOnlyList<UpstreamCarry> carrying
    )
    {
        ArgumentNullException.ThrowIfNull(tree);
        ArgumentNullException.ThrowIfNull(local);
        ArgumentNullException.ThrowIfNull(root);
        ArgumentNullException.ThrowIfNull(vault);
        ArgumentNullException.ThrowIfNull(carrying);

        var named = new HashSet<string>(StringComparer.Ordinal);
        var held = new Dictionary<string, string>(StringComparer.Ordinal);

        foreach (var entry in local)
        {
            if (entry.Path.Length > 0)
            {
                named.Add(entry.Path);
            }

            held[entry.Uuid] = entry.Path;
        }

        var under = destination is { Length: > 0 } ? Relative(root, destination) : null;

        List<BringStep> steps = [];
        List<BringPlace> places = [];

        foreach (var carry in carrying)
        {
            // What a drag carried is the thing that was let go on the directory, so a drop lands it where it was put
            // rather than where the Vault has it: each carried path keeps only its own name under the destination, and
            // a folder keeps the tree it is. The button writes no destination, and then the Vault's path is the answer.
            var carried = Unplaced(carry.Path);
            var name = carried[(carried.LastIndexOf('/') + 1)..];

            // What the carry itself lands as, which is the row a reader sees waiting before anything inside it.
            // A folder is one the Vault's tree holds paths under; a tree of the work is one entry with no outer
            // path under it, and is read as a file here for the same reason the place draws it as one.
            var landed = under switch
            {
                null => carried,
                { Length: 0 } => name,
                _ => under + "/" + name,
            };

            places.Add(
                new BringPlace(
                    landed,
                    tree.Find(carry.Path) is { HasChildren: true }
                        ? EntryKind.Directory
                        : EntryKind.File,
                    carry.Path
                )
            );

            foreach (var entry in UpstreamBringing.Entries(tree, [carry]))
            {
                // The Vault's unplaced mark is not part of the path here: `@/new/a.fbx` is an entry the Vault holds no
                // path for yet, and a directory called `@` would be a decision this side made on the Vault's behalf.
                var placeable = Unplaced(entry.Path);

                var landing = under switch
                {
                    null => placeable,
                    // What the entry keeps below the carried path: itself, and what stands under it.
                    { Length: 0 } => Joined(string.Empty, name, placeable, carried),
                    _ => Joined(under, name, placeable, carried),
                };

                var here = held.TryGetValue(entry.Uuid, out var at) ? at : null;

                steps.Add(
                    new BringStep(
                        entry,
                        landing,
                        here,
                        Blocker(root, landing, here, named),
                        tree.Find(entry.Path) is { HasChildren: true }
                            ? EntryKind.Directory
                            : EntryKind.File,
                        carry.Path
                    )
                );
            }
        }

        // A path that is a file and a directory at once cannot be written: an entry landing at `A` while another lands
        // under `A/` is a run that would fail half way, so both are known before anything is asked.
        var landings = steps.Select(step => step.Landing).ToList();

        steps =
        [
            .. steps.Select(step =>
                Collides(step.Landing, landings)
                    ? step with { Blocker = BringBlocker.Taken }
                    : step
            ),
        ];

        return new BringPlan(vault, root, destination, steps, places, carrying);
    }

    /// <summary>Where an entry a drag carried lands under a destination.</summary>
    /// <remarks>
    /// The carried path is what the entry hangs from, so the entry itself keeps the carried name and what stands under
    /// it keeps that tree: a folder brought in is the folder, remade where it was put, rather than its whole Vault path
    /// under the destination.
    /// </remarks>
    /// <param name="under">The directory it lands under, which is empty for the Workspace's root.</param>
    /// <param name="name">The name of what the drag carried.</param>
    /// <param name="placeable">The entry's own path, with the unplaced mark taken off.</param>
    /// <param name="carried">The carried path, with the unplaced mark taken off.</param>
    /// <returns>The path it lands at.</returns>
    private static string Joined(string under, string name, string placeable, string carried)
    {
        var rest = string.Equals(placeable, carried, StringComparison.Ordinal)
            ? name
            : placeable.StartsWith(carried + "/", StringComparison.Ordinal)
                ? name + placeable[carried.Length..]
                : placeable;

        return under.Length == 0 ? rest : under + "/" + rest;
    }

    /// <summary>A path as it is written here, with the Vault's unplaced mark taken off the front.</summary>
    /// <remarks>
    /// `@/new/a.fbx` is an entry the Vault holds no path for yet, and `@/removed/a.fbx` one taken out of the work: the
    /// mark is the Vault's bookkeeping, so what lands here is the name below it, `new/a.fbx`, and a directory called
    /// `@` is never made. A path that is nothing but the mark is left as it is, since there is no name to place.
    /// </remarks>
    /// <param name="path">The path the Vault's copy names the entry by.</param>
    /// <returns>The path it lands at here.</returns>
    private static string Unplaced(string path) =>
        path.StartsWith(UnplacedMark, StringComparison.Ordinal) && path.Length > UnplacedMark.Length
            ? path[UnplacedMark.Length..]
            : path;

    /// <summary>Whether something else lands at a path, or at one inside it.</summary>
    /// <param name="landing">The path this one would land at.</param>
    /// <param name="landings">Every path the run would land at.</param>
    /// <returns>Whether the path cannot be what it is as well as what the others make of it.</returns>
    private static bool Collides(string landing, IReadOnlyList<string> landings) =>
        landings.Any(other =>
            !string.Equals(other, landing, StringComparison.Ordinal)
            && (other.StartsWith(landing + "/", StringComparison.Ordinal)
                || landing.StartsWith(other + "/", StringComparison.Ordinal))
        );

    /// <summary>What an entry's landing path comes to here, relative to the Workspace's root.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <param name="directory">The directory a drag was dropped on.</param>
    /// <returns>The path, with the separators a Layout path is written with, or empty for the root itself.</returns>
    internal static string Relative(string root, string directory)
    {
        ArgumentNullException.ThrowIfNull(root);
        ArgumentNullException.ThrowIfNull(directory);

        var relative = Path.GetRelativePath(root, directory);

        return relative is "." or ""
            ? string.Empty
            : relative.Replace('\\', '/').Trim('/');
    }

    /// <summary>What is in the way of one entry, given what this side holds and where it is named.</summary>
    /// <param name="root">The Workspace's root, which a file on disk is found under.</param>
    /// <param name="landing">The path it would be named at.</param>
    /// <param name="held">The path this side names it at, or nothing when it does not hold it.</param>
    /// <param name="named">Every path the Layout being worked in names.</param>
    /// <returns>What is in the way.</returns>
    private static BringBlocker Blocker(
        string root,
        string landing,
        string? held,
        HashSet<string> named
    )
    {
        if (held is { Length: > 0 })
        {
            return string.Equals(held, landing, StringComparison.Ordinal)
                ? BringBlocker.Here
                : Occupied(root, landing, named)
                    ? BringBlocker.Taken
                    : BringBlocker.Elsewhere;
        }

        // An entry this side holds at no path at all is one nothing can be moved from, so it is no different from a
        // landing path that is taken.
        if (held is not null)
        {
            return BringBlocker.Taken;
        }

        return Occupied(root, landing, named) ? BringBlocker.Taken : BringBlocker.None;
    }

    /// <summary>Whether a path already names something here, on disk or in the Layout being worked in.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <param name="path">The Layout path to ask about.</param>
    /// <param name="named">Every path the Layout being worked in names.</param>
    /// <returns>Whether it is taken.</returns>
    private static bool Occupied(string root, string path, HashSet<string> named)
    {
        var on = Path.Combine(root, path.Replace('/', Path.DirectorySeparatorChar));

        return named.Contains(path) || File.Exists(on) || Directory.Exists(on);
    }
}

/// <summary>
/// One bring, from what a drag carried to what the user was told about it.
/// </summary>
/// <remarks>
/// The writer brings in what it is given, or nothing: the first reference it refuses is the run refused, so
/// "bring in what fits and leave the rest" is a plan made here and then run, see <see cref="BringPlanning"/>. What
/// the user is asked when the plan does not fit is the one question this class puts: move the files that are already
/// here under another name, leave them out, or let the bring go.
/// <para>
/// Nothing here is done on the window's thread: the plan is two reads of the work, the bring and the local move are
/// two writes, and the question is the only thing the window answers. The move is the writer's, the same one the bring
/// uses, see <see cref="LayoutPathMoveCommand.Move"/>.
/// </para>
/// </remarks>
internal sealed class BringFlow
{
    /// <summary>The host, for the question and for what a failure is shown through.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a base that is not a Workspace is judged.</summary>
    private readonly ILog _log;

    /// <summary>What every command this plugin runs reports itself to.</summary>
    private readonly IProgressHub _hub;

    /// <summary>What says whether the Vault could be reached, which a bring of what is here goes on without.</summary>
    private readonly VaultFetch _fetch;

    /// <summary>What the place read of a Layout, which a bring's entries are read from.</summary>
    private readonly Func<UpstreamSource, UpstreamEntriesTree?> _tree;

    /// <summary>Makes the flow over the readings it plans from.</summary>
    /// <param name="host">The host, for the question and for what a failure is shown through.</param>
    /// <param name="log">Where a base that is not a Workspace is judged.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="fetch">What says whether the Vault could be reached.</param>
    /// <param name="tree">What the place read of a Layout.</param>
    public BringFlow(
        IPluginHost host,
        ILog log,
        IProgressHub hub,
        VaultFetch fetch,
        Func<UpstreamSource, UpstreamEntriesTree?> tree
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(fetch);
        ArgumentNullException.ThrowIfNull(tree);

        _host = host;
        _log = log;
        _hub = hub;
        _fetch = fetch;
        _tree = tree;
    }

    /// <summary>
    /// Works out a bring of what a drag carried and asks about it where it does not fit, then does what was chosen.
    /// </summary>
    /// <remarks>
    /// A drag of a folder carries everything under it, and a drag of several rows carries the lot: what is planned is every
    /// entry they come to, which is what makes a folder a thing that can be brought in at all.
    /// </remarks>
    /// <param name="source">The Layout the entries are of.</param>
    /// <param name="carrying">What the drag carried, in the order it carried it.</param>
    /// <param name="destination">The directory they are to land under, or nothing for the paths the Vault names.</param>
    public void Start(
        UpstreamSource source,
        IReadOnlyList<UpstreamCarry> carrying,
        string? destination
    )
    {
        ArgumentNullException.ThrowIfNull(source);
        ArgumentNullException.ThrowIfNull(carrying);

        if (Program() is null || BaseDirectory.Current is not { Length: > 0 } directory)
        {
            RefuseNoProgram();

            return;
        }

        // A destination is a path the run is made in and resolved against, so it is made absolute once, here: the plan
        // reads it against the Workspace's root, and the run is handed the same words.
        var to = destination is { Length: > 0 } at ? Path.GetFullPath(at) : null;

        _ = Task.Run(() => Plan(source, carrying, to, directory));
    }

    /// <summary>Brings in the entries of what the Vault holds, as a right-click asks for one.</summary>
    /// <param name="source">The Layout the entry is of.</param>
    /// <param name="entry">The entry a row stands for.</param>
    /// <param name="path">The path that row is at.</param>
    public void Start(UpstreamSource source, LayoutEntry entry, string path)
    {
        ArgumentNullException.ThrowIfNull(entry);

        Start(source, [new UpstreamCarry(source, path, entry)], destination: null);
    }

    /// <summary>Works out the plan off the window's thread, and asks about it on the window's.</summary>
    /// <param name="source">The Layout the entries are of.</param>
    /// <param name="carrying">What the drag carried.</param>
    /// <param name="destination">The directory they are to land under, or nothing.</param>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    private void Plan(
        UpstreamSource source,
        IReadOnlyList<UpstreamCarry> carrying,
        string? destination,
        string directory
    )
    {
        if (_tree(source) is not { } tree)
        {
            Post(() => Refuse(
                "rorolala_vcs.place.bring_failed",
                RolaI18N.Get("rorolala_vcs.place.refused_reading")
            ));

            return;
        }

        var local = LayoutEntriesCommand.Local(directory);

        if (!local.Done)
        {
            Post(() => Refuse("rorolala_vcs.place.bring_failed", local.Said));

            return;
        }

        if (Root(directory) is not { } root)
        {
            Post(() => Refuse(
                "rorolala_vcs.place.bring_failed",
                RolaI18N.Get("rorolala_vcs.place.not_workspace")
            ));

            return;
        }

        var plan = BringPlanning.Of(
            tree,
            local.Entries,
            root,
            destination,
            source.Vault,
            carrying
        );

        Post(() => Confirm(plan, directory));
    }

    /// <summary>
    /// Asks the user what to do about a plan that does not fit, or runs it as it stands.
    /// </summary>
    /// <remarks>
    /// The question offers moving the local files over only when moving them is what would settle every conflict: a
    /// landing path something else already occupies is not something moving a local file can fix, so a plan that has one
    /// is asked as "leave them out or let it go" rather than with an answer that would fail.
    /// </remarks>
    /// <param name="plan">What the bring comes to.</param>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    private void Confirm(BringPlan plan, string directory)
    {
        if (plan.Steps.Count == 0)
        {
            return;
        }

        // A plan with nothing in the way and nothing already here is run as it stands. One that has something here, where
        // the Vault names it, is asked about as well: nothing is in the way of those entries, but the run leaves them out,
        // and a run that half did something is not one to start without saying so.
        if (!plan.Blocks && plan.Here == 0)
        {
            Bring(plan, directory);

            return;
        }

        List<DialogChoice> choices = [];

        // Moving a local file over is offered only where moving one would settle every conflict: a landing path something
        // else already occupies is not something a move can fix, and an entry already where the Vault names it has nothing
        // left to move.
        if (plan.Elsewhere.Count > 0 && plan.Taken.Count == 0)
        {
            choices.Add(
                new DialogChoice(
                    RolaI18N.Get("rorolala_vcs.place.bring_move", plan.Elsewhere.Count),
                    () => Move(plan, directory)
                )
            );
        }

        choices.Add(
            new DialogChoice(
                RolaI18N.Get("rorolala_vcs.place.bring_partial", plan.Left),
                () =>
                {
                    Bring(plan, directory);
                    Notify(
                        NotificationLevel.Warning,
                        "rorolala_vcs.place.bring_abandoned_title",
                        RolaI18N.Get("rorolala_vcs.place.bring_abandoned", plan.Left)
                    );
                }
            )
        );

        // The answer that does nothing is last, and is what closing the window comes to: a bring left alone is not a
        // failure and is not worth a word.
        choices.Add(
            new DialogChoice(
                RolaI18N.Get("rorolala_vcs.place.bring_abandon"),
                () => { }
            )
        );

        _host.Dialogs.Choose(new Choice(
            RolaI18N.Get("rorolala_vcs.place.bring_question"),
            Question(plan),
            choices
        ));
    }

    /// <summary>What the question says about what does not fit.</summary>
    /// <param name="plan">What the bring comes to.</param>
    /// <returns>The lines, one per kind of thing in the way.</returns>
    private static string Question(BringPlan plan)
    {
        List<string> lines = [];

        if (plan.Elsewhere.Count > 0)
        {
            lines.Add(
                RolaI18N.Get("rorolala_vcs.place.bring_elsewhere", plan.Elsewhere.Count)
            );
        }

        if (plan.Taken.Count > 0)
        {
            lines.Add(RolaI18N.Get("rorolala_vcs.place.bring_taken", plan.Taken.Count));
        }

        if (plan.Here > 0)
        {
            lines.Add(RolaI18N.Get("rorolala_vcs.place.bring_here", plan.Here));
        }

        return string.Join((char)10, lines);
    }

    // WORKAROUND: files are moved here rather than through `rola fs-ops mv`, because a move has to be taken
    // back when the Layout record that names it does not follow. It can go when a move can be asked for and
    // undone in two calls.
    /// <summary>
    /// Moves the local copies of the entries that are here under another name to where the Vault names them.
    /// </summary>
    /// <remarks>
    /// The file is moved before the record that names it, and put back where it was if the record cannot follow: a path
    /// that moved to a file that is not there is worse than a file beside the name it already had. What actually moved is
    /// said, a run that moved files and said nothing is a run the user cannot tell from one that did not.
    /// <para>
    /// The record is the writer's: the same write the bring uses, made in the Layout being worked in. It needs nothing of
    /// the command line and nothing of the program the window was opened by.
    /// </para>
    /// </remarks>
    /// <param name="plan">What the bring comes to.</param>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    private void Move(BringPlan plan, string directory)
    {
        var run = _hub.Begin("rorolala_vcs.progress.path_move");

        _ = Task.Run(() =>
        {
            List<(string Path, string Said)> failures = [];
            var moved = 0;

            foreach (var step in plan.Elsewhere)
            {
                if (step.Local is not { Length: > 0 } from)
                {
                    continue;
                }

                var source = Path.Combine(plan.Root, from.Replace('/', Path.DirectorySeparatorChar));
                var target = Path.Combine(
                    plan.Root,
                    step.Landing.Replace('/', Path.DirectorySeparatorChar)
                );
                var file = File.Exists(source);

                try
                {
                    if (file)
                    {
                        if (Path.GetDirectoryName(target) is { Length: > 0 } parent)
                        {
                            Directory.CreateDirectory(parent);
                        }

                        File.Move(source, target, overwrite: false);
                        moved++;
                    }
                }
                catch (Exception error) when (error is not OutOfMemoryException)
                {
                    failures.Add((step.Landing, error.Message));

                    continue;
                }

                var (done, said) = LayoutPathMoveCommand.Move(
                    directory,
                    layout: null,
                    from,
                    step.Landing
                );

                if (done)
                {
                    continue;
                }

                // The record did not follow the file, so the file goes back: leaving it where the Layout does not name it
                    // would be a file the user has to find again.
                if (file)
                {
                    TryMoveBack(target, source);
                }

                failures.Add((step.Landing, said));
            }

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);

                if (moved > 0)
                {
                    Notify(
                        NotificationLevel.Message,
                        "rorolala_vcs.place.bring_moved_title",
                        RolaI18N.Get("rorolala_vcs.place.bring_moved", moved)
                    );
                }

                foreach (var (path, said) in failures)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.place.bring_failed", plan.Vault),
                        path.Length > 0 ? path + (char)10 + said : said
                    );
                }

                // Whatever could not be moved is still here, and is what the report above is about; an entry whose record
                // moved is one the work now names where the Vault does, so it is not left out of anything. The ones the
                // run leaves out are counted by the plan, so nothing is said twice.
                Bring(plan, directory);
            });
        });
    }

    /// <summary>Runs the bring of everything the plan can bring in.</summary>
    /// <remarks>
    /// The request is the plan's own: the references it worked out, the destination it was dropped on, and the
    /// tracked Vault, which is the only one a bring reads. The writer opens the Workspace itself, so this needs
    /// nothing of the command line and nothing of the program the window was opened by.
    /// </remarks>
    /// <param name="plan">What the bring comes to.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    private void Bring(BringPlan plan, string directory)
    {
        var references = plan.References();

        if (references.Count == 0)
        {
            // Nothing to hand the writer: what changed is already in the work, or was never going to be, and the
            // place is drawn from the files, so it is told they may have moved.
            _host.Files.Touch();

            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.bring");

        // What the run is going to make is shown as soon as it is known to be going ahead, one row for each
        // thing the drag carried: what is coming is seen before the first byte of it is here.
        var pending = plan.Pending;
        var placed = pending
            .Select(place =>
                Path.Combine(plan.Root, place.Path.Replace('/', Path.DirectorySeparatorChar))
            )
            .ToArray();

        for (var at = 0; at < placed.Length; at++)
        {
            VirtualEntries.Place(placed[at], pending[at].Kind, VirtualEntryState.Pending);
        }

        _host.Files.Redraw();

        _ = Task.Run(() =>
        {
            List<string> failed = [];
            string? said = null;

            try
            {
                using var workspace = WorkspaceScope.Open(directory);
                workspace.Read();

                var request = new BringRequest
                {
                    Workspace = directory,
                    Account = UserScope.CurrentAccount,
                    Target = WriteTarget.Tracked,
                    // What the Vault's copy and this side's store already hold is all a cached bring needs, so
                    // a run made while the Vault cannot be reached goes on offline rather than waiting on it.
                    Offline = _fetch.Offline,
                    Refs = references,
                    To = plan.Destination,
                    ByName = plan.Destination is { Length: > 0 },
                };

                using var native = Progress.New();

                // What the poll reads is said on the window's thread rather than the run's: the rows waiting for it are
                // bound to the catalogue's entries, and what a row is bound to moves on the drawing thread.
                using var watching = new Watching(native, run.Records, () =>
                    Dispatcher.UIThread.Post(() =>
                    {
                        Tick(placed, run.Records);
                        _hub.Touch();
                    })
                );

                failed = [.. new WorkspaceWriter(workspace).Bring(request, native).Failed];
            }
            catch (RolaWriteError error)
            {
                said = Told(error);
            }
            catch (RolaReadError error)
                when (error.Kind is RolaReadErrorKind.NoWorkspace or RolaReadErrorKind.Unavailable)
            {
                said = RolaI18N.Get("rorolala_vcs.place.not_workspace");
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                said = error.Message;
            }

            Dispatcher.UIThread.Post(() =>
            {
                // What was waiting is either here now or was not brought: the rows go either way, and the
                // touch below reads the files that arrived so each is drawn as itself.
                foreach (var path in placed)
                {
                    VirtualEntries.Withdraw(path);
                }

                _hub.End(run);

                if (said is { Length: > 0 } || failed.Count > 0)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.place.bring_failed", plan.Vault),
                        failed.Count > 0 ? string.Join((char)10, failed) : said!
                    );
                }

                // The run fetches the Vault's Layout before it resolves anything, so the copy this place was read from
                // may be newer whatever became of the work.
                _host.Files.Touch();
            });
        });
    }

    /// <summary>
    /// Says how far along the run has got to everything it put in the catalogue.
    /// </summary>
    /// <remarks>
    /// The proportion is the one the progress capsule draws, which is the phase the run is working on: the
    /// writer reports phases, not one file at a time, so every row waiting for the run fills with the same
    /// answer rather than each pretending to have a proportion of its own. It is said on the window's thread
    /// rather than the one the run is read on: the entries are bound to the rows drawn for them, so what moves
    /// them is the drawing thread's to move.
    /// </remarks>
    /// <param name="placed">What the run put in the catalogue.</param>
    /// <param name="records">What the run has said about itself.</param>
    private static void Tick(IReadOnlyList<string> placed, ProgressRecords records)
    {
        if (Fraction(records) is not { } fraction)
        {
            return;
        }

        foreach (var path in placed)
        {
            VirtualEntries.Advance(path, fraction);
        }
    }

    /// <summary>
    /// How far the phase the run is working on has got, or nothing while it has not said how long it is.
    /// </summary>
    /// <remarks>
    /// The same rule the progress capsule draws with: the phase is the last top-level task that is not
    /// finished, or the last one when they all are, and a phase that says no length has nothing to divide.
    /// </remarks>
    /// <param name="records">What the run has said about itself.</param>
    /// <returns>The share from nothing to all of it, or nothing.</returns>
    internal static float? Fraction(ProgressRecords records)
    {
        ArgumentNullException.ThrowIfNull(records);

        var tops = records.Tasks.Where(task => task.Parent is null).ToArray();

        if (tops.Length == 0)
        {
            return null;
        }

        var at = Array.FindLastIndex(
            tops,
            task => task.Total is not { } total || task.Done < total
        );

        if (at < 0)
        {
            at = tops.Length - 1;
        }

        var focus = tops[at];

        return focus.Total is { } length && length > 0
            ? (float)Math.Clamp(focus.Done / (double)length, 0, 1)
            : null;
    }

    /// <summary>Puts a notification in the host's stack.</summary>
    /// <param name="level">How serious it is.</param>
    /// <param name="title">The key of what it is called.</param>
    /// <param name="said">What it says.</param>
    private void Notify(NotificationLevel level, string title, string said) =>
        _host.Dialogs.Notify(
            new Notification(
                level,
                RolaI18N.Get(title),
                () =>
                    new Avalonia.Controls.TextBlock
                    {
                        Text = said,
                        TextWrapping = Avalonia.Media.TextWrapping.Wrap,
                        MaxWidth = 420,
                    }
            )
        );

    /// <summary>Shows a failure that stopped the bring before anything was asked.</summary>
    /// <param name="title">The key of what it is called.</param>
    /// <param name="said">What it says.</param>
    private void Refuse(string title, string said) =>
        FailureNotice.Show(_host.Dialogs, _host.Icons, RolaI18N.Get(title), said);

    /// <summary>Runs something on the window's thread with a failure that cannot be handed over on it.</summary>
    /// <param name="what">What to run.</param>
    private static void Post(Action what) => Dispatcher.UIThread.Post(what);

    /// <summary>Lets go of what was done when there was no program to do it with.</summary>
    private void RefuseNoProgram() =>
        Refuse(
            "rorolala_vcs.place.no_program",
            RolaI18N.Get("rorolala_vcs.place.no_program_why")
        );

    /// <summary>The Workspace a directory is in, or nothing when none holds it.</summary>
    /// <param name="directory">The directory to search from.</param>
    /// <returns>The Workspace's root, or nothing.</returns>
    private string? Root(string directory)
    {
        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            return workspace.Root.FullName;
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.NoWorkspace or RolaReadErrorKind.Unavailable)
        {
            return null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"the Workspace holding `{directory}` could not be read: {error.Message}");

            return null;
        }
    }

    /// <summary>What a write failure is said as in the window.</summary>
    /// <remarks>
    /// A kind a key already says is translated, and one no key says is the writer's own message once. A write
    /// failure that carries no message at all still says something: the states a bring reaches most often, no
    /// Workspace, no Vault and no account, have a key here, and a kind left over is named rather than shown as
    /// nothing.
    /// </remarks>
    /// <param name="error">The failure the writer answered with.</param>
    /// <returns>What to show.</returns>
    private static string Told(RolaWriteError error) =>
        error.Kind switch
        {
            RolaWriteErrorKind.NoWorkspace => RolaI18N.Get("rorolala_vcs.place.not_workspace"),
            RolaWriteErrorKind.NoTrack => RolaI18N.Get("rorolala_vcs.place.no_track"),
            RolaWriteErrorKind.NoAccount => RolaI18N.Get("rorolala_vcs.place.no_account"),
            // Offline, the copy a bring is read against is the one a fetch left rather than one this run
            // would have brought, so a Workspace that never fetched one has nothing to plan against.
            RolaWriteErrorKind.NotCached => RolaI18N.Get("rorolala_vcs.layout_not_fetched"),
            _ => error.Message is { Length: > 0 } message ? message : error.Kind.ToString(),
        };

    /// <summary>
    /// Reads the writer's progress while the writer works, and stops before the handle is given back.
    /// </summary>
    /// <remarks>
    /// The writer pushes what it says onto a handle rather than writing it to a stream, so something has to poll
    /// it while the run goes on. The poll runs on a worker of its own and the records are kept under the lock
    /// they already have, which is the same shape the sync's panel uses; stopping it before the handle is
    /// disposed is what keeps a reading from touching a handle that is gone.
    /// </remarks>
    private sealed class Watching : IDisposable
    {
        /// <summary>How long the window waits between two readings, in milliseconds.</summary>
        private const int Pause = 50;

        /// <summary>What the records are read from.</summary>
        private readonly Progress _native;

        /// <summary>What the readings are kept in.</summary>
        private readonly ProgressRecords _records;

        /// <summary>What is told that a record was read.</summary>
        private readonly Action _progress;

        /// <summary>What stops the poll.</summary>
        private readonly CancellationTokenSource _stop = new();

        /// <summary>The poll, which is waited for before the handle is given back.</summary>
        private readonly Task _polling;

        /// <summary>Starts reading what the writer says.</summary>
        /// <param name="native">The writer's progress handle.</param>
        /// <param name="records">What the readings are kept in.</param>
        /// <param name="progress">What is told that records were read.</param>
        public Watching(Progress native, ProgressRecords records, Action progress)
        {
            _native = native;
            _records = records;
            _progress = progress;
            _polling = Task.Run(PollAsync);
        }

        /// <summary>Stops the poll, takes its last reading, and lets the handle go.</summary>
        public void Dispose()
        {
            _stop.Cancel();
            _polling.GetAwaiter().GetResult();
            Drain();
            _stop.Dispose();
            GC.SuppressFinalize(this);
        }

        /// <summary>Reads the handle until the stop is asked for.</summary>
        /// <returns>The poll.</returns>
        private async Task PollAsync()
        {
            try
            {
                while (!_stop.IsCancellationRequested)
                {
                    Drain();
                    await Task.Delay(Pause, _stop.Token).ConfigureAwait(false);
                }
            }
            catch (OperationCanceledException)
            {
                // The stop is the only way out of the loop, so a cancelled wait is the poll ending.
            }
        }

        /// <summary>Reads every record said since the last reading, and tells the window once.</summary>
        private void Drain()
        {
            var said = false;

            while (_native.NextJson() is { } json)
            {
                _records.Read(json);
                said = true;
            }

            if (said)
            {
                _progress();
            }
        }
    }

    /// <summary>Puts a moved file back where it was, saying so when it cannot.</summary>
    /// <param name="from">Where it is now.</param>
    /// <param name="to">Where it belongs.</param>
    private void TryMoveBack(string from, string to)
    {
        try
        {
            File.Move(from, to);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"`{from}` could not be put back at `{to}`: {error.Message}");
        }
    }

    /// <summary>The program the window was opened by, or nothing when it was opened on its own.</summary>
    /// <returns>The program, or nothing.</returns>
    private static string? Program() =>
        Environment.GetEnvironmentVariable(Sync.Program) is { Length: > 0 } program ? program : null;
}

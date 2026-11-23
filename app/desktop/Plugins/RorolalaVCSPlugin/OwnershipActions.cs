using FileSystemPlugin;
using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Taking ownership and letting it go, through the writer the command line uses.
/// </summary>
/// <remarks>
/// The exchange with the Vault is the writer's: fetching its Layout, reading the gates, writing the claim, and
/// doing it here would be a second answer to the same question, kept in step by hand. A plan is made before
/// anything moves, and the plan that was made is the one applied, so what the gates allowed is what the run
/// carries. What this plugin owns is which words are put to the user, not how ownership moves.
/// <para>
/// The run may be partly done, which is what <c>--allow-partial</c> asks for: every entry that passes its gates
/// moves, the ones that do not are reported, and the report is the run's own answer rather than the first
/// refusal that stopped it. A plan made without that answer is refused whole, as the command line refuses it.
/// </para>
/// </remarks>
internal static class OwnershipActions
{
    /// <summary>
    /// Registers what an entry's menu offers about who holds it.
    /// </summary>
    /// <remarks>
    /// Two commands, each with a forced twin, offered on files and on directories under the same four
    /// words: a file is one entry and a directory is everything under it, which the command line does by
    /// itself, so nothing about the command differs between the two.
    /// </remarks>
    /// <param name="host">The host, whose context menus they are registered with.</param>
    /// <param name="hub">What every run this plugin makes is gathered in, which a run of these joins.</param>
    /// <param name="readings">
    /// What the Vault's copy says about the entries being listed, which is what says whether there is anything to act
    /// on: the question is asked once per item every time a menu is offered, so it is answered from what a listing has
    /// already read rather than by opening the Layouts again.
    /// </param>
    public static void Declare(IPluginHost host, IProgressHub hub, OwnershipReadings readings)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(readings);

        // Four words for two commands and their forced twins, offered on both kinds of entry: the
        // command line reads a path as a file or as everything under a directory by itself, so the
        // difference the old pairs of registrations made was in the words rather than in the work. The
        // registry names one context at a time, so each of the four is registered for a file and for a
        // directory, and the path is what makes the two one entry in the menu.
        foreach (var target in new[] { ContextMenuTarget.File, ContextMenuTarget.Directory })
        {
            Add(host, hub, readings, target, "menu.entry.claim", Icons.Lock, "hold", false, 10);
            Add(host, hub, readings, target, "menu.entry.waive", Icons.LockOpen, "giveup", false, 20);
            Add(host, hub, readings, target, "menu.entry.force_claim", Icons.Lock, "hold", true, 30);
            Add(host, hub, readings, target, "menu.entry.force_waive", Icons.LockOpen, "giveup", true, 40);
        }
    }

    /// <summary>Whether this plugin has anything to say about what a menu was opened on.</summary>
    /// <remarks>
    /// A Workspace, and only a Workspace: an entry outside one has no Layout to claim it in, and an action that
    /// could only fail is not worth offering. What cannot be read is answered as nothing to say rather than
    /// raised, a menu that will not open because ownership could not be read is a worse answer than a menu
    /// without these four lines in it, which is the same judgement the column makes.
    /// <para>
    /// The answer is read from what a listing has already read (see <see cref="OwnershipReadings"/>), not by opening the
    /// two Layouts here: this is asked once per contributed item every time a menu is offered, and opening a Layout per
    /// item would be a menu that took a walk of the tree to open. A directory nothing has been read for yet answers
    /// nothing, and the listing that asks is what has it read.
    /// </para>
    /// </remarks>
    /// <param name="was">What the menu was opened on.</param>
    /// <param name="readings">What the Vault's copy says about the entries being listed.</param>
    /// <returns>Whether the entry is one this plugin can act on.</returns>
    public static bool Knows(ContextTarget was, OwnershipReadings readings)
    {
        ArgumentNullException.ThrowIfNull(was);
        ArgumentNullException.ThrowIfNull(readings);

        if (was.Entry is not { } entry)
        {
            return false;
        }

        return Path.GetDirectoryName(entry.Path) is { Length: > 0 } directory
            && readings.Knows(directory);
    }

    /// <summary>Registers one of the four words, for one kind of entry.</summary>
    /// <param name="host">The host.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="readings">What the Vault's copy says about the entries being listed.</param>
    /// <param name="target">Which kind of entry it is offered for.</param>
    /// <param name="leaf">The key naming the word, under the ownership category.</param>
    /// <param name="icon">The picture it wears.</param>
    /// <param name="verb">Which command it runs.</param>
    /// <param name="force">Whether it is one of the forced pair.</param>
    /// <param name="order">Where it sits among the others of its kind.</param>
    private static void Add(
        IPluginHost host,
        IProgressHub hub,
        OwnershipReadings readings,
        ContextMenuTarget target,
        string leaf,
        string icon,
        string verb,
        bool force,
        int order
    ) =>
        host.ContextMenus.Add(
            target,
            new ContextMenuItem(
                ["menu.entry.ownership", leaf],
                order,
                was => Run(verb, was, force, host, hub),
                icon,
                force,
                was => Knows(was, readings)
            )
        );

    /// <summary>Runs one ownership action over the whole choice, and reports what it came to.</summary>
    /// <remarks>
    /// One run for the whole choice rather than one per entry: the writer takes several paths at once, and a
    /// claim that is one claim is one exchange with the Vault rather than three. A path is made absolute before
    /// it is named, because a request reads a relative name from the Workspace's root rather than from the
    /// directory the menu was opened in.
    /// <para>
    /// Run off the window's thread and not waited for: the run talks to a Vault, which takes as long as a
    /// network takes, and a window that stopped drawing until it answered would be a window that stopped. What
    /// the files may have become is said once it is done either way, because a run that ends unhappily may have
    /// claimed some of what it was given, which is what it was asked to do.
    /// </para>
    /// </remarks>
    /// <param name="verb">Which action it is.</param>
    /// <param name="target">What the menu was opened on, the entry right-clicked first.</param>
    /// <param name="force">Whether the checks are to be gone past.</param>
    /// <param name="host">The host, for the report and for saying the files may have changed.</param>
    /// <param name="hub">What every run this plugin makes is gathered in, which this run's card is begun in.</param>
    private static void Run(
        string verb,
        ContextTarget target,
        bool force,
        IPluginHost host,
        IProgressHub hub
    )
    {
        if (target.Entries.Count == 0)
        {
            host.Dialogs.Report(
                new Report(
                    RolaI18N.Get("rorolala_vcs.ownership_failed"),
                    RolaI18N.Get("rorolala_vcs.ownership_unknown")
                )
            );

            return;
        }

        var request = Request(
            verb,
            [.. target.Entries.Select(entry => Absolute(target.Directory, entry.Path))],
            force,
            allowPartial: true
        );

        // The run's card is opened before the work is handed off. The method starts on this thread and
        // reaches the hub before its first await, so the card is there from the moment the run begins;
        // see the hub for why the runs are gathered rather than kept where they were begun.
        _ = ReportAsync(host, hub, request, target.Directory, verb);
    }

    /// <summary>The request one ownership action is made with.</summary>
    /// <remarks>
    /// The Workspace is not here: the writer fills it with the root it found and reads a relative path from
    /// there. The account is the one the work acts as, which is what an ownership move is signed by, and the
    /// Vault is the tracked one, which is the only one the Layout being worked in names ownership against.
    /// </remarks>
    /// <param name="verb">Which action it is.</param>
    /// <param name="paths">The files and directories to move the ownership of, as the program names them.</param>
    /// <param name="force">Whether the checks are to be gone past.</param>
    /// <param name="allowPartial">Whether what can be moved is moved when not everything can be.</param>
    /// <returns>The request.</returns>
    internal static OwnershipPlanRequest Request(
        string verb,
        IReadOnlyList<string> paths,
        bool force,
        bool allowPartial
    )
    {
        ArgumentNullException.ThrowIfNull(verb);
        ArgumentNullException.ThrowIfNull(paths);

        return new OwnershipPlanRequest
        {
            Kind = verb == "hold" ? OwnershipKind.Hold : OwnershipKind.Giveup,
            Paths = paths,
            AllowPartial = allowPartial,
            Force = force,
            Account = UserScope.CurrentAccount,
            Target = WriteTarget.Tracked,
            Labels = new WriteLabels
            {
                Fetching = RolaI18N.Get("rorolala_vcs.progress.fetch"),
                Applying = RolaI18N.Get(
                    verb == "hold"
                        ? "rorolala_vcs.ownership.holding"
                        : "rorolala_vcs.ownership.giving"
                ),
            },
        };
    }

    /// <summary>Runs one ownership action and shows what it came to when it did not end cleanly.</summary>
    /// <param name="host">The host, for the report and for saying the files may have changed.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="request">What the run is to move.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="verb">Which action it is.</param>
    private static async Task ReportAsync(
        IPluginHost host,
        IProgressHub hub,
        OwnershipPlanRequest request,
        string directory,
        string verb
    )
    {
        var outcome = await RunAsync(host, hub, request, directory, verb);

        // A run allowed to be partly done ends well even when it refused something, so the refusals are read
        // from the result rather than from the failure: the status alone would call that run a success.
        if (!outcome.Done || outcome.Failed.Count > 0)
        {
            FailureNotice.Show(
                host.Dialogs,
                host.Icons,
                RolaI18N.Get("rorolala_vcs.ownership_failed"),
                outcome.Done ? string.Join('\n', outcome.Failed) : outcome.Said
            );
        }
    }

    /// <summary>
    /// Runs one ownership action, drawing its progress and saying the files may have changed.
    /// </summary>
    /// <remarks>
    /// One run for the whole choice rather than one per entry: the writer takes several paths at once, and a
    /// claim that is one claim is one exchange with the Vault rather than three.
    /// <para>
    /// The card is begun where the caller is, which is the window's thread for a menu and a worker for the open
    /// check. The hub is written safely from either, and it is the hub that moves the drawing to the window's
    /// thread. The run itself is on a thread of its own: it talks to a Vault, which takes as long as a network
    /// takes, and a window that stopped drawing until it answered would be a window that stopped.
    /// </para>
    /// </remarks>
    /// <param name="host">The host, for saying the files may have changed.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="request">What the run is to move.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="verb">Which action it is.</param>
    /// <returns>Whether it ended well, what to say when it did not, and what it refused.</returns>
    internal static async Task<Outcome> RunAsync(
        IPluginHost host,
        IProgressHub hub,
        OwnershipPlanRequest request,
        string directory,
        string verb
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentException.ThrowIfNullOrEmpty(directory);

        var run = hub.Begin(TitleKey(verb));

        var outcome = await Task.Run(() => Change(request, directory, run));

        // The card is let go of here rather than when the run is over, because the panel keeps it for a few
        // seconds after: what ended is drawn as ended before it is taken away.
        hub.End(run);

        // Either way the files may have moved, which is what the listing reads again for.
        host.Files.Touch();

        return outcome;
    }

    /// <summary>Takes ownership of one file, through the same plan and apply the menu runs.</summary>
    /// <remarks>
    /// This is the same request the menu makes, with one path and the partial answer the open check wants: a
    /// file that cannot be taken is reported to the check rather than shown here, because the check is what
    /// asks the question. The file is named by its whole path, so a name that starts with a dash is still a
    /// path rather than a flag, and no directory the run was made in has to be guessed at.
    /// </remarks>
    /// <param name="host">The host, for saying the files may have changed.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="file">The file to take.</param>
    /// <returns>Whether it was taken, and what to say when it was not.</returns>
    internal static async Task<(bool Done, string Said)> HoldAsync(
        IPluginHost host,
        IProgressHub hub,
        string file
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(file);

        if (Path.GetDirectoryName(file) is not { Length: > 0 } directory)
        {
            return (false, RolaI18N.Get("rorolala_vcs.open.failed_message"));
        }

        var outcome = await RunAsync(
            host,
            hub,
            Request("hold", [file], force: false, allowPartial: true),
            directory,
            "hold"
        );

        if (!outcome.Done || outcome.Failed.Count > 0)
        {
            return (false, outcome.Done ? string.Join('\n', outcome.Failed) : outcome.Said);
        }

        return (true, string.Empty);
    }

    /// <summary>Runs a plan and the apply that carries it out, and says what did not move.</summary>
    /// <remarks>
    /// The plan is made before anything moves, and the same plan is applied, so the run a caller sees is the
    /// run the gates allowed. A plan with a refusal is not applied at all unless the request allows a partial
    /// run, which is the command line's own rule and the reason the answer carries both lists.
    /// </remarks>
    /// <param name="request">What the run is to move.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="run">What the run says itself to.</param>
    /// <returns>Whether it ended well, what to say when it did not, and what it refused.</returns>
    private static Outcome Change(
        OwnershipPlanRequest request,
        string directory,
        IProgressRun run
    )
    {
        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            var writer = new WorkspaceWriter(workspace);

            using var native = Progress.New();
            using var watching = new Watching(native, run.Records, run.Touch);

            var plan = writer.PlanOwnership(request, native);

            if (plan.Refused.Count > 0 && !request.AllowPartial)
            {
                return new Outcome(false, string.Join('\n', Words(plan.Refused)), Words(plan.Refused));
            }

            // The plan itself is not handed back: its wanted entries cross as `Path` and `Uuid`, and
            // the apply rebuilds that list from them. The apply re-makes the plan and re-reads every
            // gate, which is what the command line does, so nothing stale is applied.
            var result = writer.ApplyOwnership(request, plan: null, progress: native);

            return new Outcome(true, string.Empty, Words(result.Failed));
        }
        catch (RolaWriteError error)
        {
            return new Outcome(false, Told(error), []);
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.NoWorkspace or RolaReadErrorKind.Unavailable)
        {
            return new Outcome(false, RolaI18N.Get("rorolala_vcs.not_a_workspace"), []);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new Outcome(false, error.Message, []);
        }
    }

    /// <summary>One refusal, in the words its own fields say.</summary>
    /// <param name="refusals">The refusals an ownership run answered with.</param>
    /// <returns>One line per refusal.</returns>
    private static List<string> Words(IReadOnlyList<OwnershipRefusal> refusals) =>
        refusals
            .Select(refusal =>
                refusal.Cause.Length > 0
                    ? $"{refusal.Kind}: {refusal.Cause}"
                    : refusal.Owner.Length > 0
                        ? $"{refusal.Kind}: {refusal.Path} ({refusal.Owner})"
                        : refusal.Path.Length > 0
                            ? $"{refusal.Kind}: {refusal.Path}"
                            : refusal.Kind
            )
            .ToList();

    /// <summary>What a path is, made absolute against the directory the run is made in.</summary>
    /// <remarks>
    /// A request reads a relative name from the Workspace's root, while the menu's entries are names beside the
    /// directory it was opened on; making the name absolute is what keeps the two ways of reading it from
    /// disagreeing.
    /// </remarks>
    /// <param name="directory">The directory the menu was opened on.</param>
    /// <param name="path">The path as the listing names it.</param>
    /// <returns>The path, absolute.</returns>
    private static string Absolute(string directory, string path) =>
        Path.IsPathRooted(path) ? path : Path.GetFullPath(Path.Combine(directory, path));

    /// <summary>What a write failure is said as in the window.</summary>
    /// <remarks>
    /// A kind a key already says is translated, and one no key says is the writer's own message once. A write
    /// failure that carries no message at all still says something: the states an ownership run reaches most
    /// often, no Workspace, no Vault and no account, have a key here, and a kind left over is named rather than
    /// shown as nothing.
    /// </remarks>
    /// <param name="error">The failure the writer answered with.</param>
    /// <returns>What to show.</returns>
    private static string Told(RolaWriteError error) =>
        error.Kind switch
        {
            RolaWriteErrorKind.NoWorkspace => RolaI18N.Get("rorolala_vcs.not_a_workspace"),
            RolaWriteErrorKind.NoTrack => RolaI18N.Get("rorolala_vcs.ownership.no_track"),
            RolaWriteErrorKind.NoAccount => RolaI18N.Get("rorolala_vcs.ownership.no_account"),
            _ => error.Message is { Length: > 0 } message ? message : error.Kind.ToString(),
        };

    /// <summary>What a run of one of the two actions is called in the Overview's panel.</summary>
    /// <param name="verb">Which action it is.</param>
    /// <returns>The i18n key of its name.</returns>
    private static string TitleKey(string verb) =>
        verb == "hold" ? "rorolala_vcs.progress.hold" : "rorolala_vcs.progress.giveup";

    /// <summary>
    /// Reads the writer's progress while the writer works, and stops before the handle is given back.
    /// </summary>
    /// <remarks>
    /// The writer pushes what it says onto a handle rather than writing it to a stream, so something has to poll
    /// it while the run goes on. The poll runs on a worker of its own and the records are kept under the lock
    /// they already have; stopping it before the handle is disposed is what keeps a reading from touching a
    /// handle that is gone.
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

    /// <summary>What an ownership run left for its caller to read.</summary>
    /// <param name="Done">Whether it ended well.</param>
    /// <param name="Said">What it said when it did not.</param>
    /// <param name="Failed">What it refused, one raw refusal each.</param>
    internal sealed record Outcome(bool Done, string Said, IReadOnlyList<string> Failed);
}

using System.Diagnostics;
using System.Text;
using Avalonia.Threading;
using RolaSharp;
using RolaSharp.Write;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using WritePlan = RolaSharp.Write.SyncPlan;

namespace RorolalaVCSPlugin;

/// <summary>
/// Whether a sync is worth offering, what to say when it is not, and the plan a run applies.
/// </summary>
/// <param name="Ready">Whether there is work to carry, which is what the buttons are pressable for.</param>
/// <param name="Said">Why not, when there is not.</param>
internal sealed record SyncPlan(bool Ready, string? Said)
{
    /// <summary>Where the plan was made, which is the Workspace root a run works from.</summary>
    internal string Directory { get; init; } = string.Empty;

    /// <summary>The request the plan is to be carried out with, or nothing when no plan was made.</summary>
    internal SyncRequest? Request { get; init; }

    /// <summary>The plan the writer answered with, or nothing when it could not be made.</summary>
    internal WritePlan? Planned { get; init; }
}

/// <summary>
/// Asking the writer what a sync would do, and running the plan it answered with.
/// </summary>
/// <remarks>
/// The exchange with the Vault is the writer's, and what this plugin owns is which request a direction and a
/// scope come to, and how a run's progress and failure are shown. A plan is made for two uses: one decides
/// whether the buttons may be pressed, and one is kept for the run that follows, so the plan a run is made with
/// is the plan it applies rather than one made again inside it.
/// <para>
/// The progress comes from the writer's own handle rather than from a command's streams: what the run says is
/// polled while it runs and read into the same <see cref="ProgressRecords"/> the panel draws, so the bars
/// mean the same whether the work is planned or applied.
/// </para>
/// </remarks>
internal static class Sync
{
    /// <summary>The theme the output of a command still run is asked for, which is the one this window reads.</summary>
    internal const string Theme = "desktop-friendly";

    /// <summary>The environment variable naming the program the window was opened by.</summary>
    internal const string Program = "ROLA_EXE";

    /// <summary>
    /// The status a run has when it could not be started at all, which no exit status of a program can be.
    /// </summary>
    /// <remarks>
    /// Nothing was asked, which is a different thing from a command that answered no: a report of a copy that cannot
    /// be read is about what the command said, and one of a program that is not there is about this side.
    /// </remarks>
    internal const int FailedToRun = -1;

    /// <summary>How long the window waits between two readings of the writer's progress, in milliseconds.</summary>
    /// <remarks>
    /// The writer says nothing while nothing changes, so a poll that waited for a change would wait for ever. This
    /// is short enough that a bar follows the work and long enough that reading the queue is not the work.
    /// </remarks>
    private const int ProgressPause = 50;

    /// <summary>
    /// Asks what a sync would carry, and changes nothing.
    /// </summary>
    /// <remarks>
    /// Run where the reader asked to see the work, since what a plan is about is the Layout the tree is rooted
    /// in. The answer is whether there is anything to carry: a plan with no entries is a run that would write
    /// nothing, and a button that offered it would be a button that did nothing when pressed.
    /// <para>
    /// The dock asks both ways, so what is offered is what there is to carry and not what one direction would
    /// carry: a file this side is ahead on is work for the up button, and one upstream is ahead on is work for the
    /// down button, and the plan says both. The plan is always made over the Layout even when the run leaves the
    /// Vault's Layout alone, since the comparison the plan is made from is the Layout's; a request that leaves it
    /// out has no plan to make and is refused by the writer.
    /// </para>
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run would be made.</param>
    /// <param name="way">Which way the run the plan is asked for would carry.</param>
    /// <param name="scope">What the run would carry.</param>
    /// <param name="force">Whether the run may change history rather than refuse.</param>
    /// <returns>Whether there is work to carry, what to say when there is not, and the plan a run applies.</returns>
    public static SyncPlan Plan(string directory, SyncWay way, SyncScope scope, bool force)
    {
        ArgumentNullException.ThrowIfNull(directory);

        // Every part left behind is a run that changes nothing, which is nothing to press a button for. The
        // writer would refuse a request with every side off, so this says the same answer without asking.
        if (!scope.Index && !scope.Layout && !scope.Storage)
        {
            return new SyncPlan(false, null);
        }

        try
        {
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            // The account is read after the Workspace, so a directory that holds none is answered without
            // asking the user's own configuration about work that is not there.
            var account = UserScope.CurrentAccount;
            var request = Request(directory, way, scope, force, account);
            var planned = PlanRequest(directory, way, scope, force, account);

            var writer = new WorkspaceWriter(workspace);

            // No cancellation is handed in: the panel has no stop control, so the writer makes one that
            // nothing here can set. A handle made here and never set would be a stop that never comes.
            var answer = writer.PlanSync(planned);

            return new SyncPlan(answer.Entries.Count > 0, null)
            {
                Directory = workspace.Root.FullName,
                Request = request,
                Planned = answer,
            };
        }
        catch (RolaWriteError error)
        {
            return new SyncPlan(false, Told(error));
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.NoWorkspace or RolaReadErrorKind.Unavailable)
        {
            return new SyncPlan(false, RolaI18N.Get("rorolala_vcs.not_a_workspace"));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new SyncPlan(false, error.Message);
        }
    }

    /// <summary>
    /// The request a run of one direction and scope is made with.
    /// </summary>
    /// <remarks>
    /// The account is the one the work acts as, read from the user's own file as the command line reads it. The
    /// Vault is the tracked one, which is what a sync means when it names none. A sync is never made offline: it
    /// is the exchange itself, so the watcher refuses to start one while the Vault is unreachable, and the request
    /// below is always one that may reach it.
    /// The fetching label is this panel's own, since a bar whose words were the writer's English would look
    /// broken in a window drawn in another language; every other word a sync says is the writer's, as the apply
    /// reports a direction rather than naming a step.
    /// </remarks>
    /// <param name="directory">Where the work is rooted.</param>
    /// <param name="way">Which way content may move.</param>
    /// <param name="scope">Which sides move.</param>
    /// <param name="force">Whether a version whose chain moved apart may go up.</param>
    /// <param name="account">The account the run signs as.</param>
    /// <returns>The request, whose Workspace the writer replaces with the root it found.</returns>
    internal static SyncRequest Request(
        string directory,
        SyncWay way,
        SyncScope scope,
        bool force,
        string account
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(account);

        return new SyncRequest
        {
            Workspace = directory,
            Account = account,
            Target = WriteTarget.Tracked,
            Direction = way switch
            {
                SyncWay.Up => SyncDirection.Up,
                SyncWay.Down => SyncDirection.Down,
                _ => SyncDirection.Both,
            },
            Forced = force,
            Index = scope.Index,
            Layout = scope.Layout,
            Storage = scope.Storage,
            Labels = new WriteLabels { Fetching = RolaI18N.Get("rorolala_vcs.progress.fetch") },
        };
    }

    /// <summary>
    /// The request a plan is asked with, which always reads the Vault's Layout.
    /// </summary>
    /// <remarks>
    /// A plan is the comparison with the Vault's Layout, so a request that leaves the Layout out has none to
    /// answer with. The run that follows may still leave the Layout alone, and it applies the plan it was handed
    /// without reading it in that case; what the plan is asked with and what the run moves are therefore two
    /// requests, and only the index and the storage follow the scope here.
    /// </remarks>
    /// <param name="directory">Where the work is rooted.</param>
    /// <param name="way">Which way the run the plan is asked for would carry.</param>
    /// <param name="scope">Which sides the run moves.</param>
    /// <param name="force">Whether the run may change history rather than refuse.</param>
    /// <param name="account">The account the run signs as.</param>
    /// <returns>The request, whose Layout is always read.</returns>
    internal static SyncRequest PlanRequest(
        string directory,
        SyncWay way,
        SyncScope scope,
        bool force,
        string account
    ) => Request(directory, way, scope, force, account) with { Layout = true };

    /// <summary>
    /// Runs the plan a panel kept, saying what it is doing while it goes.
    /// </summary>
    /// <remarks>
    /// The plan the writer made is the one applied, so the run carries what the plan named rather than planning
    /// again inside itself. The progress handle is made here and the window polls it while the writer works on
    /// this thread; each reading is read into the records on the window's thread, which is where the panel is
    /// drawn from them. The handle is given back only after the poll has stopped and its last reading is taken,
    /// so nothing the writer said is lost on the way out.
    /// </remarks>
    /// <param name="plan">What the plan was, which must be one <see cref="Plan"/> made.</param>
    /// <param name="records">What the run's records are read into, which the caller keeps and draws.</param>
    /// <param name="progress">What is told that a record was said, which the caller draws on its own thread.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        SyncPlan plan,
        ProgressRecords records,
        Action progress
    )
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(records);
        ArgumentNullException.ThrowIfNull(progress);

        if (
            plan.Request is not { } request
            || plan.Planned is not { } wanted
            || plan.Directory is not { Length: > 0 } directory
        )
        {
            // A plan with nothing against it is the run that leaves every side out: it changes nothing, which is
            // what it did before there was a writer, so it ends well rather than being refused for having no plan.
            return plan.Said is { Length: > 0 } said ? (false, said) : (true, string.Empty);
        }

        try
        {
            // The plan is the writer's own answer, so the run is made over the same scope rather than over the
            // directory the panel holds: a directory inside the Workspace names the Workspace by being in it.
            using var workspace = WorkspaceScope.Open(directory);
            workspace.Read();

            var writer = new WorkspaceWriter(workspace);

            using var native = Progress.New();

            // The timer belongs to the window's dispatcher rather than to the one this worker would make for
            // itself: a timer on a dispatcher with no loop never ticks, and the bar would stand still.
            var timer = new DispatcherTimer(
                TimeSpan.FromMilliseconds(ProgressPause),
                DispatcherPriority.Background,
                Dispatcher.UIThread
            );

            timer.Tick += (_, _) => Drain(native, records, progress);

            OnWindow(timer.Start);
            try
            {
                // No cancellation is handed in: the panel has no stop control, so the writer makes one.
                writer.Sync(request, wanted, native);
            }
            finally
            {
                OnWindow(() =>
                {
                    timer.Stop();
                    Drain(native, records, progress);
                });
            }

            return (true, string.Empty);
        }
        catch (RolaWriteError error)
        {
            return (false, Told(error));
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.NoWorkspace or RolaReadErrorKind.Unavailable)
        {
            return (false, RolaI18N.Get("rorolala_vcs.not_a_workspace"));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, error.Message);
        }
    }

    /// <summary>
    /// What a write failure is said as in the window.
    /// </summary>
    /// <remarks>
    /// A kind a key already says is translated, and one no key says is the writer's own message once: the
    /// failure's kind is not spelled beside it, since a reader shown both would read the same thing twice. The
    /// message is English, which is what the writer says; a key is used only where one says the same state.
    /// </remarks>
    /// <param name="error">The failure the writer answered with.</param>
    /// <returns>What to show.</returns>
    internal static string Told(RolaWriteError error)
    {
        ArgumentNullException.ThrowIfNull(error);

        return error.Kind switch
        {
            RolaWriteErrorKind.NoWorkspace => RolaI18N.Get("rorolala_vcs.not_a_workspace"),
            RolaWriteErrorKind.Json => RolaI18N.Get("rorolala_vcs.sync.plan_unreadable"),
            // A retrack that goes to a version whose content is not here is refused offline rather than left to
            // wait on the Vault, so the window says what is missing rather than the writer's English.
            RolaWriteErrorKind.Offline => RolaI18N.Get("rorolala_vcs.offline_content"),
            _ => error.Message,
        };
    }

    /// <summary>
    /// Reads every record the writer has said since the last reading, and tells the panel once.
    /// </summary>
    /// <remarks>
    /// It runs on the window's thread, from the timer and from the last reading the stop makes, so the records
    /// and the drawing they cause belong to one thread. The queue is drained rather than read one at a time so
    /// that a burst of records cannot overtake the next reading.
    /// </remarks>
    /// <param name="native">The writer's progress handle.</param>
    /// <param name="records">What the records are read into.</param>
    /// <param name="progress">What is told that records were read.</param>
    private static void Drain(Progress native, ProgressRecords records, Action progress)
    {
        var said = false;

        while (native.NextJson() is { } json)
        {
            records.Read(json);
            said = true;
        }

        if (said)
        {
            progress();
        }
    }

    /// <summary>Runs something on the window's thread, wherever it was called from.</summary>
    /// <remarks>
    /// The run itself is on a worker, so this waits for the window to take the work. The window never waits on
    /// the run in return, which is what keeps the wait from closing on itself.
    /// </remarks>
    /// <param name="said">What to do.</param>
    private static void OnWindow(Action said)
    {
        if (Dispatcher.UIThread.CheckAccess())
        {
            said();

            return;
        }

        Dispatcher.UIThread.InvokeAsync(said).GetAwaiter().GetResult();
    }

    /// <summary>Runs a command and waits, answering whether it ended well, what it said, and what it wrote.</summary>
    /// <remarks>
    /// Both streams are drained while it runs, so that a command that fills one pipe while this waits on the
    /// other cannot block against itself.
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it.</param>
    /// <returns>Whether it ended well, what it said, and what it wrote.</returns>
    internal static (bool Done, string Said, string Output) Asked(
        string program,
        IReadOnlyList<string> arguments,
        string directory
    )
    {
        var (code, said, output) = Coded(program, arguments, directory);

        return (code == 0, said, output);
    }

    /// <summary>
    /// Runs a command and waits, answering the status it ended with as well as what it said.
    /// </summary>
    /// <remarks>
    /// A status is answered beside the words because one of the ways a run fails is a state rather than a fault: the
    /// copy of a Layout that was never fetched is what a place offers a fetch for, and telling that from a copy that
    /// cannot be read means reading the status rather than the message, which is in the reader's own language.
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it.</param>
    /// <returns>The status, what to say when it is not zero, and what it wrote.</returns>
    internal static (int Code, string Said, string Output) Coded(
        string program,
        IReadOnlyList<string> arguments,
        string directory
    )
    {
        var start = new ProcessStartInfo
        {
            FileName = program,
            WorkingDirectory = directory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            var error = Task.Run(() => process.StandardError.ReadToEnd());
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());

            process.WaitForExit();

            var said = error.GetAwaiter().GetResult().Trim();
            var written = output.GetAwaiter().GetResult().Trim();

            // A failure written as data rather than as a status and an error stream: asking for a structural renderer
            // answers a failed run with the failure itself, and the renderer that would have set the status never ran.
            var failure = FailureReport.Read(written);

            if (process.ExitCode == 0)
            {
                return failure is { } answered
                    ? (FailureReport.Code, answered.Reason, written)
                    : (0, string.Empty, written);
            }

            return (
                process.ExitCode,
                said.Length > 0
                    ? said
                    : failure?.Reason is { Length: > 0 } reason
                        ? reason
                        : written.Length > 0
                            ? written
                            : $"the command ended with {process.ExitCode}",
                written
            );
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (FailedToRun, error.Message, string.Empty);
        }
    }
}

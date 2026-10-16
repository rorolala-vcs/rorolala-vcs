using System.Diagnostics;
using System.Text;
using System.Text.Json;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Which way one task's work moves, when it moves any.
/// </summary>
/// <remarks>
/// The command line's own two directions, read from a record's <c>direction</c> field: work leaving this side is
/// up, and work arriving is down. A task that moves no data — reading the Layout, planning, writing the changes
/// down — carries none, and is drawn without one.
/// </remarks>
internal enum SyncAim
{
    /// <summary>Work leaving this side.</summary>
    Up,

    /// <summary>Work arriving at this side.</summary>
    Down,
}

/// <summary>
/// One thing a run is doing, as much of it as a row needs.
/// </summary>
/// <remarks>
/// A task is what one <c>begin</c> record opens and the records that name the same id are about: how long it is,
/// how far it has got, and what it is moving right now. A task that is part of another is a bar of its own — the
/// two directions of a transfer under the transfer — while a task that only names what its parent is moving is
/// not: it is what the parent shows as a line of its own.
/// </remarks>
internal sealed class SyncTask
{
    /// <summary>Makes a task from the record that opened it.</summary>
    /// <param name="what">What the record calls it.</param>
    /// <param name="aim">Which way its work moves, or nothing when it moves no data.</param>
    /// <param name="total">How long it is, or nothing when the record did not say.</param>
    public SyncTask(string what, SyncAim? aim, long? total)
    {
        What = what;
        Aim = aim;
        Total = total;
    }

    /// <summary>What it is called, which is the command line's own word for it.</summary>
    public string What { get; }

    /// <summary>Which way its work moves, or nothing when it moves no data.</summary>
    public SyncAim? Aim { get; }

    /// <summary>How much of it there is in all, or nothing when the run has not said.</summary>
    public long? Total { get; }

    /// <summary>How much of it is behind it, counted the way its total is.</summary>
    public long Done { get; private set; }

    /// <summary>What it is moving right now, or nothing when it has not said.</summary>
    public string? Moving { get; private set; }

    /// <summary>Says how far it has got.</summary>
    /// <param name="done">How much of it is behind it.</param>
    public void Advance(long done) => Done = done;

    /// <summary>Names the piece it is moving right now.</summary>
    /// <param name="what">The name, or nothing when it has none.</param>
    public void Name(string? what) => Moving = what is { Length: > 0 } ? what : null;
}

/// <summary>
/// What a run says it is doing, as the tasks it reports.
/// </summary>
/// <remarks>
/// The records are the command line's own — one JSON object per line on its error stream, written whenever a
/// sync is asked for a structural renderer — and every task one of them opens is kept, so that a run which draws
/// several bars at once is read as several rather than as one. A run of `rola sync` is exactly that: reaching the
/// Vault and planning, carrying the content, and writing the changes down each say a bar of their own, one after
/// another, and a reader shown only the last would see a bar that jumped.
/// <para>
/// What is kept is the whole of what a task says, and nothing of it is guessed: how long it is and how far it
/// has got are what its records said, and a task whose length was never said is drawn as one still moving rather
/// than as one at nothing done.
/// </para>
/// <para>
/// One of these crosses two threads: the run reads the records on the thread it runs on and the panel draws on
/// the window's, so the tasks are written under a lock and handed out as they are rather than as the list itself.
/// </para>
/// </remarks>
internal sealed class SyncProgress
{
    /// <summary>What the tasks are written under, since the run writes them while the panel reads them.</summary>
    private readonly object _gate = new();

    /// <summary>The tasks to draw, in the order the run opened them.</summary>
    private readonly List<SyncTask> _tasks = [];

    /// <summary>The same tasks by id, which is what a later record is answered with.</summary>
    private readonly Dictionary<long, SyncTask> _byId = [];

    /// <summary>The tasks the run has opened and not yet finished, in the order it opened them.</summary>
    /// <remarks>
    /// A copy taken under the lock rather than the list itself: the run goes on adding and taking away while the
    /// panel reads what is here, and a list read while it is changed is a list walked off its own end.
    /// </remarks>
    public IReadOnlyList<SyncTask> Tasks
    {
        get
        {
            lock (_gate)
            {
                return _tasks.ToArray();
            }
        }
    }

    /// <summary>Forgets a run that is over, so that the next one's rows start where its own records do.</summary>
    public void Reset()
    {
        lock (_gate)
        {
            _tasks.Clear();
            _byId.Clear();
        }
    }

    /// <summary>Reads one record, which is one line of the run's own progress stream.</summary>
    /// <param name="record">The line, as it was written.</param>
    /// <returns>Whether the line was a record this understands.</returns>
    public bool Read(string record)
    {
        ArgumentNullException.ThrowIfNull(record);

        if (record.Length == 0 || record[0] != '{')
        {
            return false;
        }

        try
        {
            using var document = JsonDocument.Parse(record);

            return Take(document.RootElement);
        }
        catch (JsonException)
        {
            return false;
        }
    }

    /// <summary>Takes what one record says, once it is known to be a record.</summary>
    /// <param name="record">The record.</param>
    /// <returns>Whether it said anything this understands.</returns>
    private bool Take(JsonElement record)
    {
        if (
            !record.TryGetProperty("signal", out var signal)
            || signal.ValueKind is not JsonValueKind.String
        )
        {
            return false;
        }

        if (!record.TryGetProperty("id", out var id) || !id.TryGetInt64(out var at))
        {
            return false;
        }

        return Held(record, at, signal.GetString());
    }

    /// <summary>
    /// Takes one record's worth of change under the lock, since the run writes while the panel reads.
    /// </summary>
    /// <param name="record">The record.</param>
    /// <param name="at">The id it names its task by.</param>
    /// <param name="signal">What the record is.</param>
    /// <returns>Whether it said anything this understands.</returns>
    private bool Held(JsonElement record, long at, string? signal)
    {
        lock (_gate)
        {
            return signal switch
            {
                "begin" => Began(record, at),
                "advance" => Advanced(record, at),
                "finish" => Ended(at),
                _ => false,
            };
        }
    }

    /// <summary>Takes the task a begin record opens, or names the piece its parent is moving.</summary>
    /// <param name="record">The record.</param>
    /// <param name="at">The id it names the task by.</param>
    /// <returns>Whether it said anything this understands, which a begin always does.</returns>
    private bool Began(JsonElement record, long at)
    {
        var parent = Number(record, "parent");
        var aim = Aim(Text(record, "direction"));

        if (parent is { } above && aim is null)
        {
            // A task with a parent and no direction is not a bar of its own: it is the piece its parent is
            // moving right now, which the parent says as a line rather than beside its own. A parent this has
            // not kept — a piece of a task that has already ended — names nothing.
            if (_byId.TryGetValue(above, out var moving))
            {
                moving.Name(Text(record, "what"));
            }

            return true;
        }

        var task = new SyncTask(
            Text(record, "what") ?? string.Empty,
            aim,
            Number(record, "total")
        );

        _byId[at] = task;
        _tasks.Add(task);

        return true;
    }

    /// <summary>Says how far the task a record names has got.</summary>
    /// <param name="record">The record.</param>
    /// <param name="at">The id it names the task by.</param>
    /// <returns>Whether it named a task this keeps.</returns>
    private bool Advanced(JsonElement record, long at)
    {
        if (!_byId.TryGetValue(at, out var task) || Number(record, "done") is not { } done)
        {
            return false;
        }

        task.Advance(done);

        return true;
    }

    /// <summary>Takes a task off the list, which is what its finish says.</summary>
    /// <remarks>
    /// A task that is over is taken off, the way the command line takes its bar off the terminal: what is left is
    /// what is still going, and a row that stayed after its work had ended would be a row read for nothing.
    /// </remarks>
    /// <param name="at">The id it was named by.</param>
    /// <returns>Whether it was a task this keeps.</returns>
    private bool Ended(long at) => _byId.Remove(at, out var task) && _tasks.Remove(task);

    /// <summary>The number a record states under <paramref name="name"/>, or nothing when it states none.</summary>
    /// <param name="record">The record.</param>
    /// <param name="name">The field's name.</param>
    /// <returns>The number, or nothing.</returns>
    private static long? Number(JsonElement record, string name) =>
        record.TryGetProperty(name, out var value) && value.ValueKind is JsonValueKind.Number
            ? value.GetInt64()
            : null;

    /// <summary>The text a record states under <paramref name="name"/>, or nothing when it states none.</summary>
    /// <param name="record">The record.</param>
    /// <param name="name">The field's name.</param>
    /// <returns>The text, or nothing.</returns>
    private static string? Text(JsonElement record, string name) =>
        record.TryGetProperty(name, out var value) && value.ValueKind is JsonValueKind.String
            ? value.GetString()
            : null;

    /// <summary>Which way a record says its work moves, or nothing when it says none this knows.</summary>
    /// <param name="said">What the record wrote for the direction.</param>
    /// <returns>The direction, or nothing.</returns>
    private static SyncAim? Aim(string? said) =>
        said switch
        {
            "up" => SyncAim.Up,
            "down" => SyncAim.Down,
            _ => null,
        };
}

/// <summary>
/// Whether a sync is worth offering, and what to say when it is not.
/// </summary>
/// <param name="Ready">Whether there is work to carry, which is what the buttons are pressable for.</param>
/// <param name="Said">Why not, when there is not.</param>
internal sealed record SyncPlan(bool Ready, string? Said);

/// <summary>
/// Running the command line's sync, and asking it what it would do without doing it.
/// </summary>
/// <remarks>
/// The exchange with the Vault is the command line's, and what this plugin owns is which buttons put which
/// flags to it. Two runs are made of it: a plan, which is asked for whenever the dock is looked at and decides
/// whether the buttons may be pressed, and the run itself, which reports its progress while it goes.
/// <para>
/// Both are asked for a structural renderer, and that is what makes the progress readable: the same flag that
/// has the command answer in JSON has it write its progress as one JSON record per line, so the two streams of
/// one run carry the result and the account of how it was reached.
/// </para>
/// </remarks>
internal static class Sync
{
    /// <summary>The theme the output is asked for, which is the one this window reads.</summary>
    private const string Theme = "desktop-friendly";

    /// <summary>The environment variable naming the program the window was opened by.</summary>
    private const string Program = "ROLA_EXE";

    /// <summary>
    /// The flag that has the command answer in JSON and write its progress as one record per line.
    /// </summary>
    /// <remarks>
    /// The same flag for both, since a run being read by a program is the one thing both answers are for: what
    /// is drawn here is what a reader of the records would draw, and getting it and the result from one run is
    /// what makes the bar possible while the run is still going.
    /// </remarks>
    private const string Json = "--json";

    /// <summary>
    /// The flags a sync is run with, which is which way it goes and what it leaves out.
    /// </summary>
    /// <param name="way">Which way it carries.</param>
    /// <param name="scope">What it carries.</param>
    /// <param name="force">Whether history is to be changed rather than refused.</param>
    /// <returns>The arguments, after the program's name.</returns>
    public static IReadOnlyList<string> Arguments(SyncWay way, SyncScope scope, bool force)
    {
        var arguments = new List<string> { "--theme-choice=" + Theme, Json, "sync" };

        arguments.Add(
            way switch
            {
                SyncWay.Up => "--up-only",
                SyncWay.Down => "--down-only",
                _ => "--both",
            }
        );

        if (force)
        {
            arguments.Add("--force");
        }

        arguments.AddRange(scope.Flags);

        return arguments;
    }

    /// <summary>
    /// Asks what a sync would carry, and changes nothing.
    /// </summary>
    /// <remarks>
    /// Run where the reader asked to see the work, since what a plan is about is the Layout the tree is rooted
    /// in. The answer is whether there is anything to carry: a plan with no entries is a run that would write
    /// nothing, and a button that offered it would be a button that did nothing when pressed.
    /// <para>
    /// Both ways are asked of the same command as the run itself, without the one direction, so what is offered
    /// is what there is to carry and not what one direction would carry: a file this side is ahead on is work
    /// for the up button, and one upstream is ahead on is work for the down button, and the plan says both.
    /// </para>
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <returns>Whether there is work to carry, and what to say when there is not.</returns>
    public static SyncPlan Plan(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var program = Environment.GetEnvironmentVariable(Program);

        if (string.IsNullOrEmpty(program))
        {
            return new SyncPlan(false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var arguments = new List<string> { "--theme-choice=" + Theme, Json, "sync", "--dry-run" };
        arguments.AddRange(SyncSettings.Chosen().Flags);

        var (done, said, output) = Asked(program, arguments, directory);

        return done ? Planned(output) : new SyncPlan(false, said);
    }

    /// <summary>
    /// What a plan the command line printed comes to, which is whether there is anything to carry.
    /// </summary>
    /// <remarks>
    /// Read from the plan rather than from the work, because what there is to carry is the Vault's answer as much
    /// as this side's: a file this side has changed is work for an upload, and one the Vault has moved on is work
    /// for a download, and neither is anything this side can see by looking at the tree. A plan with no entries
    /// is a run that would write nothing, and the buttons are drawn unpressable for it.
    /// <para>
    /// A plan that cannot be read at all is answered the same way, with the words for it: a program that answered
    /// something else is not a program whose answer may be taken as "there is nothing to do" silently.
    /// </para>
    /// </remarks>
    /// <param name="output">What the command printed, which is one JSON object when it was asked for one.</param>
    /// <returns>Whether there is work to carry, and what to say when there is not.</returns>
    internal static SyncPlan Planned(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        try
        {
            using var document = JsonDocument.Parse(output);

            // What is asked for is only there on an object: a command that answered a list or a number is a
            // command whose answer is not a plan, and asking a list for a property is a throw rather than a no.
            if (
                document.RootElement.ValueKind is JsonValueKind.Object
                && document.RootElement.TryGetProperty("entries", out var entries)
                && entries.ValueKind is JsonValueKind.Array
            )
            {
                return new SyncPlan(entries.GetArrayLength() > 0, null);
            }
        }
        catch (JsonException)
        {
            // Said below, the same as a plan with nothing in it: what went wrong is the command line's to say,
            // and this is what the buttons are drawn grey for.
        }

        return new SyncPlan(false, RolaI18N.Get("rorolala_vcs.sync.plan_unreadable"));
    }

    /// <summary>
    /// Runs a sync, saying what it is doing while it goes.
    /// </summary>
    /// <remarks>
    /// The progress is read off the error stream line by line as the run writes it, which is why the run is not
    /// waited for before anything is read: a bar drawn from what a run said after it ended would be a bar that
    /// appeared when there was nothing left to watch. The result — which goes to the output stream as one JSON
    /// object — is read as it is written too, so that neither stream can fill and hold the run up.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="way">Which way it carries.</param>
    /// <param name="scope">What it carries.</param>
    /// <param name="force">Whether history is to be changed rather than refused.</param>
    /// <param name="records">What the run's records are read into, which the caller keeps and draws.</param>
    /// <param name="progress">What is told that a record was said, which the caller draws on its own thread.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        SyncWay way,
        SyncScope scope,
        bool force,
        SyncProgress records,
        Action progress
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(records);
        ArgumentNullException.ThrowIfNull(progress);

        var program = Environment.GetEnvironmentVariable(Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var start = new ProcessStartInfo
        {
            FileName = program,
            WorkingDirectory = directory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,

            // The words are `rola`'s own, and `rola` writes UTF-8 — as every stream this project's programs
            // write is. Left unstated, a redirected stream is decoded in the machine's codepage, which on a
            // Chinese Windows is GBK, and every word of the message comes out as mojibake.
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };

        foreach (var argument in Arguments(way, scope, force))
        {
            start.ArgumentList.Add(argument);
        }

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            var said = new StringBuilder();

            // The output is drained on a task of its own rather than read after the wait: a run that filled
            // the pipe this side is not reading would wait for a reader that is waiting for it to end.
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());

            while (process.StandardError.ReadLine() is { } line)
            {
                if (!records.Read(line) && line.Length > 0)
                {
                    said.AppendLine(line);
                }

                progress();
            }

            process.WaitForExit();

            if (process.ExitCode == 0)
            {
                return (true, string.Empty);
            }

            var told = said.ToString().Trim();

            if (told.Length == 0)
            {
                told = output.GetAwaiter().GetResult().Trim();
            }

            return (false, told.Length == 0 ? $"the command ended with {process.ExitCode}" : told);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, error.Message);
        }
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
    private static (bool Done, string Said, string Output) Asked(
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

            if (process.ExitCode == 0)
            {
                return (true, string.Empty, written);
            }

            return (
                false,
                said.Length > 0 ? said : written.Length > 0 ? written : $"the command ended with {process.ExitCode}",
                written
            );
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, error.Message, string.Empty);
        }
    }
}

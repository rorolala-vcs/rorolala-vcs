using System.Text.Json;

namespace RorolalaVCSPlugin;

/// <summary>
/// Which way one task's work moves, when it moves any.
/// </summary>
/// <remarks>
/// The command line's own two directions, read from a record's <c>direction</c> field: work leaving this side is
/// up, and work arriving is down. A task that moves no data — reading the Layout, planning, writing the changes
/// down — carries none, and is drawn without one.
/// </remarks>
internal enum MoveAim
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
internal sealed class ProgressTask
{
    /// <summary>Makes a task from the record that opened it.</summary>
    /// <param name="what">What the record calls it.</param>
    /// <param name="aim">Which way its work moves, or nothing when it moves no data.</param>
    /// <param name="total">How long it is, or nothing when the record did not say.</param>
    /// <param name="parent">The task it is part of, or nothing when it is one of the run's own.</param>
    public ProgressTask(string what, MoveAim? aim, long? total, long? parent)
    {
        What = what;
        Aim = aim;
        Total = total;
        Parent = parent;
    }

    /// <summary>What it is called, which is the command line's own word for it.</summary>
    public string What { get; }

    /// <summary>Which way its work moves, or nothing when it moves no data.</summary>
    public MoveAim? Aim { get; }

    /// <summary>How much of it there is in all, or nothing when the run has not said.</summary>
    public long? Total { get; }

    /// <summary>The task it is part of, or nothing when it is one of the run's own.</summary>
    public long? Parent { get; }

    /// <summary>How much of it is behind it, counted the way its total is.</summary>
    public long Done { get; private set; }

    /// <summary>What it is moving right now, or nothing when it has not said or has finished.</summary>
    public string? Moving { get; private set; }

    /// <summary>Says how far it has got.</summary>
    /// <param name="done">How much of it is behind it.</param>
    public void Advance(long done) => Done = done;

    /// <summary>Names the piece it is moving right now.</summary>
    /// <param name="what">The name, or nothing when it has none.</param>
    public void Name(string? what) => Moving = what is { Length: > 0 } ? what : null;

    /// <summary>
    /// Says the task is over, which is done rather than gone.
    /// </summary>
    /// <remarks>
    /// A task the run has finished is kept and marked full rather than taken away: a card outlives its run by a
    /// few seconds (see <see cref="ProgressHub"/>), and a card whose bars had all vanished the moment their work
    /// ended would show nothing at the one moment it is kept for. The task is counted as spent even where the
    /// run's last step was never said, since a finished task with a length is a task with nothing left to do.
    /// </remarks>
    public void Finish()
    {
        if (Total is { } total)
        {
            Done = total;
        }

        Moving = null;
    }
}

/// <summary>
/// What a run says it is doing, as the tasks it reports.
/// </summary>
/// <remarks>
/// The records are the command line's own — one JSON object per line on its error stream, written whenever a run
/// is asked for a structural renderer — and every task one of them opens is kept, so that a run which draws
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
internal sealed class ProgressRecords
{
    /// <summary>What the tasks are written under, since the run writes them while the panel reads them.</summary>
    private readonly object _gate = new();

    /// <summary>The tasks to draw, in the order the run opened them.</summary>
    private readonly List<ProgressTask> _tasks = [];

    /// <summary>The same tasks by id, which is what a later record is answered with.</summary>
    private readonly Dictionary<long, ProgressTask> _byId = [];

    /// <summary>The tasks the run has opened, finished ones included, in the order it opened them.</summary>
    /// <remarks>
    /// A copy taken under the lock rather than the list itself: the run goes on adding while the panel reads what
    /// is here, and a list read while it is changed is a list walked off its own end.
    /// </remarks>
    public IReadOnlyList<ProgressTask> Tasks
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

        var task = new ProgressTask(
            Text(record, "what") ?? string.Empty,
            aim,
            Number(record, "total"),
            parent
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

    /// <summary>
    /// Marks the task a record names as over, keeping it on the list.
    /// </summary>
    /// <remarks>
    /// A task that is over is kept rather than taken off — see <see cref="ProgressTask.Finish"/> for why a card
    /// outlives its run — so a finish only says the task is spent, and a piece whose bar was never kept is a
    /// finish this knows nothing of.
    /// </remarks>
    /// <param name="at">The id it was named by.</param>
    /// <returns>Whether it was a task this keeps.</returns>
    private bool Ended(long at)
    {
        if (!_byId.TryGetValue(at, out var task))
        {
            return false;
        }

        task.Finish();

        return true;
    }

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
    private static MoveAim? Aim(string? said) =>
        said switch
        {
            "up" => MoveAim.Up,
            "down" => MoveAim.Down,
            _ => null,
        };
}

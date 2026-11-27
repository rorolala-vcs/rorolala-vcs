using System.Globalization;
using RolaSharp;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What the ops log shows: the operations a Workspace recorded, and the words a row is read by.
/// </summary>
/// <remarks>
/// The operations are the stack <c>rola undo</c> pops, read through the C ABI and never through the
/// journal files themselves: the format of a log file is the writer's, and a second reading of it
/// here would be a second format to keep in step.
/// <para>
/// A time is shown as how long ago it was, because a log is read as what happened recently, and the
/// exact moment is kept for the tooltip of the row. A stamp that does not read is shown as it was
/// written rather than hidden, since a value this side cannot read is worth seeing.
/// </para>
/// </remarks>
internal static class WorkspaceOpsReading
{
    /// <summary>Reads the operations a Workspace recorded, newest first.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The operations.</returns>
    /// <exception cref="RolaReadError">The Workspace's log cannot be read.</exception>
    public static IReadOnlyList<RecordedOperation> Read(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        using var scope = new OperationLogScope(root);
        scope.Read();

        return scope.Operations;
    }

    /// <summary>What a command is called in the log.</summary>
    /// <remarks>
    /// The command line's own names are the record's, and a window says them in words a reader
    /// knows. A command this build does not know is shown as the record states it, which is a true
    /// answer where a made-up one would not be.
    /// </remarks>
    /// <param name="command">The command the record names.</param>
    /// <returns>What to show.</returns>
    public static string Command(string command)
    {
        ArgumentNullException.ThrowIfNull(command);

        return command switch
        {
            "fs-ops cp" => RolaI18N.Get("rorolala_vcs.workspace.cmd_fs_cp"),
            "fs-ops mv" => RolaI18N.Get("rorolala_vcs.workspace.cmd_fs_mv"),
            "fs-ops rm" => RolaI18N.Get("rorolala_vcs.workspace.cmd_fs_rm"),
            "align" => RolaI18N.Get("rorolala_vcs.workspace.cmd_align"),
            "bring" => RolaI18N.Get("rorolala_vcs.workspace.cmd_bring"),
            "collapse" => RolaI18N.Get("rorolala_vcs.workspace.cmd_collapse"),
            "cut" => RolaI18N.Get("rorolala_vcs.workspace.cmd_cut"),
            "retrack" => RolaI18N.Get("rorolala_vcs.workspace.cmd_retrack"),
            "sync" => RolaI18N.Get("rorolala_vcs.workspace.cmd_sync"),
            "track" => RolaI18N.Get("rorolala_vcs.workspace.cmd_track"),
            "tweak" => RolaI18N.Get("rorolala_vcs.workspace.cmd_tweak"),
            _ => command,
        };
    }

    /// <summary>The picture a command's row wears.</summary>
    /// <param name="command">The command the record names.</param>
    /// <returns>The key of the picture in this plugin's set.</returns>
    public static string CommandIcon(string command) =>
        command switch
        {
            "fs-ops cp" => Icons.Copy,
            "fs-ops mv" => Icons.ArrowRight,
            "fs-ops rm" => Icons.TrashBold,
            "align" => Icons.ArrowClockwise,
            "bring" => Icons.ArrowDown,
            "collapse" => Icons.ArrowsInLineVertical,
            "cut" => Icons.Scissors,
            "retrack" => Icons.ArrowsClockwise,
            "sync" => Icons.ArrowsClockwise,
            "track" => Icons.Record,
            "tweak" => Icons.NotePencil,
            _ => Icons.File,
        };

    /// <summary>What one change's state is called, or nothing when the state says nothing of its own.</summary>
    /// <param name="state">The state the journal recorded.</param>
    /// <returns>What to write at the row's right, or nothing.</returns>
    public static string? State(string state) =>
        state switch
        {
            "added" => RolaI18N.Get("rorolala_vcs.workspace.state_added"),
            "removed" => RolaI18N.Get("rorolala_vcs.workspace.state_removed"),
            "changed" => RolaI18N.Get("rorolala_vcs.workspace.state_changed"),
            "bound" => RolaI18N.Get("rorolala_vcs.workspace.state_bound"),
            "unbound" => RolaI18N.Get("rorolala_vcs.workspace.state_unbound"),
            "moved" => RolaI18N.Get("rorolala_vcs.workspace.state_moved"),
            "reidentified" => RolaI18N.Get("rorolala_vcs.workspace.state_reidentified"),
            _ => null,
        };

    /// <summary>How long ago a recorded time was, as a row says it.</summary>
    /// <remarks>
    /// Days are counted by the calendar rather than by elapsed hours, so a run at eleven last night
    /// is yesterday at one in the morning rather than two hours ago. A stamp this side cannot read
    /// is answered as it was written.
    /// </remarks>
    /// <param name="time">The record's UTC stamp.</param>
    /// <param name="now">The moment it is read at.</param>
    /// <returns>What to show in the row.</returns>
    public static string When(string time, DateTimeOffset now)
    {
        ArgumentNullException.ThrowIfNull(time);

        if (!Moment(time, out var at))
        {
            return time;
        }

        var elapsed = now - at;

        if (elapsed < TimeSpan.Zero)
        {
            elapsed = TimeSpan.Zero;
        }

        var days = (now.UtcDateTime.Date - at.UtcDateTime.Date).Days;

        if (days > 0)
        {
            return days == 1
                ? RolaI18N.Get("rorolala_vcs.workspace.when_yesterday")
                : days < 7
                    ? RolaI18N.Get("rorolala_vcs.workspace.when_days", days)
                    : RolaI18N.Get("rorolala_vcs.workspace.when_weeks", days / 7);
        }

        if (elapsed < TimeSpan.FromMinutes(1))
        {
            return RolaI18N.Get("rorolala_vcs.workspace.when_now");
        }

        return elapsed < TimeSpan.FromHours(1)
            ? RolaI18N.Get("rorolala_vcs.workspace.when_minutes", (int)elapsed.TotalMinutes)
            : RolaI18N.Get("rorolala_vcs.workspace.when_hours", (int)elapsed.TotalHours);
    }

    /// <summary>The exact moment a record holds, for the tooltip of its row.</summary>
    /// <param name="time">The record's UTC stamp.</param>
    /// <returns>The moment as UTC, or the stamp as it was written when it does not read.</returns>
    public static string Exact(string time)
    {
        ArgumentNullException.ThrowIfNull(time);

        return Moment(time, out var at)
            ? at.ToUniversalTime()
                .ToString("d MMM yyyy, HH:mm 'UTC'", CultureInfo.InvariantCulture)
            : time;
    }

    /// <summary>Reads a record's UTC stamp.</summary>
    /// <param name="time">The stamp, as the journal wrote it.</param>
    /// <param name="at">The moment it names.</param>
    /// <returns>Whether the stamp read.</returns>
    private static bool Moment(string time, out DateTimeOffset at) =>
        DateTimeOffset.TryParse(
            time,
            CultureInfo.InvariantCulture,
            DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal,
            out at
        );
}

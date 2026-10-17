using System.Diagnostics;
using System.Text;
using System.Text.Json;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

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
    internal const string Theme = "desktop-friendly";

    /// <summary>The environment variable naming the program the window was opened by.</summary>
    internal const string Program = "ROLA_EXE";

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
        ProgressRecords records,
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
    internal static (bool Done, string Said, string Output) Asked(
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

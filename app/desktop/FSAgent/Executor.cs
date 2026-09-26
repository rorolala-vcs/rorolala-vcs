using System.Diagnostics;
using System.Text.Json.Serialization;

namespace RorolalaFSAgent;

/// <summary>What happened to one item, as the JSON states it.</summary>
internal sealed class Result
{
    /// <summary>The source path the item named.</summary>
    [JsonPropertyName("from")]
    public required string From { get; init; }

    /// <summary>The target the command ran against, or empty when none did.</summary>
    [JsonPropertyName("to")]
    public required string To { get; init; }

    /// <summary>One of <c>done</c>, <c>skipped</c> or <c>failed</c>.</summary>
    [JsonPropertyName("result")]
    public required string Outcome { get; init; }

    /// <summary>One of <c>as-is</c>, <c>replaced</c>, <c>renamed</c>, <c>skipped</c> or <c>failed</c>.</summary>
    [JsonPropertyName("how")]
    public required string How { get; init; }

    /// <summary>Why an item failed, and nothing otherwise.</summary>
    [JsonPropertyName("note")]
    public string? Note { get; init; }

    /// <summary>The item ran and the command exited cleanly.</summary>
    /// <param name="from">The source path.</param>
    /// <param name="to">The target the command ran against.</param>
    /// <param name="how">Whether it ran as-is, after a replace, or under a new name.</param>
    public static Result Done(string from, string to, string how) =>
        new()
        {
            From = from,
            To = to,
            Outcome = "done",
            How = how,
        };

    /// <summary>Nothing ran for the item.</summary>
    /// <param name="from">The source path.</param>
    public static Result Skipped(string from) =>
        new()
        {
            From = from,
            To = string.Empty,
            Outcome = "skipped",
            How = "skipped",
        };

    /// <summary>The item could not be run, or the command it ran did not finish cleanly.</summary>
    /// <param name="from">The source path.</param>
    /// <param name="to">The target, when one was resolved.</param>
    /// <param name="note">The reason.</param>
    public static Result Failed(string from, string to, string note) =>
        new()
        {
            From = from,
            To = to,
            Outcome = "failed",
            How = "failed",
            Note = note,
        };
}

/// <summary>
/// Runs the command over every item, one at a time, and records what happened.
/// </summary>
/// <remarks>
/// Nothing here asks a question: every decision was made before this runs, so a called-off run is
/// one where no item runs and every item reads <c>skipped</c>.
/// </remarks>
internal static class Executor
{
    /// <summary>Runs every item and returns one result for each, in order.</summary>
    /// <param name="plan">The items and their decisions.</param>
    /// <param name="program">The program to start, then its fixed arguments.</param>
    public static IReadOnlyList<Result> Run(Plan plan, IReadOnlyList<string> program)
    {
        var results = new List<Result>(plan.Items.Count);

        foreach (var item in plan.Items)
        {
            results.Add(Run(plan, program, item));
        }

        return results;
    }

    /// <summary>Runs one item and returns what happened to it.</summary>
    private static Result Run(Plan plan, IReadOnlyList<string> program, Item item)
    {
        if (plan.Cancelled)
        {
            return Result.Skipped(item.From);
        }

        if (item.Problem is { } problem)
        {
            return Result.Failed(item.From, item.To, problem);
        }

        // An unanswered conflict must never run. A conflict means the command would land on something already
        // there, so running it is how that something is lost — and arriving here undecided means the window
        // that should have asked never answered, which is a failure to report rather than a choice to make.
        if (item.Conflict && item.Resolution == Resolution.Undecided)
        {
            return Result.Failed(item.From, item.To, "the conflict was never answered");
        }

        if (item.Resolution == Resolution.Skip)
        {
            return Result.Skipped(item.From);
        }

        var target = item.To;

        try
        {
            switch (item.Resolution)
            {
                case Resolution.Replace:
                    Replace(target);
                    break;
                case Resolution.Rename:
                    target = FreeName(target);
                    break;
            }
        }
        catch (Exception error)
        {
            return Result.Failed(
                item.From,
                target,
                $"the existing target could not be replaced: {error.Message}"
            );
        }

        // A transfer runs over the source and its target; a removal runs over the source alone,
        // which is why its target is empty.
        var paths = target.Length == 0 ? new[] { item.From } : new[] { item.From, target };
        var failure = Launch(program, paths);

        return failure is null
            ? Result.Done(item.From, target, How(item.Resolution))
            : Result.Failed(item.From, target, failure);
    }

    /// <summary>Removes what is already at a target, so the command can take its place.</summary>
    private static void Replace(string target)
    {
        if (Directory.Exists(target))
        {
            Directory.Delete(target, recursive: true);
            return;
        }

        File.Delete(target);
    }

    /// <summary>
    /// A free name beside an existing target, by the <c>name (2)</c>, <c>name (3)</c> … scheme.
    /// </summary>
    /// <remarks>
    /// The number is added to the whole file name, extension included, which is what the scheme as
    /// written says; a caller wanting the extension kept at the end would split it off here.
    /// </remarks>
    private static string FreeName(string target)
    {
        var directory = Path.GetDirectoryName(target) ?? string.Empty;
        var name = Path.GetFileName(target);

        for (var number = 2; ; number++)
        {
            var candidate = Path.Combine(directory, $"{name} ({number})");

            if (!Pathing.Exists(candidate))
            {
                return candidate;
            }
        }
    }

    /// <summary>The word for how an item was resolved.</summary>
    private static string How(Resolution resolution) =>
        resolution switch
        {
            Resolution.Replace => "replaced",
            Resolution.Rename => "renamed",
            _ => "as-is",
        };

    /// <summary>
    /// Starts the command over the paths and waits for it, returning its failure or nothing.
    /// </summary>
    /// <remarks>
    /// The program is started directly, never through a shell, and every path is one argument of
    /// its own: a shell would read a path as a command, and a path with a space in it would be read
    /// as two arguments if it were joined by hand. Both are avoided by handing the arguments over
    /// as they are.
    /// <para>
    /// A child's standard output is not this program's: the contract is the one JSON line, so what
    /// a child prints is passed on to standard error, where everything else that is not the answer
    /// goes, and never onto standard output.
    /// </para>
    /// </remarks>
    private static string? Launch(IReadOnlyList<string> program, IReadOnlyList<string> paths)
    {
        var start = new ProcessStartInfo
        {
            FileName = program[0],
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };

        for (var index = 1; index < program.Count; index++)
        {
            start.ArgumentList.Add(program[index]);
        }

        foreach (var path in paths)
        {
            start.ArgumentList.Add(path);
        }

        Process process;

        try
        {
            process = Process.Start(start)
                ?? throw new InvalidOperationException("the process was not started");
        }
        catch (Exception error)
        {
            return $"`{program[0]}` could not be started: {error.Message}";
        }

        using (process)
        {
            // Both streams are drained on the thread pool, so a child filling one pipe while this
            // waits on the other cannot block against itself.
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());
            var error = Task.Run(() => process.StandardError.ReadToEnd());

            process.WaitForExit();

            var standardOutput = output.GetAwaiter().GetResult();
            var standardError = error.GetAwaiter().GetResult();

            if (standardOutput.Length > 0)
            {
                Console.Error.Write(standardOutput);
            }

            if (standardError.Length > 0)
            {
                Console.Error.Write(standardError);
            }

            if (process.ExitCode == 0)
            {
                return null;
            }

            var detail = standardError.Trim();

            return detail.Length > 0
                ? $"`{program[0]}` exited with code {process.ExitCode}: {detail}"
                : $"`{program[0]}` exited with code {process.ExitCode}";
        }
    }
}

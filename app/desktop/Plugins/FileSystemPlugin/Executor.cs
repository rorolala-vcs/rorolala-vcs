using System.Diagnostics;

namespace FileSystemPlugin;

/// <summary>A command that did not finish cleanly, and what it said.</summary>
/// <remarks>
/// The command is another program's, so what went wrong is its to say: what is kept here is the
/// program, the status it left and its own error stream — not a sentence about them — so that a
/// caller can say why in its own words and in the reader's language.
/// </remarks>
/// <param name="Program">The program that was started.</param>
/// <param name="ExitCode">What it exited with, or nothing when it never started.</param>
/// <param name="Detail">What it said on its error stream, or why it could not be started.</param>
internal sealed record CommandFailure(string Program, int? ExitCode, string Detail)
{
    /// <summary>The reason, as one line.</summary>
    public string Note =>
        ExitCode is { } code
            ? Detail.Length > 0
                ? $"`{Program}` exited with code {code}: {Detail}"
                : $"`{Program}` exited with code {code}"
            : $"`{Program}` could not be started: {Detail}";
}

/// <summary>What happened to one item.</summary>
internal sealed class Result
{
    /// <summary>The source path the item named.</summary>
    public required string From { get; init; }

    /// <summary>The target the command ran against, or empty when none did.</summary>
    public required string To { get; init; }

    /// <summary>One of <c>done</c>, <c>skipped</c> or <c>failed</c>.</summary>
    public required string Outcome { get; init; }

    /// <summary>One of <c>as-is</c>, <c>replaced</c>, <c>renamed</c>, <c>skipped</c> or <c>failed</c>.</summary>
    public required string How { get; init; }

    /// <summary>Why an item failed, and nothing otherwise.</summary>
    public string? Note { get; init; }

    /// <summary>
    /// The command that failed, when one did and it said why.
    /// </summary>
    public CommandFailure? Failure { get; init; }

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
    /// <param name="failure">The command that failed, when one ran and said why.</param>
    public static Result Failed(string from, string to, string note, CommandFailure? failure = null) =>
        new()
        {
            From = from,
            To = to,
            Outcome = "failed",
            How = "failed",
            Note = note,
            Failure = failure,
        };
}

/// <summary>
/// Runs the command over every item, one at a time, and records what happened.
/// </summary>
/// <remarks>
/// Nothing here asks a question: every decision was made before this runs, so a called-off run is
/// one where no item runs and every item reads <c>skipped</c>.
/// <para>
/// The command is a program of its own — the file operations themselves are the command line's, not
/// this program's — so this blocks while each one runs and is meant to be called off the window's thread.
/// </para>
/// </remarks>
internal static class Executor
{
    /// <summary>The word a command template puts a source path at.</summary>
    public const string From = "{{from}}";

    /// <summary>The word a command template puts a target path at.</summary>
    public const string To = "{{to}}";

    /// <summary>What a command template names an environment variable with, closed by <c>}}</c>.</summary>
    /// <remarks>
    /// A variable is named rather than a path written out, because the path is not the same from one machine
    /// to the next: whoever starts the Desktop says where <c>rola</c> is, and the template asks for it by name
    /// rather than trusting a <c>PATH</c> that may hold another program of the same name.
    /// <para>
    /// A variable is a token of its own rather than something written into the middle of one, exactly as the
    /// two paths are: what a variable holds is one argument, so a program under a directory with a space in it
    /// stays one word. A template that wrote one into the middle of a word would be a template whose arguments
    /// depend on what the machine happens to hold.
    /// </para>
    /// </remarks>
    private const string VariablePrefix = "{{env:";

    /// <summary>What a named environment variable is closed with.</summary>
    private const string VariableSuffix = "}}";

    /// <summary>
    /// Why a command template cannot be run at all, or nothing when it can.
    /// </summary>
    /// <remarks>
    /// Checked once for the whole call rather than per item, because the reason names the command rather
    /// than an item. A placeholder is never the program, so a template that opens with one names no
    /// program; a transfer must name both paths, and a removal names only a source — a target it named
    /// would have nothing to put there.
    /// <para>
    /// A variable the template names is read here rather than where the command runs, so that a machine
    /// without it is a reason given before anything is planned rather than an argument that arrives empty.
    /// </para>
    /// </remarks>
    /// <param name="operation">What is being done to every item.</param>
    /// <param name="template">The command template, with the placeholders in it.</param>
    /// <returns>The reason it cannot run, or nothing when it can.</returns>
    public static string? Validate(Operation operation, string template)
    {
        var tokens = Tokens(template);

        if (tokens.Length == 0 || IsPlaceholder(tokens[0]))
        {
            return $"the command `{template}` names no program";
        }

        foreach (var token in tokens)
        {
            if (Named(token) is { } name && string.IsNullOrEmpty(Environment.GetEnvironmentVariable(name)))
            {
                return $"the command `{template}` names {token}, and no environment variable of that name is set";
            }
        }

        if (tokens.Contains(From) is false)
        {
            return $"the command `{template}` must name {From}, or it is given nothing to work on";
        }

        if (operation is Operation.RemoveDirs or Operation.RemoveFiles)
        {
            return tokens.Contains(To) ? $"the command `{template}` names {To}, but a removal has no target" : null;
        }

        return tokens.Contains(To) ? null : $"the command `{template}` must name both {From} and {To}";
    }

    /// <summary>Runs every item and returns one result for each, in order.</summary>
    /// <param name="plan">The items and their decisions.</param>
    /// <param name="template">The command template, with the placeholders in it.</param>
    public static IReadOnlyList<Result> Run(Plan plan, string template)
    {
        var tokens = Tokens(template);
        var results = new List<Result>(plan.Items.Count);

        foreach (var item in plan.Items)
        {
            results.Add(Run(plan, tokens, item));
        }

        return results;
    }

    /// <summary>Runs one item and returns what happened to it.</summary>
    private static Result Run(Plan plan, IReadOnlyList<string> tokens, Item item)
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

        // Replacing what is being copied is removing the source: the target of an in-place copy is the very
        // entry being copied, so a replace would take it away and leave nothing to copy. The window does not
        // offer it for such an item, but "the same for the rest" can carry one answer onto every remaining
        // conflict, which is why it is refused here as well.
        if (item.OntoItself && item.Resolution == Resolution.Replace)
        {
            return Result.Failed(
                item.From,
                item.To,
                "replacing what is being copied would remove the source"
            );
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

        // A transfer runs over the source and its target; a removal runs over the source alone, which is
        // why its target is empty — and why a removal template that named the target was refused before.
        var failure = Launch(Arguments(tokens, item.From, target));

        return failure is null
            ? Result.Done(item.From, target, How(item.Resolution))
            : Result.Failed(item.From, target, failure.Note, failure);
    }

    /// <summary>The template's tokens with the placeholders replaced by what they stand for.</summary>
    /// <remarks>
    /// A placeholder is a token of its own, never part of one, so a path with a space in it stays one
    /// argument exactly as it would be handed over were it appended — and so does what an environment
    /// variable holds. An item with no target — every removal — never reaches a <c>{{to}}</c>, which its
    /// template was refused for naming.
    /// </remarks>
    /// <param name="tokens">The template, split into tokens.</param>
    /// <param name="from">The item's source path.</param>
    /// <param name="to">The item's target path, or empty when it has none.</param>
    internal static List<string> Arguments(IReadOnlyList<string> tokens, string from, string to)
    {
        var arguments = new List<string>(tokens.Count);

        foreach (var token in tokens)
        {
            arguments.Add(
                token switch
                {
                    From => from,
                    To => to,
                    _ => Named(token) is { } name
                        ? Environment.GetEnvironmentVariable(name) ?? string.Empty
                        : token,
                }
            );
        }

        return arguments;
    }

    /// <summary>The environment variable a token names, or nothing when it names none.</summary>
    /// <remarks>
    /// A token names a variable or says nothing about one: a name that is empty is a variable named by
    /// nothing rather than a word to keep, which is what <c>{{env:}}</c> being read here as the empty name
    /// means — it is refused as unset, which says what is wrong with it.
    /// </remarks>
    /// <param name="token">The token to read.</param>
    /// <returns>The variable's name, or nothing.</returns>
    private static string? Named(string token) =>
        token.StartsWith(VariablePrefix, StringComparison.Ordinal)
        && token.EndsWith(VariableSuffix, StringComparison.Ordinal)
            ? token[VariablePrefix.Length..^VariableSuffix.Length]
            : null;

    /// <summary>A template split into tokens on whitespace, empty ones dropped.</summary>
    private static string[] Tokens(string template) =>
        template.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);

    /// <summary>Whether a token is one of the placeholders rather than a word to keep.</summary>
    private static bool IsPlaceholder(string token) => token is From or To;

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
    /// Starts the command with the arguments already expanded and waits for it, returning what it
    /// failed with or nothing.
    /// </summary>
    /// <remarks>
    /// Every argument is handed over as it is, so a path with a space in it is one argument: joining the
    /// line by hand and letting the platform split it again is how such a path becomes two. The first
    /// argument is the program, and what a template names after it — including a shell, as the Windows
    /// commands do with <c>cmd /c</c> — is up to the user who wrote the template.
    /// <para>
    /// Both of the child's streams are drained while it runs, so a child that fills one pipe while this
    /// waits on the other cannot block against itself; what it said is kept — the status and its own
    /// error stream, apart — so a caller can say why, in its own words and where it wants them said.
    /// </para>
    /// </remarks>
    private static CommandFailure? Launch(IReadOnlyList<string> arguments)
    {
        var start = new ProcessStartInfo
        {
            FileName = arguments[0],
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };

        for (var index = 1; index < arguments.Count; index++)
        {
            start.ArgumentList.Add(arguments[index]);
        }

        Process process;

        try
        {
            process = Process.Start(start)
                ?? throw new InvalidOperationException("the process was not started");
        }
        catch (Exception error)
        {
            return new CommandFailure(arguments[0], null, error.Message);
        }

        using (process)
        {
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());
            var error = Task.Run(() => process.StandardError.ReadToEnd());

            process.WaitForExit();

            var standardOutput = output.GetAwaiter().GetResult();
            var standardError = error.GetAwaiter().GetResult();

            if (process.ExitCode == 0)
            {
                return null;
            }

            var detail = standardError.Trim();

            if (detail.Length == 0)
            {
                detail = standardOutput.Trim();
            }

            return new CommandFailure(arguments[0], process.ExitCode, detail);
        }
    }
}

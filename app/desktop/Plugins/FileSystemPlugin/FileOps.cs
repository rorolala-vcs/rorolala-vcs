using System.Diagnostics;
using System.Text.Json;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>
/// Every file operation the plugin performs, done by the file agent rather than here.
/// </summary>
/// <remarks>
/// The work is not this program's: the agent is a program of its own, started with the operation, the
/// command that carries it out, and the items, and it is the agent that settles what happens where a name
/// is already taken — which is a question only a person can answer, and one this program has no window of
/// its own to ask in.
/// <para>
/// What is here is therefore only the invocation and the reading of the answer: a batch goes over as
/// <c>-Pairs</c> (or one at a time as <c>-From</c>/<c>-To</c> where a path would be broken by the pair
/// separators), and what comes back is one JSON line naming what became of every item.
/// </para>
/// <para>
/// The agent is reached through this plugin and nowhere else, which is why it lives inside the plugin's own
/// directory beside it.
/// </para>
/// </remarks>
internal static class FileOps
{
    /// <summary>
    /// A path without the separator a directory's may carry at its end.
    /// </summary>
    /// <remarks>
    /// This exists because of one bug that is worth not having again: a path that came from a drag is read
    /// from a URI, and a directory's URI ends in a separator — so the name taken from it is the empty string,
    /// a "directory" of no name is the directory it sits in, that directory always exists, and the entry was
    /// therefore renamed instead of arriving under its own name. Bare the path before asking anything of it.
    /// </remarks>
    /// <param name="path">The path to bare.</param>
    /// <returns>The path without trailing separators, or the path itself when it is one all the way.</returns>
    public static string Bare(string path)
    {
        var trimmed = path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        return trimmed.Length == 0 ? path : trimmed;
    }

    /// <summary>Copies sources into a directory, through the agent.</summary>
    /// <param name="sources">What to copy.</param>
    /// <param name="into">The directory to put them in.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was copied.</returns>
    public static Task<bool> Copy(IReadOnlyList<string> sources, string into, Action<string> failed) =>
        Transfer("Copy", "cp -r", sources, into, failed);

    /// <summary>Moves sources into a directory, through the agent.</summary>
    /// <param name="sources">What to move.</param>
    /// <param name="into">The directory to put them in.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was moved.</returns>
    public static Task<bool> Move(IReadOnlyList<string> sources, string into, Action<string> failed) =>
        Transfer("Move", "mv", sources, into, failed);

    /// <summary>
    /// Removes entries, through the agent.
    /// </summary>
    /// <remarks>
    /// Directories and files are one call each, because the operation names which of the two it is removing
    /// — the command that removes a file does not remove a directory, and the two are told apart here so that
    /// the agent need not be told twice about the same batch.
    /// </remarks>
    /// <param name="entries">What to remove.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was removed.</returns>
    public static async Task<bool> Remove(IReadOnlyList<Entry> entries, Action<string> failed)
    {
        var directories = entries.Where(entry => entry.Kind == EntryKind.Directory).Select(entry => entry.Path).ToArray();
        var files = entries.Where(entry => entry.Kind == EntryKind.File).Select(entry => entry.Path).ToArray();
        var removed = false;

        if (directories.Length > 0)
        {
            removed |= await Without("RemoveDirs", "rm -rf", directories, failed);
        }

        if (files.Length > 0)
        {
            removed |= await Without("RemoveFiles", "rm", files, failed);
        }

        return removed;
    }

    /// <summary>
    /// One transfer of a batch into a directory.
    /// </summary>
    /// <remarks>
    /// A batch goes over as one answer, so that the agent can offer "the same for the rest" once rather than
    /// asking per item. Paths that would be broken by the pair separators go over one at a time instead, which
    /// costs a question per item but always names the item it means.
    /// </remarks>
    /// <param name="operation">The operation the agent is told.</param>
    /// <param name="command">The program and its arguments that carry it out.</param>
    /// <param name="sources">What is being transferred.</param>
    /// <param name="into">The directory they go into.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static async Task<bool> Transfer(
        string operation,
        string command,
        IReadOnlyList<string> sources,
        string into,
        Action<string> failed
    )
    {
        if (sources.Count == 0)
        {
            return false;
        }

        if (sources.Any(Breaks) || Breaks(into))
        {
            var lone = false;

            foreach (var source in sources)
            {
                lone |= await Ask(operation, command, ["-From:" + source, "-To:" + into], failed);
            }

            return lone;
        }

        return await Ask(operation, command, ["-Pairs:" + string.Join(';', sources.Select(source => $"{source}>{into}"))], failed);
    }

    /// <summary>One removal of a batch.</summary>
    /// <param name="operation">The operation the agent is told.</param>
    /// <param name="command">The program and its arguments that carry it out.</param>
    /// <param name="paths">What is being removed.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static Task<bool> Without(string operation, string command, IReadOnlyList<string> paths, Action<string> failed) =>
        paths.Any(Breaks)
            ? OneByOne(operation, command, paths, failed)
            : Ask(operation, command, ["-Pairs:" + string.Join(';', paths)], failed);

    /// <summary>Runs one item at a time, for the paths a batch would not carry.</summary>
    /// <param name="operation">The operation the agent is told.</param>
    /// <param name="command">The program and its arguments that carry it out.</param>
    /// <param name="paths">What is being removed.</param>
    /// <param name="failed">Where a failure is reported.</param>
    private static async Task<bool> OneByOne(
        string operation,
        string command,
        IReadOnlyList<string> paths,
        Action<string> failed
    )
    {
        var removed = false;

        foreach (var path in paths)
        {
            removed |= await Ask(operation, command, ["-From:" + path], failed);
        }

        return removed;
    }

    /// <summary>
    /// Whether a path would be broken by the separators a batch is written with.
    /// </summary>
    /// <remarks>
    /// The batch form packs the items into one string, so a path carrying the character that separates items
    /// or the one that separates a source from its destination would be read as two things or as the wrong
    /// thing. It is rare and it is not fatal: such an item goes over on its own, where nothing is split.
    /// </remarks>
    /// <param name="path">The path to ask about.</param>
    private static bool Breaks(string path) =>
        path.Contains(';', StringComparison.Ordinal) || path.Contains('>', StringComparison.Ordinal);

    /// <summary>Starts the agent for one call and reads what it says became of every item.</summary>
    /// <param name="operation">The operation the agent is told.</param>
    /// <param name="command">The program and its arguments that carry it out.</param>
    /// <param name="items">What is being done, as the agent's own arguments.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was done.</returns>
    private static async Task<bool> Ask(string operation, string command, string[] items, Action<string> failed)
    {
        var agent = Agent();

        if (!File.Exists(agent))
        {
            failed($"the file agent is not beside the plugin: {agent}");

            return false;
        }

        var start = new ProcessStartInfo(agent)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };

        start.ArgumentList.Add("-Command:" + command);
        start.ArgumentList.Add("-Type:" + operation);

        if (RolaI18N.Locale is { Length: > 0 } locale)
        {
            start.ArgumentList.Add("-Lang:" + locale);
        }

        foreach (var item in items)
        {
            start.ArgumentList.Add(item);
        }

        try
        {
            using var process = Process.Start(start);

            if (process is null)
            {
                failed($"could not start the file agent: {agent}");

                return false;
            }

            // Both are read before either is waited for: a pipe that fills while nobody reads it stops the
            // agent writing, and the agent stopped writing is the agent never exiting.
            var said = process.StandardOutput.ReadToEndAsync();
            var complained = process.StandardError.ReadToEndAsync();

            await process.WaitForExitAsync();

            var answer = await said;
            var noise = await complained;

            if (noise.Length > 0)
            {
                failed(noise.TrimEnd());
            }

            return Read(answer, failed);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            failed(error.Message);

            return false;
        }
    }

    /// <summary>
    /// Reads the agent's answer: the one JSON line it says became of every item.
    /// </summary>
    /// <remarks>
    /// The last non-empty line is the answer, so that anything the agent had to say on the way can pass
    /// through without being mistaken for it. An item that failed is reported as it was named; an item that
    /// was skipped is not, because skipping is an answer and not a fault.
    /// </remarks>
    /// <param name="answer">What the agent wrote to standard output.</param>
    /// <param name="failed">Where a failure is reported.</param>
    /// <returns>Whether anything was done.</returns>
    private static bool Read(string answer, Action<string> failed)
    {
        var line = answer
            .Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .LastOrDefault();

        if (line is null)
        {
            failed("the file agent answered with nothing");

            return false;
        }

        Report? report;

        try
        {
            report = JsonSerializer.Deserialize<Report>(line, Json);
        }
        catch (JsonException error)
        {
            failed($"the file agent's answer could not be read: {error.Message}");

            return false;
        }

        var done = false;

        foreach (var result in report?.Results ?? [])
        {
            if (result.Result == "failed")
            {
                failed(result.Note ?? $"{result.From} could not be {result.How}");

                continue;
            }

            done |= result.Result == "done";
        }

        return done;
    }

    /// <summary>How the JSON the agent answers with is read: its names are lower case, this program's are not.</summary>
    private static readonly JsonSerializerOptions Json = new() { PropertyNameCaseInsensitive = true };

    /// <summary>What became of every item, as the agent reports it.</summary>
    /// <param name="Results">One answer per item, in the order they were given.</param>
    private sealed record Report(Item[] Results);

    /// <summary>What became of one item.</summary>
    /// <param name="From">The item, as it was given.</param>
    /// <param name="To">Where it ended up, or empty when it did not move.</param>
    /// <param name="Result">`done`, `skipped` or `failed`.</param>
    /// <param name="How">`as-is`, `replaced`, `renamed`, `skipped` or `failed`.</param>
    /// <param name="Note">Why it failed, when it did.</param>
    private sealed record Item(string From, string To, string Result, string How, string? Note);

    /// <summary>
    /// Where the file agent is: inside this plugin's own directory, which is where a plugin's own things are
    /// laid because nothing else lays them.
    /// </summary>
    private static string Agent()
    {
        var beside = Path.GetDirectoryName(typeof(FileOps).Assembly.Location)!;
        var name = OperatingSystem.IsWindows() ? "rola-desktop-fs-agent.exe" : "rola-desktop-fs-agent";

        return Path.Combine(beside, "FileSystemPlugin", "RorolalaFSAgent", name);
    }
}

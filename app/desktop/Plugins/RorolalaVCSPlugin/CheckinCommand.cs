using System.Text.Json;

namespace RorolalaVCSPlugin;

/// <summary>One entry a checkin brought in.</summary>
/// <param name="Uuid">The uuid it is known by.</param>
/// <param name="RemotePath">The path the Vault's Layout names it by, when it names one.</param>
/// <param name="LocalPath">The path this Layout now names it by.</param>
internal sealed record CheckinLanded(string Uuid, string? RemotePath, string LocalPath);

/// <summary>What a checkin came to.</summary>
/// <remarks>
/// The entries and the failures are both read from the result rather than from the status: a run given a
/// structural renderer has its result serialized, so the renderer that would have set a non-zero status for what
/// it could not bring in is never reached. A run that brought nothing in answers with an empty list either way.
/// </remarks>
/// <param name="Done">Whether the run ended well, which a run that brought some in and failed some still does.</param>
/// <param name="Said">What it said when it did not end well.</param>
/// <param name="Landed">What it brought in.</param>
/// <param name="Failed">What it could not bring in, one line each.</param>
internal sealed record CheckinAnswer(
    bool Done,
    string Said,
    IReadOnlyList<CheckinLanded> Landed,
    IReadOnlyList<string> Failed
);

/// <summary>
/// Running <c>rola checkin</c>: bringing what the Vault holds into the Layout being worked in.
/// </summary>
/// <remarks>
/// The references are paths of the Vault's Layout, so what a run brings in is what the tree showed at the path
/// the reader chose — an entry upstream has moved since is refused rather than brought from somewhere else.
/// A path that reads as a pattern is a path the command line would take for one, so a reference holding a
/// character a glob reads is passed as its uuid instead: what lands is the same either way, and the uuid cannot
/// be read as anything but the entry.
/// <para>
/// A run fetches the Vault's Layout itself before it resolves anything, which is why the place reads again whatever
/// became of the run: the copy the tree was drawn from may have been replaced under it.
/// </para>
/// </remarks>
internal static class CheckinCommand
{
    /// <summary>The characters a reference is read as a pattern for.</summary>
    /// <remarks>
    /// The command line reads a reference with any of these as a glob over the Vault's Layout paths, so a path
    /// holding one is not a path to it. A space or a quote is nothing here: the operands cross as one argument
    /// each, so ordinary names need no quoting.
    /// </remarks>
    private const string PatternCharacters = "*?[{";

    /// <summary>The arguments a checkin of these references is run with, after the program's name.</summary>
    /// <param name="references">What is to be brought in.</param>
    /// <param name="to">The directory to land them under, or nothing to land them at their upstream paths.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(IReadOnlyList<string> references, string? to = null)
    {
        ArgumentNullException.ThrowIfNull(references);

        List<string> arguments = ["--theme-choice=" + Sync.Theme, "--json", "checkin", .. references];

        // The destination is the last operand, which is how the command line reads them: every word but the last is a
        // reference, and a run with one word has none. It is a path of this run's rather than of the Vault's, which is
        // why it is handed over as an absolute one — what it is resolved against is where the run is made.
        if (to is { Length: > 0 })
        {
            arguments.Add(to);
        }

        return arguments;
    }

    /// <summary>
    /// What one of the chosen entries is named by on the command line: its path, or its uuid when the path reads
    /// as a pattern.
    /// </summary>
    /// <param name="entry">The entry.</param>
    /// <returns>The reference to give the command.</returns>
    public static string Reference(LayoutEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return entry.Path.AsSpan().IndexOfAny(PatternCharacters) < 0 ? entry.Path : entry.Uuid;
    }

    /// <summary>Runs the checkin and reads what it answered.</summary>
    /// <param name="program">The program the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="references">What is to be brought in.</param>
    /// <param name="records">What the run's records are read into, which the panel draws.</param>
    /// <param name="progress">What is told that a record was said, which redraws on the window's thread.</param>
    /// <param name="to">The directory to land them under, or nothing to land them at their upstream paths.</param>
    /// <returns>What the run came to.</returns>
    public static CheckinAnswer Run(
        string program,
        string directory,
        IReadOnlyList<string> references,
        ProgressRecords records,
        Action progress,
        string? to = null
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(references);
        ArgumentNullException.ThrowIfNull(records);
        ArgumentNullException.ThrowIfNull(progress);

        var (done, said, output) = CommandLine.Asked(
            program,
            Arguments(references, to),
            directory,
            records,
            progress
        );

        if (!done)
        {
            return new CheckinAnswer(false, said, [], []);
        }

        try
        {
            return Parse(output);
        }
        catch (JsonException error)
        {
            return new CheckinAnswer(false, error.Message, [], []);
        }
    }

    /// <summary>Reads the result a checkin answered with.</summary>
    /// <param name="output">What it wrote, which is one JSON object when it was asked for one.</param>
    /// <returns>What it brought in, and what it could not.</returns>
    /// <exception cref="JsonException">The output is not JSON.</exception>
    internal static CheckinAnswer Parse(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        using var document = JsonDocument.Parse(output);

        var landed = new List<CheckinLanded>();

        if (
            document.RootElement.ValueKind is JsonValueKind.Object
            && document.RootElement.TryGetProperty("entries", out var entries)
            && entries.ValueKind is JsonValueKind.Array
        )
        {
            foreach (var item in entries.EnumerateArray())
            {
                if (
                    item.ValueKind is not JsonValueKind.Object
                    || Text(item, "uuid") is not { Length: > 0 } uuid
                    || Text(item, "local_path") is not { Length: > 0 } local
                )
                {
                    continue;
                }

                landed.Add(new CheckinLanded(uuid, Text(item, "remote_path"), local));
            }
        }

        return new CheckinAnswer(true, string.Empty, landed, Failed(document.RootElement));
    }

    /// <summary>Reads what a successful result says could not be brought in.</summary>
    /// <remarks>
    /// The same reading the ownership commands' answers take: with a structural renderer the refusals are a list
    /// in the result, and the status alone would call a run that brought one of three in a success.
    /// </remarks>
    /// <param name="answer">The result.</param>
    /// <returns>The lines, or nothing when there were none.</returns>
    private static IReadOnlyList<string> Failed(JsonElement answer) =>
        answer.ValueKind is JsonValueKind.Object
        && answer.TryGetProperty("failed", out var failed)
        && failed.ValueKind is JsonValueKind.Array
            ?
            [
                .. failed
                    .EnumerateArray()
                    .Where(item => item.ValueKind is JsonValueKind.String)
                    .Select(item => item.GetString())
                    .Where(line => line is { Length: > 0 })
                    .Select(line => line!),
            ]
            : [];

    /// <summary>The string a member of an object holds, or nothing when it holds none.</summary>
    /// <param name="item">The object.</param>
    /// <param name="name">The member's name.</param>
    /// <returns>The text, or nothing.</returns>
    private static string? Text(JsonElement item, string name) =>
        item.ValueKind is JsonValueKind.Object
        && item.TryGetProperty(name, out var value)
        && value.ValueKind is JsonValueKind.String
            ? value.GetString()
            : null;
}

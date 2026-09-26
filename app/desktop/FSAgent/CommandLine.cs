namespace RorolalaFSAgent;

/// <summary>What is being done to each item.</summary>
internal enum Operation
{
    /// <summary>Copy a source to a destination directory.</summary>
    Copy,

    /// <summary>Move a source to a destination directory.</summary>
    Move,

    /// <summary>Remove directories.</summary>
    RemoveDirs,

    /// <summary>Remove files.</summary>
    RemoveFiles,
}

/// <summary>One source and, for a transfer, the destination directory it goes to.</summary>
/// <param name="From">The source path, as it was given.</param>
/// <param name="To">
/// The destination directory, or empty when the item names none — which is every removal, and a
/// transfer whose entry was malformed.
/// </param>
internal readonly record struct Pair(string From, string To);

/// <summary>The command line, once it is known to name a run.</summary>
/// <param name="Operation">What is being done to every item.</param>
/// <param name="Program">The program to start, then its fixed arguments.</param>
/// <param name="Pairs">The items, in the order they were given.</param>
/// <param name="Lang">The locale to speak, defaulting to <c>en</c>.</param>
internal sealed record ParsedCommand(
    Operation Operation,
    IReadOnlyList<string> Program,
    IReadOnlyList<Pair> Pairs,
    string Lang
);

/// <summary>
/// Reads the agent's arguments, refusing anything that is not a run.
/// </summary>
/// <remarks>
/// The shape is fixed and small, so it is read by hand rather than with a parser: five arguments,
/// each named by a prefix on the same token. An argument that names none of them is left alone
/// rather than reported, following the Desktop, which reads the same command line and must not be
/// stopped by a word meant for another reader.
/// </remarks>
internal static class CommandLine
{
    private const string CommandPrefix = "-Command:";
    private const string TypePrefix = "-Type:";
    private const string LangPrefix = "-Lang:";
    private const string FromPrefix = "-From:";
    private const string ToPrefix = "-To:";
    private const string PairsPrefix = "-Pairs:";

    /// <summary>The locale spoken when the run names none.</summary>
    private const string DefaultLang = "en";

    /// <summary>
    /// Reads the arguments into the run they name.
    /// </summary>
    /// <param name="args">The command line.</param>
    /// <param name="error">The reason no run was read, when none was.</param>
    /// <returns>The run, or nothing when the arguments were not one.</returns>
    public static ParsedCommand? Parse(IReadOnlyList<string> args, out string? error)
    {
        string? command = null;
        string? type = null;
        string? lang = null;
        string? from = null;
        string? to = null;
        string? pairs = null;

        // A later occurrence names the same argument again, so it is the one that counts: the last
        // word is what the caller meant.
        foreach (var arg in args)
        {
            if (arg.StartsWith(CommandPrefix, StringComparison.Ordinal))
            {
                command = Value(arg, CommandPrefix);
            }
            else if (arg.StartsWith(TypePrefix, StringComparison.Ordinal))
            {
                type = Value(arg, TypePrefix);
            }
            else if (arg.StartsWith(LangPrefix, StringComparison.Ordinal))
            {
                lang = Value(arg, LangPrefix);
            }
            else if (arg.StartsWith(FromPrefix, StringComparison.Ordinal))
            {
                from = Value(arg, FromPrefix);
            }
            else if (arg.StartsWith(ToPrefix, StringComparison.Ordinal))
            {
                to = Value(arg, ToPrefix);
            }
            else if (arg.StartsWith(PairsPrefix, StringComparison.Ordinal))
            {
                pairs = Value(arg, PairsPrefix);
            }
        }

        error = null;

        if (string.IsNullOrEmpty(command))
        {
            error = "the -Command argument is required and must name a program";
            return null;
        }

        if (string.IsNullOrEmpty(type))
        {
            error = "the -Type argument is required";
            return null;
        }

        var operation = ParseOperation(type);

        if (operation is null)
        {
            error = $"the -Type argument must be Copy, Move, RemoveDirs or RemoveFiles, not `{type}`";
            return null;
        }

        // The program and its fixed arguments are one whitespace-separated string. A path with a
        // space in it cannot be named here — the item paths appended below are separate arguments
        // precisely so that they can be — and quoting inside this word is not honoured.
        var program = command.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);

        if (program.Length == 0)
        {
            error = "the -Command argument names no program";
            return null;
        }

        if (!ReadItems(operation.Value, from, to, pairs, out var items, out error))
        {
            return null;
        }

        return new ParsedCommand(
            operation.Value,
            program,
            items,
            string.IsNullOrEmpty(lang) ? DefaultLang : lang
        );
    }

    /// <summary>Reads the items from either the pairs or the from/to shorthand.</summary>
    private static bool ReadItems(
        Operation operation,
        string? from,
        string? to,
        string? pairs,
        out List<Pair> items,
        out string? error
    )
    {
        items = [];
        error = null;

        if (pairs is not null)
        {
            if (from is not null || to is not null)
            {
                error = "give either -Pairs or -From and -To, not both";
                return false;
            }

            foreach (var entry in pairs.Split(';', StringSplitOptions.RemoveEmptyEntries))
            {
                items.Add(ReadEntry(operation, entry));
            }

            if (items.Count == 0)
            {
                error = "the -Pairs argument names no entries";
                return false;
            }

            return true;
        }

        if (from is null)
        {
            error = "no items were given; give -Pairs, or -From with -To for Copy and Move";
            return false;
        }

        if (operation is Operation.Copy or Operation.Move)
        {
            if (to is null)
            {
                error = "the -To argument is required with -From for Copy and Move";
                return false;
            }

            items.Add(new Pair(from, to));
            return true;
        }

        if (to is not null)
        {
            error = "the -To argument does not apply to RemoveDirs and RemoveFiles";
            return false;
        }

        items.Add(new Pair(from, string.Empty));
        return true;
    }

    /// <summary>
    /// Reads one pair entry: <c>from&gt;to</c> for a transfer, a bare path otherwise.
    /// </summary>
    /// <remarks>
    /// A transfer entry with no <c>&gt;</c> is kept rather than refused, with an empty destination,
    /// so that it is reported against the one item it names instead of failing the whole run. The
    /// first <c>&gt;</c> separates, since a path may carry one and the destination directory is the
    /// shorter side.
    /// </remarks>
    private static Pair ReadEntry(Operation operation, string entry)
    {
        if (operation is not (Operation.Copy or Operation.Move))
        {
            return new Pair(entry, string.Empty);
        }

        var separator = entry.IndexOf('>', StringComparison.Ordinal);

        return separator < 0
            ? new Pair(entry, string.Empty)
            : new Pair(entry[..separator], entry[(separator + 1)..]);
    }

    /// <summary>
    /// The value of an argument, with the quoting a shell would have removed taken off.
    /// </summary>
    /// <remarks>
    /// The documented form is quoted — <c>-Pairs:"a;b"</c> — and whether the quotes survive depends
    /// on who started this program, so one matching pair around the whole value is removed here and
    /// quoting inside it is left as it stands.
    /// </remarks>
    private static string Value(string arg, string prefix)
    {
        var value = arg[prefix.Length..];

        return value.Length >= 2 && value[0] == '"' && value[^1] == '"' ? value[1..^1] : value;
    }

    /// <summary>The operation a name stands for, or nothing when it is none of them.</summary>
    private static Operation? ParseOperation(string name) =>
        name switch
        {
            "Copy" => Operation.Copy,
            "Move" => Operation.Move,
            "RemoveDirs" => Operation.RemoveDirs,
            "RemoveFiles" => Operation.RemoveFiles,
            _ => null,
        };
}

using System.Text;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace GitVCSPlugin;

/// <summary>
/// What the project's record says about the work, as `git rola check` answers it.
/// </summary>
/// <remarks>
/// The command is the one thing that compares a material's content with the hash the lock names, and
/// it is a run rather than a read: a file's bytes are not something a listing may read on the window's
/// thread for every row. The answer is therefore kept per project and the run is made on a task of
/// its own, with the drawing that follows it showing what landed.
/// <para>
/// A path is wrong when the check named it, or named something under it: a tree is one material of
/// the lock, and what the check reports about it are the paths inside it. A missing material is named
/// by the check as well, and is drawn the same way: what the lock records is not there either.
/// </para>
/// </remarks>
internal sealed class GitRolaCheck
{
    /// <summary>The host, whose redraw the answer is shown by.</summary>
    private readonly IPluginHost _host;

    /// <summary>What each project's check answered, by the project's root.</summary>
    private readonly Dictionary<string, Answer> _answers = new(StringComparer.Ordinal);

    /// <summary>The projects a check is running for, so one project is asked once.</summary>
    private readonly HashSet<string> _running = new(StringComparer.Ordinal);

    /// <summary>Which reading of the work the answers are of, so a run that lands late is dropped.</summary>
    private int _generation;

    /// <summary>The lock the answers are guarded with, since a run lands on another thread.</summary>
    private readonly object _gate = new();

    /// <summary>The check, offered to the plugins that mark what it says.</summary>
    /// <param name="host">The host.</param>
    public GitRolaCheck(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        _host = host;
    }

    /// <summary>
    /// Whether what `path` holds is not what the project's lock records.
    /// </summary>
    /// <param name="path">The path to ask about, which is on disk.</param>
    /// <returns>Whether it is wrong.</returns>
    public bool Wrong(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (GitRolaToml.ProjectOf(path) is not { } root)
        {
            return false;
        }

        Reading(root);

        lock (_gate)
        {
            if (!_answers.TryGetValue(root, out var answer))
            {
                return false;
            }

            var relative = System.IO.Path.GetRelativePath(root, path).Replace('\\', '/');

            return answer.Paths.Any(one =>
                string.Equals(one, relative, StringComparison.Ordinal)
                || one.StartsWith(relative + "/", StringComparison.Ordinal)
            );
        }
    }

    /// <summary>
    /// Lets go of what was read, since the files may have changed.
    /// </summary>
    /// <remarks>
    /// What was read is kept until the next answer lands rather than dropped here: a file a bring has
    /// just placed makes every view read again, and an answer dropped before the check that replaces
    /// it has landed would be a colour that went back to grey for as long as the run took.
    /// </remarks>
    public void Forget()
    {
        lock (_gate)
        {
            _generation++;
        }
    }

    /// <summary>What the check answered for a project, for the suite that drives this plugin.</summary>
    /// <param name="root">The project's root.</param>
    /// <returns>The paths, or nothing when no answer has landed.</returns>
    internal IReadOnlyList<string>? AnswerFor(string root)
    {
        lock (_gate)
        {
            return _answers.TryGetValue(root, out var answer) ? answer.Paths : null;
        }
    }

    /// <summary>
    /// Runs the check for a project that has no answer, unless one is already running.
    /// </summary>
    /// <param name="root">The project's root.</param>
    private void Reading(string root)
    {
        int generation;

        lock (_gate)
        {
            if ((_answers.TryGetValue(root, out var known) && known.Generation == _generation) || !_running.Add(root))
            {
                return;
            }

            generation = _generation;
        }

        var program = GitRolaPrograms.Git;

        if (program is null)
        {
            lock (_gate)
            {
                _answers[root] = new Answer(generation, []);
                _running.Remove(root);
            }

            return;
        }

        _ = Task.Run(() =>
        {
            var (_, output, _) = GitRolaCommandLine.Run(program, ["check"], root);
            var changed = Parse(output);

            lock (_gate)
            {
                _running.Remove(root);

                if (generation == _generation)
                {
                    _answers[root] = new Answer(generation, changed);
                }
            }

            Dispatcher.UIThread.Post(_host.Files.Redraw);
        });
    }

    /// <summary>
    /// The paths a check named, out of the short status it writes.
    /// </summary>
    /// <remarks>
    /// A line is a space, the mark, a space, and the path, which the command quotes the way git does:
    /// a path it wraps in quotes is read back here, octals and all, so a name outside ASCII is the
    /// name on disk rather than the escape git wrote it as.
    /// </remarks>
    /// <param name="output">What the command wrote.</param>
    /// <returns>The paths, as the project writes them.</returns>
    internal static IReadOnlyList<string> Parse(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        var paths = new List<string>();

        foreach (var line in output.Split('\n'))
        {
            // A line is a mark and a space before the path, and the first line of a command's output
            // may have lost its own leading space on the way here: the mark is read from wherever it
            // starts rather than from a column.
            var trimmed = line.TrimEnd('\r').TrimStart();

            if (trimmed.Length < 3)
            {
                continue;
            }

            var space = trimmed.IndexOf(' ', StringComparison.Ordinal);

            if (space < 0 || space == trimmed.Length - 1)
            {
                continue;
            }

            var path = Unquote(trimmed[(space + 1)..]).Replace('\\', '/');

            if (path.Length > 0)
            {
                paths.Add(path);
            }
        }

        return paths;
    }

    /// <summary>What a check said, and which reading of the work it was of.</summary>
    /// <param name="Generation">The reading of the work the answer is of.</param>
    /// <param name="Paths">The paths the check named.</param>
    private sealed record Answer(int Generation, IReadOnlyList<string> Paths);

    /// <summary>A path git quoted, read back as the bytes it was written from.</summary>
    /// <param name="text">What the line states.</param>
    /// <returns>The path.</returns>
    private static string Unquote(string text)
    {
        if (text.Length < 2 || text[0] != '"' || text[^1] != '"')
        {
            return text;
        }

        var bytes = new List<byte>(text.Length);

        for (var at = 1; at < text.Length - 1; at++)
        {
            var character = text[at];

            if (character != '\\')
            {
                bytes.AddRange(Encoding.UTF8.GetBytes(character.ToString()));

                continue;
            }

            at++;

            if (at >= text.Length - 1)
            {
                break;
            }

            switch (text[at])
            {
                case '\\':
                    bytes.Add((byte)'\\');
                    break;
                case '"':
                    bytes.Add((byte)'"');
                    break;
                case 'a':
                    bytes.Add(7);
                    break;
                case 'b':
                    bytes.Add(8);
                    break;
                case 'f':
                    bytes.Add(12);
                    break;
                case 'n':
                    bytes.Add(10);
                    break;
                case 'r':
                    bytes.Add(13);
                    break;
                case 't':
                    bytes.Add(9);
                    break;
                case 'v':
                    bytes.Add(11);
                    break;
                default:
                    Octal(text, ref at, bytes);
                    break;
            }
        }

        return Encoding.UTF8.GetString(bytes.ToArray());
    }

    /// <summary>Reads the octal escape at `at`, one to three digits, into `bytes`.</summary>
    /// <param name="text">The text the escape is in.</param>
    /// <param name="at">Where the first digit is, which is left on the last one.</param>
    /// <param name="bytes">What the escape is a byte of.</param>
    private static void Octal(string text, ref int at, List<byte> bytes)
    {
        if (text[at] is < '0' or > '7')
        {
            bytes.AddRange(Encoding.UTF8.GetBytes(text[at].ToString()));

            return;
        }

        var value = 0;
        var digits = 0;

        while (digits < 3 && at < text.Length - 1 && text[at] is >= '0' and <= '7')
        {
            value = (value * 8) + (text[at] - '0');
            at++;
            digits++;
        }

        at--;

        bytes.Add((byte)value);
    }
}

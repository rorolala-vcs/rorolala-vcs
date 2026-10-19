using System.Diagnostics;
using System.Text;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running one command and watching it: what it reports as it goes, and what it said when it was done.
/// </summary>
/// <remarks>
/// A command asked for a structural renderer writes its progress as one JSON record per line on its error stream,
/// beside the words it has to say. The two are one stream, so what is a record is kept and what is left is what
/// the command said — which is what a failure is shown with.
/// <para>
/// The streams are read while the command runs rather than after it ends: a card drawn from what a run said once
/// it was over would be a card that appeared when there was nothing left to watch, and a run that filled the pipe
/// this side is not reading would wait for a reader that is waiting for it to end.
/// </para>
/// </remarks>
internal static class CommandLine
{
    /// <summary>Runs a command and waits, watching what it reports and keeping what it says.</summary>
    /// <remarks>
    /// What it said on its error stream is reported unphrased: the words are the command's own, and it is the
    /// one that knows what went wrong. A run with nothing to say is answered with what it wrote on its output
    /// stream, since a command that failed before saying anything may still have answered there.
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it, which is where the names it was given are read from.</param>
    /// <param name="records">What the run's records are read into, which the caller keeps and draws.</param>
    /// <param name="progress">What is told that a record was said, which the caller draws on its own thread.</param>
    /// <returns>Whether it ended well, what to say when it did not, and what it wrote.</returns>
    public static (bool Done, string Said, string Output) Asked(
        string program,
        IReadOnlyList<string> arguments,
        string directory,
        ProgressRecords records,
        Action progress
    )
    {
        var (code, said, output) = Coded(program, arguments, directory, records, progress);

        return (code == 0, said, output);
    }

    /// <summary>
    /// Runs a command and waits, answering the status it ended with as well as what it said.
    /// </summary>
    /// <remarks>
    /// The status is answered beside the words because one of the ways a run fails is a state rather than a
    /// fault: a Layout whose copy was never fetched is what a place offers a fetch for, and telling that from a
    /// copy that cannot be read means reading the status rather than the message, which is in the reader's
    /// language.
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it.</param>
    /// <param name="records">What the run's records are read into.</param>
    /// <param name="progress">What is told that a record was said.</param>
    /// <returns>The status, what to say when it is not zero, and what it wrote.</returns>
    public static (int Code, string Said, string Output) Coded(
        string program,
        IReadOnlyList<string> arguments,
        string directory,
        ProgressRecords records,
        Action progress
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(arguments);
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(records);
        ArgumentNullException.ThrowIfNull(progress);

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

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            // The output is drained on a task of its own rather than read after the wait: a run that filled the
            // pipe this side is not reading would wait for a reader that is waiting for it to end.
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());
            var said = new StringBuilder();

            while (process.StandardError.ReadLine() is { } line)
            {
                if (!records.Read(line) && line.Length > 0)
                {
                    said.AppendLine(line);
                }

                progress();
            }

            process.WaitForExit();

            var written = output.GetAwaiter().GetResult().Trim();

            if (process.ExitCode == 0)
            {
                // What it said is kept even here: a run read through a structural renderer answers with its result, and
                // a command that wrote about a refusal to its error stream has said something whether or not its status
                // is zero. A caller that reads only what it needs is unaffected by the words it does not look at.
                return (0, said.ToString().Trim(), written);
            }

            var told = said.ToString().Trim();

            if (told.Length == 0)
            {
                told = written;
            }

            return (
                process.ExitCode,
                told.Length == 0 ? $"the command ended with {process.ExitCode}" : told,
                written
            );
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            // A run that could not be started is a status of its own, which is what tells a caller that nothing was
            // asked rather than that the command answered no.
            return (Sync.FailedToRun, error.Message, string.Empty);
        }
    }
}

using System.Diagnostics;
using System.Text;

namespace GitVCSPlugin;

/// <summary>Running one of the two programs beside the Desktop and reading what it said.</summary>
/// <remarks>
/// The commands this plugin runs are `rola fs-ops` and `git-rola ...`; both write UTF-8, as every
/// stream this project's programs write is. Left unstated, a redirected stream is decoded in the
/// machine's codepage, and on a Chinese Windows that is GBK.
/// <para>
/// The output is drained on a task of its own rather than read after the wait: a run that filled the
/// pipe this side is not reading would wait for a reader that is waiting for it to end.
/// </para>
/// </remarks>
internal static class GitRolaCommandLine
{
    /// <summary>Runs the program in the directory and waits, keeping the two streams.</summary>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it.</param>
    /// <returns>The status, what it wrote, and what it said.</returns>
    public static (int Code, string Output, string Said) Run(
        string program,
        IReadOnlyList<string> arguments,
        string directory
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(arguments);
        ArgumentNullException.ThrowIfNull(directory);

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
            using var process = Process.Start(start);

            if (process is null)
            {
                return (-1, string.Empty, "the program was not started");
            }

            var output = Task.Run(() => process.StandardOutput.ReadToEnd());
            var said = process.StandardError.ReadToEnd();

            process.WaitForExit();

            return (process.ExitCode, output.GetAwaiter().GetResult().TrimEnd(), said.TrimEnd());
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (-1, string.Empty, error.Message);
        }
    }
}

/// <summary>The two programs this plugin runs, as the Desktop was handed them.</summary>
/// <remarks>
/// Both are set by `rola desktop` from the directory it was exported into, so what is run is the
/// program beside the window rather than one a `PATH` happens to answer. Absent is possible when the
/// plugin is started by a test, and absent is answered as nothing rather than guessed at.
/// </remarks>
internal static class GitRolaPrograms
{
    /// <summary>The variable naming the `git-rola` program.</summary>
    public const string GitVariable = "ROLA_GIT_EXE";

    /// <summary>The variable naming the `rola` program.</summary>
    public const string RolaVariable = "ROLA_EXE";

    /// <summary>The `git-rola` program, or nothing when the window was handed none.</summary>
    public static string? Git => Environment.GetEnvironmentVariable(GitVariable);

    /// <summary>The `rola` program, or nothing when the window was handed none.</summary>
    public static string? Rola => Environment.GetEnvironmentVariable(RolaVariable);
}

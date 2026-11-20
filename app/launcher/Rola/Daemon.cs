using System.Diagnostics;
using System.Text;

namespace RorolalaLauncher.Rola;

/// <summary>What a reading of the daemon's registry came to.</summary>
/// <param name="Namespaces">The namespaces it serves, in its own order.</param>
/// <param name="Reason">Why it could not be read, when it could not.</param>
internal sealed record DaemonList(IReadOnlyList<string> Namespaces, string Reason)
{
    /// <summary>Whether the registry was read.</summary>
    public bool Done => Reason.Length == 0;
}

/// <summary>
/// The daemon alongside this program, asked for the namespaces it serves.
/// </summary>
/// <remarks>
/// The address book is the daemon's own table, and reading the table's file here would be a second
/// reading of a format that is the daemon's to state. So the daemon is asked, and what it prints is
/// taken as the list: one namespace a line, which is the listing's default shape.
/// <para>
/// It is not the command line this program otherwise reaches: <c>rola-daemon</c> is a program of its
/// own, and a run of it is asked for the same language as the rest, since a reason it reports is read
/// in this window.
/// </para>
/// </remarks>
internal sealed class Daemon
{
    /// <summary>How long the daemon is waited for before it is taken to have hung.</summary>
    private static readonly TimeSpan Limit = TimeSpan.FromSeconds(15);

    /// <summary>The program this reaches.</summary>
    private readonly string _program;

    /// <summary>Makes the daemon runner.</summary>
    /// <param name="program">The daemon's path.</param>
    public Daemon(string program)
    {
        ArgumentException.ThrowIfNullOrEmpty(program);

        _program = program;
    }

    /// <summary>
    /// Lists the namespaces the daemon serves.
    /// </summary>
    /// <remarks>
    /// A daemon that is not running is not a failure of the window: the address book is then empty and
    /// the rest of the dock still works, which is why the reason is reported rather than thrown.
    /// </remarks>
    /// <param name="directory">Where to run it.</param>
    /// <param name="language">The language to ask it to speak.</param>
    /// <returns>The namespaces, or why they could not be read.</returns>
    public DaemonList List(string directory, string language)
    {
        var start = new ProcessStartInfo
        {
            FileName = _program,
            WorkingDirectory = directory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };

        start.ArgumentList.Add("--lang");
        start.ArgumentList.Add(language);
        start.ArgumentList.Add("list");

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            var said = Task.Run(() => process.StandardError.ReadToEnd());
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());

            if (!process.WaitForExit(Limit))
            {
                process.Kill(entireProcessTree: true);

                return new DaemonList([], "timed out");
            }

            var error = said.GetAwaiter().GetResult().Trim();
            var written = output.GetAwaiter().GetResult().Trim();

            if (process.ExitCode != 0)
            {
                return new DaemonList([], error.Length > 0 ? error : $"exit {process.ExitCode}");
            }

            var namespaces = written
                .Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Where(line => line.Length > 0)
                .ToList();

            return new DaemonList(namespaces, string.Empty);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new DaemonList([], error.Message);
        }
    }
}

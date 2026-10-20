using System.Diagnostics;
using System.Text;
using RorolalaDesktop.Theming;

namespace RorolalaLauncher.Rola;

/// <summary>What a run of the command line came to.</summary>
/// <param name="Done">Whether it ended well.</param>
/// <param name="Code">The code it ended with.</param>
/// <param name="Output">What it wrote where results go.</param>
/// <param name="Said">What it said where errors go.</param>
internal sealed record RolaResult(bool Done, int Code, string Output, string Said)
{
    /// <summary>What to show a person about a run that did not end well.</summary>
    public string Reason =>
        Said.Length > 0 ? Said : Output.Length > 0 ? Output : $"exit {Code}";
}

/// <summary>
/// The command line program, as this program reaches it.
/// </summary>
/// <remarks>
/// The program is the one beside this one, found by where this one is rather than by a `PATH`: what a
/// machine's `PATH` holds may be another `rola` of another version, against another Vault, and the two
/// programs are exported together so there is no reason to look anywhere else.
/// <para>
/// Every run is asked for the look the Desktop program reads — <c>desktop-friendly</c> — so what comes
/// back is plain text rather than a drawing meant for a terminal, and every stream is decoded as UTF-8,
/// which is what these programs write: left unstated, a redirected stream is decoded in the machine's own
/// codepage, and on a Chinese Windows that is GBK.
/// </para>
/// <para>
/// Every run is also told the language this program speaks, so that what the command line says back — a
/// reason a check failed, a name a dialog is about to show — is in the language the person reading the
/// window is reading. Left to itself the command line reads the machine's own locale, and a window that is
/// speaking one language while its reasons are written in another is a window that reads as two programs.
/// </para>
/// </remarks>
/// <param name="program">The command line program's path.</param>
/// <param name="language">The language this program speaks, handed to every run.</param>
internal sealed class RolaRunner(string program, string language)
{
    /// <summary>The look a run is asked for, which is the one this program's own text is read in.</summary>
    private const string Theme = "--theme-choice=desktop-friendly";

    /// <summary>How long a run is waited for before it is taken to have hung.</summary>
    private static readonly TimeSpan Limit = TimeSpan.FromSeconds(30);

    /// <summary>
    /// The language every run is asked to speak.
    /// </summary>
    /// <remarks>
    /// It changes while the program runs: the language a window speaks is a choice of the person reading it,
    /// and the command line the window asks for things has to be asked in it — a reason written in one
    /// language under a window speaking another reads as two programs rather than one.
    /// </remarks>
    public string Language { get; set; } = language;

    /// <summary>The program this reaches.</summary>
    public string Program => program;

    /// <summary>Whether there is a program to reach.</summary>
    public bool Exists => File.Exists(program);

    /// <summary>
    /// Runs the command line and waits for it, answering what it said.
    /// </summary>
    /// <param name="arguments">What to run it with, after its name.</param>
    /// <param name="directory">Where to run it.</param>
    /// <returns>Whether it ended well, its code, and both of its streams.</returns>
    public RolaResult Run(IEnumerable<string> arguments, string directory)
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

        start.ArgumentList.Add(Theme);
        start.ArgumentList.Add("--lang");
        start.ArgumentList.Add(Language);

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            // Both streams are drained while it runs, so a command that fills one pipe while this waits
            // on the other cannot block against itself.
            var said = Task.Run(() => process.StandardError.ReadToEnd());
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());

            if (!process.WaitForExit(Limit))
            {
                // A check that hangs is a check that failed: what this program does with the answer is
                // say so and let the reader try again, which is better than a window that never moves.
                process.Kill(entireProcessTree: true);

                return new RolaResult(false, -1, string.Empty, "timed out");
            }

            var error = said.GetAwaiter().GetResult().Trim();
            var written = output.GetAwaiter().GetResult().Trim();

            return process.ExitCode == 0
                ? new RolaResult(true, 0, written, string.Empty)
                : new RolaResult(false, process.ExitCode, written, error);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return new RolaResult(false, -1, string.Empty, error.Message);
        }
    }

    /// <summary>
    /// Asks the command line how this machine draws a window's own frame.
    /// </summary>
    /// <remarks>
    /// The question is not this program's to answer: a window style is decided from what the desktop and
    /// the window manager left in the environment, and the command line is where that reading is written
    /// once. What comes back is the name a run may write back to <c>--window-style</c>.
    /// </remarks>
    /// <param name="directory">Where to run it.</param>
    /// <returns>The style the machine is drawn in, or nothing when it could not be asked.</returns>
    public WindowStyle? ProbeWindowStyle(string directory)
    {
        if (!Exists)
        {
            return null;
        }

        var result = Run(["desktop", "--query-window-style"], directory);

        if (!result.Done)
        {
            return null;
        }

        var line = result.Output.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .FirstOrDefault();

        return line is null ? null : WindowStyles.Named(line);
    }

    /// <summary>
    /// Starts the Desktop program at a directory, and does not wait for it.
    /// </summary>
    /// <remarks>
    /// The launcher closes as the window opens, so what is started is started to be left behind: what a
    /// reader is looking at from then on is the window, and this program has nothing left to do. It is
    /// reached through the command line rather than run by name, because opening a window onto a
    /// Workspace is what that command is, and the two programs agree about what a Workspace is only there.
    /// </remarks>
    /// <param name="workspace">The Workspace to open the window onto.</param>
    /// <param name="style">How the window's own frame is to be drawn.</param>
    /// <returns>Whether a window was asked for.</returns>
    public bool Launch(string workspace, WindowStyle style)
    {
        var start = new ProcessStartInfo
        {
            FileName = program,
            WorkingDirectory = workspace,
            UseShellExecute = false,
            CreateNoWindow = true,
        };

        start.ArgumentList.Add("--lang");
        start.ArgumentList.Add(Language);
        start.ArgumentList.Add("desktop");
        start.ArgumentList.Add("--window-style=" + Written(style));

        try
        {
            return Process.Start(start) is not null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return false;
        }
    }

    /// <summary>The name a style is written as on the command line.</summary>
    /// <param name="style">The style.</param>
    /// <returns>The word a run writes.</returns>
    public static string Written(WindowStyle style) =>
        style switch
        {
            WindowStyle.Mac => "mac",
            WindowStyle.TillingWm => "tillingwm",
            _ => "windows",
        };
}

using System.Diagnostics;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Taking ownership and letting it go, by running the command line that does it.
/// </summary>
/// <remarks>
/// The exchange with the Vault is the command line's — fetching its Layout, reading the gates, writing the
/// claim — and doing it here would be a second answer to the same question, kept in step by hand. So an entry's
/// menu runs <c>rola hold</c> or <c>rola giveup</c> and reports what came of it: what this plugin owns is which
/// words are put to the user, not how ownership moves.
/// <para>
/// The program is the one this window was opened by, named by the environment the run handed over, because the
/// two are exported together: a <c>rola</c> found on the <c>PATH</c> could be another one entirely, of another
/// version, against another Vault.
/// </para>
/// </remarks>
internal static class OwnershipActions
{
    /// <summary>The environment variable naming the program the window was opened by.</summary>
    /// <remarks>
    /// The same name <c>rola desktop</c> hands over, which is where it is set: see the command line's own
    /// <c>cmd_desktop</c>.
    /// </remarks>
    private const string Program = "ROLA_EXE";

    /// <summary>
    /// Registers what an entry's menu offers about who holds it.
    /// </summary>
    /// <remarks>
    /// Two commands, each with a forced twin, for each of the two kinds of entry: a file is one entry, and a
    /// directory is everything under it, which the command line does by itself — so the words say the
    /// difference and the command does not.
    /// </remarks>
    /// <param name="host">The host, whose context menus they are registered with.</param>
    public static void Declare(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        Add(host, ContextMenuTarget.File, "hold_file", Icons.Lock, "hold", false, 10);
        Add(host, ContextMenuTarget.File, "give_file", Icons.LockOpen, "giveup", false, 20);
        Add(host, ContextMenuTarget.File, "hold_file_forced", Icons.Lock, "hold", true, 30);
        Add(host, ContextMenuTarget.File, "give_file_forced", Icons.LockOpen, "giveup", true, 40);

        Add(host, ContextMenuTarget.Directory, "hold_directory", Icons.Lock, "hold", false, 10);
        Add(host, ContextMenuTarget.Directory, "give_directory", Icons.LockOpen, "giveup", false, 20);
        Add(host, ContextMenuTarget.Directory, "hold_directory_forced", Icons.Lock, "hold", true, 30);
        Add(host, ContextMenuTarget.Directory, "give_directory_forced", Icons.LockOpen, "giveup", true, 40);
    }

    /// <summary>Whether this plugin has anything to say about what a menu was opened on.</summary>
    /// <remarks>
    /// A Workspace, and only a Workspace: an entry outside one has no Layout to claim it in, and an action that
    /// could only fail is not worth offering. What cannot be read is answered as nothing to say rather than
    /// raised — a menu that will not open because ownership could not be read is a worse answer than a menu
    /// without these four lines in it, which is the same judgement the column makes.
    /// </remarks>
    /// <param name="was">What the menu was opened on.</param>
    /// <returns>Whether the entry is one this plugin can act on.</returns>
    public static bool Knows(ContextTarget was)
    {
        ArgumentNullException.ThrowIfNull(was);

        if (was.Entry is not { } entry)
        {
            return false;
        }

        try
        {
            var directory = Path.GetDirectoryName(entry.Path);

            return !string.IsNullOrEmpty(directory) && RolaOwnership.Locate(directory) is not null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return false;
        }
    }

    /// <summary>Registers one of the eight.</summary>
    /// <param name="host">The host.</param>
    /// <param name="target">Which kind of entry it is offered for.</param>
    /// <param name="label">The name of the label key, under the menu's own branch.</param>
    /// <param name="icon">The picture it wears.</param>
    /// <param name="verb">Which command it runs.</param>
    /// <param name="force">Whether it is one of the forced pair.</param>
    /// <param name="order">Where it sits among the others of its kind.</param>
    private static void Add(
        IPluginHost host,
        ContextMenuTarget target,
        string label,
        string icon,
        string verb,
        bool force,
        int order
    ) =>
        host.ContextMenus.Add(
            target,
            new ContextMenuItem(
                $"rorolala_vcs.ownership_menu.{label}",
                order,
                was => Run(verb, was, force, host),
                icon,
                force,
                Knows
            )
        );

    /// <summary>Runs one ownership command over the whole choice, and reports what it came to.</summary>
    /// <remarks>
    /// One run for the whole choice rather than one per entry: the command line takes several paths at once, and
    /// a claim that is one claim is one exchange with the Vault rather than three.
    /// <para>
    /// Run off the window's thread and not waited for: the command talks to a Vault, which takes as long as a
    /// network takes, and a window that stopped drawing until it answered would be a window that stopped. What
    /// the files may have become is said once it is done either way, because a run that ends unhappily may have
    /// claimed some of what it was given — which is what it was asked to do.
    /// </para>
    /// </remarks>
    /// <param name="verb">Which command it is.</param>
    /// <param name="target">What the menu was opened on, the entry right-clicked first.</param>
    /// <param name="force">Whether the checks are to be gone past.</param>
    /// <param name="host">The host, for the report and for saying the files may have changed.</param>
    private static void Run(string verb, ContextTarget target, bool force, IPluginHost host)
    {
        var program = Environment.GetEnvironmentVariable(Program);

        if (string.IsNullOrEmpty(program) || target.Entries.Count == 0)
        {
            host.Dialogs.Report(
                new Report(RolaI18N.Get("rorolala_vcs.ownership_failed"), RolaI18N.Get("rorolala_vcs.ownership_unknown"))
            );

            return;
        }

        var arguments = new List<string> { verb };

        if (force)
        {
            arguments.Add("--force");
        }

        // A batch that cannot be done whole is not half done, and a user who chose three files wants the two
        // that can be claimed rather than none of them: what was refused is said, and the run ends unhappily.
        arguments.Add("--allow-partial");

        foreach (var entry in target.Entries)
        {
            // A name inside the directory the menu was opened in, because that is where the run is made: the
            // command line reads a path as a place in the Workspace being worked in, so the run has to be inside
            // that Workspace for it to be found, and a name is what it is given there.
            arguments.Add(Path.GetRelativePath(target.Directory, entry.Path));
        }

        _ = Task.Run(() =>
        {
            var (done, said) = Ask(program, arguments, target.Directory);

            // Either way the files may have moved, which is what the listing reads again for.
            host.Files.Touch();

            if (!done)
            {
                host.Dialogs.Report(new Report(RolaI18N.Get("rorolala_vcs.ownership_failed"), said));
            }
        });
    }

    /// <summary>Runs it and waits, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Both streams are drained while it runs, so that a command that fills one pipe while this waits on the
    /// other cannot block against itself. What it said on its error stream is what is reported, unphrased:
    /// the words are the command's own, and it is the one that knows what went wrong.
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it, which is where the names it was given are read from.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Ask(
        string program,
        IReadOnlyList<string> arguments,
        string directory
    )
    {
        var start = new ProcessStartInfo
        {
            FileName = program,
            WorkingDirectory = directory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        try
        {
            using var process =
                Process.Start(start) ?? throw new InvalidOperationException("the process was not started");

            var error = Task.Run(() => process.StandardError.ReadToEnd());
            var output = Task.Run(() => process.StandardOutput.ReadToEnd());

            process.WaitForExit();

            var said = error.GetAwaiter().GetResult().Trim();

            if (process.ExitCode == 0)
            {
                return (true, string.Empty);
            }

            if (said.Length == 0)
            {
                said = output.GetAwaiter().GetResult().Trim();
            }

            return (false, said.Length == 0 ? $"the command ended with {process.ExitCode}" : said);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, error.Message);
        }
    }
}

using System.Diagnostics;
using System.Text;
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
    /// <summary>The theme the output is asked for, which is the one this window reads.</summary>
    /// <remarks>
    /// Stated on every run rather than left to the command line's own choice, because what is being read is not a
    /// terminal: the level of every reported line is a word at the head of it, and what it says follows as it was
    /// written, so that this window draws the marks rather than the escapes a terminal would have been given.
    /// </remarks>
    private const string Theme = "desktop-friendly";

    /// <summary>The environment variable naming the program the window was opened by.</summary>
    /// <remarks>
    /// The same name <c>rola desktop</c> hands over, which is where it is set: see the command line's own
    /// <c>cmd_desktop</c>.
    /// </remarks>
    private const string Program = "ROLA_EXE";

    /// <summary>
    /// The flag that has the command write its progress as one JSON record per line on its error stream.
    /// </summary>
    /// <remarks>
    /// The same flag the sync is run with: what is read here is the records the command writes about itself, and
    /// they are written only when the run is being read by a program rather than watched by a person. The result
    /// crosses as JSON too, which is no loss — nothing here reads it on the way through.
    /// </remarks>
    private const string Json = "--json";

    /// <summary>
    /// Registers what an entry's menu offers about who holds it.
    /// </summary>
    /// <remarks>
    /// Two commands, each with a forced twin, for each of the two kinds of entry: a file is one entry, and a
    /// directory is everything under it, which the command line does by itself — so the words say the
    /// difference and the command does not.
    /// </remarks>
    /// <param name="host">The host, whose context menus they are registered with.</param>
    /// <param name="hub">What every run this plugin makes is gathered in, which a run of these joins.</param>
    public static void Declare(IPluginHost host, ProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);

        Add(host, hub, ContextMenuTarget.File, "hold_file", Icons.Lock, "hold", false, 10);
        Add(host, hub, ContextMenuTarget.File, "give_file", Icons.LockOpen, "giveup", false, 20);
        Add(host, hub, ContextMenuTarget.File, "hold_file_forced", Icons.Lock, "hold", true, 30);
        Add(host, hub, ContextMenuTarget.File, "give_file_forced", Icons.LockOpen, "giveup", true, 40);

        Add(host, hub, ContextMenuTarget.Directory, "hold_directory", Icons.Lock, "hold", false, 10);
        Add(host, hub, ContextMenuTarget.Directory, "give_directory", Icons.LockOpen, "giveup", false, 20);
        Add(host, hub, ContextMenuTarget.Directory, "hold_directory_forced", Icons.Lock, "hold", true, 30);
        Add(host, hub, ContextMenuTarget.Directory, "give_directory_forced", Icons.LockOpen, "giveup", true, 40);
    }

    /// <summary>Whether this plugin has anything to say about what a menu was opened on.</summary>
    /// <remarks>
    /// A Workspace, and only a Workspace: an entry outside one has no Layout to claim it in, and an action that
    /// could only fail is not worth offering. What cannot be read is answered as nothing to say rather than
    /// raised — a menu that will not open because ownership could not be read is a worse answer than a menu
    /// without these four lines in it, which is the same judgement the column makes.
    /// <para>
    /// What is read is given back at once, because the only question here is whether there is anything to
    /// say: an ownership not released holds two opened Layouts for the life of the run, and this is asked
    /// once per contributed item every time a menu is offered. What a listing draws its column from is the
    /// provider's own reading, which is kept per directory and let go of when the files may have changed
    /// (see <see cref="RorolalaLocks"/>).
    /// </para>
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
            if (Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory)
            {
                return false;
            }

            using var ownership = RolaOwnership.Locate(directory);

            return ownership is not null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return false;
        }
    }

    /// <summary>Registers one of the eight.</summary>
    /// <param name="host">The host.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="target">Which kind of entry it is offered for.</param>
    /// <param name="label">The name of the label key, under the menu's own branch.</param>
    /// <param name="icon">The picture it wears.</param>
    /// <param name="verb">Which command it runs.</param>
    /// <param name="force">Whether it is one of the forced pair.</param>
    /// <param name="order">Where it sits among the others of its kind.</param>
    private static void Add(
        IPluginHost host,
        ProgressHub hub,
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
                was => Run(verb, was, force, host, hub),
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
    /// <param name="hub">What every run this plugin makes is gathered in, which this run's card is begun in.</param>
    private static void Run(
        string verb,
        ContextTarget target,
        bool force,
        IPluginHost host,
        ProgressHub hub
    )
    {
        var program = Environment.GetEnvironmentVariable(Program);

        if (string.IsNullOrEmpty(program) || target.Entries.Count == 0)
        {
            host.Dialogs.Report(
                new Report(RolaI18N.Get("rorolala_vcs.ownership_failed"), RolaI18N.Get("rorolala_vcs.ownership_unknown"))
            );

            return;
        }

        var arguments = new List<string> { "--theme-choice=" + Theme, Json, verb };

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

        // The run's card down the Overview is opened before the work is handed off, so that it is there to be
        // drawn from the moment the command begins — see the hub for why the runs are gathered rather than kept
        // where they were begun.
        var run = hub.Begin(TitleKey(verb));

        _ = Task.Run(() =>
        {
            var (done, said) = Ask(program, arguments, target.Directory, run.Records, hub.Touch);

            // The card is let go of here rather than when the command is over, because the panel keeps it for a
            // few seconds after: what ended is drawn as ended before it is taken away.
            hub.End(run);

            // Either way the files may have moved, which is what the listing reads again for.
            host.Files.Touch();

            if (!done)
            {
                host.Dialogs.Present(
                RolaI18N.Get("rorolala_vcs.ownership_failed"),
                () =>
                    ReportView.Of(
                        ReportLines.Parse(said),
                        key => host.Icons.Find(key),
                        colour => Ink(colour)
                    )
            );
            }
        });
    }

    /// <summary>What a run of one of the two commands is called in the Overview's panel.</summary>
    /// <param name="verb">Which command it is.</param>
    /// <returns>The i18n key of its name.</returns>
    private static string TitleKey(string verb) =>
        verb == "hold" ? "rorolala_vcs.progress.hold" : "rorolala_vcs.progress.giveup";

    /// <summary>
    /// What a colour named inside a message stands for.
    /// </summary>
    /// <remarks>
    /// The three a level is reported in are answered with the look's own resources rather than with the terminal's
    /// palette, so that a message says the same thing in a window as it says in a terminal while being drawn in
    /// the colours the window is drawn in. A colour written as six digits is the colour it says, and a name
    /// nothing answers to is left as it stands.
    /// </remarks>
    /// <param name="colour">The colour the message named.</param>
    /// <returns>The colour or theme resource to draw it in, or nothing.</returns>
    private static string? Ink(string colour) =>
        colour.StartsWith('#')
            ? colour
            : colour.ToLowerInvariant() switch
            {
                "red" => PaletteKeys.Del,
                "yellow" => PaletteKeys.Warn,
                "green" => PaletteKeys.Add,
                _ => null,
            };

    /// <summary>Runs it and waits, answering whether it ended well and what it said.</summary>
    /// <remarks>
    /// Both streams are drained while it runs, so that a command that fills one pipe while this waits on the
    /// other cannot block against itself. What it said on its error stream is what is reported, unphrased:
    /// the words are the command's own, and it is the one that knows what went wrong.
    /// <para>
    /// The error stream carries the command's progress as well as its words: a line that reads as a record is
    /// what the run is drawn from, and what is left is what it said. The records are read line by line as they
    /// are written, rather than the whole stream at once, so that the card moves while the command works.
    /// </para>
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it, which is where the names it was given are read from.</param>
    /// <param name="records">What the run's records are read into, which the panel draws.</param>
    /// <param name="progress">What is told that a record was said, which redraws on the window's thread.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    private static (bool Done, string Said) Ask(
        string program,
        IReadOnlyList<string> arguments,
        string directory,
        ProgressRecords records,
        Action progress
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
            // The words are `rola`'s own, and `rola` writes UTF-8 — as every stream this project's
            // programs write is. Left unstated, a redirected stream is decoded in the machine's codepage,
            // which on a Chinese Windows is GBK, and every word of the message comes out as mojibake.
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

            // The output is drained on a task of its own rather than read after the wait: a run that filled
            // the pipe this side is not reading would wait for a reader that is waiting for it to end.
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

            if (process.ExitCode == 0)
            {
                return (true, string.Empty);
            }

            var told = said.ToString().Trim();

            if (told.Length == 0)
            {
                told = output.GetAwaiter().GetResult().Trim();
            }

            return (false, told.Length == 0 ? $"the command ended with {process.ExitCode}" : told);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return (false, error.Message);
        }
    }
}

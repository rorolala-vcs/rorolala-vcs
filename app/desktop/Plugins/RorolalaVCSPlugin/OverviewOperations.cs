using System.Diagnostics;
using System.Text;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What may be done about one state of a row, which is a mode of <c>rola align</c> apiece — except cancelling
/// a merge, which is a file taken away and a record confirmed after it.
/// </summary>
/// <remarks>
/// One operation is one thing a reader means, which is usually one mode, because that is what the command line
/// offers: the command settles one thing about one path, so a button that settled two would be two buttons
/// wearing one label. What a state offers is read from the command's own modes rather than invented here (see
/// <c>cmd_align</c>), so a mode added there is a button added here.
/// <para>
/// Ending a merge is the one that is not one mode: the command line ends a merge because its variant file is
/// gone, and refuses while it is still there, so cancelling one is that file being taken away and the record
/// confirmed after it. A reader means one thing by it and the program runs two, which is where this file's
/// own words are worth less than the command's.
/// </para>
/// <para>
/// The words are this plugin's and are named by a key, because a button says what it does and the lookup is the
/// catalogue's: the command's own flag names are a command line's, not a window's.
/// </para>
/// </remarks>
internal enum Operation
{
    /// <summary>Put a lost path back where the Layout names it, out of the store.</summary>
    Rebuild,

    /// <summary>Put a moved file back where the Layout names it, leaving what it holds alone.</summary>
    RestoreMove,

    /// <summary>Write the version the Layout names back over a changed file.</summary>
    RestoreContent,

    /// <summary>Confirm a move the reading found, whichever side of it the path is.</summary>
    ConfirmRename,

    /// <summary>
    /// Confirm that the path the Layout names is really gone, and drop the name.
    /// </summary>
    /// <remarks>
    /// The one thing to do about a loss that keeps nothing: the command reads a path the Layout names but the
    /// tree does not hold and takes the name away, which is the answer when the file is not coming back. It is
    /// the other end of <see cref="Rebuild"/>, which brings the file back instead.
    /// </remarks>
    ConfirmLost,

    /// <summary>
    /// Assert that a lost path moved to the file it is paired with, which is what connecting two rows does.
    /// </summary>
    /// <remarks>
    /// The one operation that is not one row's own: it is made of two paths — the one the Layout names and the
    /// one the tree holds — and neither is a whole answer without the other. That is why it is the one button
    /// that waits for a second click rather than running when it is pressed.
    /// </remarks>
    Connect,

    /// <summary>Claim what another account holds, by running the command line that does it.</summary>
    TakeOwnership,

    /// <summary>End the merge a variant file is waiting for, taking that file and the record away.</summary>
    /// <remarks>
    /// The file the variant is for is left as it holds now: what is cancelled is the joining, not what has
    /// become of either file. A reader who wants the variant's content instead records the merge, which is
    /// `rola track` on the file it is for.
    /// </remarks>
    CancelMerge,
}

/// <summary>
/// What may be done about the states a row is in, and how each is run.
/// </summary>
/// <remarks>
/// The operations are the row's rather than the state's, because a row may be in more than one state at once:
/// a file that moved and was edited is a move and a change of content, and what a reader has to do about it is
/// both — so the buttons of both states are on the one row, in the order the states are read in. Two states
/// that want the same operation want it once, which is what the deduplication is for.
/// </remarks>
internal static class OverviewOperations
{
    /// <summary>The theme the output is asked for, which is the one this window reads.</summary>
    /// <remarks>
    /// Stated on every run rather than left to the command line's own choice, because what is being read is not
    /// a terminal: the level of every reported line is a word at the head of it, and what it says follows as it
    /// was written, so that this window draws the marks rather than the escapes a terminal would have been
    /// given. The same theme the ownership commands ask for, since it is the same window reading both.
    /// </remarks>
    private const string Theme = "desktop-friendly";

    /// <summary>The environment variable naming the program the window was opened by.</summary>
    /// <remarks>
    /// The same name <c>rola desktop</c> hands over, which is where it is set: see the command line's own
    /// <c>cmd_desktop</c>.
    /// </remarks>
    private const string Program = "ROLA_EXE";

    /// <summary>
    /// What each state offers, in the order the buttons are read.
    /// </summary>
    /// <remarks>
    /// A state that is nothing to act on offers nothing, which is an answer rather than an absence: the column
    /// is there, and the row says by having no buttons in it that there is nothing here to press. A merge is
    /// not one of those: it is settled by joining its variant in, which is what <c>rola track</c> is for, and
    /// it is ended by cancelling it, which is this column's own.
    /// </remarks>
    /// <param name="state">The state a row is in.</param>
    /// <returns>What may be done about it.</returns>
    public static IReadOnlyList<Operation> Of(State state) =>
        state switch
        {
            State.Lost => [Operation.Rebuild, Operation.Connect, Operation.ConfirmLost],
            State.Untagged => [Operation.Connect],
            State.RenamedLikely => [Operation.ConfirmRename, Operation.RestoreMove, Operation.Connect],
            State.Renamed => [Operation.ConfirmRename, Operation.RestoreMove],
            State.Modified => [Operation.RestoreContent],
            State.Stale => [Operation.RestoreContent],
            State.NotMine => [Operation.RestoreContent, Operation.TakeOwnership],
            State.Merging or State.Broken => [Operation.CancelMerge],
            _ => [],
        };

    /// <summary>
    /// The buttons a row wears: what each of its states offers, once each, in the order they are read.
    /// </summary>
    /// <remarks>
    /// Deduplicated rather than repeated, because two states wanting one thing is one thing to do: a move whose
    /// content is behind is two states that both want the content put back, and a row with two buttons that did
    /// the same thing would be a row asking twice.
    /// </remarks>
    /// <param name="states">The states the row is in, in the order its pills are read.</param>
    /// <returns>What may be done about it.</returns>
    public static IReadOnlyList<Operation> Of(IEnumerable<State> states)
    {
        ArgumentNullException.ThrowIfNull(states);

        var all = new List<Operation>();

        foreach (var state in states)
        {
            foreach (var operation in Of(state))
            {
                if (!all.Contains(operation))
                {
                    all.Add(operation);
                }
            }
        }

        return all;
    }

    /// <summary>The i18n key a button is labelled by, which is also what it says when hovered.</summary>
    /// <param name="operation">The operation.</param>
    /// <returns>The key.</returns>
    public static string Key(Operation operation) =>
        operation switch
        {
            Operation.Rebuild => "rorolala_vcs.operations.rebuild",
            Operation.RestoreMove => "rorolala_vcs.operations.restore_move",
            Operation.RestoreContent => "rorolala_vcs.operations.restore_content",
            Operation.ConfirmRename => "rorolala_vcs.operations.confirm_rename",
            Operation.ConfirmLost => "rorolala_vcs.operations.confirm_lost",
            Operation.Connect => "rorolala_vcs.operations.connect",
            Operation.CancelMerge => "rorolala_vcs.operations.cancel_merge",
            _ => "rorolala_vcs.operations.take_ownership",
        };

    /// <summary>
    /// Whether an operation is the one that waits for a second click.
    /// </summary>
    /// <remarks>
    /// Asked rather than written twice, since what waits is one of the six and the three places that care about
    /// it — which button begins it, which buttons stay pressable while it is on, and which side of it a row may
    /// be — all have to agree about which one it is.
    /// </remarks>
    /// <param name="operation">The operation.</param>
    /// <returns>Whether it is <see cref="Operation.Connect"/>.</returns>
    public static bool IsPairing(Operation operation) => operation is Operation.Connect;

    /// <summary>
    /// Whether a row whose states are <paramref name="mine"/> may be the other side of a connection.
    /// </summary>
    /// <remarks>
    /// A connection is a loss on one side and a gain on the other, and the command line insists on which is
    /// which: <c>--move</c> names the file a <em>lost</em> path moved to, and that file must be one the Layout
    /// does not know — which is exactly an untagged one. So a lost row pairs with an untagged row and no other
    /// way round: two losses have nowhere to go, and two gains have nothing that went.
    /// </remarks>
    /// <param name="mine">The states of the row being clicked.</param>
    /// <param name="theirs">The states of the row it would pair with.</param>
    /// <returns>Whether the two are the two sides of one connection.</returns>
    public static bool Pairs(IReadOnlyList<State> mine, IReadOnlyList<State> theirs)
    {
        ArgumentNullException.ThrowIfNull(mine);
        ArgumentNullException.ThrowIfNull(theirs);

        return (mine.Contains(State.Lost) && theirs.Contains(State.Untagged))
            || (mine.Contains(State.Untagged) && theirs.Contains(State.Lost));
    }

    /// <summary>
    /// Runs one operation over one path, and reports what became of it.
    /// </summary>
    /// <remarks>
    /// The same shape as the ownership commands, and for the same reason: the exchange is the command line's,
    /// and what this plugin owns is which words are put to the user. Run off the window's thread and not waited
    /// for, because a mode that fetches the Vault's Layout takes as long as a network takes.
    /// <para>
    /// Either way the files may have changed, which is what the listing reads again for: a mode that moved a
    /// path back or wrote a version over one has changed the tree, and a mode that failed may still have done
    /// part of what it was asked.
    /// </para>
    /// </remarks>
    /// <param name="operation">What is to be done.</param>
    /// <param name="path">The path it is done to, read where the run is made.</param>
    /// <param name="directory">Where the run is made, which is what the path is read from.</param>
    /// <param name="paired">The other side of a connection, or nothing for an operation that has one path.</param>
    /// <param name="host">The host, for the report and for saying the files may have changed.</param>
    /// <param name="finished">
    /// What is told the run is over, however it ended, which is what holds the table while one is going. Told on
    /// the thread the run was made on rather than the window's, so whoever draws does so through the dispatcher.
    /// </param>
    public static void Run(
        Operation operation,
        string path,
        string directory,
        string? paired,
        IPluginHost host,
        Action finished
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(finished);

        var program = Environment.GetEnvironmentVariable(Program);

        if (string.IsNullOrEmpty(program))
        {
            host.Dialogs.Report(
                new Report(
                    RolaI18N.Get("rorolala_vcs.operations.failed"),
                    RolaI18N.Get("rorolala_vcs.ownership_unknown")
                )
            );

            finished();
            return;
        }

        // Cancelling a merge is the one operation that is two runs rather than one, so it is taken before the
        // one command the rest of them are spelled into (see `Cancel`).
        if (operation is Operation.CancelMerge)
        {
            Cancel(path, directory, program, host, finished);
            return;
        }

        var arguments = new List<string> { "--theme-choice=" + Theme, "align", path };

        switch (operation)
        {
            case Operation.Rebuild:
                arguments.Add("--restore-delete");
                break;
            case Operation.RestoreMove:
                arguments.Add("--restore-move");
                break;
            case Operation.RestoreContent:
                arguments.Add("--restore-modify");
                break;
            case Operation.ConfirmRename:
                arguments.Add("--rename");
                break;
            case Operation.ConfirmLost:
                // The lost path's name is dropped from the record: the file is not coming back, so what the
                // Layout says about it is taken away rather than put back.
                arguments.Add("--delete");
                break;
            case Operation.Connect:
                // The lost path is the one that moved, so it is the one named first and the file it went to is
                // what `--move` names. A connection with no other side was never offered a button.
                arguments.Add("--move");
                arguments.Add(paired ?? string.Empty);
                break;
            default:
                // Ownership is not a mode of the command line's align: it is its own command, and it is the one
                // the grid's context menu runs, so the two ways in do one thing rather than two.
                arguments = ["--theme-choice=" + Theme, "hold", "--allow-partial", path];
                break;
        }

        _ = Task.Run(() =>
        {
            try
            {
                var (done, said) = Ask(program, arguments, directory);

                host.Files.Touch();

                if (!done)
                {
                    Said(host, said);
                }
            }
            finally
            {
                // The table is held for as long as the run is, so it is let go of however the run ended — a mode
                // that failed has still stopped being one that is going.
                finished();
            }
        });
    }

    /// <summary>
    /// Ends the merge a variant file is waiting for: the file is taken away, and the record with it.
    /// </summary>
    /// <remarks>
    /// The file first and the record second, which is the order the command line insists on: `rola align
    /// --delete` ends a merge whose variant file is gone and refuses while it is still there, so a run that
    /// wants the record ended takes the file away first. A file already gone — a merge the reading calls
    /// broken, which is most of them — is nothing to remove, and the record is ended either way.
    /// <para>
    /// Two runs are two chances to fail, and what the second says is what is reported: a removal that failed
    /// has left the merge as it was, and the record that follows would refuse for a reason the first run's
    /// own words already explain.
    /// </para>
    /// </remarks>
    /// <param name="path">The variant file's path, read where the run is made.</param>
    /// <param name="directory">Where the run is made, which is what the path is read from.</param>
    /// <param name="program">The program to run.</param>
    /// <param name="host">The host, for the report and for saying the files may have changed.</param>
    /// <param name="finished">What is told the run is over, however it ended.</param>
    private static void Cancel(
        string path,
        string directory,
        string program,
        IPluginHost host,
        Action finished
    )
    {
        _ = Task.Run(() =>
        {
            try
            {
                // The listing names the file the way the system does and the run is made where the row was read,
                // so the two answer the same question: whether there is a file here to take away.
                if (File.Exists(Path.Combine(directory, path)))
                {
                    var (removed, why) = Ask(
                        program,
                        ["--theme-choice=" + Theme, "fs-ops", "rm", path],
                        directory
                    );

                    if (!removed)
                    {
                        host.Files.Touch();
                        Said(host, why);

                        return;
                    }
                }

                var (done, said) = Ask(
                    program,
                    ["--theme-choice=" + Theme, "align", path, "--delete"],
                    directory
                );

                host.Files.Touch();

                if (!done)
                {
                    Said(host, said);
                }
            }
            finally
            {
                finished();
            }
        });
    }

    /// <summary>Reports what a command said about its own failure, in the window's own words for the frame.</summary>
    /// <param name="host">The host, whose window the report is shown over.</param>
    /// <param name="said">What the command said.</param>
    private static void Said(IPluginHost host, string said) =>
        host.Dialogs.Present(
            RolaI18N.Get("rorolala_vcs.operations.failed"),
            () =>
                ReportView.Of(
                    ReportLines.Parse(said),
                    key => host.Icons.Find(key),
                    colour => Ink(colour)
                )
        );

    /// <summary>
    /// What a colour named inside a message stands for.
    /// </summary>
    /// <remarks>
    /// The three a level is reported in are answered with the look's own resources rather than with the
    /// terminal's palette, so that a message says the same thing in a window as it says in a terminal while
    /// being drawn in the colours the window is drawn in.
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
    /// other cannot block against itself. What it said on its error stream is what is reported, unphrased: the
    /// words are the command's own, and it is the one that knows what went wrong.
    /// </remarks>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">What to run it with.</param>
    /// <param name="directory">Where to run it, which is where the path it was given is read from.</param>
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

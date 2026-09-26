using RorolalaDesktop.I18n;

namespace RorolalaFSAgent;

/// <summary>
/// The file-system agent: run one command over a batch of paths, asking about conflicts first.
/// </summary>
/// <remarks>
/// The program is deliberately two-sided. What it says on standard output is a single JSON line and
/// nothing else, so a caller reads outcomes rather than prose; what it says on standard error is for
/// a person. Between the two sits a window, shown only when a conflict has to be answered — a run
/// with no conflict never wakes the windowing system at all.
/// </remarks>
internal static class Program
{
    /// <summary>Runs the agent over the arguments it was handed.</summary>
    /// <param name="args">The command line, as <c>-Command:… -Type:…</c> and so on.</param>
    /// <returns>
    /// <c>0</c> when a run happened — read the JSON for what each item did — and <c>1</c> when the
    /// arguments were not a run at all.
    /// </returns>
    [STAThread]
    public static int Main(string[] args)
    {
        var parsed = CommandLine.Parse(args, out var error);

        // Bad arguments are not a run: there is no item list to report on, so there is no JSON to
        // print either. The reason goes where diagnostics go and the exit code says which it was.
        if (parsed is null)
        {
            Console.Error.WriteLine(error);
            return 1;
        }

        // Named before anything asks for a form, and before the window is made, so that what the
        // window says is already the language this run speaks.
        RolaI18N.SetTranslationDirectory(Path.Combine(AppContext.BaseDirectory, "i18n"));
        RolaI18N.SetLocale(parsed.Lang);

        var plan = Plan.Build(parsed);

        if (plan.HasConflicts)
        {
            ConflictFlow.Decide(plan, ThemeChoice.Load());
        }

        var results = Executor.Run(plan, parsed.Program);

        // The last line on standard output, and the only thing this program puts there.
        Console.Out.WriteLine(Json.Serialize(results));

        return 0;
    }
}

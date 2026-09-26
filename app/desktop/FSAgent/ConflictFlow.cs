using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Themes.Simple;
using Avalonia.Styling;

namespace RorolalaFSAgent;

/// <summary>
/// The Avalonia program the conflict window is shown by.
/// </summary>
/// <remarks>
/// It exists only to show windows: the run itself happens after it has stopped, on the thread that
/// started it. It is therefore made only when there is a conflict to answer, and a run with none
/// never touches the windowing system.
/// </remarks>
internal sealed class AgentApp : Application
{
    /// <summary>The plan whose conflicts are being answered.</summary>
    public static Plan? Pending { get; set; }

    /// <summary>
    /// The look the window is drawn in, replaced before Avalonia starts and only read after that.
    /// </summary>
    public static ThemeChoice Theme { get; set; } = new(ThemeVariant.Default, default);

    /// <inheritdoc/>
    public override void Initialize()
    {
        Styles.Add(new SimpleTheme());
    }

    /// <inheritdoc/>
    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            // The window is closed as soon as it is answered, and the run goes on after; without
            // this the closing of the last window would stop the program before it could.
            desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;

            RequestedThemeVariant = Theme.Variant;

            foreach (var style in Look.Styles(Theme.Primary))
            {
                Styles.Add(style);
            }

            _ = DecideAsync(desktop);
        }

        base.OnFrameworkInitializationCompleted();
    }

    /// <summary>Asks about every conflict, one at a time, then stops the program.</summary>
    private static async Task DecideAsync(IClassicDesktopStyleApplicationLifetime desktop)
    {
        var plan = Pending!;
        var conflicts = plan.Items.Where(item => item.Conflict).ToList();

        for (var index = 0; index < conflicts.Count; index++)
        {
            var item = conflicts[index];

            // An answer already given for the rest is not asked again.
            if (item.Resolution != Resolution.Undecided)
            {
                continue;
            }

            var window = new ConflictWindow(item, conflicts.Count - index - 1);
            window.Show();

            var choice = await window.Choice;
            window.Close();

            if (choice.Action == ConflictAction.Cancel)
            {
                plan.Cancelled = true;
                desktop.Shutdown();
                return;
            }

            var resolution = choice.Action switch
            {
                ConflictAction.Replace => Resolution.Replace,
                ConflictAction.Skip => Resolution.Skip,
                _ => Resolution.Rename,
            };

            item.Resolution = resolution;

            if (choice.ApplyToAll)
            {
                for (var rest = index + 1; rest < conflicts.Count; rest++)
                {
                    conflicts[rest].Resolution = resolution;
                }

                break;
            }
        }

        desktop.Shutdown();
    }
}

/// <summary>Starts Avalonia to answer the plan's conflicts.</summary>
internal static class ConflictFlow
{
    /// <summary>
    /// Shows a window for each conflict and records the answer, stopping when it is called off.
    /// </summary>
    /// <remarks>
    /// A windowing system that cannot be started leaves the run with conflicts unanswered; rather
    /// than crash, those items are marked as failed with the reason, and the rest of the run goes on.
    /// </remarks>
    /// <param name="plan">The plan to decide for.</param>
    /// <param name="look">The look to draw the windows in.</param>
    public static void Decide(Plan plan, ThemeChoice look)
    {
        AgentApp.Pending = plan;
        AgentApp.Theme = look;

        try
        {
            AppBuilder
                .Configure<AgentApp>()
                .UsePlatformDetect()
                .LogToTrace()
                .StartWithClassicDesktopLifetime([]);
        }
        catch (Exception error)
        {
            foreach (var item in plan.Items)
            {
                if (item.Conflict && item.Resolution == Resolution.Undecided)
                {
                    item.Problem =
                        $"the conflict with `{item.To}` could not be answered: {error.Message}";
                }
            }
        }
    }
}

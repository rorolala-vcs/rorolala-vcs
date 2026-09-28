using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Threading;

namespace FileSystemPlugin;

/// <summary>
/// Asks about a plan's conflicts over the host's own window, one at a time.
/// </summary>
/// <remarks>
/// In the host's process rather than a program of its own, so that the question is a child of the window
/// the user is working in and needs no windowing system of its own: a run with no conflict never shows
/// anything, which is why this is reached only when there is one to answer.
/// </remarks>
internal static class ConflictFlow
{
    /// <summary>
    /// Shows a window for each conflict and records the answer, stopping when it is called off.
    /// </summary>
    /// <param name="plan">The plan to decide for.</param>
    public static async Task Decide(Plan plan)
    {
        // On the loop's own thread, because a window is made and shown there; the callers answer a
        // paste or a drop, which may be a worker's.
        await Dispatcher.UIThread.InvokeAsync(async () =>
        {
            var owner = Owner();
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

                if (owner is not null)
                {
                    await window.ShowDialog(owner);
                }
                else
                {
                    window.Show();
                }

                var choice = await window.Choice;

                if (choice.Action == ConflictAction.Cancel)
                {
                    plan.Cancelled = true;
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
        });
    }

    /// <summary>
    /// The host's main window, which a conflict is shown over.
    /// </summary>
    /// <remarks>
    /// Reached through the application rather than through the contract, because a plugin has no window of
    /// its own and the contract carries none: this is the host's own program, and the window its run is in.
    /// Nothing means the run has no window yet, which is a conflict shown on its own rather than not at all.
    /// </remarks>
    private static Window? Owner() =>
        (Application.Current?.ApplicationLifetime as IClassicDesktopStyleApplicationLifetime)?.MainWindow;
}

using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// Starts a Rorolala command on the host's progress stack, with the picture the command is known by.
/// </summary>
/// <remarks>
/// The host draws the capsule and knows nothing of this plugin's pictures, so the key is chosen here: a command
/// that is syncing is a sync arrow, and one that is fetching is a cloud with an arrow, in the same set the rest of
/// the plugin's furniture is drawn from. A key the icon library does not hold leaves the capsule without a
/// picture rather than without a run.
/// </remarks>
internal static class VcsProgress
{
    /// <summary>Begins a run named by a key, headed with the picture that key's command wears.</summary>
    /// <param name="hub">The host's progress hub.</param>
    /// <param name="titleKey">The i18n key of what the run is called.</param>
    /// <returns>The run.</returns>
    public static IProgressRun Begin(this IProgressHub hub, string titleKey) =>
        hub.Begin(titleKey, IconOf(titleKey));

    /// <summary>The picture a command is headed with.</summary>
    /// <param name="titleKey">What the command is called.</param>
    /// <returns>The key of its picture.</returns>
    private static string IconOf(string titleKey) => titleKey switch
    {
        "rorolala_vcs.progress.sync" => Icons.ArrowsClockwise,
        "rorolala_vcs.progress.fetch" => Icons.CloudArrowDown,
        "rorolala_vcs.progress.bring" => Icons.CloudArrowDown,
        "rorolala_vcs.progress.hold" => Icons.Lock,
        "rorolala_vcs.progress.giveup" => Icons.LockOpen,
        "rorolala_vcs.progress.track" => Icons.Plus,
        "rorolala_vcs.progress.retrack" => Icons.ArrowsSplit,
        "rorolala_vcs.progress.join" => Icons.GitMerge,
        "rorolala_vcs.progress.fetch_content" => Icons.ArrowDown,
        "rorolala_vcs.progress.restore" => Icons.ArrowDown,
        "rorolala_vcs.progress.unmerge" => Icons.GitFork,
        "rorolala_vcs.progress.path_move" => Icons.ArrowRight,
        "rorolala_vcs.progress.path_remove" => Icons.WarningCircle,
        _ => Icons.Briefcase,
    };
}

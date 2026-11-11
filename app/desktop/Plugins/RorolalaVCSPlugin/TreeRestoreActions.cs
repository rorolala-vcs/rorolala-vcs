using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a file inside a tree offers about putting it back: to the version the tree records, or to what
/// the tree has staged.
/// </summary>
/// <remarks>
/// A tree is one Layout entry, so these are not the file's own version and staging area: the command
/// line reads the tree's two structures and writes the one entry for the path. The run is the command
/// line's for the same reason every other run here is, and it reports itself to the window's progress.
/// </remarks>
internal static class TreeRestoreActions
{
    /// <summary>The two commands the menu item names, under the branch their words live in.</summary>
    private const string Baseline = "baseline";

    /// <summary>The command that restores to what the tree has staged.</summary>
    private const string Staged = "staged";

    /// <summary>Registers the two items on a file inside a tree.</summary>
    /// <param name="host">The host, whose context menus they are registered with.</param>
    /// <param name="hub">What every run this plugin makes is gathered in.</param>
    public static void Declare(IPluginHost host, IProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);

        Add(host, hub, Baseline, 50);
        Add(host, hub, Staged, 60);
    }

    /// <summary>Registers one of the two.</summary>
    /// <param name="host">The host.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="which">Which restore it is, and the name of its words.</param>
    /// <param name="order">Where it sits among the file menu's items.</param>
    private static void Add(IPluginHost host, IProgressHub hub, string which, int order) =>
        host.ContextMenus.Add(
            ContextMenuTarget.File,
            new ContextMenuItem(
                $"rorolala_vcs.tree_restore.{which}",
                order,
                was => Ask(host, hub, was, which == Staged),
                Icons.Refresh,
                false,
                IsTreeFile
            )
        );

    /// <summary>Whether the menu is open on a file a tree holds.</summary>
    /// <param name="was">What the menu was opened on.</param>
    /// <returns>Whether it is a file inside a tree that can be read.</returns>
    private static bool IsTreeFile(ContextTarget was)
    {
        ArgumentNullException.ThrowIfNull(was);

        return was.Entry is { Kind: EntryKind.File } entry
            && RolaTrees.RootOf(entry.Path) is { } tree
            && Directory.Exists(Path.Combine(tree, ".rola-tree"));
    }

    /// <summary>Asks before the change is lost, and runs the restore when it is confirmed.</summary>
    /// <param name="host">The host, for the question and the marks of a failure.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="was">What the menu was opened on.</param>
    /// <param name="staged">Whether the tree's staging area is the version to put back.</param>
    private static void Ask(IPluginHost host, IProgressHub hub, ContextTarget was, bool staged)
    {
        if (was.Entry is not { Kind: EntryKind.File } entry || !IsTreeFile(was))
        {
            return;
        }

        var key = staged ? Staged : Baseline;
        host.Dialogs.Show(
            new Dialog(
                RolaI18N.Get($"rorolala_vcs.tree_restore.{key}_confirm_title"),
                RolaI18N.Get($"rorolala_vcs.tree_restore.{key}_confirm_message", entry.Path),
                () => Run(host, hub, entry.Path, staged)
            )
        );
    }

    /// <summary>Runs the restore off the window's thread, and says what came of it.</summary>
    /// <param name="host">The host, whose files are told to read again and whose marks say a failure.</param>
    /// <param name="hub">What the run's card is begun in.</param>
    /// <param name="path">The file to put back.</param>
    /// <param name="staged">Whether the tree's staging area is the version to put back.</param>
    private static void Run(IPluginHost host, IProgressHub hub, string path, bool staged)
    {
        if (Path.GetDirectoryName(path) is not { Length: > 0 } directory)
        {
            return;
        }

        var run = hub.Begin("rorolala_vcs.progress.restore");

        _ = Task.Run(() =>
        {
            var (done, said) = staged
                ? AlignCommand.RestoreStaged(directory, path)
                : AlignCommand.RestoreBaseline(directory, path);

            Dispatcher.UIThread.Post(() =>
            {
                hub.End(run);
                host.Files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        host.Dialogs,
                        host.Icons,
                        RolaI18N.Get("rorolala_vcs.tree_restore.failed"),
                        said
                    );
                }
            });
        });
    }
}

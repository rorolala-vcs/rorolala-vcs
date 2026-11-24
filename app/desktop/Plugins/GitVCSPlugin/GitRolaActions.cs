using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace GitVCSPlugin;

/// <summary>The git-rola commands a right-click on the work offers, run where the project is.</summary>
/// <remarks>
/// They are the command line's own words: a refresh applies the lock, an update resolves the project
/// file through the Vaults and rewrites the lock, and a restore puts named materials back from it.
/// What this side decides is only which project they run in, which paths they name, and what to say
/// when they do not finish.
/// </remarks>
internal static class GitRolaActions
{
    /// <summary>The category the two whole-project commands sit under.</summary>
    public const string Category = "rorolala_git.menu.category";

    /// <summary>The category the restore sits under, which is the one the Rorolala plugin opens.</summary>
    public const string RestoreCategory = "menu.entry.restore";

    /// <summary>The restore's own word, under its category.</summary>
    public const string RestoreToLock = "menu.entry.to_lock";

    /// <summary>The picture the restore wears, drawn into this plugin's own set.</summary>
    public const string RestoreIcon = Icons.ArrowClockwise;

    /// <summary>The refresh's word, under the whole-project category.</summary>
    public const string Refresh = "rorolala_git.menu.refresh";

    /// <summary>The update's word, under the whole-project category.</summary>
    public const string Update = "rorolala_git.menu.update";

    /// <summary>Whether the target is inside a git-rola project.</summary>
    /// <param name="target">What the menu was opened on.</param>
    /// <returns>Whether there is a project to run a command in.</returns>
    public static bool InProject(ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        return GitRolaToml.ProjectOf(target.Directory) is not null;
    }

    /// <summary>Whether the target is files of a git-rola project, which a restore is about.</summary>
    /// <param name="target">What the menu was opened on.</param>
    /// <returns>Whether the choice is files a lock may name.</returns>
    public static bool FilesInProject(ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        return target.Entries.Count > 0
            && target.Entries.All(entry => entry.Kind != EntryKind.Directory)
            && InProject(target);
    }

    /// <summary>
    /// The directory a choice is about: the directory clicked, or the one the menu was opened in.
    /// </summary>
    /// <param name="target">What the menu was opened on.</param>
    /// <returns>The path of the directory the entry is, or of the listing.</returns>
    public static string Chosen(ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        return target.Entry is { Kind: EntryKind.Directory } entry ? entry.Path : target.Directory;
    }

    /// <summary>
    /// Whether the target is a tree the project's lock names, which a restore puts back whole.
    /// </summary>
    /// <param name="target">What the menu was opened on.</param>
    /// <returns>Whether the directory is a tree material.</returns>
    public static bool TreeInProject(ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        var chosen = Chosen(target);

        if (GitRolaToml.ProjectOf(chosen) is not { } root)
        {
            return false;
        }

        var lockPath = System.IO.Path.Combine(root, GitRolaLock.FileName);

        if (!File.Exists(lockPath))
        {
            return false;
        }

        var relative = System.IO.Path.GetRelativePath(root, chosen).Replace('\\', '/');

        return GitRolaLock.Trees(File.ReadAllText(lockPath)).Contains(relative);
    }

    /// <summary>Applies the lock to the work tree.</summary>
    /// <param name="host">The host, which is where a failure is said and a read is asked for.</param>
    /// <param name="target">What the menu was opened on.</param>
    public static void RunRefresh(IPluginHost host, ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(target);

        Run(host, target.Directory, "refresh", []);
    }

    /// <summary>Resolves the project file and rewrites the lock.</summary>
    /// <param name="host">The host, which is where a failure is said and a read is asked for.</param>
    /// <param name="target">What the menu was opened on.</param>
    public static void RunUpdate(IPluginHost host, ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(target);

        Run(host, target.Directory, "update", []);
    }

    /// <summary>Puts the chosen files back from the lock.</summary>
    /// <param name="host">The host, which is where a failure is said and a read is asked for.</param>
    /// <param name="target">What the menu was opened on.</param>
    public static void RunRestore(IPluginHost host, ContextTarget target)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(target);

        if (GitRolaToml.ProjectOf(target.Directory) is not { } root)
        {
            return;
        }

        var paths = target.Entries.Count > 0
            ? target
                .Entries.Select(entry =>
                    System.IO.Path.GetRelativePath(root, entry.Path).Replace('\\', '/')
                )
                .Where(path => path.Length > 0)
                .ToList()
            : [System.IO.Path.GetRelativePath(root, target.Directory).Replace('\\', '/')];

        if (paths.Count == 0)
        {
            return;
        }

        Run(host, root, "restore", paths);
    }

    /// <summary>Runs one command in the project, off the window's thread, and says how it ended.</summary>
    /// <param name="host">The host, which is where a failure is said and a read is asked for.</param>
    /// <param name="directory">The directory to look the project up from.</param>
    /// <param name="verb">The command's own word.</param>
    /// <param name="paths">What to name after it, if anything.</param>
    private static void Run(
        IPluginHost host,
        string directory,
        string verb,
        IReadOnlyList<string> paths
    )
    {
        if (GitRolaToml.ProjectOf(directory) is not { } root || GitRolaPrograms.Git is not { } program)
        {
            return;
        }

        var arguments = new List<string> { verb };

        arguments.AddRange(paths);

        _ = Task.Run(() =>
        {
            var (code, _, said) = GitRolaCommandLine.Run(program, arguments, root);

            Dispatcher.UIThread.Post(() =>
            {
                if (code != 0)
                {
                    Notify(host, said);
                }

                // A refresh, an update and a restore all change what is on disk or what the record
                // says about it, so every view of the work is read again rather than drawn again.
                host.Files.Touch();
            });
        });
    }

    /// <summary>Puts what a failed command said in the host's notification stack.</summary>
    /// <param name="host">The host.</param>
    /// <param name="said">What the command line said.</param>
    private static void Notify(IPluginHost host, string said) =>
        host.Dialogs.Notify(
            new Notification(
                NotificationLevel.Warning,
                RolaI18N.Get("rorolala_git.notice.command_failed"),
                () =>
                    new Avalonia.Controls.TextBlock
                    {
                        Text = said,
                        TextWrapping = Avalonia.Media.TextWrapping.Wrap,
                        MaxWidth = 420,
                    }
            )
        );
}

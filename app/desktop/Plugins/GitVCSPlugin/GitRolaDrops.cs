using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace GitVCSPlugin;

/// <summary>
/// What a directory does with a drag of a Vault's Layout: signs the entries in where they were dropped.
/// </summary>
/// <remarks>
/// The other direction of the place: the place is read, and what it holds is put into the work by
/// being dropped on a directory of it. The work itself is `git rola bring`, which writes the
/// mappings, the lock and the content, so this only decides the destination and runs it.
/// <para>
/// A choice travels as one object per row, so a drag of one row is a one-element array and the drag
/// of several is longer: what is taken is every carry the choice holds, and a choice of two Vaults
/// or one mixing a row with something else is refused rather than half taken.
/// </para>
/// <para>
/// The destination is the directory, with `--by-name` telling the command that each entry keeps its
/// own name there: a Vault's path is not where a person wanted it, which is the whole point of
/// signing it in.
/// </para>
/// </remarks>
internal sealed class GitRolaDrops : IDropTarget
{
    /// <summary>The host, which is where a failure is said and a redraw is asked for.</summary>
    private readonly IPluginHost _host;

    /// <summary>The drop target, offered to the host's registry by the plugin's initialization.</summary>
    /// <param name="host">The host.</param>
    public GitRolaDrops(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        _host = host;
    }

    /// <inheritdoc />
    public bool Accepts(string directory, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(drag);

        return Accepted(directory, drag) is not null;
    }

    /// <inheritdoc />
    public bool Drop(string directory, LogicalDrag drag)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(drag);

        if (Accepted(directory, drag) is not { } accepted)
        {
            return false;
        }

        var (root, project, carrying) = accepted;
        var program = GitRolaPrograms.Git;

        if (program is null)
        {
            return false;
        }

        var arguments = new List<string> { "bring", "--by-name" };

        // A Vault that is the project's default is what a bare `bring` reads, so naming it would be
        // saying the same thing twice; one that is not has to be named, or the entry would be read
        // from the wrong Layout.
        if (!string.Equals(project.DefaultVault, carrying[0].Vault, StringComparison.Ordinal))
        {
            arguments.Add("--vault");
            arguments.Add(carrying[0].Vault);
        }

        arguments.AddRange(carrying.Select(carry => carry.Path));
        arguments.Add(directory);

        _ = Task.Run(() =>
        {
            var (code, _, said) = GitRolaCommandLine.Run(program, arguments, root);

            Dispatcher.UIThread.Post(() =>
            {
                if (code != 0)
                {
                    Notify(said);
                }

                // Told rather than redrawn: what a bring places is new on disk, so every listing and
                // tree that may show it has to read its directory again rather than draw the answer
                // it already holds.
                _host.Files.Touch();
            });
        });

        return true;
    }

    /// <summary>Every carry a drag holds, or nothing when it holds one this plugin did not make.</summary>
    /// <param name="data">What the drag carries.</param>
    /// <returns>The carries, in the order they were chosen.</returns>
    internal static IReadOnlyList<GitRolaCarry> Carried(object? data) =>
        data switch
        {
            GitRolaCarry one => [one],
            object[] many => [.. many.OfType<GitRolaCarry>()],
            IReadOnlyList<object> many => [.. many.OfType<GitRolaCarry>()],
            _ => [],
        };

    /// <summary>
    /// What a directory would take from a drag, or nothing when it takes it not.
    /// </summary>
    /// <remarks>
    /// Every carry has to come from one Vault, and that Vault has to be one the project at the
    /// directory declares: a drop is one command, and a command reads one Layout. A choice holding
    /// anything this plugin did not make is refused whole rather than partly taken, since the
    /// dropped-on directory would otherwise get some of what was dragged and hear nothing about the
    /// rest.
    /// </remarks>
    /// <param name="directory">The directory it would be dropped on.</param>
    /// <param name="drag">What the drag carries.</param>
    /// <returns>The project, and the carries, or nothing.</returns>
    private static (string Root, GitRolaProject Project, IReadOnlyList<GitRolaCarry> Carrying)? Accepted(
        string directory,
        LogicalDrag drag
    )
    {
        var carried = Carried(drag.Data);

        if (carried.Count == 0 || GitRolaPrograms.Git is not { Length: > 0 })
        {
            return null;
        }

        var vault = carried[0].Vault;

        if (!carried.All(carry => string.Equals(carry.Vault, vault, StringComparison.Ordinal)))
        {
            return null;
        }

        if (GitRolaToml.ProjectOf(directory) is not { } root)
        {
            return null;
        }

        var project = GitRolaToml.ReadFile(GitRolaToml.PathOf(root));

        return project.Vaults.Contains(vault) ? (root, project, carried) : null;
    }

    /// <summary>Puts what a failed bring said in the host's notification stack.</summary>
    /// <param name="said">What the command line said.</param>
    private void Notify(string said) =>
        _host.Dialogs.Notify(
            new Notification(
                NotificationLevel.Warning,
                RolaI18N.Get("rorolala_git.notice.bring_failed"),
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

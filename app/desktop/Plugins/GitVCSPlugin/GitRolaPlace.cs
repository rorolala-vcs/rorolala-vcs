using System.Collections.Concurrent;
using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace GitVCSPlugin;

/// <summary>What a drag of one of the place's entries carries: the Vault it came from, and the path.</summary>
/// <remarks>
/// The type is what tells this drag from a file's: a path of a Vault's Layout is not on disk, and
/// what putting one in a directory means is this plugin's answer rather than a copy.
/// </remarks>
/// <param name="Vault">The Vault whose Layout it was read from.</param>
/// <param name="Path">The path in that Layout.</param>
internal sealed record GitRolaCarry(string Vault, string Path);

/// <summary>What was read of one Vault's Layout, or that the read is still going.</summary>
/// <param name="Entries">What the Layout names, once it is read.</param>
/// <param name="Said">What the run said, when it failed.</param>
/// <param name="Pending">Whether the read is still going.</param>
internal sealed record GitRolaReading(
    IReadOnlyList<GitRolaUpstreamEntry> Entries,
    string? Said,
    bool Pending = false
)
{
    /// <summary>A read that is going and has nothing to show yet.</summary>
    public static GitRolaReading Waiting { get; } = new([], null, true);
}

/// <summary>
/// The <c>git-rola@</c> place: the Vaults a git-rola project declares, and each one's Layout.
/// </summary>
/// <remarks>
/// The place shows the upstream alone. What is local, the mappings in <c>rola.toml</c> and the
/// materials in <c>rola.lock</c>, is the File System plugin's file tree, where the materials are
/// marked and where a move is made; repeating it here would be a second view of one answer. So the
/// rows are read-only: a drag carries one to a directory of the tree, which is what signs it in.
/// <para>
/// The root lists the Vaults, because which Vault a project uses is the project's question and the
/// name of one is what `git rola upstream` takes. A level of a Vault is read by running that
/// command, which reaches the daemon, so it is read on a task of its own and the drawing that
/// follows shows it.
/// </para>
/// </remarks>
internal sealed class GitRolaPlace : ILogicalViewProvider
{
    /// <summary>The prefix this place answers, which the contract's grammar allows a hyphen in.</summary>
    public const string Scheme = "git-rola";

    /// <summary>The host, which is where a row is opened and a read is asked to redraw.</summary>
    private readonly IPluginHost _host;

    /// <summary>What was read of each level, by the project and Vault it was read for.</summary>
    private readonly ConcurrentDictionary<string, GitRolaReading> _readings = new(StringComparer.Ordinal);

    /// <summary>The project the kept answers are of.</summary>
    private string _readAt = string.Empty;

    /// <summary>The place, offered to the host's registry by the plugin's initialization.</summary>
    /// <param name="host">The host.</param>
    public GitRolaPlace(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        _host = host;
    }

    /// <summary>Opens the root of the place in the directory dock.</summary>
    public void OpenRoot() =>
        _host.Windows.Show(
            global::FileSystemPlugin.FileSystemPlugin.DirectoryDockNameId,
            Scheme + "@"
        );

    /// <inheritdoc />
    public string Title(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        if (Parts(address) is not { } at)
        {
            return RolaI18N.Get("rorolala_git.place.title");
        }

        return at.Path.Length == 0
            ? RolaI18N.Get("rorolala_git.place.vault", at.Vault)
            : RolaI18N.Get("rorolala_git.place.inside", at.Vault, at.Path);
    }

    /// <inheritdoc />
    public IReadOnlyList<LogicalEntry> Entries(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        if (BaseDirectory.Current is not { Length: > 0 } root || !GitRolaToml.IsProject(root))
        {
            return [];
        }

        if (!string.Equals(_readAt, root, StringComparison.Ordinal))
        {
            _readings.Clear();
            _readAt = root;
        }

        var project = GitRolaToml.ReadFile(GitRolaToml.PathOf(root));

        if (project.Vaults.Count == 0)
        {
            return [];
        }

        if (Parts(address) is not { } at)
        {
            return project
                .Vaults.Select(vault => new LogicalEntry(
                    vault,
                    Draggable: false,
                    Opens: Of(vault),
                    Picture: EntryIcons.Folder
                ))
                .ToList();
        }

        var reading = Reading(root, at.Vault);
        var up = Parent(address);

        if (reading is null)
        {
            return [];
        }

        var rows = new List<LogicalEntry>();

        // Every level but the root has a way back, as `rola@` gives one: a row of the listing rather
        // than a control, since a place opened in a window of its own has nowhere else to offer it.
        if (up is not null)
        {
            rows.Add(
                new LogicalEntry("..", Draggable: false, Opens: up, Picture: EntryIcons.Folder)
            );
        }

        if (reading.Said is { Length: > 0 } said)
        {
            rows.Add(Failed(root, at.Vault, said));

            return rows;
        }

        rows.AddRange(
            GitRolaUpstream
                .Level(reading.Entries, at.Path)
                .Select(row => new LogicalEntry(
                    row.Name,
                    Draggable: true,
                    Opens: row.Folder ? Of(at.Vault, row.Path) : null,
                    Data: new GitRolaCarry(at.Vault, row.Path),
                    Picture: size =>
                        EntryIcons.For(
                            row.Path,
                            row.Folder ? EntryKind.Directory : EntryKind.File,
                            size
                        )
                ))
        );

        return rows;
    }

    /// <inheritdoc />
    public LogicalAddress? Parent(LogicalAddress address)
    {
        ArgumentNullException.ThrowIfNull(address);

        if (Parts(address) is not { } at)
        {
            return null;
        }

        if (at.Path.Length == 0)
        {
            return new LogicalAddress(Scheme, string.Empty);
        }

        var slash = at.Path.LastIndexOf('/');

        return slash < 0
            ? new LogicalAddress(Scheme, at.Vault)
            : new LogicalAddress(Scheme, at.Vault + "/" + at.Path[..slash]);
    }

    /// <summary>
    /// Whether the place takes a drag, which it never does: what it holds is the Vault's, and a
    /// drop here would be a change to a Layout this side does not own.
    /// </summary>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>Always false.</returns>
    public bool Accepts(LogicalAddress address, LogicalDrag drag) => false;

    /// <summary>Takes a drag, which none ever is.</summary>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    public void Drop(LogicalAddress address, LogicalDrag drag)
    {
        // Nothing to do: `Accepts` answers no, so a drop never follows one.
    }

    /// <summary>The columns a level shows, which are none: a name and a picture are the whole row.</summary>
    /// <param name="address">The place being read.</param>
    /// <returns>An empty list.</returns>
    public IReadOnlyList<LogicalColumn> Columns(LogicalAddress address) => [];

    /// <summary>What a payload names: the Vault, and the path in its Layout, or nothing for the root.</summary>
    /// <param name="address">The address.</param>
    /// <returns>The two parts, or nothing at the root.</returns>
    private static (string Vault, string Path)? Parts(LogicalAddress address)
    {
        if (address.Payload.Length == 0)
        {
            return null;
        }

        var slash = address.Payload.IndexOf('/', StringComparison.Ordinal);

        return slash < 0
            ? (address.Payload, string.Empty)
            : (address.Payload[..slash], address.Payload[(slash + 1)..]);
    }

    /// <summary>The address of a Vault's root.</summary>
    /// <param name="vault">The Vault.</param>
    /// <returns>The address.</returns>
    private static LogicalAddress Of(string vault) => new(Scheme, vault);

    /// <summary>The address of a path in a Vault's Layout.</summary>
    /// <param name="vault">The Vault.</param>
    /// <param name="path">The path.</param>
    /// <returns>The address.</returns>
    private static LogicalAddress Of(string vault, string path) => new(Scheme, vault + "/" + path);

    /// <summary>
    /// What was read of a Vault's Layout, asking for the read when there is none.
    /// </summary>
    /// <remarks>
    /// The answer is nothing while the read is going rather than a row saying so: a place being read
    /// is a place with nothing to say about itself, and the drawing that follows the read is what
    /// brings the rows in. One read is asked for per Vault, which the pending entry is what keeps.
    /// </remarks>
    /// <param name="root">The project's root.</param>
    /// <param name="vault">The Vault.</param>
    /// <returns>The reading, or nothing while it is going.</returns>
    private GitRolaReading? Reading(string root, string vault)
    {
        var key = root + "|" + vault;

        if (_readings.TryGetValue(key, out var known))
        {
            return known.Pending ? null : known;
        }

        var program = GitRolaPrograms.Git;

        if (program is null)
        {
            var missing = new GitRolaReading([], RolaI18N.Get("rorolala_git.place.no_program"));

            _readings[key] = missing;

            return missing;
        }

        if (!_readings.TryAdd(key, GitRolaReading.Waiting))
        {
            return null;
        }

        _ = Task.Run(() =>
        {
            var (entries, said) = GitRolaUpstream.Read(program, root, vault);

            _readings[key] = new GitRolaReading(entries, said);

            Dispatcher.UIThread.Post(_host.Files.Redraw);
        });

        return null;
    }

    /// <summary>A row that could not be read, with reading it again in its menu.</summary>
    /// <param name="root">The project's root.</param>
    /// <param name="vault">The Vault.</param>
    /// <param name="said">What the run said.</param>
    /// <returns>The row.</returns>
    private LogicalEntry Failed(string root, string vault, string said) =>
        new(
            RolaI18N.Get("rorolala_git.place.failed", vault, said),
            Draggable: false,
            Menu:
            [
                new ContextMenuItem(
                    "rorolala_git.menu.read_again",
                    10,
                    _ => Forget(root, vault)
                ),
            ]
        );

    /// <summary>Lets go of a failed answer and draws the place again, which reads it again.</summary>
    /// <param name="root">The project's root.</param>
    /// <param name="vault">The Vault.</param>
    private void Forget(string root, string vault)
    {
        _readings.TryRemove(root + "|" + vault, out _);
        _host.Files.Redraw();
    }
}

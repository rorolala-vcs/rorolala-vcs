using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What one row of a suggested folder carries, which is what says the row is the place's own to name and remove.
/// </summary>
/// <param name="Path">The path of the folder.</param>
internal sealed record Suggested(string Path);

/// <summary>
/// A place a plugin answers for, which holds empty folders of its own.
/// </summary>
/// <remarks>
/// Enough of a place for what a listing does with one, without a Workspace, a Vault or a run: reading it answers what
/// was made since the last read, and the redraw that makes a view ask again is said through the host's own channel,
/// which is the one thing a real place and this one must agree on, since it is what makes the next listing exist.
/// <para>
/// What was done to a row is kept, so a check reads the answer rather than a log line: a rename is the name that was
/// taken, and a removal is what is gone.
/// </para>
/// </remarks>
internal sealed class VirtualPlace : ILogicalViewProvider, ILogicalFolderEditor
{
    /// <summary>The place's rows, which is what each read answers from.</summary>
    private readonly List<LogicalEntry> _rows = [];

    /// <summary>The folder just made and not yet named.</summary>
    private string? _made;

    /// <summary>How many folders have been made, so that each is offered a name of its own.</summary>
    private int _madeCount;

    /// <summary>The host, which is where a change to what the place holds is said.</summary>
    private readonly IPluginHost _host;

    /// <summary>Makes a place that says what it changed through one host.</summary>
    /// <param name="host">The host, whose files channel every listing listens to.</param>
    public VirtualPlace(IPluginHost host) => _host = host;

    /// <summary>The name of the folder just made, which is the row a name box should be put on.</summary>
    public string Made { get; private set; } = string.Empty;

    /// <summary>What the rows answered to being renamed, in the order they were, with the name taken.</summary>
    public List<(object Data, string Name)> Renamed { get; } = [];

    /// <summary>What the rows answered to being removed, in the order they were.</summary>
    public List<object> Removed { get; } = [];

    /// <summary>Whether a name is refused rather than taken, which is a place refusing one for a reason of its own.</summary>
    public bool RefusesNames { get; set; }

    /// <summary>How many rows the place holds, which is what a key that made or removed one changes.</summary>
    public int RowCount => _rows.Count;

    /// <summary>Adds a suggested folder whose row the place lets be named and removed.</summary>
    /// <param name="name">What the row is called.</param>
    /// <returns>What the row carries, which is what says it is the place's own.</returns>
    public object Suggest(string name)
    {
        var path = name;
        var data = new Suggested(path);

        _rows.Add(
            new LogicalEntry(
                name,
                Opens: new LogicalAddress("logic", path),
                Data: data
            )
        );

        return data;
    }

    /// <summary>Adds a row the work of the place holds, which is a row nothing here may name.</summary>
    /// <param name="name">What the row is called.</param>
    public void Holds(string name) =>
        _rows.Add(new LogicalEntry(name, Opens: new LogicalAddress("logic", name), Data: name));

    /// <summary>Says that what the place holds changed, which every view reading it is drawn again for.</summary>
    /// <remarks>
    /// What a reading landing looks like from a dock: nothing on disk changed, and the place is drawn again out of the
    /// answer it has just worked out.
    /// </remarks>
    public void Redraw() => _host.Files.Redraw();

    /// <inheritdoc />
    public string Title(LogicalAddress address) => "the place";

    /// <inheritdoc />
    public IReadOnlyList<LogicalEntry> Entries(LogicalAddress address) => [.. _rows];

    /// <inheritdoc />
    public LogicalAddress? Parent(LogicalAddress address) => null;

    /// <inheritdoc />
    public bool Accepts(LogicalAddress address, LogicalDrag drag) => false;

    /// <inheritdoc />
    public void Drop(LogicalAddress address, LogicalDrag drag) { }

    /// <inheritdoc />
    public IReadOnlyList<LogicalColumn> Columns(LogicalAddress address) => [];

    /// <inheritdoc />
    public bool MakesFolders(LogicalAddress address) => true;

    /// <inheritdoc />
    public bool NewFolder(LogicalAddress address)
    {
        _madeCount++;

        var name = _madeCount == 1 ? "New folder" : "New folder " + _madeCount;

        _ = Suggest(name);
        Made = name;
        _made = address.Payload.Length == 0 ? name : address.Payload + "/" + name;

        // Said as a redraw rather than a read: what was made is an answer the place holds, which is exactly what a real
        // place answers when a folder is made in it.
        _host.Files.Redraw();

        return true;
    }

    /// <inheritdoc />
    public string? Pending(LogicalAddress address)
    {
        var made = _made;

        _made = null;

        return made;
    }

    /// <inheritdoc />
    public bool Renamable(object data) => data is Suggested;

    /// <inheritdoc />
    public bool Rename(LogicalAddress address, object data, string name)
    {
        if (RefusesNames)
        {
            return false;
        }

        Renamed.Add((data, name));

        for (var at = 0; at < _rows.Count; at++)
        {
            if (ReferenceEquals(_rows[at].Data, data))
            {
                _rows[at] = _rows[at] with { Name = name };
            }
        }

        _host.Files.Redraw();

        return true;
    }

    /// <inheritdoc />
    public bool Removable(object data) => data is Suggested;

    /// <inheritdoc />
    public bool Remove(LogicalAddress address, object data)
    {
        Removed.Add(data);
        _ = _rows.RemoveAll(row => ReferenceEquals(row.Data, data));
        _host.Files.Redraw();

        return true;
    }
}

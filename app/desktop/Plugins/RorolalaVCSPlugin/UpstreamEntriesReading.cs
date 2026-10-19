using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>What one read of the two sides of the work came to.</summary>
/// <remarks>
/// The states are told apart because what the place has to say — and what it can offer — is different for each: a copy
/// that was never fetched is a fetch to offer, and one that cannot be read is a report.
/// </remarks>
internal enum UpstreamState
{
    /// <summary>The Layout's copy was read.</summary>
    Ok,

    /// <summary>A read of that Layout's copy is going, so there is nothing to draw for it yet.</summary>
    Reading,

    /// <summary>The base the browser is rooted at is not a Workspace.</summary>
    NotWorkspace,

    /// <summary>That Vault's Layout was never fetched here, so there is no copy to read.</summary>
    NotFetched,

    /// <summary>The command line refused the reading, and what it said is why.</summary>
    Failed,
}

/// <summary>What one Layout says, as one read.</summary>
/// <param name="State">What the read came to.</param>
/// <param name="Name">The Vault's name, or the Layout's, which is what the read was about.</param>
/// <param name="Upstream">What the Layout's copy holds.</param>
/// <param name="Local">What the Layout being worked in holds, which is empty for a Layout of the Workspace's own.</param>
/// <param name="Said">What the command line said when it refused the read.</param>
internal sealed record UpstreamReading(
    UpstreamState State,
    string Name,
    IReadOnlyList<LayoutEntry> Upstream,
    IReadOnlyList<LayoutEntry> Local,
    string Said
)
{
    /// <summary>A reading that came to nothing, which is what every state but one is.</summary>
    /// <param name="state">What it came to.</param>
    /// <param name="name">The Vault or Layout it was about.</param>
    /// <returns>The reading.</returns>
    public static UpstreamReading Nothing(UpstreamState state, string name = "") =>
        new(state, name, [], [], string.Empty);

    /// <summary>A reading the command line refused.</summary>
    /// <param name="name">The Vault or Layout it was about.</param>
    /// <param name="said">What it said.</param>
    /// <returns>The reading.</returns>
    public static UpstreamReading Refused(string name, string said) =>
        new(UpstreamState.Failed, name, [], [], said);
}

/// <summary>
/// Reading one Layout beside what the Layout being worked in holds.
/// </summary>
/// <remarks>
/// A Vault's copy is read from what the Workspace fetched — <c>--layout truth@VAULT</c> — and the Layout being worked in
/// is read beside it, which is what tells an entry that is already here from one to bring in. The Vault is named by the
/// place rather than resolved from what the Layout tracks: a place is the upstream of one Vault, and which Vault that is
/// is what the address says.
/// <para>
/// A Layout of the Workspace's own is read by its name, and has no second side: nothing is to be brought into it from it,
/// so what this side holds is not a question the place asks about it.
/// </para>
/// </remarks>
internal static class UpstreamEntriesReading
{
    /// <summary>Reads one Layout, beside what this side holds when it is a Vault's copy.</summary>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    /// <param name="source">The Layout to read.</param>
    /// <param name="log">Where a base that is not a Workspace is judged.</param>
    /// <returns>What the two sides say.</returns>
    public static UpstreamReading Read(string directory, UpstreamSource source, ILog log)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(source);
        ArgumentNullException.ThrowIfNull(log);

        if (!WorkspaceNotice.Holds(directory, log))
        {
            return UpstreamReading.Nothing(UpstreamState.NotWorkspace, source.Name);
        }

        return source.Vault.Length > 0
            ? Vault(directory, source.Name)
            : Layout(directory, source.Name);
    }

    /// <summary>Reads a Vault's copy of its Layout, beside what the Layout being worked in holds.</summary>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    /// <param name="vault">The Vault whose copy is read.</param>
    /// <returns>What the two sides say.</returns>
    private static UpstreamReading Vault(string directory, string vault)
    {
        var local = LayoutEntriesCommand.Local(directory);

        if (!local.Done)
        {
            return UpstreamReading.Refused(vault, local.Said);
        }

        var upstream = LayoutEntriesCommand.Upstream(directory, vault);

        if (!upstream.Done)
        {
            return upstream.Code == LayoutEntriesCommand.NotCached
                ? UpstreamReading.Nothing(UpstreamState.NotFetched, vault)
                : UpstreamReading.Refused(vault, upstream.Said);
        }

        return new UpstreamReading(UpstreamState.Ok, vault, upstream.Entries, local.Entries, string.Empty);
    }

    /// <summary>Reads a Layout of the Workspace's own.</summary>
    /// <param name="directory">Where the work is rooted, and where the runs are made.</param>
    /// <param name="layout">The Layout's name.</param>
    /// <returns>What the Layout holds.</returns>
    private static UpstreamReading Layout(string directory, string layout)
    {
        var answer = LayoutEntriesCommand.Layout(directory, layout);

        return answer.Done
            ? new UpstreamReading(UpstreamState.Ok, layout, answer.Entries, [], string.Empty)
            : UpstreamReading.Refused(layout, answer.Said);
    }
}

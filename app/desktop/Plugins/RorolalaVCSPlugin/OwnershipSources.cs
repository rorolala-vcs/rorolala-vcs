using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The ownership a file's open check reads, from the copy the Workspace has fetched.
/// </summary>
/// <remarks>
/// The read opens the two Layouts, so it is run on a thread of its own: a double-click must not wait
/// for a walk of the Layout before the question can be drawn. Nothing is reached for over the network;
/// what is answered is what the Vault held when the copy was fetched.
/// </remarks>
internal sealed class RolaOwnershipLookup : IOwnershipLookup
{
    /// <summary>Where a read that failed says why.</summary>
    private readonly ILog _log;

    /// <summary>Makes the lookup over the log.</summary>
    /// <param name="log">Where a read that failed says why.</param>
    public RolaOwnershipLookup(ILog log)
    {
        ArgumentNullException.ThrowIfNull(log);

        _log = log;
    }

    /// <inheritdoc />
    public Task<EntryLock?> LookAsync(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return Task.Run(() => Look(path));
    }

    /// <summary>Reads what holds one file, answering nothing when nothing can be read.</summary>
    /// <remarks>
    /// The scope is made over the file's own directory, so the Workspace is located upwards by the native
    /// read rather than by the cheap check that would refuse a directory which does not itself hold the
    /// Workspace's data directory.
    /// </remarks>
    /// <param name="path">The file to ask about.</param>
    /// <returns>What holds it, or nothing when the file is in no Workspace.</returns>
    private EntryLock? Look(string path)
    {
        try
        {
            if (Path.GetDirectoryName(path) is not { Length: > 0 } directory)
            {
                return null;
            }

            using var ownership = new OwnershipScope(directory) { NoCheck = true };
            ownership.Read();

            return ownership.LockOf(path);
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.Unavailable or RolaReadErrorKind.NoWorkspace)
        {
            // No Workspace, or a Workspace tracking no Vault, is the absence the open check is answered
            // with nothing for, and nothing is said about it here.
            return null;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"the ownership of `{path}` could not be read: {error.Message}");

            return null;
        }
    }
}

/// <summary>
/// Taking ownership of one file, through the writer that already knows how.
/// </summary>
/// <remarks>
/// The writer fetches the Vault's Layout, reads every gate and writes the claim, and doing that again
/// here would be a second answer kept in step by hand. What this adds is only the one file and the
/// place the run is made in.
/// </remarks>
internal sealed class WriterOwnershipHold : IOwnershipHold
{
    /// <summary>The host, for the progress card and for saying the files may have changed.</summary>
    private readonly IPluginHost _host;

    /// <summary>What every run this plugin makes is gathered in.</summary>
    private readonly IProgressHub _hub;

    /// <summary>Makes the hold over the host and the progress hub.</summary>
    /// <param name="host">The host, for the progress card and the changed files.</param>
    /// <param name="hub">What every run this plugin makes is gathered in.</param>
    public WriterOwnershipHold(IPluginHost host, IProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);

        _host = host;
        _hub = hub;
    }

    /// <inheritdoc />
    public async Task<OwnershipHoldOutcome> HoldAsync(string path)
    {
        var (done, said) = await OwnershipActions.HoldAsync(_host, _hub, path);

        return new OwnershipHoldOutcome(done, said);
    }
}

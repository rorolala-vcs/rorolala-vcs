namespace RorolalaLauncher.Rola;

/// <summary>How a check of a Vault's address came out.</summary>
internal enum ConnectionState
{
    /// <summary>The daemon answered the handshake.</summary>
    Reached,

    /// <summary>There is no account to speak as, which a handshake cannot be made without.</summary>
    NoAccount,

    /// <summary>The daemon did not answer, or did not answer as the address said it would.</summary>
    Failed,
}

/// <summary>What a check of a Vault's address came to.</summary>
/// <param name="State">How it came out.</param>
/// <param name="Reason">What went wrong, when something did.</param>
internal sealed record ConnectionReport(ConnectionState State, string Reason)
{
    /// <summary>Whether the daemon answered.</summary>
    public bool Reached => State == ConnectionState.Reached;
}

/// <summary>
/// Checks that a Vault can be reached, by speaking the handshake to it.
/// </summary>
/// <remarks>
/// The exchange between the work and its Vault is spoken from inside a Workspace — that is what says which
/// Vault is reached and whose key an account comes from — and a creation is checked before there is a
/// Workspace to speak from. So the check is made from one of this program's own: a Workspace with no work
/// in it, made once and kept, which exists to be a place to stand for the length of a handshake.
/// <para>
/// Nothing is written into the Workspace being created, and nothing is bound anywhere: the address is
/// handed to the command line directly, so what is checked is the address that was typed rather than
/// something this program wrote down first.
/// </para>
/// </remarks>
/// <param name="rola">The command line program.</param>
/// <param name="accounts">The accounts a handshake is spoken as.</param>
/// <param name="probe">The Workspace the check is made from.</param>
internal sealed class Connection(RolaRunner rola, Accounts accounts, string probe)
{
    /// <summary>The directory a Workspace keeps its own files under.</summary>
    private const string DataDirectory = ".rola";

    /// <summary>
    /// Speaks the handshake to the daemon an address names.
    /// </summary>
    /// <param name="address">The address, as it was typed.</param>
    /// <returns>How it came out.</returns>
    public ConnectionReport Check(string address)
    {
        // The account is read from beside the Workspace rather than from inside it: the Workspace the check
        // is spoken from is only made below, and a run in a directory that is not there can read nothing.
        var here = Path.GetDirectoryName(probe) ?? probe;
        var spoke = accounts.List(here);

        if (spoke.Current is null)
        {
            // Said before anything is reached: a handshake is made as someone, and a machine with no
            // account has nobody to make it as — which is a different thing to say than "it did not work".
            return new ConnectionReport(ConnectionState.NoAccount, string.Empty);
        }

        if (!System.IO.Directory.Exists(Path.Combine(probe, DataDirectory)))
        {
            var made = Stand();

            if (made is not null)
            {
                return new ConnectionReport(ConnectionState.Failed, made);
            }
        }

        var result = rola.Run(["vault", "handshake", address], probe);

        return result.Done
            ? new ConnectionReport(ConnectionState.Reached, string.Empty)
            : new ConnectionReport(ConnectionState.Failed, result.Reason);
    }

    /// <summary>
    /// Makes the Workspace the check is spoken from, if it is not there.
    /// </summary>
    /// <remarks>
    /// Made by the command line rather than by hand, because a Workspace is what the command line says one
    /// is: a directory this program assembled would be one nothing else had made.
    /// </remarks>
    /// <returns>Why it could not be made, or nothing when it was.</returns>
    private string? Stand()
    {
        var parent = Path.GetDirectoryName(probe);

        if (string.IsNullOrEmpty(parent))
        {
            return null;
        }

        Directory.CreateDirectory(parent);

        var made = rola.Run(["create", probe], parent);

        return made.Done ? null : made.Reason;
    }
}

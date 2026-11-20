using RorolalaDesktop.I18n;

namespace RorolalaLauncher;

/// <summary>Where an address in the Vaults dock comes from.</summary>
internal enum VaultSource
{
    /// <summary>The <c>rola-daemon</c> registry, which is read and never written here.</summary>
    Registry,

    /// <summary>An address this launcher keeps in <c>vaults.toml</c>.</summary>
    Hosted,
}

/// <summary>What the last handshake with a Vault came to.</summary>
internal enum VaultStatus
{
    /// <summary>No handshake has been spoken since the window opened.</summary>
    Unchecked,

    /// <summary>A handshake is being spoken.</summary>
    Checking,

    /// <summary>The daemon answered.</summary>
    Reachable,

    /// <summary>The daemon did not answer, or there was no account to speak as.</summary>
    Failed,
}

/// <summary>
/// One address in the Vaults dock, with the state its last handshake left it in.
/// </summary>
/// <remarks>
/// The same shape is used for both sources because the dock draws them alike: an address book entry and
/// a hosted one differ in what may be done to them, not in what they are. What the handshake says is
/// written here rather than in the control, so a redrawn dock shows the last answer without speaking
/// again.
/// </remarks>
internal sealed class VaultEntry
{
    /// <summary>What the last handshake came to.</summary>
    private VaultStatus _status;

    /// <summary>Why the last handshake failed.</summary>
    private string _reason = string.Empty;

    /// <summary>Makes an entry for one address.</summary>
    /// <param name="name">What the address is called in the dock.</param>
    /// <param name="address">The Vault's address.</param>
    /// <param name="source">Where the entry came from.</param>
    public VaultEntry(string name, string address, VaultSource source)
    {
        ArgumentException.ThrowIfNullOrEmpty(name);
        ArgumentException.ThrowIfNullOrEmpty(address);

        Name = name;
        Address = address;
        Source = source;
    }

    /// <summary>Raised when the handshake state changed, which is what a window showing it redraws on.</summary>
    public event EventHandler? Changed;

    /// <summary>What the address is called in the dock.</summary>
    public string Name { get; set; }

    /// <summary>The Vault's address.</summary>
    public string Address { get; set; }

    /// <summary>Where the entry came from.</summary>
    public VaultSource Source { get; }

    /// <summary>What the last handshake came to.</summary>
    public VaultStatus Status
    {
        get => _status;

        set
        {
            if (_status == value)
            {
                return;
            }

            _status = value;

            Changed?.Invoke(this, EventArgs.Empty);
        }
    }

    /// <summary>Why the last handshake failed, in the words the command line used.</summary>
    public string Reason
    {
        get => _reason;

        set
        {
            if (string.Equals(_reason, value, StringComparison.Ordinal))
            {
                return;
            }

            _reason = value;

            Changed?.Invoke(this, EventArgs.Empty);
        }
    }

    /// <summary>
    /// Which handshake this entry is waiting on, so that a check begun before a newer one is not applied.
    /// </summary>
    public int Token { get; set; }
}


/// <summary>
/// What the dock says about a state: the short word beside it and the sentence a pointer shows.
/// </summary>
/// <remarks>
/// One statement of the words, because the slot, the lamp's tooltip and the picker in the create window all
/// say the same thing about the same state, and three copies of a sentence are three sentences to keep in
/// step.
/// </remarks>
internal static class VaultSpeech
{
    /// <summary>What a state is called in one word.</summary>
    /// <param name="status">The state.</param>
    /// <returns>The word.</returns>
    public static string Label(VaultStatus status) =>
        status switch
        {
            VaultStatus.Reachable => RolaI18N.Get("launcher.vaults.reachable"),
            VaultStatus.Failed => RolaI18N.Get("launcher.vaults.failed"),
            VaultStatus.Checking => RolaI18N.Get("launcher.vaults.checking"),
            _ => RolaI18N.Get("launcher.vaults.unchecked"),
        };

    /// <summary>What a pointer says about a state, with the reason a handshake failed.</summary>
    /// <param name="entry">The address.</param>
    /// <returns>The sentence.</returns>
    public static string Tip(VaultEntry entry) =>
        entry.Status switch
        {
            VaultStatus.Reachable => RolaI18N.Get("launcher.vaults.reachable_tip"),
            VaultStatus.Failed => entry.Reason.Length > 0
                ? RolaI18N.Get("launcher.vaults.failed_tip", entry.Reason)
                : RolaI18N.Get("launcher.vaults.failed"),
            VaultStatus.Checking => RolaI18N.Get("launcher.vaults.checking_tip"),
            _ => RolaI18N.Get("launcher.vaults.unchecked_tip"),
        };
}

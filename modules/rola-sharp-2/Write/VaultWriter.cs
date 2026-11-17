namespace RolaSharp.V2.Write;

/// <summary>A write over a Vault a caller has read.</summary>
public sealed class VaultWriter : Writer
{
    private readonly VaultScoop _vault;

    /// <summary>Makes a writer over a Vault.</summary>
    /// <param name="vault">The Vault, which must have been read.</param>
    public VaultWriter(VaultScoop vault) =>
        _vault = vault ?? throw new ArgumentNullException(nameof(vault));

    /// <summary>Locks the Vault for as long as the guard lives.</summary>
    /// <returns>The guard, or nothing when the Vault was locked already.</returns>
    public VaultLock? Lock()
    {
        var guard = RorolalaBinding.RolaVault_get_locking_guard(_vault.Handle);

        return guard == 0 ? null : new VaultLock(guard);
    }
}

/// <summary>A Vault lock a writer holds, and gives back when it is disposed.</summary>
public sealed class VaultLock : IDisposable
{
    private nint _guard;

    /// <summary>Makes a guard over a native lock.</summary>
    /// <param name="guard">The native guard, which must not be null.</param>
    internal VaultLock(nint guard) => _guard = guard;

    /// <summary>Gives the lock back.</summary>
    public void Dispose()
    {
        if (_guard != 0)
        {
            RorolalaBinding.RolaVault_drop_locking_guard(_guard);
            _guard = 0;
        }

        GC.SuppressFinalize(this);
    }
}

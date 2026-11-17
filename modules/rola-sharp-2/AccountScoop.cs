namespace RolaSharp.V2;

/// <summary>An account the run can sign as, and the key files it was found in.</summary>
public sealed class AccountScoop : IDisposable
{
    private nint _handle;

    private AccountScoop(nint handle) => _handle = handle;

    /// <summary>Finds the account a key search comes to, when one does.</summary>
    /// <param name="vault">The Vault the search is scoped beside, or nothing.</param>
    /// <returns>The account, or nothing when no key names one.</returns>
    public static AccountScoop? Locate(VaultScoop? vault = null)
    {
        using var rule = KeyRule.New();
        var handle = RorolalaBinding.rola_account_locate(vault?.Handle ?? 0, rule.Handle);

        return handle == 0 ? null : new AccountScoop(handle);
    }

    /// <summary>The native account, for the actions that take one.</summary>
    internal nint Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return _handle;
        }
    }

    /// <summary>The account's name, which is the stem of its private key file.</summary>
    public string Name
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return NativeText.Taken(RorolalaBinding.rola_account_account_name(_handle));
        }
    }

    /// <summary>Where the account's private key file was found.</summary>
    public string KeyPath
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return NativeText.Taken(RorolalaBinding.rola_account_account_key_path(_handle));
        }
    }

    /// <summary>Gives the native account back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_account(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}

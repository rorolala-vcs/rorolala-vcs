namespace RolaSharp;

/// <summary>An account the run can sign as, and the key files it was found in.</summary>
public sealed class AccountScope : IDisposable
{
    private nint _handle;

    private AccountScope(nint handle) => _handle = handle;

    /// <summary>Finds the account a key search comes to, when one does.</summary>
    /// <param name="vault">The Vault the search is scoped beside, or nothing.</param>
    /// <returns>The account, or nothing when no key names one.</returns>
    public static AccountScope? Locate(VaultScope? vault = null)
    {
        using var rule = KeyRule.New();

        // The search answers every account it finds, not one: the scope this returns is the first
        // of them, which is the one the search ranks highest. `lazyffi` cannot receive a null
        // Vault for the export that requires one, so a search with none beside it is the local
        // form, which is the same search with the Vault's root left out.
        var accounts =
            vault is null
                ? RorolalaBinding.rola_account_locate_local(rule.Handle)
                : RorolalaBinding.rola_account_locate(vault.Handle, rule.Handle);
        if (accounts == 0)
        {
            return null;
        }

        try
        {
            if (RorolalaBinding.is_rola_accounts_empty(accounts))
            {
                return null;
            }

            var handle = RorolalaBinding.read_rola_accounts_by_index(accounts, 0);

            return handle == 0 ? null : new AccountScope(handle);
        }
        finally
        {
            RorolalaBinding.free_rola_accounts(accounts);
        }
    }

    /// <summary>Finds the account with a name, when the current scopes hold its key.</summary>
    /// <param name="name">The account's name.</param>
    /// <returns>The account, or nothing when no key names one.</returns>
    public static AccountScope? Find(string name)
    {
        ArgumentNullException.ThrowIfNull(name);

        using var rule = KeyRule.New();
        var account = Utf8(name);
        try
        {
            var handle = RorolalaBinding.rola_account_find(account, rule.Handle);

            return handle == 0 ? null : new AccountScope(handle);
        }
        finally
        {
            System.Runtime.InteropServices.Marshal.FreeCoTaskMem(account);
        }
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

    private static nint Utf8(string text) =>
        System.Runtime.InteropServices.Marshal.StringToCoTaskMemUTF8(text);
}

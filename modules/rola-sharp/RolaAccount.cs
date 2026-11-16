using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>An account: the private key a run signs with, and the name it goes by.</summary>
/// <remarks>
/// The name and the key's path are read from the native account, which owns both; the wrapper owns
/// the account itself and gives it back on <see cref="Dispose"/>.
/// </remarks>
public sealed class RolaAccount : IDisposable
{
    /// <summary>The native account, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native account, which it takes ownership of.</summary>
    /// <param name="handle">The native account, which must not be null.</param>
    internal RolaAccount(nint handle) => _handle = handle;

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

/// <summary>The accounts a search found, in the order it ranked them.</summary>
/// <remarks>
/// A search is scoped by a <see cref="RolaKeyLocateRule"/>: the keys beside the current Vault, the
/// user's own, the machine's, and the ones the environment names. The list is read one account at a
/// time, and each account it hands out is the caller's to release.
/// </remarks>
public sealed class RolaAccounts : IDisposable
{
    /// <summary>The native account list, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native account list, which it takes ownership of.</summary>
    /// <param name="handle">The native list, which must not be null.</param>
    private RolaAccounts(nint handle) => _handle = handle;

    /// <summary>Finds the accounts a key scope holds.</summary>
    /// <param name="vault">
    /// The Vault the search is scoped to, when one is being worked in. It is what the local scope
    /// searches beside, and nothing is searched there for nothing.
    /// </param>
    /// <param name="rule">Which key directories the search may read.</param>
    /// <returns>The accounts found, or nothing when the list could not be made.</returns>
    public static RolaAccounts? Locate(RolaVault? vault, RolaKeyLocateRule rule)
    {
        ArgumentNullException.ThrowIfNull(rule);

        var handle = RorolalaBinding.rola_account_locate(vault?.Handle ?? 0, rule.Handle);

        return handle == 0 ? null : new RolaAccounts(handle);
    }

    /// <summary>How many accounts the search found.</summary>
    public int Count
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return (int)RorolalaBinding.rola_accounts_len(_handle);
        }
    }

    /// <summary>Whether the search found no accounts at all.</summary>
    public bool IsEmpty
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return RorolalaBinding.is_rola_accounts_empty(_handle);
        }
    }

    /// <summary>The account at a position, counting from zero.</summary>
    /// <param name="index">The position.</param>
    /// <returns>The account, which the caller releases, or nothing past the end.</returns>
    public RolaAccount? this[int index]
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            if (index < 0 || index >= Count)
            {
                return null;
            }

            var handle = RorolalaBinding.read_rola_accounts_by_index(_handle, (nuint)index);

            return handle == 0 ? null : new RolaAccount(handle);
        }
    }

    /// <summary>Gives the native list back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_accounts(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}

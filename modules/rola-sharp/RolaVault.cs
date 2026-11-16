using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>A Vault: the remote place a Workspace's work is shared through.</summary>
/// <remarks>
/// A Vault is found from a directory rather than built from a pointer; absence, no Vault holds the
/// directory, is <see langword="null"/> rather than a pointer to remember to check. The native Vault
/// is owned while this lives and given back on <see cref="Dispose"/>.
/// </remarks>
public sealed class RolaVault : IDisposable
{
    /// <summary>The native Vault, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native Vault, which it takes ownership of.</summary>
    /// <param name="handle">The native Vault, which must not be null.</param>
    private RolaVault(nint handle) => _handle = handle;

    /// <summary>The native Vault, for a wrapper that reads the identities a Vault scopes.</summary>
    internal nint Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return _handle;
        }
    }

    /// <summary>Finds the Vault holding a directory, by searching upwards from it.</summary>
    /// <param name="directory">The directory to search from.</param>
    /// <returns>The Vault, or nothing when no Vault holds the directory.</returns>
    public static RolaVault? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var handle = RorolalaBinding.locate_rola_vault(path);

            return handle == 0 ? null : new RolaVault(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Whether the Vault is locked.</summary>
    /// <remarks>
    /// A Vault locked by another run and one whose lock was left behind by a run that did not
    /// finish are both answered as locked: a lock is the file, and not a claim about who holds it.
    /// </remarks>
    public bool IsLocking
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return RorolalaBinding.RolaVault_is_locking(_handle);
        }
    }

    /// <summary>Gives the native Vault back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_vault(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}

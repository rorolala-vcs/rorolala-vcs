using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>What holds one entry of a Workspace's tracked Vault.</summary>
public enum EntryLockKind
{
    /// <summary>Nothing says: the Layout names no entry, or the Vault's copy was never fetched.</summary>
    Unnamed,

    /// <summary>No account holds it.</summary>
    Free,

    /// <summary>The account the work acts as holds it.</summary>
    Mine,

    /// <summary>Another account holds it.</summary>
    Held,
}

/// <summary>What holds one entry, and which account does when it is another's.</summary>
/// <param name="Kind">Which of the four answers it is.</param>
/// <param name="Holder">The account that holds it, when it is another's.</param>
public sealed record EntryLock(EntryLockKind Kind, string? Holder = null);

/// <summary>What holds each entry of a Workspace's tracked Vault, from the copy it fetched.</summary>
public sealed class OwnershipScope : Scope, IDisposable
{
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the ownership reading a directory sits in.</summary>
    /// <param name="directory">Any directory inside the Workspace.</param>
    public OwnershipScope(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The account the work acts as, or an empty string when none is named.</summary>
    public string Account
    {
        get
        {
            EnsureHandle();
            return NativeText.Taken(RorolalaBinding.rola_ownership_account(_handle));
        }
    }

    /// <summary>What holds the entry at a path.</summary>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>What holds it, or that nothing is known.</returns>
    public EntryLock LockOf(string path)
    {
        ArgumentNullException.ThrowIfNull(path);
        EnsureHandle();

        var utf8 = Marshal.StringToCoTaskMemUTF8(path);
        try
        {
            return Read(RorolalaBinding.rola_ownership_lock_of(_handle, utf8));
        }
        finally
        {
            Marshal.FreeCoTaskMem(utf8);
        }
    }

    /// <summary>Where the Vault's copy names the entry at a path.</summary>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>The path upstream names it by, or an empty string when it names none.</returns>
    public string UpstreamOf(string path)
    {
        ArgumentNullException.ThrowIfNull(path);
        EnsureHandle();

        var utf8 = Marshal.StringToCoTaskMemUTF8(path);
        try
        {
            return NativeText.Taken(RorolalaBinding.rola_ownership_upstream_of(_handle, utf8));
        }
        finally
        {
            Marshal.FreeCoTaskMem(utf8);
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _root,
                "no Workspace holds the directory"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        var path = Marshal.StringToCoTaskMemUTF8(_root);
        try
        {
            var handle = RorolalaBinding.rola_ownership_locate(path);
            if (handle == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.Unavailable,
                    _root,
                    "the Workspace has no ownership to read"
                );
            }

            _handle = handle;
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Gives the native ownership back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_ownership(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private void EnsureHandle()
    {
        if (_handle == 0)
        {
            throw new InvalidOperationException("call Read first");
        }
    }

    private static EntryLock Read(RolaEntryLock value) =>
        value.tag switch
        {
            RolaEntryLockTag.RolaEntryLock_Mine => new EntryLock(EntryLockKind.Mine),
            RolaEntryLockTag.RolaEntryLock_Free => new EntryLock(EntryLockKind.Free),
            RolaEntryLockTag.RolaEntryLock_Held => new EntryLock(
                EntryLockKind.Held,
                NativeText.Taken(value.payload.Held)
            ),
            _ => new EntryLock(EntryLockKind.Unnamed),
        };
}

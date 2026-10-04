using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>What holds one entry of a Workspace's tracked Vault.</summary>
/// <remarks>
/// The four outcomes are the whole of what a Layout can say about an entry: it is held by an
/// account, held by nobody, or named by neither the work's Layout nor the Vault's copy — and the
/// last is one answer rather than two, because both mean the same thing to a reader.
/// </remarks>
public enum EntryLockKind
{
    /// <summary>
    /// Nothing says: the Layout names no entry at the path, or the Vault's copy was never fetched.
    /// </summary>
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
/// <param name="Holder">The account that holds it, when it is another's, and nothing otherwise.</param>
public sealed record EntryLock(EntryLockKind Kind, string? Holder = null);

/// <summary>
/// What holds each entry of a Workspace's tracked Vault.
/// </summary>
/// <remarks>
/// The same shape as <see cref="RolaWorkspace"/>: the native value is owned while this lives and
/// given back on <see cref="Dispose"/>, and absence — no Workspace holds the directory, or the
/// Workspace tracks no Vault — is <see langword="null"/> rather than a pointer to remember to check.
/// <para>
/// What it reads is the Vault copy the Workspace has fetched, so nothing is reached for and the
/// answer says what the Vault held when it was last fetched. The two Layouts are opened once, when
/// this is made, which is what makes one directory cost one read rather than one per entry.
/// </para>
/// </remarks>
public sealed class RolaOwnership : IDisposable
{
    /// <summary>The native value, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native ownership, which it takes ownership of.</summary>
    /// <param name="handle">The native ownership, which must not be null.</param>
    private RolaOwnership(nint handle) => _handle = handle;

    /// <summary>
    /// Prepares the ownership answers of the Workspace holding a directory.
    /// </summary>
    /// <param name="directory">The directory to search upwards from.</param>
    /// <returns>The answers, or nothing when there is nothing to answer with.</returns>
    public static RolaOwnership? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        // A path crosses as bytes, and the platform's own encoding is UTF-8 — which is what this
        // writes. The buffer is this side's to free, which is why it is in a `finally`: the search
        // raising is no reason to lose it.
        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var handle = RorolalaBinding.locate_rola_ownership(path);

            return handle == 0 ? null : new RolaOwnership(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>The account the work acts as, or an empty string when none is named.</summary>
    public string Account
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return NativeText.Taken(RorolalaBinding.rola_ownership_account(_handle));
        }
    }

    /// <summary>What holds the entry at a path.</summary>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>What holds it, or that nothing is known.</returns>
    public EntryLock LockOf(string path)
    {
        ArgumentNullException.ThrowIfNull(path);
        ObjectDisposedException.ThrowIf(_handle == 0, this);

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

    /// <summary>Gives the native ownership back.</summary>
    public void Dispose()
    {
        // Nothing to give back the second time, and nothing was lost the first: the handle is
        // forgotten as soon as it is released, so a double `Dispose` releases nothing twice.
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_ownership(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    /// <summary>Reads what the native side answered, taking the holder's name with it.</summary>
    /// <param name="value">What crossed.</param>
    /// <returns>The answer as a caller wants it.</returns>
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

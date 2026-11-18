using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp.V2.Write;

/// <summary>A write over a Vault a caller has read.</summary>
public sealed class VaultWriter : Writer
{
    private readonly VaultScoop _vault;

    /// <summary>Makes a writer over a Vault.</summary>
    /// <param name="vault">The Vault, which must have been read.</param>
    public VaultWriter(VaultScoop vault) =>
        _vault = vault ?? throw new ArgumentNullException(nameof(vault));

    /// <summary>Removes a path's record, leaving the work where it is.</summary>
    /// <param name="path">The path, relative to the Vault root.</param>
    public void Untrack(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        if (!NoCheck)
        {
            var layout = _vault.Layout;
            layout.Read();
            var entry =
                layout.Find(path)
                ?? throw new RolaWriteError(
                    RolaWriteErrorKind.InvalidPath,
                    path,
                    "the Layout names no such path"
                );

            new LayoutWriter(layout).RemoveEntry(Guid.Parse(entry.Uuid));
        }
    }

    /// <summary>Lays the Vault's store and index out afresh.</summary>
    /// <param name="storage">Whether to pack the store.</param>
    /// <param name="index">Whether to pack the VCS index.</param>
    /// <returns>What changed.</returns>
    public Packed Pack(bool storage = true, bool index = true)
    {
        var directory = Utf8(_vault.Root.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_pack_vault(directory, storage, index);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return new Packed(RolaJson.Flag(document.RootElement, "changed"));
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }

    /// <summary>Locks the Vault for as long as the guard lives.</summary>
    /// <returns>The guard, or nothing when the Vault was locked already.</returns>
    public VaultLock? Lock()
    {
        var guard = RorolalaBinding.RolaVault_get_locking_guard(_vault.Handle);

        return guard == 0 ? null : new VaultLock(guard);
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
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

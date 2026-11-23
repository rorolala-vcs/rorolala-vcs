using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>Puts content into a store, and takes it back out.</summary>
public sealed class StorageWriter : Writer
{
    private readonly StorageScope _storage;

    /// <summary>Makes a writer over a store a caller has read.</summary>
    /// <param name="storage">The store.</param>
    public StorageWriter(StorageScope storage) =>
        _storage = storage ?? throw new ArgumentNullException(nameof(storage));

    /// <summary>Makes a directory a store.</summary>
    /// <param name="storage">The directory to make a store of.</param>
    public static void Create(DirectoryInfo storage)
    {
        ArgumentNullException.ThrowIfNull(storage);

        var root = Utf8(storage.FullName);
        try
        {
            var answer = RorolalaBinding.rola_storage_create(root);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(root);
        }
    }

    /// <summary>Lays the store out afresh.</summary>
    /// <returns>What changed.</returns>
    public Packed Pack()
    {
        var directory = Utf8(_storage.ScopeRoot.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_pack_storage(directory);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new Packed(RorolalaBinding.rola_write_pack_changed(handle));
            }
            finally
            {
                RorolalaBinding.free_rola_write_pack(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }

    /// <summary>Puts a file's content into the store.</summary>
    /// <param name="file">The file to store.</param>
    /// <param name="policy">The cut to use, or nothing for the store's own choice.</param>
    /// <returns>The key the content is kept under.</returns>
    public Stored Write(FileInfo file, ChunkingPolicy? policy = null)
    {
        ArgumentNullException.ThrowIfNull(file);

        if (!NoCheck && !file.Exists)
        {
            throw new RolaWriteError(RolaWriteErrorKind.InvalidPath, file.FullName, "the file is not there");
        }

        var directory = Utf8(_storage.ScopeRoot.FullName);
        var path = Utf8(file.FullName);
        var cut = Utf8(policy is null ? string.Empty : ChunkingPolicies.WireName(policy.Value));
        try
        {
            var answer = RorolalaBinding.rola_write_storage_put_file(directory, path, cut);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new Stored(
                    NativeText.Taken(RorolalaBinding.rola_write_storage_put_key(handle)),
                    RorolalaBinding.rola_write_storage_put_chunked(handle)
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_storage_put(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(path);
            Marshal.FreeCoTaskMem(cut);
        }
    }

    /// <summary>Puts content in memory into the store, through a temporary file.</summary>
    /// <param name="content">The bytes to store.</param>
    /// <param name="policy">The cut to use, or nothing for the store's own choice.</param>
    /// <returns>The key the content is kept under.</returns>
    public Stored Write(byte[] content, ChunkingPolicy? policy = null)
    {
        ArgumentNullException.ThrowIfNull(content);

        var file = Path.Combine(Path.GetTempPath(), $"rola-write-{Guid.NewGuid():N}.tmp");
        try
        {
            File.WriteAllBytes(file, content);

            return Write(new FileInfo(file), policy);
        }
        finally
        {
            if (File.Exists(file))
            {
                File.Delete(file);
            }
        }
    }

    /// <summary>Takes the content stored under a key out to a destination.</summary>
    /// <param name="key">The content key.</param>
    /// <param name="destination">Where to write it.</param>
    public void Extract(string key, FileInfo destination)
    {
        ArgumentNullException.ThrowIfNull(key);
        ArgumentNullException.ThrowIfNull(destination);

        if (!NoCheck && destination.Exists)
        {
            throw new RolaWriteError(
                RolaWriteErrorKind.InvalidPath,
                destination.FullName,
                "the destination is already there"
            );
        }

        var directory = Utf8(_storage.ScopeRoot.FullName);
        var hash = Utf8(key);
        var path = Utf8(destination.FullName);
        try
        {
            var answer = RorolalaBinding.rola_storage_extract(directory, hash, path);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.FromStorage(answer.payload);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(hash);
            Marshal.FreeCoTaskMem(path);
        }
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

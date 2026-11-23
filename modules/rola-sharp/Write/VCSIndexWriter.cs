using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>Writes objects into a VCS index.</summary>
public sealed class VCSIndexWriter : Writer
{
    private readonly VCSIndexScope _index;

    /// <summary>Makes a writer over an index a caller has read.</summary>
    /// <param name="index">The index.</param>
    public VCSIndexWriter(VCSIndexScope index) =>
        _index = index ?? throw new ArgumentNullException(nameof(index));

    /// <summary>Writes one variant.</summary>
    /// <param name="variant">The variant.</param>
    /// <returns>The key it was written under.</returns>
    public string Write(VcsVariant variant)
    {
        ArgumentNullException.ThrowIfNull(variant);

        using var memory = new NativeRequest();

        return WriteObject(
            memory,
            new RolaWriteIndexObjectRequest
            {
                tag = RolaWriteIndexObjectRequestTag.RolaWriteIndexObjectRequest_Variant,
                payload = new RolaWriteIndexObjectRequestPayload
                {
                    Variant = new RolaWriteIndexObjectRequestVariant
                    {
                        storage = memory.Text(variant.Storage),
                        base_version = memory.Text(variant.BaseVersion),
                        join = memory.Text(variant.Join),
                        creator = memory.Text(variant.Creator),
                        message = memory.Text(variant.Message),
                    },
                },
            }
        );
    }

    /// <summary>Writes one version.</summary>
    /// <param name="version">The version.</param>
    /// <returns>The key it was written under.</returns>
    public string Write(VcsVersion version)
    {
        ArgumentNullException.ThrowIfNull(version);

        using var memory = new NativeRequest();

        return WriteObject(
            memory,
            new RolaWriteIndexObjectRequest
            {
                tag = RolaWriteIndexObjectRequestTag.RolaWriteIndexObjectRequest_Version,
                payload = new RolaWriteIndexObjectRequestPayload
                {
                    Version = new RolaWriteIndexObjectRequestVersion
                    {
                        variant = memory.Text(version.Variant),
                    },
                },
            }
        );
    }

    /// <summary>Writes one creator.</summary>
    /// <param name="name">The name.</param>
    /// <returns>The key it was written under.</returns>
    public string WriteCreator(string name)
    {
        ArgumentNullException.ThrowIfNull(name);

        using var memory = new NativeRequest();

        return WriteObject(
            memory,
            new RolaWriteIndexObjectRequest
            {
                tag = RolaWriteIndexObjectRequestTag.RolaWriteIndexObjectRequest_Creator,
                payload = new RolaWriteIndexObjectRequestPayload
                {
                    Creator = new RolaWriteIndexObjectRequestCreator
                    {
                        text = memory.Text(name),
                    },
                },
            }
        );
    }

    /// <summary>Writes one message.</summary>
    /// <param name="text">The text.</param>
    /// <returns>The key it was written under.</returns>
    public string WriteMessage(string text)
    {
        ArgumentNullException.ThrowIfNull(text);

        using var memory = new NativeRequest();

        return WriteObject(
            memory,
            new RolaWriteIndexObjectRequest
            {
                tag = RolaWriteIndexObjectRequestTag.RolaWriteIndexObjectRequest_Message,
                payload = new RolaWriteIndexObjectRequestPayload
                {
                    Message = new RolaWriteIndexObjectRequestMessage
                    {
                        text = memory.Text(text),
                    },
                },
            }
        );
    }

    /// <summary>Lays the index out afresh.</summary>
    /// <returns>What changed.</returns>
    public Packed Pack()
    {
        var directory = Utf8(_index.Root.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_pack_index(directory);
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

    /// <summary>Records a file's content as a variant and a version.</summary>
    /// <param name="storage">The content's key.</param>
    /// <param name="baseVersion">The variant the file was on.</param>
    /// <param name="join">The variant a merge brings in, or nothing.</param>
    /// <param name="creator">The creator's name.</param>
    /// <param name="message">What the version says was done.</param>
    /// <returns>The variant and the version.</returns>
    public FileVersion WriteFileVersion(
        string storage,
        string baseVersion,
        string? join,
        string creator,
        string message
    )
    {
        ArgumentNullException.ThrowIfNull(storage);
        ArgumentNullException.ThrowIfNull(baseVersion);
        ArgumentNullException.ThrowIfNull(creator);
        ArgumentNullException.ThrowIfNull(message);

        var directory = Utf8(_index.Root.FullName);
        var content = Utf8(storage);
        var baseAt = Utf8(baseVersion);
        var joined = Utf8(join ?? string.Empty);
        var author = Utf8(creator);
        var words = Utf8(message);
        try
        {
            var answer = RorolalaBinding.rola_write_index_file_version(
                directory,
                content,
                baseAt,
                joined,
                author,
                words
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return new FileVersion(
                    NativeText.Taken(RorolalaBinding.rola_write_file_version_variant(handle)),
                    NativeText.Taken(RorolalaBinding.rola_write_file_version_version(handle))
                );
            }
            finally
            {
                RorolalaBinding.free_rola_write_file_version(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(content);
            Marshal.FreeCoTaskMem(baseAt);
            Marshal.FreeCoTaskMem(joined);
            Marshal.FreeCoTaskMem(author);
            Marshal.FreeCoTaskMem(words);
        }
    }

    /// <summary>The UTF-8 text of a string, in memory the caller frees.</summary>
    /// <param name="text">The text.</param>
    /// <returns>The pointer.</returns>
    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);

    /// <summary>Writes one object into the index and answers the key it was written under.</summary>
    /// <param name="memory">Where the request's strings are held.</param>
    /// <param name="request">The object.</param>
    /// <returns>The key.</returns>
    private string WriteObject(NativeRequest memory, RolaWriteIndexObjectRequest request)
    {
        var directory = Marshal.StringToCoTaskMemUTF8(_index.Root.FullName);
        try
        {
            var answer = RorolalaBinding.rola_write_index_object(directory, request);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            var handle = answer.payload;
            try
            {
                return NativeText.Taken(RorolalaBinding.rola_write_index_object_key(handle));
            }
            finally
            {
                RorolalaBinding.free_rola_write_index_object(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
        }
    }
}

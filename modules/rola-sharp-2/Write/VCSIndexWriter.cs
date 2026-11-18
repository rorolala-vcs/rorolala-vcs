using System.Runtime.InteropServices;
using System.Text.Json;

using RolaSharp.V2;

namespace RolaSharp.V2.Write;

/// <summary>Writes objects into a VCS index.</summary>
public sealed class VCSIndexWriter : Writer
{
    private readonly VCSIndexScoop _index;

    /// <summary>Makes a writer over an index a caller has read.</summary>
    /// <param name="index">The index.</param>
    public VCSIndexWriter(VCSIndexScoop index) =>
        _index = index ?? throw new ArgumentNullException(nameof(index));

    /// <summary>Writes one variant.</summary>
    /// <param name="variant">The variant.</param>
    /// <returns>The key it was written under.</returns>
    public string Write(VcsVariant variant)
    {
        ArgumentNullException.ThrowIfNull(variant);

        return WriteJson(
            JsonSerializer.Serialize(
                new
                {
                    kind = "variant",
                    storage = variant.Storage,
                    base_version = variant.BaseVersion,
                    join = variant.Join,
                    creator = variant.Creator,
                    message = variant.Message,
                }
            )
        );
    }

    /// <summary>Writes one version.</summary>
    /// <param name="version">The version.</param>
    /// <returns>The key it was written under.</returns>
    public string Write(VcsVersion version)
    {
        ArgumentNullException.ThrowIfNull(version);

        return WriteJson(JsonSerializer.Serialize(new { kind = "version", variant = version.Variant }));
    }

    /// <summary>Writes one creator.</summary>
    /// <param name="name">The name.</param>
    /// <returns>The key it was written under.</returns>
    public string WriteCreator(string name)
    {
        ArgumentNullException.ThrowIfNull(name);

        return WriteJson(JsonSerializer.Serialize(new { kind = "creator", text = name }));
    }

    /// <summary>Writes one message.</summary>
    /// <param name="text">The text.</param>
    /// <returns>The key it was written under.</returns>
    public string WriteMessage(string text)
    {
        ArgumentNullException.ThrowIfNull(text);

        return WriteJson(JsonSerializer.Serialize(new { kind = "message", text }));
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

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return new Packed(RolaJson.Flag(document.RootElement, "changed"));
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

            using var document = JsonDocument.Parse(NativeText.Taken(answer.payload));

            return new FileVersion(
                RolaJson.Text(document.RootElement, "variant") ?? string.Empty,
                RolaJson.Text(document.RootElement, "version") ?? string.Empty
            );
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

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);

    private string WriteJson(string request)
    {
        var directory = Marshal.StringToCoTaskMemUTF8(_index.Root.FullName);
        var json = Marshal.StringToCoTaskMemUTF8(request);
        try
        {
            var answer = RorolalaBinding.rola_write_index_object_json(directory, json);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            return NativeText.Taken(answer.payload);
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            Marshal.FreeCoTaskMem(json);
        }
    }
}

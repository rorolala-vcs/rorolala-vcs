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

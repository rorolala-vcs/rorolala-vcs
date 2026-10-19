using System.Text.Json;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// One variant the chain does not draw: a variant nothing joins into, and no version points at.
/// </summary>
/// <param name="VersionNumber">The number of the version the variant is based on.</param>
/// <param name="VersionHash">The hash of the version the variant is based on.</param>
/// <param name="Hash">The variant's own hash.</param>
/// <param name="Creator">Who made it, as text.</param>
/// <param name="Message">What it says, as text.</param>
internal sealed record UnjoinedVariant(
    ulong VersionNumber,
    string VersionHash,
    string Hash,
    string Creator,
    string Message
);

/// <summary>
/// Asking the command line which unjoined variants a file's chain has.
/// </summary>
/// <remarks>
/// The index is the command line's to walk: a variant no link points at is reached from no chain the
/// Version Graph's own reading follows, so the only answer is the one <c>rola status</c> gives with
/// <c>--show-unjoined-variants</c>. What this owns is the question and the shape of the answer.
/// </remarks>
internal static class UnjoinedCommand
{
    /// <summary>The arguments the listing is asked with, after the program's name.</summary>
    /// <param name="file">The file whose unjoined variants are wanted.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string file) =>
        [
            "--theme-choice=" + Sync.Theme,
            "--json",
            "status",
            "--show-unjoined-variants",
            file,
        ];

    /// <summary>Asks for the unjoined variants of one file.</summary>
    /// <remarks>
    /// Run off the window's thread: the command walks the index, and the question is asked again
    /// whenever the work may have changed while the switch is on.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="file">The file whose unjoined variants are wanted.</param>
    /// <returns>Whether it ended well, what to say when it did not, and the variants it named.</returns>
    public static (bool Done, string Said, IReadOnlyList<UnjoinedVariant> Variants) Run(
        string directory,
        string file
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(file);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"), []);
        }

        var (done, said, output) = Sync.Asked(program, Arguments(file), directory);

        if (!done)
        {
            return (false, said, []);
        }

        try
        {
            return (true, string.Empty, Parse(output));
        }
        catch (JsonException error)
        {
            return (false, error.Message, []);
        }
    }

    /// <summary>Reads the listing the command answered with.</summary>
    /// <param name="output">What the command wrote.</param>
    /// <returns>The variants it named, or nothing when the answer is not a listing.</returns>
    /// <exception cref="JsonException">The output is not JSON.</exception>
    private static IReadOnlyList<UnjoinedVariant> Parse(string output)
    {
        using var document = JsonDocument.Parse(output);

        if (
            document.RootElement.ValueKind is not JsonValueKind.Object
            || !document.RootElement.TryGetProperty("variants", out var variants)
            || variants.ValueKind is not JsonValueKind.Array
        )
        {
            return [];
        }

        var found = new List<UnjoinedVariant>(variants.GetArrayLength());

        foreach (var item in variants.EnumerateArray())
        {
            if (
                item.ValueKind is not JsonValueKind.Object
                || Text(item, "version_hash") is not { } version
                || Text(item, "variant_hash") is not { } hash
            )
            {
                continue;
            }

            found.Add(
                new UnjoinedVariant(
                    Number(item, "version_number") ?? 0,
                    version,
                    hash,
                    Text(item, "creator") ?? string.Empty,
                    Text(item, "message") ?? string.Empty
                )
            );
        }

        return found;
    }

    /// <summary>The string a member of an object holds, or nothing when it holds none.</summary>
    /// <param name="item">The object.</param>
    /// <param name="name">The member's name.</param>
    /// <returns>The text, or nothing.</returns>
    private static string? Text(JsonElement item, string name) =>
        item.TryGetProperty(name, out var value) && value.ValueKind is JsonValueKind.String
            ? value.GetString()
            : null;

    /// <summary>The number a member of an object holds, or nothing when it holds none.</summary>
    /// <param name="item">The object.</param>
    /// <param name="name">The member's name.</param>
    /// <returns>The number, or nothing.</returns>
    private static ulong? Number(JsonElement item, string name) =>
        item.TryGetProperty(name, out var value) && value.ValueKind is JsonValueKind.Number
        && value.TryGetUInt64(out var number)
            ? number
            : null;
}

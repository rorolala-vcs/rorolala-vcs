using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>One entry of a Layout, as <c>rola layout entries --json</c> writes it.</summary>
/// <param name="Path">The path it is at, <c>.</c> when it is at none.</param>
/// <param name="Uuid">The <c>Uuid</c> it is known by.</param>
/// <param name="Version">The version it is at, as hex.</param>
/// <param name="Owner">The account that holds it, empty when none does.</param>
/// <param name="Description">What it says about itself.</param>
/// <param name="Deprecated">Whether the Vault has moved it under <c>@/removed/</c>.</param>
public sealed record LayoutEntry(
    string Path,
    string Uuid,
    string Version,
    string Owner,
    string Description,
    bool Deprecated
);

/// <summary>How a reading of a Layout came out.</summary>
/// <remarks>
/// The states are the ones the native side tells apart, and they are told apart because what a
/// caller does next differs: a copy that was never fetched is what a place offers a fetch for, a
/// missing name is a mistake to correct, and a read that failed is a report.
/// </remarks>
public enum LayoutReadState
{
    /// <summary>The Layout was read.</summary>
    Ok,

    /// <summary>No Workspace holds the directory, and it is not inside a Vault either.</summary>
    NoWorkspace,

    /// <summary>The Workspace is not working in a Layout.</summary>
    NoLayout,

    /// <summary>The copy of a Vault's Layout a name asked for was never fetched here.</summary>
    NotCached,

    /// <summary>The name is not one a place has a Layout under.</summary>
    Missing,

    /// <summary>The Layout is there, but it could not be read.</summary>
    Read,
}

/// <summary>
/// What one Layout holds, read as the JSON <c>rola layout entries --json</c> writes.
/// </summary>
/// <remarks>
/// The name says which Layout is read: nothing reads the one being worked in, <c>main</c> names a
/// Workspace's, <c>truth</c> names a Vault's, and <c>name@vault</c> names the read-only copy a fetch
/// left. The state says how it came out, so a caller that offered a fetch for one that was never
/// fetched can tell that from a name that is simply wrong.
/// </remarks>
public sealed class RolaLayoutEntries
{
    /// <summary>Makes a reading of a Layout.</summary>
    /// <param name="state">How the reading came out.</param>
    /// <param name="entries">What the Layout holds, empty when it could not be read.</param>
    /// <param name="reason">Why it could not be read, empty when it was.</param>
    private RolaLayoutEntries(
        LayoutReadState state,
        IReadOnlyList<LayoutEntry> entries,
        string reason
    )
    {
        State = state;
        Entries = entries;
        Reason = reason;
    }

    /// <summary>How the reading came out.</summary>
    public LayoutReadState State { get; }

    /// <summary>What the Layout holds, in path order and then <c>Uuid</c> order.</summary>
    public IReadOnlyList<LayoutEntry> Entries { get; }

    /// <summary>Why the Layout could not be read, empty when it was.</summary>
    public string Reason { get; }

    /// <summary>Reads what a Layout of the place holding a directory holds.</summary>
    /// <param name="directory">The directory to search from, anywhere inside the place.</param>
    /// <param name="layout">
    /// The Layout to read, as <c>layout entries --layout</c> names it, or nothing for the one being
    /// worked in.
    /// </param>
    /// <returns>The reading, whose <see cref="State"/> says how it went.</returns>
    public static RolaLayoutEntries Locate(string directory, string? layout = null)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var at = Marshal.StringToCoTaskMemUTF8(directory);
        var name = layout is null ? 0 : Marshal.StringToCoTaskMemUTF8(layout);

        try
        {
            var answer = RorolalaBinding.rola_layout_entries_json(at, name);

            if (answer.tag == RorolalaResultTag.RorolalaResult_Ok)
            {
                var json = NativeText.Taken(answer.payload);

                return new RolaLayoutEntries(LayoutReadState.Ok, Parse(json), string.Empty);
            }

            var error = Marshal.PtrToStructure<RolaLayoutError>(answer.payload);
            var reason =
                error.tag is RolaLayoutErrorTag.RolaLayoutError_Read
                    ? NativeText.Copied(error.payload.Read.reason)
                    : string.Empty;

            RorolalaBinding.free_rola_layout_error(answer.payload);

            return new RolaLayoutEntries(StateOf(error.tag), [], reason);
        }
        finally
        {
            Marshal.FreeCoTaskMem(at);

            if (name != 0)
            {
                Marshal.FreeCoTaskMem(name);
            }
        }
    }

    /// <summary>The state a tag of the native error stands for.</summary>
    /// <param name="tag">The tag.</param>
    /// <returns>The state.</returns>
    private static LayoutReadState StateOf(RolaLayoutErrorTag tag) =>
        tag switch
        {
            RolaLayoutErrorTag.RolaLayoutError_NoWorkspace => LayoutReadState.NoWorkspace,
            RolaLayoutErrorTag.RolaLayoutError_NoLayout => LayoutReadState.NoLayout,
            RolaLayoutErrorTag.RolaLayoutError_NotCached => LayoutReadState.NotCached,
            RolaLayoutErrorTag.RolaLayoutError_Missing => LayoutReadState.Missing,
            _ => LayoutReadState.Read,
        };

    /// <summary>Reads the entries the native side wrote.</summary>
    /// <param name="json">The answer, which may be the JSON <c>null</c>.</param>
    /// <returns>The entries it names.</returns>
    private static IReadOnlyList<LayoutEntry> Parse(string json)
    {
        if (json.Length == 0)
        {
            return [];
        }

        using var document = JsonDocument.Parse(json);

        if (RolaJson.Items(document.RootElement, "entries") is not { } items)
        {
            return [];
        }

        var found = new List<LayoutEntry>(items.GetArrayLength());

        foreach (var item in items.EnumerateArray())
        {
            if (RolaJson.Text(item, "uuid") is not { Length: > 0 } uuid)
            {
                continue;
            }

            found.Add(
                new LayoutEntry(
                    RolaJson.Text(item, "path") ?? string.Empty,
                    uuid,
                    RolaJson.Text(item, "version") ?? string.Empty,
                    RolaJson.Text(item, "owner") ?? string.Empty,
                    RolaJson.Text(item, "description") ?? string.Empty,
                    RolaJson.Flag(item, "deprecated")
                )
            );
        }

        return found;
    }
}

using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>One Layout, as a place lists it.</summary>
/// <param name="Name">The label it is listed under.</param>
/// <param name="IsCurrent">Whether it is the one the run is working in.</param>
/// <param name="Tracking">The Vault upstream it tracks, empty when it tracks none.</param>
public sealed record LayoutListing(string Name, bool IsCurrent, string Tracking);

/// <summary>
/// The Layouts a place holds, read as the JSON <c>rola layout ls --json</c> writes.
/// </summary>
/// <remarks>
/// A Workspace has one Layout and a Vault keeps one, so the answer is one listing: the label it goes
/// by, that it is the one being worked in, and the Vault it tracks. A directory no place holds reads
/// as an empty list, which is the same answer the command gives a caller that treats a failure as no
/// layouts.
/// </remarks>
public static class RolaLayouts
{
    /// <summary>Finds the Layouts of the place holding a directory.</summary>
    /// <param name="directory">The directory to search from, anywhere inside the place.</param>
    /// <returns>The listings, or an empty list when no place holds the directory.</returns>
    public static IReadOnlyList<LayoutListing> Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var json = NativeText.Taken(RorolalaBinding.rola_layouts_json(path));

            return Parse(json);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Reads the listing the native side wrote.</summary>
    /// <param name="json">The answer, which may be the JSON <c>null</c>.</param>
    /// <returns>The listings it names.</returns>
    private static IReadOnlyList<LayoutListing> Parse(string json)
    {
        if (json.Length == 0)
        {
            return [];
        }

        using var document = JsonDocument.Parse(json);

        if (RolaJson.Items(document.RootElement, "layouts") is not { } items)
        {
            return [];
        }

        var found = new List<LayoutListing>(items.GetArrayLength());

        foreach (var item in items.EnumerateArray())
        {
            if (RolaJson.Text(item, "name") is not { Length: > 0 } name)
            {
                continue;
            }

            found.Add(
                new LayoutListing(
                    name,
                    RolaJson.Flag(item, "is_current"),
                    RolaJson.Text(item, "tracking") ?? string.Empty
                )
            );
        }

        return found;
    }
}

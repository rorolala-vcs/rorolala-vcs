using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>One Vault a Workspace knows, as <c>rola vault ls</c> shows it.</summary>
/// <param name="Name">The name the Workspace knows it by.</param>
/// <param name="Address">The address it answers at.</param>
/// <param name="IsDefault">Whether it is the Vault the Workspace reaches for.</param>
public sealed record VaultBinding(string Name, string Address, bool IsDefault);

/// <summary>
/// A Workspace's configuration, read as the JSON <c>rola vault ls --json</c> writes.
/// </summary>
/// <remarks>
/// The answer is a superset of what the command writes: beside the Vaults and which one is reached
/// for, it carries what a collecting run in the Workspace is told. A directory no Workspace holds,
/// and a configuration that cannot be read, both read as nothing, which is the answer the command
/// gives a caller that treats a failure as no bindings.
/// </remarks>
public sealed class RolaWorkspaceConfig
{
    /// <summary>Makes a reading of a Workspace's configuration.</summary>
    /// <param name="vaults">The Vaults it knows, in name order.</param>
    /// <param name="fallback">The Vault it reaches for, if one has been chosen.</param>
    /// <param name="grace">How long a collecting run keeps what nothing references, in seconds.</param>
    private RolaWorkspaceConfig(
        IReadOnlyList<VaultBinding> vaults,
        string? fallback,
        ulong? grace
    )
    {
        Vaults = vaults;
        Default = fallback;
        GcGrace = grace;
    }

    /// <summary>The Vaults the Workspace knows, in name order.</summary>
    public IReadOnlyList<VaultBinding> Vaults { get; }

    /// <summary>The Vault the Workspace reaches for, if one has been chosen.</summary>
    public string? Default { get; }

    /// <summary>How long a collecting run keeps what nothing references, in seconds.</summary>
    public ulong? GcGrace { get; }

    /// <summary>Reads the configuration of the Workspace holding a directory.</summary>
    /// <param name="directory">The directory to search from, anywhere inside the Workspace.</param>
    /// <returns>The configuration, or nothing when no Workspace holds the directory or it has none.</returns>
    public static RolaWorkspaceConfig? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var json = NativeText.Taken(RorolalaBinding.rola_workspace_config_json(path));

            return Parse(json);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>Reads the configuration the native side wrote.</summary>
    /// <param name="json">The answer, which may be the JSON <c>null</c>.</param>
    /// <returns>The configuration, or nothing when the answer says there is none.</returns>
    private static RolaWorkspaceConfig? Parse(string json)
    {
        if (json.Length == 0)
        {
            return null;
        }

        using var document = JsonDocument.Parse(json);

        if (document.RootElement.ValueKind is not JsonValueKind.Object)
        {
            return null;
        }

        var vaults = new List<VaultBinding>();

        if (RolaJson.Items(document.RootElement, "vaults") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (RolaJson.Text(item, "name") is not { Length: > 0 } name)
                {
                    continue;
                }

                vaults.Add(
                    new VaultBinding(
                        name,
                        RolaJson.Text(item, "address") ?? string.Empty,
                        RolaJson.Flag(item, "is_default")
                    )
                );
            }
        }

        ulong? grace = null;

        if (
            document.RootElement.TryGetProperty("gc", out var gc)
            && gc.ValueKind is JsonValueKind.Object
        )
        {
            grace = RolaJson.Number(gc, "grace");
        }

        return new RolaWorkspaceConfig(
            vaults,
            RolaJson.Text(document.RootElement, "default"),
            grace
        );
    }
}

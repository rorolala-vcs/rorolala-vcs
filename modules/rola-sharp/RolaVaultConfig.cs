using System.Runtime.InteropServices;
using System.Text.Json;

namespace RolaSharp;

/// <summary>
/// What a Vault says about itself, read as one JSON document.
/// </summary>
/// <remarks>
/// The name and description are the Vault's own, the administrators are the members that may move
/// any file in its Layout, and the concurrency is the most transfers its daemon serves at once. A
/// directory no Vault holds, and a configuration that cannot be read, both read as nothing.
/// </remarks>
public sealed class RolaVaultConfig
{
    /// <summary>Makes a reading of a Vault's configuration.</summary>
    /// <param name="name">The name the Vault goes by.</param>
    /// <param name="description">What the Vault says about itself.</param>
    /// <param name="admins">Who may act as an administrator.</param>
    /// <param name="maxConcurrency">The most transfers its daemon carries at once.</param>
    private RolaVaultConfig(
        string name,
        string description,
        IReadOnlyList<string> admins,
        int maxConcurrency
    )
    {
        Name = name;
        Description = description;
        Admins = admins;
        MaxConcurrency = maxConcurrency;
    }

    /// <summary>The name the Vault goes by.</summary>
    public string Name { get; }

    /// <summary>What the Vault says about itself.</summary>
    public string Description { get; }

    /// <summary>Who may act as an administrator, in the order the file names them.</summary>
    public IReadOnlyList<string> Admins { get; }

    /// <summary>The most transfers its daemon carries at once.</summary>
    public int MaxConcurrency { get; }

    /// <summary>Reads the configuration of the Vault holding a directory.</summary>
    /// <param name="directory">The directory to search from, anywhere inside the Vault.</param>
    /// <returns>The configuration, or nothing when no Vault holds the directory or it has none.</returns>
    public static RolaVaultConfig? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var json = NativeText.Taken(RorolalaBinding.rola_vault_config_json(path));

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
    private static RolaVaultConfig? Parse(string json)
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

        var admins = new List<string>();

        if (RolaJson.Items(document.RootElement, "admins") is { } items)
        {
            foreach (var item in items.EnumerateArray())
            {
                if (item.ValueKind is JsonValueKind.String && item.GetString() is { } admin)
                {
                    admins.Add(admin);
                }
            }
        }

        return new RolaVaultConfig(
            RolaJson.Text(document.RootElement, "name") ?? string.Empty,
            RolaJson.Text(document.RootElement, "description") ?? string.Empty,
            admins,
            (int)(RolaJson.Number(document.RootElement, "max_concurrency") ?? 0)
        );
    }
}

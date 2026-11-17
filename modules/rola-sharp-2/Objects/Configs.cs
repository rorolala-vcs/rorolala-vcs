namespace RolaSharp.V2;

/// <summary>One Vault a Workspace knows, as the Workspace's configuration names it.</summary>
/// <param name="Name">The name the Workspace knows it by.</param>
/// <param name="Address">The address it answers at.</param>
/// <param name="IsDefault">Whether it is the Vault the Workspace reaches for.</param>
public sealed record VaultBinding(string Name, string Address, bool IsDefault);

/// <summary>What a collecting run is told, as a Workspace's configuration says it.</summary>
/// <param name="Grace">How long an unreferenced entry is kept, in seconds.</param>
public sealed record GcConfig(ulong? Grace);

/// <summary>A Workspace's configuration: the Vaults it knows and what it collects by.</summary>
/// <param name="Vaults">The Vaults it knows, in name order.</param>
/// <param name="Default">The name it reaches for, when it names one.</param>
/// <param name="Gc">What a collecting run is told, when the file says.</param>
public sealed record WorkspaceConfig(
    IReadOnlyList<VaultBinding> Vaults,
    string? Default,
    GcConfig? Gc
);

/// <summary>A Vault's configuration: what it says about itself.</summary>
/// <param name="Name">The name it goes by.</param>
/// <param name="Description">What it says about itself.</param>
/// <param name="Admins">The accounts that may administer it.</param>
/// <param name="MaxConcurrency">How many connections its daemon keeps at once.</param>
public sealed record VaultConfig(
    string Name,
    string Description,
    IReadOnlyList<string> Admins,
    ushort MaxConcurrency
);

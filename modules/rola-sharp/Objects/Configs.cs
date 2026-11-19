namespace RolaSharp;

/// <summary>One Vault a Workspace knows, as the Workspace's configuration names it.</summary>
/// <param name="Name">The name the Workspace knows it by.</param>
/// <param name="Address">The address it answers at.</param>
/// <param name="IsDefault">Whether it is the Vault the Workspace reaches for.</param>
public sealed record VaultBinding(string Name, string Address, bool IsDefault);

/// <summary>One store a place binds, as its configuration names it.</summary>
/// <param name="Path">The path as the configuration wrote it.</param>
/// <param name="Root">The resolved, absolute path.</param>
/// <param name="IsOwn">Whether this is the place's own store.</param>
/// <param name="Present">Whether the directory is a store this build can open.</param>
/// <param name="Writable">Whether the store would accept a write.</param>
/// <param name="Reason">Why it cannot be used, when it cannot.</param>
public sealed record StorageBinding(
    string Path,
    string Root,
    bool IsOwn,
    bool Present,
    bool Writable,
    string? Reason
);

/// <summary>What a collecting run is told, as a Workspace's configuration says it.</summary>
/// <param name="Grace">How long an unreferenced entry is kept, in seconds.</param>
public sealed record GcConfig(ulong? Grace);

/// <summary>A Workspace's configuration: the Vaults it knows and what it collects by.</summary>
/// <param name="Vaults">The Vaults it knows, in name order.</param>
/// <param name="Default">The name it reaches for, when it names one.</param>
/// <param name="Gc">What a collecting run is told, when the file says.</param>
/// <param name="Storages">The stores it works on, in the order it reaches for them.</param>
public sealed record WorkspaceConfig(
    IReadOnlyList<VaultBinding> Vaults,
    string? Default,
    GcConfig? Gc,
    IReadOnlyList<StorageBinding> Storages
);

/// <summary>A Vault's configuration: what it says about itself.</summary>
/// <param name="Name">The name it goes by.</param>
/// <param name="Description">What it says about itself.</param>
/// <param name="Admins">The accounts that may administer it.</param>
/// <param name="MaxConcurrency">How many connections its daemon keeps at once.</param>
/// <param name="Storages">The stores it works on, in the order it reaches for them.</param>
public sealed record VaultConfig(
    string Name,
    string Description,
    IReadOnlyList<string> Admins,
    ushort MaxConcurrency,
    IReadOnlyList<StorageBinding> Storages
);

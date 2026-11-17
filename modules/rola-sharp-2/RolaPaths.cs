namespace RolaSharp.V2;

/// <summary>The paths and names the programs have to agree on, read from the library.</summary>
public static class RolaPaths
{
    private static readonly Lazy<string> VaultName = new(() =>
        NativeText.Taken(RorolalaBinding.ROLA_VAULT_LAYOUT_NAME())
    );

    /// <summary>The name a Vault's single Layout goes by, which is <c>truth</c>.</summary>
    public static string VaultLayoutName => VaultName.Value;

    /// <summary>Where a Workspace keeps the ignore rules it is managed by.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory the rule files are kept in, as an absolute path.</returns>
    public static string WorkspaceIgnoreRules(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_IGNORE_RULES_DIR);

    /// <summary>Where a Workspace keeps its store.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The store's directory, as an absolute path.</returns>
    public static string WorkspaceStorage(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_STORAGE_DIR);

    /// <summary>Where a Vault keeps its store.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The store's directory, as an absolute path.</returns>
    public static string VaultStorage(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_STORAGE_DIR);

    /// <summary>Resolves a relative path the native side names against a place's root.</summary>
    /// <param name="root">The place's root.</param>
    /// <param name="read">The native reader of the relative path.</param>
    /// <returns>The absolute path.</returns>
    private static string Resolve(string root, Func<nint> read)
    {
        ArgumentNullException.ThrowIfNull(root);

        var relative = NativeText.Taken(read());

        return Path.GetFullPath(Path.Combine(root, relative));
    }
}

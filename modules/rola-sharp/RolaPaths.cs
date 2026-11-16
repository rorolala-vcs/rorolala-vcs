namespace RolaSharp;

/// <summary>
/// The paths and names the two programs have to agree on, read from the library rather than written
/// out again.
/// </summary>
/// <remarks>
/// The values are the command line's own: they cross the C ABI as functions so that one definition
/// serves both programs. A caller that wrote the literal instead would keep a second copy of a name
/// that is the contract across the boundary, which is what this class exists to prevent.
/// </remarks>
public static class RolaPaths
{
    /// <summary>The name a Vault's single Layout goes by.</summary>
    private static readonly Lazy<string> VaultName = new(() =>
        NativeText.Taken(RorolalaBinding.ROLA_VAULT_LAYOUT_NAME())
    );

    /// <summary>The name a Vault's single Layout goes by, which is <c>truth</c>.</summary>
    public static string VaultLayoutName => VaultName.Value;

    /// <summary>Where a Workspace keeps the ignore rules it is managed by.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory the rule files are kept in, as an absolute path.</returns>
    public static string WorkspaceIgnoreRules(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        var relative = NativeText.Taken(
            RorolalaBinding.ROLA_WORKSPACE_IGNORE_RULES_DIR()
        );

        return Path.GetFullPath(Path.Combine(root, relative));
    }
}

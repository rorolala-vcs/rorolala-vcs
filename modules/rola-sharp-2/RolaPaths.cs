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

    /// <summary>Where a Vault keeps the ignore rules it hands down.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The directory the rule files are kept in, as an absolute path.</returns>
    public static string VaultIgnoreRules(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_IGNORE_RULES_DIR);

    /// <summary>Where a Workspace keeps the data that makes it one.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string WorkspaceData(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_DATA_DIR);

    /// <summary>Where a Workspace keeps its store.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The store's directory, as an absolute path.</returns>
    public static string WorkspaceStorage(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_STORAGE_DIR);

    /// <summary>Where a Workspace keeps its Layouts.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string WorkspaceLayout(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_LAYOUT_DIR);

    /// <summary>Where a Workspace keeps its VCS index.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string WorkspaceIndex(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_INDEX_DIR);

    /// <summary>Where a Workspace keeps the copies a fetch left here.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string WorkspaceReadonlyLayouts(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_READONLY_LAYOUTS_DIR);

    /// <summary>Where a Workspace keeps the accounts it knows.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string WorkspaceKeys(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_KEYS_DIR);

    /// <summary>Where a Workspace keeps its readings cache.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string WorkspaceCache(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_CACHE_DIR);

    /// <summary>The file a run edits to write the next message.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The file's path, as an absolute path.</returns>
    public static string WorkspaceEditing(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_EDITING_PATH);

    /// <summary>The Workspace configuration file.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The file's path, as an absolute path.</returns>
    public static string WorkspaceConfig(string root) =>
        Resolve(root, RorolalaBinding.ROLA_WORKSPACE_CONFIG_PATH);

    /// <summary>Where a Vault keeps its Layout.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string VaultLayout(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_LAYOUT_DIR);

    /// <summary>Where a Vault keeps its VCS index.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string VaultIndex(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_INDEX_DIR);

    /// <summary>The Vault configuration file.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The file's path, as an absolute path.</returns>
    public static string VaultConfig(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_CONFIG_PATH);

    /// <summary>Where a Vault keeps the accounts it knows.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string VaultKeys(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_KEYS_DIR);

    /// <summary>Where a Vault keeps the Vaults inside it.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string VaultVaults(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_VAULTS_DIR);

    /// <summary>Where a Vault keeps its store.</summary>
    /// <param name="root">The Vault's root.</param>
    /// <returns>The store's directory, as an absolute path.</returns>
    public static string VaultStorage(string root) =>
        Resolve(root, RorolalaBinding.ROLA_VAULT_STORAGE_DIR);

    /// <summary>Where a Workspace records the Vault its Layout tracks.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The track file's path, as an absolute path.</returns>
    public static string LayoutTrackFile(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        var layout = NativeText.Taken(RorolalaBinding.ROLA_WORKSPACE_LAYOUT_DIR());
        var track = NativeText.Taken(RorolalaBinding.ROLA_LAYOUT_TRACK_FILE());

        return Path.GetFullPath(Path.Combine(root, layout, track));
    }

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

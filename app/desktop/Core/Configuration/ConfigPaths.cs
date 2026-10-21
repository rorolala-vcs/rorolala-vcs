namespace RorolalaDesktop.Configuration;

/// <summary>
/// Where the Desktop program keeps its own files.
/// </summary>
/// <remarks>
/// The files sit under the user's data directory, following the convention the command line keeps
/// its key directory under, so a person looking for either finds both. On Linux the root is
/// <c>~/.local/share</c>; the exact root is whatever the platform's data-directory resolution
/// returns, and <c>~/.local/share</c> is the Linux form only.
/// </remarks>
internal static class ConfigPaths
{
    /// <summary>The file name the plugins' user state is kept in.</summary>
    private const string PluginsFile = "plugins.json";

    /// <summary>The file name the user's preferences are kept in.</summary>
    private const string PreferenceFile = "preference.json";

    /// <summary>The file name the look's three settings are kept in.</summary>
    private const string ThemeFile = "theme.json";

    /// <summary>The file name the dock layout is kept in.</summary>
    private const string LayoutFile = "layout.json";

    /// <summary>The directory the program writes its configuration to.</summary>
    public static string Root { get; } = ResolveRoot();

    /// <summary>Where the plugins' user state is kept.</summary>
    public static string Plugins => Path.Combine(Root, PluginsFile);

    /// <summary>Where the user's preferences are kept.</summary>
    public static string Preference => Path.Combine(Root, PreferenceFile);

    /// <summary>Where the look's three settings are kept.</summary>
    public static string Theme => Path.Combine(Root, ThemeFile);

    /// <summary>Where the dock layout is kept.</summary>
    public static string Layout => Path.Combine(Root, LayoutFile);

    /// <summary>
    /// The directory a plugin keeps what it can make again in.
    /// </summary>
    /// <remarks>
    /// It stands beside the configuration rather than among it, so that a person emptying the cache empties
    /// one directory, and nothing written to it can be mistaken for a setting somebody chose.
    /// </remarks>
    public static string Cache => Path.Combine(Root, "cache");

    /// <summary>
    /// The platform's data directory, with this program's own directory under it.
    /// </summary>
    private static string ResolveRoot()
    {
        var data = Environment.GetFolderPath(
            Environment.SpecialFolder.LocalApplicationData,
            Environment.SpecialFolderOption.DoNotVerify
        );

        // A platform that answers with nothing has no data directory to speak of; keeping the files
        // under the working directory at least leaves the program runnable rather than unwritable.
        var root = string.IsNullOrEmpty(data) ? "." : data;

        return Path.Combine(root, "rola", "desktop");
    }
}

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

    /// <summary>The file name the dock layout is kept in.</summary>
    private const string LayoutFile = "layout.json";

    /// <summary>The directory the program writes its configuration to.</summary>
    public static string Root { get; } = ResolveRoot();

    /// <summary>Where the plugins' user state is kept.</summary>
    public static string Plugins => Path.Combine(Root, PluginsFile);

    /// <summary>Where the user's preferences are kept.</summary>
    public static string Preference => Path.Combine(Root, PreferenceFile);

    /// <summary>Where the dock layout is kept.</summary>
    public static string Layout => Path.Combine(Root, LayoutFile);

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

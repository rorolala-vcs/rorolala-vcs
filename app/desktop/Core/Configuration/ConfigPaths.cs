using Rorolala;

namespace RorolalaDesktop.Configuration;

/// <summary>
/// Where the Desktop program keeps its own files.
/// </summary>
/// <remarks>
/// The files sit under the user's data directory, under the userdata name this build was made
/// for, beside the key directory the command line keeps, so a person looking for either finds
/// both. On Linux that is <c>~/.local/share/rola/&lt;name&gt;</c>; the base is whatever the
/// platform's data-directory resolution returns, and <c>~/.local/share</c> is the Linux form
/// only. <see cref="UserDataPaths"/> is the one place that answers the question.
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
    public static string Root { get; } = UserDataPaths.Desktop;

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
}

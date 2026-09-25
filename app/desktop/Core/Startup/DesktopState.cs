using RorolalaDesktop.Configuration;
using RorolalaDesktop.Plugins;

namespace RorolalaDesktop;

/// <summary>
/// What a run works out before Avalonia starts.
/// </summary>
/// <remarks>
/// Configuration and the plugin load order depend on each other and on nothing Avalonia provides, so
/// they are worked out first, while a failure can still be a reason on standard error and an exit
/// code rather than a window with nothing in it.
/// </remarks>
internal sealed class DesktopState
{
    /// <summary>The user's preferences.</summary>
    public required PreferenceConfiguration Preference { get; init; }

    /// <summary>The two things the user chooses about how the program looks.</summary>
    public required ThemeConfiguration Theme { get; init; }

    /// <summary>The plugins, discovered and ordered.</summary>
    public required PluginManager Plugins { get; init; }
}

/// <summary>
/// The part of startup that happens before Avalonia: the configuration, discovery, and ordering.
/// </summary>
internal static class DesktopStartup
{
    /// <summary>The directory the plugin assemblies sit in, beside the program.</summary>
    public static string PluginDirectory { get; } = Path.Combine(AppContext.BaseDirectory, "plugins");

    /// <summary>
    /// Reads the configuration, finds the plugins, and works out the load order.
    /// </summary>
    /// <returns>Everything a run works out before Avalonia starts.</returns>
    /// <exception cref="ConfigurationFailure">The configuration cannot be read or honoured.</exception>
    public static DesktopState Load()
    {
        // The preference is read first so that a language named only there can be spoken by the
        // reasons below, and the plugins are read second so that discovery can check the file's keys
        // against what was actually found.
        var preference = ConfigurationLoader.LoadPreference();
        var theme = ConfigurationLoader.LoadTheme();
        var plugins = new PluginManager(PluginDirectory);

        plugins.Load(ConfigurationLoader.LoadPlugins());

        return new DesktopState
        {
            Preference = preference,
            Theme = theme,
            Plugins = plugins,
        };
    }
}

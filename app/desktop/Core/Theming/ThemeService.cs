using Avalonia;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Theming;

/// <summary>
/// Applies the theme <c>preference.json</c> names as an overlay on the base theme.
/// </summary>
/// <remarks>
/// <c>FluentTheme</c> is always loaded as the base, because Avalonia's controls need it; the named
/// theme is layered on top. The value <c>fluent</c> means the base alone. A theme with no provider
/// stops the program before the window exists, since the user's preference cannot be honoured
/// (Section 5.5). A change of theme takes effect on the next start.
/// </remarks>
internal sealed class ThemeService
{
    /// <summary>The id that means the base theme with no overlay.</summary>
    public const string Fluent = "fluent";

    /// <summary>Where the registered themes are.</summary>
    private readonly ThemeRegistry _themes;

    /// <summary>Makes a service over the registered themes.</summary>
    /// <param name="themes">Where the registered themes are.</param>
    public ThemeService(ThemeRegistry themes) => _themes = themes;

    /// <summary>
    /// Applies the named theme.
    /// </summary>
    /// <param name="themeId">The id <c>preference.json</c> names.</param>
    /// <exception cref="ConfigurationFailure">No provider supplies that id.</exception>
    public void Apply(string themeId)
    {
        if (themeId == Fluent)
        {
            return;
        }

        var provider = _themes.Find(themeId);

        if (provider is null)
        {
            throw new ConfigurationFailure(
                ExitCode.Unavailable,
                $"{ConfigPaths.Preference}: no theme provider supplies `{themeId}`"
            );
        }

        var styles = Application.Current?.Styles;

        if (styles is null)
        {
            return;
        }

        foreach (var style in provider.Styles)
        {
            styles.Add(style);
        }
    }
}

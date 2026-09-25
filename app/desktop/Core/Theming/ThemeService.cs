using Avalonia;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Theming;

/// <summary>
/// Applies the theme <c>preference.json</c> names as an overlay on the base theme.
/// </summary>
/// <remarks>
/// <c>SimpleTheme</c> is always loaded as the base, because Avalonia's controls need a theme to have a
/// template at all; the named theme is layered on top. The value <c>simple</c> means the base alone. A
/// theme with no provider stops the program before the window exists, since the user's preference
/// cannot be honoured (Section 5.5). A change of theme takes effect on the next start.
/// <para>
/// Simple rather than Fluent is the base by design: Fluent is a whole look, and half of what an
/// overlay writes on top of it is spent overriding what Fluent had already decided — its resources,
/// its rounded templates, its accent family. Simple decides little, so a theme on top of it says what
/// it means. What it costs is that Simple is plain: a hover it does not draw is a hover nobody draws
/// (Section 10).
/// </para>
/// <para>
/// The overlay is added to <c>Application.Styles</c> once, before the window is made, and that order
/// is load-bearing rather than tidy. Avalonia re-applies the whole list to every element that is
/// already styled when the list grows, and on that second pass the base theme's setters win over the
/// overlay's: a style added after the window exists silently stops applying to what is already on
/// screen. Nothing may therefore be added to <c>Application.Styles</c> after this has run.
/// </para>
/// </remarks>
internal sealed class ThemeService
{
    /// <summary>The id that means the base theme with no overlay.</summary>
    public const string Simple = "simple";

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
        if (themeId == Simple)
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

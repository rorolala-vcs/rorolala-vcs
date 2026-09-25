using Avalonia.Styling;

namespace RorolalaDesktop.Contract;

/// <summary>
/// A theme a plugin supplies, applied as an overlay on top of the always-present base theme.
/// </summary>
/// <remarks>
/// A plugin-provided theme is treated exactly like the built-in one; only its origin differs. The
/// theme a run uses is named in <c>preference.json</c> and takes effect on the next start.
/// </remarks>
public interface IThemeProvider
{
    /// <summary>The id <c>preference.json</c> names this theme by.</summary>
    string ThemeId { get; }

    /// <summary>The styles layered over the base theme.</summary>
    IReadOnlyList<IStyle> Styles { get; }
}

/// <summary>Where a plugin registers themes.</summary>
public interface IThemeRegistry
{
    /// <summary>
    /// Registers a theme.
    /// </summary>
    /// <param name="provider">The theme to register.</param>
    void Add(IThemeProvider provider);
}

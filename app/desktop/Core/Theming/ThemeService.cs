using Avalonia;
using Avalonia.Styling;
using RorolalaDesktop.Configuration;

namespace RorolalaDesktop.Theming;

/// <summary>
/// Applies how the program looks: the one look it has, in the two things the user chooses about it.
/// </summary>
/// <remarks>
/// There is one look and it is the program's own (Section 10). What a run chooses is the variant it is
/// drawn in and the accent everything accented is drawn in, both read from <c>theme.json</c> before
/// there is a window.
/// <para>
/// The base theme is already loaded, because Avalonia's controls have no template without one, so what
/// is applied here is the overlay that says what the program looks like.
/// </para>
/// <para>
/// The overlay is added to <c>Application.Styles</c> once, before the window is made, and that order
/// is load-bearing rather than tidy. Avalonia re-applies the whole list to every element that is
/// already styled when the list grows, and on that second pass the base theme's setters win over the
/// overlay's: a style added after the window exists silently stops applying to what is already on
/// screen. Nothing may therefore be added to <c>Application.Styles</c> after this has run.
/// </para>
/// </remarks>
internal static class ThemeService
{
    /// <summary>
    /// Applies the look, in the variant and the accent the user chose.
    /// </summary>
    /// <param name="theme">What the user chose.</param>
    public static void Apply(ThemeConfiguration theme)
    {
        if (Application.Current is not { } application)
        {
            return;
        }

        // Set on the application rather than named by the overlay: which of the two variants is being
        // drawn is every colour the base theme owns, and `System` is a variant that goes on following
        // the desktop as the desktop changes.
        application.RequestedThemeVariant = Variant(theme.Mode);

        foreach (var style in new RorolalaTheme(theme.Accent).Styles)
        {
            application.Styles.Add(style);
        }
    }

    /// <summary>The variant a mode stands for.</summary>
    /// <param name="mode">The mode the file names.</param>
    /// <returns>The variant to draw in.</returns>
    private static ThemeVariant Variant(ColorMode mode) =>
        mode switch
        {
            ColorMode.Light => ThemeVariant.Light,
            ColorMode.Dark => ThemeVariant.Dark,
            _ => ThemeVariant.Default,
        };
}

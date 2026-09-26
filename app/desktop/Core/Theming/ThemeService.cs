using Avalonia;
using Avalonia.Styling;
using RorolalaDesktop.Configuration;

namespace RorolalaDesktop.Theming;

/// <summary>
/// Applies how the program looks: the one look it has, in the three things the user chooses about it.
/// </summary>
/// <remarks>
/// There is one look and it is the program's own (Section 10). What a run chooses is the variant it is
/// drawn in and the two colours it is drawn with, all read from <c>theme.json</c> before there is a
/// window.
/// <para>
/// The base theme is already loaded, because Avalonia's controls have no template without one, so what
/// is applied here is the overlay that says what the program looks like.
/// </para>
/// <para>
/// The overlay is added to <c>Application.Styles</c> once, before the window is made, and that order is
/// load-bearing rather than tidy. Avalonia re-applies the whole list to every element that is already
/// styled when the list grows, and on that second pass the base theme's setters win over the overlay's:
/// a style added after the window exists silently stops applying to what is already on screen. Nothing
/// may therefore be added to <c>Application.Styles</c> after this has run — which is why a colour
/// changed while the program runs replaces a <em>resource</em> rather than a style (Section 10).
/// </para>
/// </remarks>
internal sealed class ThemeService
{
    /// <summary>The application whose styles and variant are set, or nothing where there is none.</summary>
    /// <remarks>
    /// Nothing is the case a test that only reads what the panel shows runs under: there is no
    /// application to dress, and applying there is a no-op rather than a failure.
    /// </remarks>
    private readonly Application? _application;

    /// <summary>What the user chose, which the preference panel edits in place.</summary>
    private readonly ThemeConfiguration _theme;

    /// <summary>The look that is attached, once one is: what a later colour replaces the palette of.</summary>
    private RorolalaTheme? _look;

    /// <summary>Makes the service over what the user chose.</summary>
    /// <param name="application">The application to dress, or nothing where there is none.</param>
    /// <param name="theme">What the user chose about how it looks.</param>
    public ThemeService(Application? application, ThemeConfiguration theme)
    {
        _application = application;
        _theme = theme;
    }

    /// <summary>What the user chose, which the panel reads and edits.</summary>
    public ThemeConfiguration Theme => _theme;

    /// <summary>
    /// Applies the look, in the variant and the colours the user chose.
    /// </summary>
    /// <remarks>
    /// The first call attaches the look; every call after it replaces the colours of the palette already
    /// attached, so that a colour changed while the program runs reaches every control at once and no
    /// style is added after the window (Section 10).
    /// </remarks>
    public void Apply()
    {
        if (_application is null)
        {
            return;
        }

        // Set on the application rather than named by the overlay: which of the two variants is being
        // drawn is every colour the base theme owns, and `System` is a variant that goes on following
        // the desktop as the desktop changes.
        _application.RequestedThemeVariant = Variant(_theme.ModeOrDefault);

        var look = new RorolalaTheme(_theme.PrimaryOrDefault, _theme.AccentOrDefault, _theme.PrimaryText);

        if (_look is null)
        {
            foreach (var style in look.Styles)
            {
                _application.Styles.Add(style);
            }

            _look = look;
            return;
        }

        look.Recolour(_look.Palette, _look.LightPalette, _look.DarkPalette);
    }

    /// <summary>
    /// Writes what the user chose and applies it, which is what an edit in the panel does.
    /// </summary>
    /// <remarks>
    /// The file is written at once rather than at exit, because a choice the user made and a run that
    /// then failed is a choice that would otherwise be lost with the failure.
    /// </remarks>
    public void Save()
    {
        ConfigurationLoader.WriteTheme(_theme);
        Apply();
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

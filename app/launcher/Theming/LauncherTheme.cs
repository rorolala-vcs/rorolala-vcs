using Avalonia;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;

namespace RorolalaLauncher.Theming;

/// <summary>
/// How the launcher looks: the day/night choice and the colours, read from the Desktop program's own file.
/// </summary>
/// <remarks>
/// The two programs share <c>theme.json</c>, so a choice made in either is the choice in force in both.
/// The file states a variant and seven colours; which variant is in force is the application's answer when
/// the file says "follow the desktop", and asking the application is what makes following the desktop work
/// without a second mechanism.
/// <para>
/// A colour changed while the program runs raises <see cref="Changed"/>, which is what every view of the
/// window repaints from: there is no second statement of a colour anywhere to keep in step.
/// </para>
/// </remarks>
/// <param name="application">The application whose variant is set.</param>
/// <param name="configuration">What the user chose about how the programs look.</param>
internal sealed class LauncherTheme(Application application, ThemeConfiguration configuration)
{
    /// <summary>What the user chose, which the settings dialog edits.</summary>
    public ThemeConfiguration Configuration => configuration;

    /// <summary>The colours in force.</summary>
    public Skin Skin { get; private set; } = new(ThemePalette.DefaultLight);

    /// <summary>The variant the file asks for.</summary>
    public ColorMode Mode => configuration.ModeOrDefault;

    /// <summary>Raised when the colours in force have changed.</summary>
    public event EventHandler? Changed;

    /// <summary>
    /// Applies what the file states: the variant, and the colours of that variant.
    /// </summary>
    public void Apply()
    {
        var asked = Variant(Mode);

        if (application.RequestedThemeVariant != asked)
        {
            application.RequestedThemeVariant = asked;
        }

        Refresh();
    }

    /// <summary>
    /// Reads the colours of the variant now in force, which is what following the desktop means.
    /// </summary>
    public void Refresh()
    {
        Skin = new Skin(configuration.PaletteOrDefault(application.ActualThemeVariant));

        Changed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>
    /// Chooses a variant and writes it, which is what a choice in the settings dialog does.
    /// </summary>
    /// <remarks>
    /// The file is written at once rather than at exit, because a choice the user made and a run that
    /// then ended is a choice that would otherwise be lost.
    /// </remarks>
    /// <param name="mode">The variant to draw in.</param>
    /// <exception cref="ConfigurationFailure">The file could not be written.</exception>
    public void Choose(ColorMode mode)
    {
        configuration.Mode = mode;

        ConfigurationLoader.WriteTheme(configuration);

        Apply();
    }

    /// <summary>The variant a mode stands for.</summary>
    /// <param name="mode">The mode the file names.</param>
    /// <returns>The variant to ask the application for.</returns>
    private static ThemeVariant Variant(ColorMode mode) =>
        mode switch
        {
            ColorMode.Light => ThemeVariant.Light,
            ColorMode.Dark => ThemeVariant.Dark,
            _ => ThemeVariant.Default,
        };
}

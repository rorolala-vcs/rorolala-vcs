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
/// The file is watched while the program runs, so a colour changed in the Desktop program while this one is
/// open is read and drawn here rather than at the next start: the two are drawn from one choice, and a choice
/// made beside this window is one this window can see. A colour changed here raises <see cref="Changed"/>, which
/// is what every view of the window repaints from: there is no second statement of a colour anywhere to keep in
/// step.
/// </para>
/// </remarks>
internal sealed class LauncherTheme : IDisposable
{
    /// <summary>The application whose variant is set.</summary>
    private readonly Application _application;

    /// <summary>What the user chose about how the programs look.</summary>
    private readonly ThemeConfiguration _configuration;

    /// <summary>Who follows the file while the program runs, so a choice made beside it is seen here.</summary>
    private readonly ThemeFileWatch _watch;

    /// <summary>Makes the look over the file the two programs share.</summary>
    /// <param name="application">The application whose variant is set.</param>
    /// <param name="configuration">What the user chose about how the programs look.</param>
    public LauncherTheme(Application application, ThemeConfiguration configuration)
    {
        ArgumentNullException.ThrowIfNull(application);
        ArgumentNullException.ThrowIfNull(configuration);

        _application = application;
        _configuration = configuration;
        _watch = new ThemeFileWatch(ConfigPaths.Root, "theme.json", Followed);
    }

    /// <summary>What the user chose, which the settings dialog edits.</summary>
    public ThemeConfiguration Configuration => _configuration;

    /// <summary>The colours in force.</summary>
    public Skin Skin { get; private set; } = new(ThemePalette.DefaultLight);

    /// <summary>The variant the file asks for.</summary>
    public ColorMode Mode => _configuration.ModeOrDefault;

    /// <summary>Raised when the colours in force have changed.</summary>
    public event EventHandler? Changed;

    /// <summary>
    /// Applies what the file states: the variant, and the colours of that variant.
    /// </summary>
    public void Apply()
    {
        var asked = Variant(Mode);

        if (_application.RequestedThemeVariant != asked)
        {
            _application.RequestedThemeVariant = asked;
        }

        Refresh();
    }

    /// <summary>
    /// Reads the colours of the variant now in force, which is what following the desktop means.
    /// </summary>
    public void Refresh()
    {
        Skin = new Skin(_configuration.PaletteOrDefault(_application.ActualThemeVariant));

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
        _configuration.Mode = mode;

        ConfigurationLoader.WriteTheme(_configuration);

        Apply();
    }

    /// <summary>Stops following the file.</summary>
    public void Dispose()
    {
        _watch.Dispose();

        GC.SuppressFinalize(this);
    }

    /// <summary>
    /// Reads the file again, which is what a choice made in the Desktop program looks like.
    /// </summary>
    /// <remarks>
    /// What is read is put into the object already held rather than in place of it, because that object is what
    /// the settings dialog edits and what the colours are worked out from. A file caught while it is being
    /// written is not a choice, so a read that fails is left for the next write.
    /// </remarks>
    private void Followed()
    {
        ThemeConfiguration latest;

        try
        {
            latest = ConfigurationLoader.LoadTheme();
        }
        catch (ConfigurationFailure)
        {
            return;
        }

        _configuration.CopyFrom(latest);
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

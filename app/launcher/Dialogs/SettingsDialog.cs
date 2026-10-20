using Avalonia.Controls;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// What this program may be told about how it is drawn: the day/night choice, which is the Desktop
/// program's own file, and the window's frame, which is this program's own choice.
/// </summary>
/// <remarks>
/// The day/night choice is written to <c>theme.json</c>, which the Desktop program reads, so a choice made
/// here is the choice in force in both. The frame is written to this program's settings and handed to the
/// Desktop program when a window is opened from here, because the frame is the launcher's own: the two
/// programs are drawn alike because they are told alike, not because one writes the other's file.
/// </remarks>
internal sealed class SettingsDialog : DialogWindow
{
    /// <summary>The look, which owns the file the day/night choice is written to.</summary>
    private readonly LauncherTheme _theme;

    /// <summary>What this program was told, which is what a choice is written to.</summary>
    private readonly LauncherSettings _settings;

    /// <summary>Where the settings are kept.</summary>
    private readonly LauncherFiles _files;

    /// <summary>How this machine draws a frame, which is what an unchosen frame is.</summary>
    private readonly WindowStyle _machine;

    /// <summary>The colours of the variant in force.</summary>
    private Skin _skin;

    /// <summary>Makes the window.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="theme">The look.</param>
    /// <param name="settings">What this program was told.</param>
    /// <param name="files">Where the settings are kept.</param>
    /// <param name="machine">How this machine draws a frame.</param>
    public SettingsDialog(
        Skin skin,
        LauncherTheme theme,
        LauncherSettings settings,
        LauncherFiles files,
        WindowStyle machine
    )
        : base(skin, RolaI18N.Get("launcher.settings.title"), 480, 420)
    {
        ArgumentNullException.ThrowIfNull(theme);
        ArgumentNullException.ThrowIfNull(settings);
        ArgumentNullException.ThrowIfNull(files);

        _skin = skin;
        _theme = theme;
        _settings = settings;
        _files = files;
        _machine = machine;

        Build();
    }

    /// <summary>The frame in force once the window is closed, which is what the launcher is drawn in.</summary>
    public WindowStyle Frame => _settings.Frame ?? _machine;

    /// <summary>
    /// Draws the choices, with the one in force standing out.
    /// </summary>
    /// <remarks>
    /// The window is drawn again whole after a choice rather than recoloured in place: what a choice moves
    /// is which button stands out, and the two are one statement of what is in force.
    /// </remarks>
    private void Build()
    {
        Body.Children.Clear();

        Body.Children.Add(Line(_skin, RolaI18N.Get("launcher.settings.appearance"), 12, muted: true));

        // The three choices are pictures of the window each one would draw, rather than the three names: the
        // light one and the dark one are the same window in the colours of each, and the one that follows the
        // system is that window split across its own diagonal.
        var modes = new StackPanel { Orientation = Avalonia.Layout.Orientation.Horizontal, Spacing = 8 };

        var light = _theme.Configuration.LightTheme;
        var dark = _theme.Configuration.Theme;

        modes.Children.Add(
            new ThemeButton(ColorMode.Light, ThemePreview.Of(light), _theme.Mode == ColorMode.Light, _skin, Choose)
        );
        modes.Children.Add(
            new ThemeButton(ColorMode.Dark, ThemePreview.Of(dark), _theme.Mode == ColorMode.Dark, _skin, Choose)
        );
        modes.Children.Add(
            new ThemeButton(
                ColorMode.System,
                ThemePreview.HalfAndHalf(light, dark),
                _theme.Mode == ColorMode.System,
                _skin,
                Choose
            )
        );

        Body.Children.Add(modes);

        Body.Children.Add(Line(_skin, RolaI18N.Get("launcher.settings.frame"), 12, muted: true));

        var frames = new StackPanel { Orientation = Avalonia.Layout.Orientation.Horizontal, Spacing = 8 };

        frames.Children.Add(
            Choice(Frame == WindowStyle.Windows, RolaI18N.Get("launcher.settings.style_windows"), () => ChooseFrame(WindowStyle.Windows))
        );
        frames.Children.Add(
            Choice(Frame == WindowStyle.Mac, RolaI18N.Get("launcher.settings.style_mac"), () => ChooseFrame(WindowStyle.Mac))
        );
        frames.Children.Add(
            Choice(
                Frame == WindowStyle.TillingWm,
                RolaI18N.Get("launcher.settings.style_tillingwm"),
                () => ChooseFrame(WindowStyle.TillingWm)
            )
        );

        Body.Children.Add(frames);

        var done = new FlatButton(Line(_skin, RolaI18N.Get("launcher.common.done")), _skin, ButtonKind.Primary);

        done.Clicked += (_, _) => Close();

        Body.Children.Add(Row(done));
    }

    /// <summary>A choice, which stands out when it is the one in force.</summary>
    /// <param name="chosen">Whether it is the one in force.</param>
    /// <param name="label">What it is called.</param>
    /// <param name="choose">What choosing it does.</param>
    /// <returns>The button.</returns>
    private FlatButton Choice(bool chosen, string label, Action choose)
    {
        var button = new FlatButton(
            Line(_skin, label),
            _skin,
            chosen ? ButtonKind.Primary : ButtonKind.Surface
        );

        button.Clicked += (_, _) => choose();

        return button;
    }

    /// <summary>Chooses the day/night variant, writes it, and draws the window again.</summary>
    /// <param name="mode">The variant.</param>
    private void Choose(ColorMode mode)
    {
        try
        {
            _theme.Choose(mode);
        }
        catch (ConfigurationFailure failure)
        {
            // The file is the Desktop program's own and this program only edits it. A write that failed
            // leaves the choice where it was, and the window drawn in what is still in force.
            _ = failure;
        }

        _skin = _theme.Skin;

        Repaint(_skin);
        Build();
    }

    /// <summary>Chooses how the frame is drawn, writes it, and draws the window again.</summary>
    /// <param name="style">The style.</param>
    private void ChooseFrame(WindowStyle style)
    {
        _settings.Frame = style;

        try
        {
            _settings.Write(_files.Settings);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            // A choice that could not be kept is still a choice for this run, which is what the window
            // is showing; the next run asks the machine again.
            _ = error;
        }

        Build();
    }
}

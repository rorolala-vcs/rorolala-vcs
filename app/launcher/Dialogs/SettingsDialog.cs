using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// What this program may be told about how the windows it opens are drawn: the day/night choice, which is
/// the Desktop program's own file, and the frame of the Desktop window it opens.
/// </summary>
/// <remarks>
/// The day/night choice is written to <c>theme.json</c>, which the Desktop program reads, so a choice made
/// here is the choice in force in both. The frame is written to this program's settings and handed to the
/// Desktop program when a window is opened from here. The launcher's own window keeps the system title bar
/// and border, so this choice is about the Desktop window and not about the launcher.
/// <para>
/// Each choice is a segmented control rather than a row of buttons: the options are one control with the
/// one in force filled, which is how a setting reads as one setting rather than as several actions.
/// </para>
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
        : base(skin, RolaI18N.Get("launcher.settings.title"), 440, 320)
    {
        ArgumentNullException.ThrowIfNull(theme);
        ArgumentNullException.ThrowIfNull(settings);
        ArgumentNullException.ThrowIfNull(files);

        _skin = skin;
        _theme = theme;
        _settings = settings;
        _files = files;
        _machine = machine;

        Height = double.NaN;
        SizeToContent = SizeToContent.Height;
        Body.Margin = new Thickness(18, 0, 18, 8);
        Body.Spacing = 0;

        Build();
    }

    /// <summary>The frame in force once the window is closed, which is what the Desktop window is drawn in.</summary>
    public WindowStyle Frame => _settings.Frame ?? _machine;

    /// <summary>
    /// Draws the two choices, with the one in force filled.
    /// </summary>
    /// <remarks>
    /// The window is drawn again whole after a choice rather than recoloured in place: what a choice moves
    /// is which segment stands out, and the two are one statement of what is in force.
    /// </remarks>
    private void Build()
    {
        Body.Children.Clear();

        var theme = new Segmented(
            [
                ("light", RolaI18N.Get("launcher.settings.light")),
                ("dark", RolaI18N.Get("launcher.settings.dark")),
                ("system", RolaI18N.Get("launcher.settings.system")),
            ],
            Mode(_theme.Mode),
            _skin,
            PickMode
        );

        var frame = new Segmented(
            [
                ("windows", RolaI18N.Get("launcher.settings.style_windows")),
                ("mac", RolaI18N.Get("launcher.settings.style_mac")),
                ("tiling", RolaI18N.Get("launcher.settings.style_tilingwm")),
            ],
            Style(Frame),
            _skin,
            PickFrame
        );

        Body.Children.Add(
            Setting(_skin, RolaI18N.Get("launcher.settings.appearance"), theme, first: true)
        );

        Body.Children.Add(
            Setting(_skin, RolaI18N.Get("launcher.settings.frame"), frame, first: false)
        );

        var done = new FlatButton(Line(_skin, RolaI18N.Get("launcher.common.done")), _skin, ButtonKind.Primary)
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
            Margin = new Thickness(0, 12, 0, 0),
        };

        done.Clicked += (_, _) => Close();

        var footer = Row(done);

        footer.Margin = new Thickness(0, 12, 0, 8);
        Body.Children.Add(footer);
    }

    /// <summary>One setting: its name and the control that chooses it.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="title">What the setting is called.</param>
    /// <param name="control">The control.</param>
    /// <param name="first">Whether it is the first row, which has no line above it.</param>
    /// <returns>The row.</returns>
    private static Control Setting(Skin skin, string title, Control control, bool first)
    {
        ArgumentNullException.ThrowIfNull(skin);

        // The name stands on its own now that the row carries no sentence beside it, so it sits on the
        // control's own middle line rather than at the top of a block of copy.
        var label = new TextBlock
        {
            Text = title,
            FontSize = 13,
            FontWeight = FontWeight.SemiBold,
            Foreground = skin.Front,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            ColumnSpacing = 18,
            Margin = new Thickness(0, 12, 0, 12),
        };

        row.Children.Add(label);

        Grid.SetColumn(control, 1);
        control.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(control);

        if (first)
        {
            return row;
        }

        return new Border
        {
            BorderThickness = new Thickness(0, 1, 0, 0),
            BorderBrush = skin.BorderSoft,
            Child = row,
        };
    }

    /// <summary>The word a day/night choice is named by.</summary>
    /// <param name="mode">The choice.</param>
    /// <returns>The word.</returns>
    private static string Mode(ColorMode mode) =>
        mode switch
        {
            ColorMode.Light => "light",
            ColorMode.Dark => "dark",
            _ => "system",
        };

    /// <summary>The word a frame is named by.</summary>
    /// <param name="style">The frame.</param>
    /// <returns>The word.</returns>
    private static string Style(WindowStyle style) =>
        style switch
        {
            WindowStyle.Mac => "mac",
            WindowStyle.TilingWm => "tiling",
            _ => "windows",
        };

    /// <summary>Chooses a word's day/night variant.</summary>
    /// <param name="word">The word chosen.</param>
    private void PickMode(string word)
    {
        Choose(
            word switch
            {
                "light" => ColorMode.Light,
                "dark" => ColorMode.Dark,
                _ => ColorMode.System,
            }
        );
    }

    /// <summary>Chooses a word's frame.</summary>
    /// <param name="word">The word chosen.</param>
    private void PickFrame(string word) =>
        ChooseFrame(
            word switch
            {
                "mac" => WindowStyle.Mac,
                "tiling" => WindowStyle.TilingWm,
                _ => WindowStyle.Windows,
            }
        );

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

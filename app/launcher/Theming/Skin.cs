using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaLauncher.Theming;

/// <summary>
/// The colours the launcher is drawn from, which are the colours the Desktop program is drawn from.
/// </summary>
/// <remarks>
/// The palette is the one <c>theme.json</c> states, so the two programs are drawn from one choice: the
/// variant in force and the seven colours are read there, and this only turns them into the brushes a
/// control is given. What is added is what a picture needs and a palette does not name, a muted ink, a
/// hovered surface, an ink that can be read on the primary, and each is worked out from the palette
/// rather than written down, so that a colour the user chooses does not bring a second colour with it.
/// </remarks>
internal sealed class Skin
{
    /// <summary>The palette this was made from.</summary>
    public ThemePalette Palette { get; }

    /// <summary>The face a card or a dialog is filled with.</summary>
    public IBrush Bright { get; }

    /// <summary>The ground the window is drawn on.</summary>
    public IBrush Mid { get; }

    /// <summary>The hollow an input is filled with.</summary>
    public IBrush Dark { get; }

    /// <summary>The ink things are written in.</summary>
    public IBrush Front { get; }

    /// <summary>The line that tells one surface from another.</summary>
    public IBrush Splitter { get; }

    /// <summary>What is chosen, and the one fill that carries white or black on it.</summary>
    public IBrush Primary { get; }

    /// <summary>The warm colour the marks a drag draws are drawn in.</summary>
    public IBrush Accent { get; }

    /// <summary>What is written on a surface filled with the primary.</summary>
    public IBrush OnPrimary { get; }

    /// <summary>An ink for what is beside the thing it belongs to rather than the thing itself.</summary>
    public IBrush Muted { get; }

    /// <summary>An ink for what is present but not yet read, such as a mark that is not checked.</summary>
    public IBrush Faint { get; }

    /// <summary>The surface a pointer is over.</summary>
    public IBrush Hover { get; }

    /// <summary>The line between two slots of one rack, which is lighter than the rack's own edge.</summary>
    public IBrush BorderSoft { get; }

    /// <summary>The wash the ear of a rack slot is cut into.</summary>
    public IBrush Ear { get; }

    /// <summary>The wash a chosen row is filled with.</summary>
    public IBrush Selection { get; }

    /// <summary>What a warning or a failure is written in.</summary>
    public IBrush Alarm { get; }

    /// <summary>The colour <see cref="Alarm"/> is made of, for a wash that is the same colour thinned.</summary>
    public Color AlarmColour { get; }

    /// <summary>What a warning is written in.</summary>
    public IBrush Warning { get; }

    /// <summary>The colour <see cref="Warning"/> is made of.</summary>
    public Color WarningColour { get; }

    /// <summary>What a warning's filled button is written in.</summary>
    public IBrush OnWarning { get; }

    /// <summary>What something that is settled is written in.</summary>
    public IBrush Success { get; }

    /// <summary>The colour <see cref="Success"/> is made of.</summary>
    public Color SuccessColour { get; }

    /// <summary>The family addresses and paths are written in.</summary>
    public FontFamily Mono { get; } = new("Cascadia Mono, Consolas, DejaVu Sans Mono, monospace");

    /// <summary>Makes the brushes from a palette.</summary>
    /// <param name="palette">The colours in force.</param>
    public Skin(ThemePalette palette)
    {
        Palette = palette;

        Bright = new SolidColorBrush(palette.Bright);
        Mid = new SolidColorBrush(palette.Mid);
        Dark = new SolidColorBrush(palette.Dark);
        Front = new SolidColorBrush(palette.Front);
        Splitter = new SolidColorBrush(palette.Splitter);
        Primary = new SolidColorBrush(palette.Primary);
        Accent = new SolidColorBrush(palette.Accent);

        OnPrimary = new SolidColorBrush(Readable(palette.Primary));
        Muted = new SolidColorBrush(palette.Front, 0.62);
        Faint = new SolidColorBrush(palette.Front, 0.38);
        Hover = new SolidColorBrush(
            IsLight(palette.Mid)
                ? Color.FromArgb(0x14, 0x00, 0x00, 0x00)
                : Color.FromArgb(0x14, 0xFF, 0xFF, 0xFF)
        );
        BorderSoft = new SolidColorBrush(palette.Splitter, 0.58);

        Ear = new SolidColorBrush(
            Colors.Black,
            IsLight(palette.Mid) ? 0.03 : 0.22
        );
        Selection = new SolidColorBrush(palette.Primary, IsLight(palette.Mid) ? 0.14 : 0.19);

        // The semantic states are the look's own, rather than a colour worked out from the accent: a
        // state means the same thing wherever it is drawn, and the Desktop program states the same red,
        // orange and green.
        AlarmColour = palette.Semantic.Error;
        WarningColour = palette.Semantic.Warning;
        SuccessColour = palette.Semantic.Success;

        Alarm = new SolidColorBrush(AlarmColour);
        Warning = new SolidColorBrush(WarningColour);
        Success = new SolidColorBrush(SuccessColour);

        OnWarning = new SolidColorBrush(Color.FromRgb(0x16, 0x17, 0x1B));
    }

    /// <summary>
    /// One colour mixed into another, which is how a wash is made from a semantic colour.
    /// </summary>
    /// <param name="ground">What is underneath.</param>
    /// <param name="tint">What is mixed in.</param>
    /// <param name="amount">How much of the tint, from 0 to 1.</param>
    /// <returns>The mixed colour.</returns>
    public static Color Mix(Color ground, Color tint, double amount)
    {
        var part = Math.Clamp(amount, 0, 1);
        var rest = 1 - part;

        return Color.FromRgb(
            (byte)Math.Round((ground.R * rest) + (tint.R * part)),
            (byte)Math.Round((ground.G * rest) + (tint.G * part)),
            (byte)Math.Round((ground.B * rest) + (tint.B * part))
        );
    }

    /// <summary>Whether a colour is light enough that dark ink is read on it.</summary>
    /// <param name="colour">The colour.</param>
    /// <returns>Whether it is a light colour.</returns>
    public static bool IsLight(Color colour) =>
        (((0.2126 * colour.R) + (0.7152 * colour.G) + (0.0722 * colour.B)) / 255.0) > 0.6;

    /// <summary>An ink that can be read on a fill.</summary>
    /// <param name="fill">What it stands on.</param>
    /// <returns>Black or white.</returns>
    public static Color Readable(Color fill) => IsLight(fill) ? Colors.Black : Colors.White;
}

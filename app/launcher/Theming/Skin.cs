using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaLauncher.Theming;

/// <summary>
/// The colours the launcher is drawn from, which are the colours the Desktop program is drawn from.
/// </summary>
/// <remarks>
/// The palette is the one <c>theme.json</c> states, so the two programs are drawn from one choice: the
/// variant in force and the seven colours are read there, and this only turns them into the brushes a
/// control is given. What is added is what a picture needs and a palette does not name — a muted ink, a
/// hovered surface, an ink that can be read on the primary — and each is worked out from the palette
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

    /// <summary>The surface a pointer is over.</summary>
    public IBrush Hover { get; }

    /// <summary>What a warning or a failure is written in.</summary>
    public IBrush Alarm { get; }

    /// <summary>The colour <see cref="Alarm"/> is made of, for a wash that is the same colour thinned.</summary>
    public Color AlarmColour { get; }

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
        Hover = new SolidColorBrush(palette.Front, 0.08);

        // The palette names no colour for a thing that went wrong, so one is worked out: the red the
        // accent is a pink of, darkened or lightened until it can be read on the ground it stands on.
        AlarmColour = IsLight(palette.Mid)
            ? Color.FromRgb(0xB4, 0x1E, 0x2C)
            : Color.FromRgb(0xFF, 0x7B, 0x86);

        Alarm = new SolidColorBrush(AlarmColour);
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

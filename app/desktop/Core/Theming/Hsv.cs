using Avalonia.Media;

namespace RorolalaDesktop.Theming;

/// <summary>
/// A colour as a hue, how much of it, and how light: what a plate is dragged over.
/// </summary>
/// <remarks>
/// The look is drawn in red, green and blue, and a person choosing a colour thinks in these three — a place
/// around the wheel, how much of that hue is in it, and how light it is. What is here is the way between the
/// two, which is the whole of what a colour plate needs and the part of it that can be worked out without a
/// screen: the plate itself is a square and a bar, and what they come to is this.
/// </remarks>
internal static class Hsv
{
    /// <summary>How many degrees the hue runs through before it is the first hue again.</summary>
    public const double Turn = 360;

    /// <summary>The three a colour is, as a place around the wheel and two fractions of the whole.</summary>
    /// <param name="colour">The colour to take apart.</param>
    /// <returns>Its hue in degrees, and how much of it and how light it is, each from nothing to one.</returns>
    public static (double Hue, double Saturation, double Value) Of(Color colour)
    {
        var red = colour.R / 255.0;
        var green = colour.G / 255.0;
        var blue = colour.B / 255.0;

        var most = Math.Max(red, Math.Max(green, blue));
        var least = Math.Min(red, Math.Min(green, blue));
        var span = most - least;

        // Which way round the wheel the colour stands is which of the three is the most of it: the span is
        // how far it is from the other two, and where in that span the third one sits is the fraction of a
        // sixth of the turn that is left.
        var hue = span switch
        {
            0 => 0,
            _ when most == red => 60 * (((green - blue) / span) % 6),
            _ when most == green => 60 * (((blue - red) / span) + 2),
            _ => 60 * (((red - green) / span) + 4),
        };

        return (((hue % Turn) + Turn) % Turn, most == 0 ? 0 : span / most, most);
    }

    /// <summary>The colour three fractions stand for.</summary>
    /// <remarks>
    /// A fraction of a whole rather than a byte, because that is what a plate has to hand: where the pointer
    /// is in a square of a known width and height. What is handed back is rounded to the byte a colour is
    /// written in, so that a colour taken apart and put back together is the colour it started as.
    /// </remarks>
    /// <param name="hue">Where around the wheel, in degrees, which wraps into the turn.</param>
    /// <param name="saturation">How much of the hue, from nothing to one.</param>
    /// <param name="value">How light it is, from nothing to one.</param>
    /// <returns>The colour.</returns>
    public static Color To(double hue, double saturation, double value)
    {
        var at = ((hue % Turn) + Turn) % Turn;
        var held = Math.Clamp(saturation, 0, 1);
        var lit = Math.Clamp(value, 0, 1);

        // How far the brightest channel is from the dimmest, and which of the sixths of the turn the hue is
        // in: the third channel is the one that climbs or falls across the middle of each sixth.
        var span = lit * held;
        var middle = span * (1 - Math.Abs(((at / 60) % 2) - 1));
        var least = lit - span;

        var (red, green, blue) = at switch
        {
            < 60 => (span, middle, 0.0),
            < 120 => (middle, span, 0.0),
            < 180 => (0.0, span, middle),
            < 240 => (0.0, middle, span),
            < 300 => (middle, 0.0, span),
            _ => (span, 0.0, middle),
        };

        return Color.FromRgb(Byte(red + least), Byte(green + least), Byte(blue + least));
    }

    /// <summary>A fraction of the whole as the byte a colour is written in.</summary>
    /// <param name="fraction">The fraction, which is held between nothing and one.</param>
    /// <returns>The byte.</returns>
    private static byte Byte(double fraction) =>
        (byte)Math.Round(Math.Clamp(fraction, 0, 1) * 255, MidpointRounding.AwayFromZero);
}

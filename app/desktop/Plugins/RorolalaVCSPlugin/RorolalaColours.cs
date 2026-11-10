using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The colours this plugin says its states in, read from the look's semantic set.
/// </summary>
/// <remarks>
/// A state means the same thing wherever the program draws it, so the colours are the look's rather than the
/// plugin's: what is new is success, what changed is modify, what must not be recorded is fail, and what moved
/// is the first status colour. The look states them apart for the light and dark variants, which is why a
/// colour is read as it is drawn rather than kept here.
/// <para>
/// The two shades worked out from a colour are still this plugin's: what is written on a colour and the line
/// around it are shades of that same colour rather than another state.
/// </para>
/// </remarks>
internal static class RorolalaColours
{
    /// <summary>The look in force, or nothing before the plugin has been given a host.</summary>
    private static IPalette? _palette;

    /// <summary>Takes the look the plugin draws in.</summary>
    /// <param name="palette">The look in force.</param>
    public static void Bind(IPalette palette)
    {
        ArgumentNullException.ThrowIfNull(palette);

        _palette = palette;
    }

    /// <summary>What cannot be acted on at the moment.</summary>
    public static Color Disabled => Semantic.Disabled;

    /// <summary>What changed and can be recorded.</summary>
    public static Color Modify => Semantic.Modify;

    /// <summary>What asks for attention but is not a failure.</summary>
    public static Color Warning => Semantic.Warning;

    /// <summary>What went wrong in an operation.</summary>
    public static Color Error => Semantic.Error;

    /// <summary>What is wrong with the work itself.</summary>
    public static Color Fail => Semantic.Fail;

    /// <summary>What is new or otherwise settled.</summary>
    public static Color Success => Semantic.Success;

    /// <summary>What may be acted on, which is the lock this account holds.</summary>
    public static Color Allow => Semantic.Allow;

    /// <summary>What may not be acted on, which is the lock another account holds.</summary>
    public static Color Disallow => Semantic.Disallow;

    /// <summary>What a run does only when it is forced.</summary>
    public static Color Force => Semantic.Force;

    /// <summary>The first status colour, which is what a move is drawn in.</summary>
    public static Color StatusAlpha => Semantic.StatusAlpha;

    /// <summary>The second status colour.</summary>
    public static Color StatusBeta => Semantic.StatusBeta;

    /// <summary>The third status colour.</summary>
    public static Color StatusDelta => Semantic.StatusDelta;

    /// <summary>What is in the tree and not in the Layout yet.</summary>
    public static Color Untagged => Success;

    /// <summary>What changed, and is this side's to record.</summary>
    public static Color UnderWay => Modify;

    /// <summary>What must not be recorded: another account's, or a base upstream has moved past.</summary>
    public static Color Wrong => Fail;

    /// <summary>What is somewhere else than the Layout names it.</summary>
    public static Color Moved => StatusAlpha;

    /// <summary>How far a shade is mixed towards the end it is drawn to, for the ink on a colour.</summary>
    private const double InkShade = 0.82;

    /// <summary>How far a shade is mixed towards the end it is drawn to, for the line around a colour.</summary>
    private const double EdgeShade = 0.42;

    /// <summary>
    /// The brightness a colour has to be past for dark ink to read on it.
    /// </summary>
    /// <remarks>
    /// A weighted sum rather than a proper contrast ratio, because what it decides is which of two shades of one
    /// colour is drawn: either reads, and the two are told apart by what is easier on the eye rather than by a
    /// standard. The weights are the eye's, green is what it sees most of, and the cut is where the lightest
    /// of these colours stops wanting a light ink.
    /// </remarks>
    private const double LightEnough = 0.6;

    /// <summary>The semantic set in force, or the dark one before the plugin has a host.</summary>
    private static SemanticPalette Semantic => _palette?.Semantic ?? SemanticPalette.Dark;

    /// <summary>
    /// What is written on a colour: a shade of the colour itself, dark or light as it reads.
    /// </summary>
    /// <remarks>
    /// A shade of its own family rather than white or black, because the words and the ground they sit on are
    /// one thing, a pill is a state said loudly, and words in another colour would be two things said at once.
    /// </remarks>
    /// <param name="colour">What the words are written on.</param>
    /// <returns>What to write them in.</returns>
    public static Color On(Color colour) =>
        Brightness(colour) > LightEnough
            ? Shade(colour, InkShade, towardsWhite: false)
            : Shade(colour, InkShade, towardsWhite: true);

    /// <summary>
    /// The line around a mark in a colour: a darker shade of it.
    /// </summary>
    /// <param name="colour">What the mark is coloured.</param>
    /// <returns>What to draw the line in.</returns>
    public static Color Edge(Color colour) => Shade(colour, EdgeShade, towardsWhite: false);

    /// <summary>How bright a colour reads, weighted the way the eye weighs the three channels.</summary>
    /// <param name="colour">The colour.</param>
    /// <returns>Nothing at black and one at white.</returns>
    private static double Brightness(Color colour) =>
        ((0.299 * colour.R) + (0.587 * colour.G) + (0.114 * colour.B)) / 255;

    /// <summary>A shade of a colour: the colour mixed towards white or towards black.</summary>
    /// <param name="colour">The colour to shade.</param>
    /// <param name="amount">How far towards the end it is mixed, from nothing to all of it.</param>
    /// <param name="towardsWhite">Whether the end is white rather than black.</param>
    /// <returns>The shade.</returns>
    private static Color Shade(Color colour, double amount, bool towardsWhite)
    {
        var end = towardsWhite ? 255.0 : 0.0;

        return Color.FromArgb(
            colour.A,
            (byte)Math.Round(colour.R + ((end - colour.R) * amount)),
            (byte)Math.Round(colour.G + ((end - colour.G) * amount)),
            (byte)Math.Round(colour.B + ((end - colour.B) * amount))
        );
    }
}

using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The colours this plugin says its states in, as the settings that hold them.
/// </summary>
/// <remarks>
/// They are the plugin's rather than the theme's, because what a state is coloured is the state and not a
/// preference: green for what is not recorded yet, amber for what is under way, red for what must not be
/// recorded, and blue for what is somewhere else than the Layout says. What a user may change is the two of
/// them each is drawn in — the preference panel holds them as the hexes below — and nothing about them follows
/// the look: the same four read on a light ground and on a dark one, which is what makes them the plugin's
/// answer rather than the theme's.
/// <para>
/// Everything else is worked out from them: what is written on a colour is a shade of that colour rather than
/// white or black, so a row of pills reads as one family, and the line around a mark is a darker shade of its
/// own colour, since a mark that is its own edge is a mark that is not there.
/// </para>
/// </remarks>
internal static class RorolalaColours
{
    /// <summary>What is in the tree and not in the Layout yet.</summary>
    public static Color Untagged => Read(UntaggedSetting, UntaggedDefault);

    /// <summary>What changed, and is this side's to record.</summary>
    public static Color UnderWay => Read(UnderWaySetting, UnderWayDefault);

    /// <summary>What must not be recorded: another account's, or a base upstream has moved past.</summary>
    public static Color Wrong => Read(WrongSetting, WrongDefault);

    /// <summary>What is somewhere else than the Layout names it.</summary>
    public static Color Moved => Read(MovedSetting, MovedDefault);

    /// <summary>The setting the green is kept under, and what it is until it is changed.</summary>
    public const string UntaggedSetting = "State/untagged";

    /// <summary>What the green is until it is changed.</summary>
    public const string UntaggedDefault = "#8BC34A";

    /// <summary>The setting the amber is kept under.</summary>
    public const string UnderWaySetting = "State/underway";

    /// <summary>What the amber is until it is changed.</summary>
    public const string UnderWayDefault = "#FFC107";

    /// <summary>The setting the red is kept under.</summary>
    public const string WrongSetting = "State/wrong";

    /// <summary>What the red is until it is changed.</summary>
    public const string WrongDefault = "#FF5252";

    /// <summary>The setting the blue is kept under.</summary>
    public const string MovedSetting = "State/moved";

    /// <summary>What the blue is until it is changed.</summary>
    public const string MovedDefault = "#2196F3";

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
    /// standard. The weights are the eye's — green is what it sees most of — and the cut is where the lightest
    /// of these colours stops wanting a light ink.
    /// </remarks>
    private const double LightEnough = 0.6;

    /// <summary>Where the settings are read from, once the plugin has declared them.</summary>
    private static IPluginConfig? _config;

    /// <summary>
    /// Declares the four settings, so that the preference panel shows them and keeps what is chosen.
    /// </summary>
    /// <remarks>
    /// The colours are read from the settings as they are drawn rather than read once and kept: a setting
    /// changed in the panel is one the next pill or dot is drawn in, and what draws them draws again on its
    /// own — the Overview on its reading, a listing when the files may have changed.
    /// </remarks>
    /// <param name="config">The plugin's own settings.</param>
    public static void Declare(IPluginConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);

        _config = config;

        Declare(config, UntaggedSetting, "rorolala_vcs.setting.state.untagged", UntaggedDefault, 10);
        Declare(config, UnderWaySetting, "rorolala_vcs.setting.state.underway", UnderWayDefault, 20);
        Declare(config, WrongSetting, "rorolala_vcs.setting.state.wrong", WrongDefault, 30);
        Declare(config, MovedSetting, "rorolala_vcs.setting.state.moved", MovedDefault, 40);
    }

    /// <summary>
    /// What is written on a colour: a shade of the colour itself, dark or light as it reads.
    /// </summary>
    /// <remarks>
    /// A shade of its own family rather than white or black, because the words and the ground they sit on are
    /// one thing — a pill is a state said loudly, and words in another colour would be two things said at once.
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
    /// <remarks>
    /// One shade for both looks rather than one per look: the line is part of the colour's own answer, and a
    /// mark that is its own edge on one ground is a smudge on the other. A darker shade of these four reads on
    /// either, since the four are bright enough to be told from the dark ground and dark enough to be told from
    /// the light one.
    /// </remarks>
    /// <param name="colour">What the mark is coloured.</param>
    /// <returns>What to draw the line in.</returns>
    public static Color Edge(Color colour) => Shade(colour, EdgeShade, towardsWhite: false);

    /// <summary>Declares one of the four, as free text that is read back as a colour.</summary>
    /// <param name="config">The plugin's own settings.</param>
    /// <param name="id">The setting's identity.</param>
    /// <param name="labelKey">What the preference panel names it.</param>
    /// <param name="standard">What it is until it is changed.</param>
    /// <param name="order">Where it stands among the plugin's settings.</param>
    private static void Declare(
        IPluginConfig config,
        string id,
        string labelKey,
        string standard,
        int order
    ) => config.Add(new PluginSetting(id, SettingKind.Colour, labelKey, standard, order));

    /// <summary>What one of the four is set to, which is the standard where it is not set to anything.</summary>
    /// <param name="id">The setting's identity.</param>
    /// <param name="standard">What it is until it is changed.</param>
    /// <returns>The colour, which is the standard where what is set is not a colour.</returns>
    private static Color Read(string id, string standard) =>
        _config?.ReadKeyAs(id, standard) is { Length: > 0 } said
        && Color.TryParse(said, out var colour)
            ? colour
            : Color.Parse(standard);

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

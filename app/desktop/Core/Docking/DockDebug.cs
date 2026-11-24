using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The host's own settings about drawing a drag, which is how the answers are watched while they are drawn.
/// </summary>
/// <remarks>
/// A debugging answer rather than a look's: where the layout moves to say where a drop would land, the answers
/// themselves are invisible, and a switch that draws them over the room is what makes the two comparable.
/// <para>
/// The value is kept here rather than read where it is used, because the area asks once per answer and the
/// setting is written by a panel that cannot reach every window's area. It is written once when the kernel
/// declares it and again on every change, so a switch turned while the program runs is the next drag's answer.
/// </para>
/// </remarks>
internal static class DockDebug
{
    /// <summary>
    /// The setting that draws the answers a drag can land on.
    /// </summary>
    /// <remarks>
    /// Its identity is also where the panel lists it: the part before the slash is the group it is drawn under,
    /// and a group is drawn as it is written rather than translated, which is what every owner's groups do.
    /// </remarks>
    public const string AimSetting = "Debug/aim";

    /// <summary>What the setting is called, as the key its two languages are read from.</summary>
    public const string AimKey = "core.setting.debug_aim";

    /// <summary>Whether the answers are drawn even where the layout moves to say them.</summary>
    public static bool Aim { get; private set; }

    /// <summary>Declares the settings, and follows them from here on.</summary>
    /// <param name="settings">What every owner declared, and what each is worth.</param>
    public static void Declare(SettingRegistry settings)
    {
        ArgumentNullException.ThrowIfNull(settings);

        // One view for the run: it is what reads a value as its own kind, and what hears the changes of the
        // kernel's own settings rather than of every owner's.
        var kernel = new PluginConfigView(settings, Core.Id);

        kernel.Add(new PluginSetting(AimSetting, SettingKind.Bool, AimKey, "false", 0));

        Aim = kernel.ReadKeyAs(AimSetting, false);

        kernel.Changed += id =>
        {
            if (string.Equals(id, AimSetting, StringComparison.Ordinal))
            {
                Aim = kernel.ReadKeyAs(AimSetting, false);
            }
        };
    }
}

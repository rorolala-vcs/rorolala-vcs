using Avalonia.Media;

namespace RorolalaDesktop.Fonts;

/// <summary>
/// The face the desktop programs are drawn in, which travels with them rather than being asked of the
/// machine.
/// </summary>
/// <remarks>
/// The family is Noto Sans, embedded from the files `cargo dev fonts` fetches at its pinned commit.
/// The same files are laid beside the programs under <c>THIRD_PARTY/Fonts/NotoSans</c>, with the
/// license they are distributed under, so a program carries both the face and the terms it comes with.
/// <para>
/// <see cref="Address"/> is a property rather than a constant: reading it is what loads this assembly,
/// whose resources Avalonia's asset loader reads for the family.
/// </para>
/// </remarks>
public static class NotoSans
{
    /// <summary>The address Avalonia reads the family from, with the family name after the hash.</summary>
    public static string Address { get; } =
        "avares://RorolalaDesktopFonts/Assets/Fonts/NotoSans#Noto Sans";

    /// <summary>The family every word of the desktop programs is set in.</summary>
    public static FontFamily Family { get; } = new(Address);
}

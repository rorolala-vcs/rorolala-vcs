using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a window style is named, and which one an environment is drawn in.
/// </summary>
/// <remarks>
/// The window and its look are both drawn from the answer, and neither can be asked about it in a test: what
/// is checked here is the reading of the two things that choose one — the name on the command line, and the
/// environment a tiling window manager leaves behind.
/// </remarks>
public sealed class WindowStyleTests
{
    /// <summary>Each name is read as the style it names, whichever case it is written in.</summary>
    /// <remarks>
    /// The two spellings are one name to whoever writes it: `rola desktop --window-style` writes one and the
    /// window's own argument the other, and the two programs are told the same thing either way.
    /// </remarks>
    [Fact]
    public void ANameIsReadAsItsStyle()
    {
        Assert.Equal(WindowStyle.Windows, WindowStyles.Named("windows"));
        Assert.Equal(WindowStyle.Windows, WindowStyles.Named("Windows"));
        Assert.Equal(WindowStyle.Windows, WindowStyles.Named("WINDOWS"));
        Assert.Equal(WindowStyle.Mac, WindowStyles.Named("mac"));
        Assert.Equal(WindowStyle.Mac, WindowStyles.Named("Mac"));
        Assert.Equal(WindowStyle.Mac, WindowStyles.Named(" mac "));
        Assert.Equal(WindowStyle.TillingWm, WindowStyles.Named("tillingwm"));
        Assert.Equal(WindowStyle.TillingWm, WindowStyles.Named("TillingWM"));
    }

    /// <summary>A name that is none of the three names nothing.</summary>
    /// <param name="name">What was written.</param>
    [Theory]
    [InlineData("")]
    [InlineData("gnome")]
    [InlineData("tiling")]
    [InlineData("macos")]
    [InlineData("windows11")]
    public void AnotherNameNamesNothing(string name) => Assert.Null(WindowStyles.Named(name));

    /// <summary>The four window managers this knows are recognized by the socket each publishes.</summary>
    /// <param name="variable">The variable the socket is named in.</param>
    [Theory]
    [InlineData("SWAYSOCK")]
    [InlineData("I3SOCK")]
    [InlineData("HYPRLAND_INSTANCE_SIGNATURE")]
    [InlineData("NIRI_SOCKET")]
    public void AWindowManagerIsRecognizedByItsSocket(string variable) =>
        Assert.True(WindowStyles.Tiling(name => name == variable ? "/run/one/socket" : null));

    /// <summary>And by the name the session gives itself.</summary>
    /// <param name="variable">The variable the session is named in.</param>
    /// <param name="named">What it says.</param>
    [Theory]
    [InlineData("XDG_CURRENT_DESKTOP", "sway")]
    [InlineData("XDG_CURRENT_DESKTOP", "i3")]
    [InlineData("XDG_SESSION_DESKTOP", "Hyprland")]
    [InlineData("DESKTOP_SESSION", "niri")]
    public void AWindowManagerIsRecognizedByItsSession(string variable, string named) =>
        Assert.True(WindowStyles.Tiling(name => name == variable ? named : null));

    /// <summary>A desktop that is none of the four is not a tiling one.</summary>
    /// <remarks>
    /// What a wrong answer costs is the reason this is read at all: three marks drawn on a desktop that has a
    /// frame of its own are three marks too many, and none drawn on one that has not leaves a window with
    /// nothing to close it by. Nothing known is therefore not tiling.
    /// </remarks>
    [Fact]
    public void AnotherDesktopIsNotTiling()
    {
        Assert.False(WindowStyles.Tiling(_ => null));
        Assert.False(WindowStyles.Tiling(name => name == "XDG_CURRENT_DESKTOP" ? "ubuntu:GNOME" : null));
        Assert.False(WindowStyles.Tiling(name => name == "SWAYSOCK" ? string.Empty : null));
    }
}

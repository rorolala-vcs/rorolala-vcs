using RorolalaDesktop.SysIcons;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The icon theme the desktop names is resolved once, however many icons are asked for.
/// </summary>
/// <remarks>
/// A theme is named in files a desktop writes, and on a GNOME desktop in GSettings, which is a program
/// to run. Asking for each new kind of file would run it again, so what is read here is that the
/// second question for a second kind answers from what the first already found.
/// </remarks>
public sealed class IconThemeTests
{
    /// <summary>Begins each test with nothing read.</summary>
    public IconThemeTests() => IconTheme.ResetForTest();

    /// <summary>The theme name is resolved once, not once per kind of file.</summary>
    [Fact]
    public void TheThemeTheDesktopNamesIsReadOnce()
    {
        IconTheme.Drawn(Kind.File, "hero.psd", 16);
        IconTheme.Drawn(Kind.File, "notes.txt", 16);

        Assert.Equal(1, IconTheme.Resolutions);
    }
}

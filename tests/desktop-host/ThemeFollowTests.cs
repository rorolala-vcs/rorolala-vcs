using Avalonia;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Configuration;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The look's file, which the Desktop program and the launcher share.
/// </summary>
/// <remarks>
/// The two programs are drawn from one <c>theme.json</c>, and each reads it when it starts. What is checked here
/// is the half that makes a choice visible without a restart: a write to the file is heard, and a reading of it
/// is put into the choice already held rather than in place of it.
/// </remarks>
public sealed class ThemeFollowTests
{
    /// <summary>A write to the file is said once the burst of it has settled.</summary>
    [Fact]
    public void AWriteToTheFileIsHeard()
    {
        var directory = Scratch.New("theme-watch");
        var file = Path.Combine(directory, "theme.json");

        File.WriteAllText(file, "{}");

        var seen = 0;

        using var watch = new ThemeFileWatch(directory, "theme.json", () => seen++);

        File.WriteAllText(file, "{\"version\":1}");

        Wait(watch, () => seen > 0);

        var first = seen;

        File.WriteAllText(file, "{\"version\":1,\"mode\":\"dark\"}");

        Wait(watch, () => seen > first);
    }

    /// <summary>A watch that was let go of does not say anything of a write after it.</summary>
    [Fact]
    public void AWatchThatWasLetGoOfHearsNothing()
    {
        var directory = Scratch.New("theme-watch-gone");
        var file = Path.Combine(directory, "theme.json");

        File.WriteAllText(file, "{}");

        var seen = 0;
        var watch = new ThemeFileWatch(directory, "theme.json", () => seen++);

        watch.Dispose();

        File.WriteAllText(file, "{\"version\":1}");
        Thread.Sleep(300);
        Pump();

        Assert.Equal(0, seen);
    }

    /// <summary>
    /// A reading of the file is taken into the choice already held, every field of it.
    /// </summary>
    /// <remarks>
    /// In place rather than by replacement, because the object is what the preference panel edits and what the
    /// palette answers from: a reader following a choice made in the other program must reach the same instance.
    /// </remarks>
    [Fact]
    public void AReadingIsTakenIntoTheChoiceAlreadyHeld()
    {
        var latest = new ThemeConfiguration
        {
            Mode = ColorMode.Dark,
            Primary = Color.FromRgb(0x11, 0x22, 0x33),
            PrimaryText = Color.FromRgb(0x44, 0x55, 0x66),
            Accent = Color.FromRgb(0x77, 0x88, 0x99),
            ThemeDarkBright = Color.FromRgb(1, 2, 3),
            ThemeDarkMid = Color.FromRgb(4, 5, 6),
            ThemeDarkDark = Color.FromRgb(7, 8, 9),
            ThemeDarkFront = Color.FromRgb(10, 11, 12),
            ThemeDarkSplitter = Color.FromRgb(13, 14, 15),
            ThemeLightBright = Color.FromRgb(16, 17, 18),
            ThemeLightMid = Color.FromRgb(19, 20, 21),
            ThemeLightDark = Color.FromRgb(22, 23, 24),
            ThemeLightFront = Color.FromRgb(25, 26, 27),
            ThemeLightSplitter = Color.FromRgb(28, 29, 30),
            ThemeDarkDisabled = Color.FromRgb(31, 32, 33),
            ThemeDarkModify = Color.FromRgb(34, 35, 36),
            ThemeDarkWarning = Color.FromRgb(37, 38, 39),
            ThemeDarkError = Color.FromRgb(40, 41, 42),
            ThemeDarkFail = Color.FromRgb(43, 44, 45),
            ThemeDarkSuccess = Color.FromRgb(46, 47, 48),
            ThemeDarkAllow = Color.FromRgb(49, 50, 51),
            ThemeDarkDisallow = Color.FromRgb(52, 53, 54),
            ThemeDarkForce = Color.FromRgb(55, 56, 57),
            ThemeDarkStatusAlpha = Color.FromRgb(58, 59, 60),
            ThemeDarkStatusBeta = Color.FromRgb(61, 62, 63),
            ThemeDarkStatusDelta = Color.FromRgb(64, 65, 66),
            ThemeLightDisabled = Color.FromRgb(67, 68, 69),
            ThemeLightModify = Color.FromRgb(70, 71, 72),
            ThemeLightWarning = Color.FromRgb(73, 74, 75),
            ThemeLightError = Color.FromRgb(76, 77, 78),
            ThemeLightFail = Color.FromRgb(79, 80, 81),
            ThemeLightSuccess = Color.FromRgb(82, 83, 84),
            ThemeLightAllow = Color.FromRgb(85, 86, 87),
            ThemeLightDisallow = Color.FromRgb(88, 89, 90),
            ThemeLightForce = Color.FromRgb(91, 92, 93),
            ThemeLightStatusAlpha = Color.FromRgb(94, 95, 96),
            ThemeLightStatusBeta = Color.FromRgb(97, 98, 99),
            ThemeLightStatusDelta = Color.FromRgb(100, 101, 102),
        };

        var held = new ThemeConfiguration { Mode = ColorMode.Light };

        held.CopyFrom(latest);

        Assert.Equal(latest.Mode, held.Mode);
        Assert.Equal(latest.Primary, held.Primary);
        Assert.Equal(latest.PrimaryText, held.PrimaryText);
        Assert.Equal(latest.Accent, held.Accent);
        Assert.Equal(latest.ThemeDarkBright, held.ThemeDarkBright);
        Assert.Equal(latest.ThemeDarkMid, held.ThemeDarkMid);
        Assert.Equal(latest.ThemeDarkDark, held.ThemeDarkDark);
        Assert.Equal(latest.ThemeDarkFront, held.ThemeDarkFront);
        Assert.Equal(latest.ThemeDarkSplitter, held.ThemeDarkSplitter);
        Assert.Equal(latest.ThemeLightBright, held.ThemeLightBright);
        Assert.Equal(latest.ThemeLightMid, held.ThemeLightMid);
        Assert.Equal(latest.ThemeLightDark, held.ThemeLightDark);
        Assert.Equal(latest.ThemeLightFront, held.ThemeLightFront);
        Assert.Equal(latest.ThemeLightSplitter, held.ThemeLightSplitter);
        Assert.Equal(latest.ThemeDarkDisabled, held.ThemeDarkDisabled);
        Assert.Equal(latest.ThemeDarkModify, held.ThemeDarkModify);
        Assert.Equal(latest.ThemeDarkWarning, held.ThemeDarkWarning);
        Assert.Equal(latest.ThemeDarkError, held.ThemeDarkError);
        Assert.Equal(latest.ThemeDarkFail, held.ThemeDarkFail);
        Assert.Equal(latest.ThemeDarkSuccess, held.ThemeDarkSuccess);
        Assert.Equal(latest.ThemeDarkAllow, held.ThemeDarkAllow);
        Assert.Equal(latest.ThemeDarkDisallow, held.ThemeDarkDisallow);
        Assert.Equal(latest.ThemeDarkForce, held.ThemeDarkForce);
        Assert.Equal(latest.ThemeDarkStatusAlpha, held.ThemeDarkStatusAlpha);
        Assert.Equal(latest.ThemeDarkStatusBeta, held.ThemeDarkStatusBeta);
        Assert.Equal(latest.ThemeDarkStatusDelta, held.ThemeDarkStatusDelta);
        Assert.Equal(latest.ThemeLightDisabled, held.ThemeLightDisabled);
        Assert.Equal(latest.ThemeLightModify, held.ThemeLightModify);
        Assert.Equal(latest.ThemeLightWarning, held.ThemeLightWarning);
        Assert.Equal(latest.ThemeLightError, held.ThemeLightError);
        Assert.Equal(latest.ThemeLightFail, held.ThemeLightFail);
        Assert.Equal(latest.ThemeLightSuccess, held.ThemeLightSuccess);
        Assert.Equal(latest.ThemeLightAllow, held.ThemeLightAllow);
        Assert.Equal(latest.ThemeLightDisallow, held.ThemeLightDisallow);
        Assert.Equal(latest.ThemeLightForce, held.ThemeLightForce);
        Assert.Equal(latest.ThemeLightStatusAlpha, held.ThemeLightStatusAlpha);
        Assert.Equal(latest.ThemeLightStatusBeta, held.ThemeLightStatusBeta);
        Assert.Equal(latest.ThemeLightStatusDelta, held.ThemeLightStatusDelta);
    }

    /// <summary>Runs the window's loop until what is being waited for happens, settling the watch each turn.</summary>
    /// <param name="watch">The watch whose burst is settled rather than waited out.</param>
    /// <param name="until">What is being waited for.</param>
    private static void Wait(ThemeFileWatch watch, Func<bool> until)
    {
        var started = DateTime.UtcNow;

        while (!until())
        {
            if (DateTime.UtcNow > started.AddSeconds(10))
            {
                throw new InvalidOperationException("what was waited for never happened");
            }

            Thread.Sleep(20);
            Pump();
            watch.Flush();
        }
    }

    /// <summary>Runs the window's loop for a few turns.</summary>
    private static void Pump()
    {
        Dispatcher.UIThread.RunJobs();
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
    }
}

using Avalonia;
using Avalonia.Headless;
using RorolalaDesktop;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Puts a platform under the tests that make a window, once for the run.
/// </summary>
/// <remarks>
/// Most of what this suite drives needs no screen: the registries, the configuration and the docks are
/// bookkeeping, and a test that reads them is honest without one. What does need one is anything that
/// <em>makes</em> a window — the shell a dock is drawn in, and the row of menus across the top of it — and the
/// headless platform is what provides one: it is the toolkit's own window, drawing nothing and showing
/// nothing, so what a window is made of is exercised for real rather than stood in for.
/// <para>
/// Set up once here rather than per test, because the platform is the process's: an application is made
/// before the first test body runs, and what it answers — the cursor factory a dock's headers ask for, the
/// font manager a row of menus is measured with — is the same for every test after it.
/// </para>
/// </remarks>
internal static class Platform
{
    /// <summary>Whether the headless platform is up.</summary>
    public static bool IsUp { get; private set; }

    /// <summary>Starts the platform before anything asks it for a window.</summary>
    [System.Runtime.CompilerServices.ModuleInitializer]
    internal static void SetUp()
    {
        try
        {
            AppBuilder
                .Configure<App>()
                .UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = true })
                .SetupWithoutStarting();

            IsUp = true;
        }
        catch (InvalidOperationException)
        {
            // A platform that is already up is the one this was going to make, and a test run that reaches
            // here twice is one run: what is being asked for is that there is a platform, not that this made
            // it.
            IsUp = true;
        }
    }
}

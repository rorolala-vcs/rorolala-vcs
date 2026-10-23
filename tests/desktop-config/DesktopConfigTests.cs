using System.Diagnostics;
using System.Reflection;

namespace RorolalaDesktopConfig.IntegrationTests;

/// <summary>
/// What the Desktop program does when its configuration cannot be honoured.
/// </summary>
/// <remarks>
/// The real program is started, as a caller starts it, and what is checked is what a caller meets:
/// the code it exits with and the reason it leaves on standard error. Every one of these fails
/// before the window exists, which is also why they can be checked without a display and on a
/// machine where nothing is logged in.
/// <para>
/// The codes are written as numbers rather than named from the program, because naming them would
/// mean referencing the program's own types and checking the two against each other rather than
/// against what a caller is told.
/// </para>
/// </remarks>
public sealed class DesktopConfigTests
{
    /// <summary>A plugins file that will not read stops with code 1.</summary>
    [Fact]
    public void APluginsFileThatIsNotJsonStopsWithCodeOne()
    {
        var run = Start(plugins: "{ not json", preference: null);

        Assert.Equal(1, run.ExitCode);
        Assert.Contains("not valid JSON", run.Error, StringComparison.Ordinal);
    }

    /// <summary>A plugins file of a version this program cannot read stops with code 1.</summary>
    [Fact]
    public void APluginsVersionThisProgramCannotReadStopsWithCodeOne()
    {
        var run = Start(plugins: """{"_version": 3, "plugins": []}""", preference: null);

        Assert.Equal(1, run.ExitCode);
        Assert.Contains("_version", run.Error, StringComparison.Ordinal);
    }

    /// <summary>A plugins name that answers to no discovered plugin stops with code 1.</summary>
    [Fact]
    public void APluginsNameThatNamesNoDiscoveredPluginStopsWithCodeOne()
    {
        var run = Start(
            plugins: """{"_version": 1, "plugins": [{"id": "it.nothing"}]}""",
            preference: null
        );

        Assert.Equal(1, run.ExitCode);
        Assert.Contains("names no discovered plugin", run.Error, StringComparison.Ordinal);
    }

    /// <summary>A preference file that will not read stops with code 2.</summary>
    [Fact]
    public void APreferenceFileThatIsNotJsonStopsWithCodeTwo()
    {
        var run = Start(plugins: null, preference: "{ not json");

        Assert.Equal(2, run.ExitCode);
        Assert.Contains("not valid JSON", run.Error, StringComparison.Ordinal);
    }

    /// <summary>A preference file of a version this program cannot read stops with code 2.</summary>
    [Fact]
    public void APreferenceVersionThisProgramCannotReadStopsWithCodeTwo()
    {
        var run = Start(plugins: null, preference: """{"_version": 9}""");

        Assert.Equal(2, run.ExitCode);
        Assert.Contains("_version", run.Error, StringComparison.Ordinal);
    }

    /// <summary>A theme file that is not JSON stops with code four.</summary>
    [Fact]
    public void AThemeFileThatIsNotJsonStopsWithCodeFour()
    {
        var run = Start(plugins: null, preference: null, theme: "{ not json");

        Assert.Equal(4, run.ExitCode);
        Assert.Contains("not valid JSON", run.Error, StringComparison.Ordinal);
    }

    /// <summary>An accent that is not a colour stops with code four.</summary>
    [Fact]
    public void AnAccentThatIsNotAColourStopsWithCodeFour()
    {
        var run = Start(
            plugins: null,
            preference: null,
            theme: """{"_version": 1, "accent": "lemon"}"""
        );

        Assert.Equal(4, run.ExitCode);
        Assert.Contains("#RRGGBB", run.Error, StringComparison.Ordinal);
    }

    /// <summary>The program under test, where it was built beside the solution.</summary>
    private static string Program { get; } =
        typeof(DesktopConfigTests)
            .Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
            .Single(attribute => attribute.Key == "RorolalaDesktop")
            .Value!;

    /// <summary>
    /// Starts the program with a scratch data directory holding the given files.
    /// </summary>
    /// <param name="plugins">The whole of <c>plugins.json</c>, or nothing to leave the file away.</param>
    /// <param name="preference">
    /// The whole of <c>preference.json</c>, or nothing to leave the file away.
    /// </param>
    /// <param name="theme">The whole of <c>theme.json</c>, or nothing to leave the file away.</param>
    /// <returns>What the program exited with, and what it said on standard error.</returns>
    /// <exception cref="InvalidOperationException">The program did not stop.</exception>
    private static Run Start(string? plugins, string? preference, string? theme = null)
    {
        var data = Path.Combine(Path.GetTempPath(), $"rorolala-config-{Guid.NewGuid():N}");
        var configuration = Path.Combine(data, "rola", "desktop");
        Directory.CreateDirectory(configuration);

        if (plugins is not null)
        {
            File.WriteAllText(Path.Combine(configuration, "plugins.json"), plugins);
        }

        if (preference is not null)
        {
            File.WriteAllText(Path.Combine(configuration, "preference.json"), preference);
        }

        if (theme is not null)
        {
            File.WriteAllText(Path.Combine(configuration, "theme.json"), theme);
        }

        var start = new ProcessStartInfo("dotnet")
        {
            RedirectStandardError = true,
            RedirectStandardOutput = true,
            UseShellExecute = false,
        };
        start.ArgumentList.Add(Program);
        // Both names, because they say the same thing to different readers: `ROLA_DATA_HOME` is this
        // project's own and is read on every platform, while `XDG_DATA_HOME` is the name a Unix
        // machine knows — a Windows program would read neither it nor anything else here, and would
        // go on to its window with the real user's configuration instead of the scratch one.
        start.Environment["ROLA_DATA_HOME"] = data;
        start.Environment["XDG_DATA_HOME"] = data;

        using var process = Process.Start(start)!;

        // Reading and waiting at the same time: a program that filled a pipe while waiting to be
        // reaped would wait for a reader that was waiting for it, so the reads are started first and
        // the wait is what comes back with an answer. A program that never stops is killed and the
        // test fails, rather than the run hanging until the job's own limit.
        var error = process.StandardError.ReadToEndAsync();
        var output = process.StandardOutput.ReadToEndAsync();

        if (!process.WaitForExit(30_000))
        {
            process.Kill(entireProcessTree: true);
            process.WaitForExit();
            throw new InvalidOperationException("the program did not stop");
        }

        var stripped = error.GetAwaiter().GetResult();
        _ = output.GetAwaiter().GetResult();

        Directory.Delete(data, recursive: true);

        return new Run(process.ExitCode, stripped);
    }

    /// <summary>What the program exited with, and what it said on standard error.</summary>
    /// <param name="ExitCode">The code the process exited with.</param>
    /// <param name="Error">What it wrote to standard error.</param>
    private sealed record Run(int ExitCode, string Error);
}

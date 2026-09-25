using System.Runtime.CompilerServices;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Points the host's configuration at a scratch root for the whole run.
/// </summary>
/// <remarks>
/// The host writes its files under the platform's data directory. A test run must not touch the
/// person's own, so the standard <c>XDG_DATA_HOME</c> — which is what the platform's data-directory
/// resolution reads — is set before anything reads it. The module initializer runs when this
/// assembly is loaded, which is before any test body, and the root is removed when the run ends.
/// <para>
/// The redirection is the XDG one, so it is Linux and macOS where this bites; on Windows the data
/// directory comes from the shell rather than from the environment and cannot be pointed elsewhere
/// this way.
/// </para>
/// </remarks>
internal static class DataHome
{
    /// <summary>The scratch root every configuration file is written under for this run.</summary>
    public static string Root { get; } = Path.Combine(
        Path.GetTempPath(),
        $"rorolala-host-{Guid.NewGuid():N}"
    );

    /// <summary>Sets the data directory before anything asks where it is.</summary>
    [ModuleInitializer]
    internal static void SetUp()
    {
        Directory.CreateDirectory(Root);
        System.Environment.SetEnvironmentVariable("XDG_DATA_HOME", Root);

        AppDomain.CurrentDomain.ProcessExit += (_, _) => Remove();
    }

    /// <summary>
    /// Forgets every configuration file, so a test begins with none.
    /// </summary>
    public static void Clean()
    {
        var configuration = Path.Combine(Root, "rola", "desktop");

        if (Directory.Exists(configuration))
        {
            Directory.Delete(configuration, recursive: true);
        }
    }

    /// <summary>Removes the scratch root, tolerating a file the run still holds.</summary>
    private static void Remove()
    {
        try
        {
            Directory.Delete(Root, recursive: true);
        }
        catch (IOException)
        {
            // A file still held when the run ends is nothing to fail a test run over.
        }
    }
}

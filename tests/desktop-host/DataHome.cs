using System.Runtime.CompilerServices;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Points the host's configuration at a scratch root for the whole run.
/// </summary>
/// <remarks>
/// The host writes its files under the platform's data directory. A test run must not touch the
/// person's own, so <c>ROLA_DATA_HOME</c> — the name this project's own resolution reads on every
/// platform — is set before anything reads it. The module initializer runs when this assembly is
/// loaded, which is before any test body, and the root is removed when the run ends. The XDG name is
/// set beside it for anything that reads the standard one.
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
        System.Environment.SetEnvironmentVariable("ROLA_DATA_HOME", Root);
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

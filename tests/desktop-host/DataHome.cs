using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Points the host's configuration at a scratch root for the whole run.
/// </summary>
/// <remarks>
/// The host writes its files under the platform's data directory. A test run must not touch the
/// person's own, so <c>ROLA_DATA_HOME</c>, the name this project's own resolution reads on every
/// platform, is set before anything reads it. The module initializer runs when this assembly is
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
        Name("ROLA_DATA_HOME", Root);
        Name("XDG_DATA_HOME", Root);

        // The VCS plugin's fetch loop reaches a Vault; a test host has none, and a background
        // exchange between tests is a test waiting on a network it did not ask for.
        Name("ROLA_DESKTOP_TEST", "1");

        AppDomain.CurrentDomain.ProcessExit += (_, _) => Remove();
    }

    /// <summary>Names one variable to the whole process, managed and native alike.</summary>
    /// <remarks>
    /// On Unix, <see cref="Environment.SetEnvironmentVariable(string, string)"/> answers managed
    /// readers and the environment a child is started in, but the C library goes on handing out what
    /// the process began with. The plugins ask the C ABI who the account is, and that answer is read
    /// from the data home, so the variable is set in the C library as well. Windows has one
    /// environment for both readers, and its managed set already changes that one.
    /// </remarks>
    /// <param name="name">The variable's name.</param>
    /// <param name="value">What it is set to.</param>
    private static void Name(string name, string value)
    {
        Environment.SetEnvironmentVariable(name, value);

        if (!OperatingSystem.IsWindows())
        {
            _ = SetEnv(name, value, 1);
        }
    }

    /// <summary>Sets a variable in the C library's own environment.</summary>
    /// <param name="name">The variable's name.</param>
    /// <param name="value">What it is set to.</param>
    /// <param name="overwrite">Whether a value already there is replaced.</param>
    /// <returns>Zero when it succeeded.</returns>
    [DllImport("libc", EntryPoint = "setenv")]
    private static extern int SetEnv(string name, string value, int overwrite);

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

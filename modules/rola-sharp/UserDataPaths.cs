using System.Reflection;

namespace Rorolala;

/// <summary>
/// The directories a build's user data lives under.
/// </summary>
/// <remarks>
/// The name is compiled into the assembly rather than read when it runs: <c>cargo dev export</c>
/// sets <c>ROLA_USERDATA_NAME</c> for the .NET build, and a build that sets nothing is
/// <c>release</c>, which is the shape a program is handed over in. The base is the platform's
/// local data directory, or <c>ROLA_DATA_HOME</c> for a run that must keep its files to itself,
/// which is the same rule the Rust side applies. One file is compiled into every assembly that
/// needs a path, so there is one C# answer rather than one per program.
/// </remarks>
internal static class UserDataPaths
{
    /// <summary>The directory, under the base, Rorolala's own files sit in.</summary>
    private const string UserDataDirectory = "rola";

    /// <summary>The name this assembly was built for.</summary>
    public static string Name { get; } = ReadName();

    /// <summary>Where this assembly's name keeps its own files, under the base this run names.</summary>
    public static string Root => RootFor(Name);

    /// <summary>Where this assembly's name keeps its keys.</summary>
    public static string Keys => KeysFor(Name);

    /// <summary>Where the Desktop program keeps its own files for this assembly's name.</summary>
    public static string Desktop => DesktopFor(Name);

    /// <summary>Where a name's own files sit, under the base this run names.</summary>
    /// <param name="name">The userdata name.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string RootFor(string name)
    {
        var named = Environment.GetEnvironmentVariable("ROLA_DATA_HOME");
        var data = string.IsNullOrEmpty(named)
            ? Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData,
                Environment.SpecialFolderOption.DoNotVerify
            )
            : named;

        // A platform that answers with nothing has no data directory to speak of; keeping the files
        // under the working directory at least leaves the program runnable rather than unwritable.
        return RootIn(string.IsNullOrEmpty(data) ? "." : data, name);
    }

    /// <summary>Where a name's own files sit, under a base the caller names.</summary>
    /// <param name="home">The base directory.</param>
    /// <param name="name">The userdata name.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string RootIn(string home, string name)
    {
        ArgumentException.ThrowIfNullOrEmpty(home);
        ArgumentException.ThrowIfNullOrEmpty(name);

        return Path.Combine(home, UserDataDirectory, name);
    }

    /// <summary>Where a name's keys are kept.</summary>
    /// <param name="name">The userdata name.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string KeysFor(string name) => Path.Combine(RootFor(name), "keys");

    /// <summary>Where the Desktop program's own files are kept for a name.</summary>
    /// <param name="name">The userdata name.</param>
    /// <returns>The directory, as an absolute path.</returns>
    public static string DesktopFor(string name) => Path.Combine(RootFor(name), "desktop");

    /// <summary>The name the build stamped into this assembly.</summary>
    /// <returns>The userdata name, or <c>release</c> when the build stamped none.</returns>
    private static string ReadName() =>
        typeof(UserDataPaths)
            .Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
            .FirstOrDefault(attribute => attribute.Key == "RolaUserDataName")
            ?.Value ?? "release";
}

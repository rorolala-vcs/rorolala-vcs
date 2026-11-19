using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using RolaSharp;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Loads the C ABI library the plugins reach through, from the directory the built programs are in.
/// </summary>
/// <remarks>
/// A test process is not a published program: nothing lays <c>rorolala</c> beside it, and a plugin that asks
/// who holds an entry asks the library for the answer. <c>ROLA_BIN_DIR</c> is what the running suites are
/// told their programs are in, so the same answer names the library here; a run that never asks is
/// unaffected, and one that asks without the directory fails the way a broken install would.
/// <para>
/// The resolver is registered per assembly, so it is registered on the one the bindings are compiled
/// into. A suite whose first native read comes through another assembly has its own resolver there.
/// </para>
/// </remarks>
internal static class NativeRorolala
{
    /// <summary>The library's name, as the bindings import it.</summary>
    private const string Library = "rorolala";

    /// <summary>What the platform's shared library is called, in the order the loader may find them.</summary>
    private static readonly string[] Names = ["rorolala.dll", "librorolala.so", "librorolala.dylib"];

    /// <summary>Answers the bindings' import from the directory of the built programs.</summary>
    [ModuleInitializer]
    internal static void SetUp() =>
        NativeLibrary.SetDllImportResolver(typeof(Scope).Assembly, Resolve);

    /// <summary>Finds the library in the directory the built programs are in.</summary>
    /// <param name="name">The library being asked for.</param>
    /// <param name="assembly">The assembly asking, which is the bindings'.</param>
    /// <param name="search">What the platform would search, which this does not use.</param>
    /// <returns>The loaded library, or zero when this does not have it.</returns>
    private static nint Resolve(string name, Assembly assembly, DllImportSearchPath? search)
    {
        if (
            !string.Equals(name, Library, StringComparison.Ordinal)
            || Environment.GetEnvironmentVariable("ROLA_BIN_DIR") is not { Length: > 0 } directory
        )
        {
            return 0;
        }

        foreach (var file in Names)
        {
            var candidate = Path.Combine(directory, file);

            if (File.Exists(candidate) && NativeLibrary.TryLoad(candidate, out var handle))
            {
                return handle;
            }
        }

        return 0;
    }
}

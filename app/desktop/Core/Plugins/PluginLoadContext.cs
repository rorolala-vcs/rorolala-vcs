using System.Reflection;
using System.Runtime.InteropServices;
using System.Runtime.Loader;

namespace RorolalaDesktop.Plugins;

/// <summary>
/// The one load context every plugin assembly is loaded into.
/// </summary>
/// <remarks>
/// One context, not one per plugin: plugins may depend on each other, and a private dependency two
/// of them share must be one copy so that both see the same types.
/// <para>
/// The shared assemblies are delegated to the host's own context, so there is exactly one of each in
/// the process: the contract, Avalonia and its satellites, and the base class libraries. Delegating
/// is what keeps a <c>Control</c> a plugin builds the host's <c>Control</c>, which the host's visual
/// tree can hold; a private copy of Avalonia would give a second type of the same name that would be
/// refused where the first is wanted. Everything else resolves from the plugins' directory.
/// </para>
/// <para>
/// The context is not collectible. A plugin assembly is never unloaded in-process: enabling,
/// disabling, or reordering plugins takes effect on the next start, and there is no hot reload.
/// </para>
/// </remarks>
internal sealed class PluginLoadContext : AssemblyLoadContext
{
    /// <summary>The directory every plugin and its private dependencies sit in.</summary>
    private readonly string _directory;

    /// <summary>Makes the context over the directory the plugins sit in.</summary>
    /// <param name="directory">The directory the plugins sit in.</param>
    public PluginLoadContext(string directory)
        : base("rorolala.plugins", isCollectible: false) => _directory = directory;

    /// <summary>
    /// Resolves an assembly, answering nothing for the shared ones so the default context does.
    /// </summary>
    /// <param name="name">The assembly being asked for.</param>
    /// <returns>The assembly, or nothing to let the default context answer.</returns>
    protected override Assembly? Load(AssemblyName name)
    {
        if (IsShared(name.Name))
        {
            return null;
        }

        var file = Path.Combine(_directory, $"{name.Name}.dll");

        return File.Exists(file) ? LoadFromAssemblyPath(Path.GetFullPath(file)) : null;
    }

    /// <summary>
    /// Resolves an unmanaged library, answering the program's own copy where the runtime would not.
    /// </summary>
    /// <remarks>
    /// The runtime's probing for a context of its own looks in the calling assembly's directory — the
    /// plugins' directory — and on the platform's search path, and the program's own directory is
    /// neither. A plugin that binds the C ABI reaches for a library the program carries beside the
    /// program, so without this it is found by nothing, and the binding fails at first use rather than
    /// where it was written.
    /// <para>
    /// The program's directory comes first because that is where the host's own libraries are, and
    /// then the plugins' for a library a plugin brought. Everything else is left to the runtime.
    /// </para>
    /// </remarks>
    /// <param name="unmanagedDllName">The name the binding asked for.</param>
    /// <returns>The library, or nothing to let the runtime go on looking.</returns>
    protected override nint LoadUnmanagedDll(string unmanagedDllName)
    {
        foreach (var directory in new[] { AppContext.BaseDirectory, _directory })
        {
            foreach (var name in Names(unmanagedDllName))
            {
                var candidate = Path.Combine(directory, name);

                if (File.Exists(candidate) && NativeLibrary.TryLoad(candidate, out var library))
                {
                    return library;
                }
            }
        }

        return base.LoadUnmanagedDll(unmanagedDllName);
    }

    /// <summary>
    /// The file names a platform gives the shared library `name`.
    /// </summary>
    /// <remarks>
    /// What a binding asks for is the bare name, and what a platform calls the file is a spelling of
    /// it: the runtime knows the spellings, and asking it is not an option here — this is the probing
    /// that happens before there is anything to ask. The bare name is tried first so that a path the
    /// caller wrote itself is taken as it is.
    /// </remarks>
    /// <param name="name">The name the binding asked for.</param>
    /// <returns>The names to look for, in the order they are worth trying.</returns>
    private static IEnumerable<string> Names(string name)
    {
        yield return name;

        if (OperatingSystem.IsWindows())
        {
            yield return $"{name}.dll";
            yield break;
        }

        yield return $"lib{name}.so";
        yield return $"{name}.so";

        if (OperatingSystem.IsMacOS())
        {
            yield return $"lib{name}.dylib";
            yield return $"{name}.dylib";
        }
    }

    /// <summary>
    /// Whether an assembly is one the host's own context must answer for.
    /// </summary>
    /// <remarks>
    /// Answered by name rather than by probing, so the answer does not depend on what happens to be
    /// beside the plugins.
    /// </remarks>
    private static bool IsShared(string? name)
    {
        if (string.IsNullOrEmpty(name))
        {
            return true;
        }

        return name == "RorolalaDesktop.Contract"
            || name == "RorolalaDesktopI18n"
            || name == "RorolalaDesktopSysIcons"
            || name.StartsWith("Avalonia", StringComparison.Ordinal)
            || name.StartsWith("System", StringComparison.Ordinal)
            || name.StartsWith("Microsoft.", StringComparison.Ordinal)
            || name is "netstandard" or "mscorlib";
    }
}

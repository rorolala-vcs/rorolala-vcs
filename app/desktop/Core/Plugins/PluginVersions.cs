using System.Reflection;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Plugins;

/// <summary>
/// The versions a plugin must agree with, and the check that says whether it does.
/// </summary>
/// <remarks>
/// A mismatch is not fatal to the program: the plugin is not loaded, and neither are the plugins
/// that depend on it, and both are reported (Section 14.2). Refusing to load it is deliberate — a
/// plugin built against another contract or another Avalonia would fail later, at first use, where
/// the failure is harder to explain.
/// </remarks>
internal static class PluginVersions
{
    /// <summary>The contract version this host speaks.</summary>
    public static Version Contract { get; } = Of(typeof(IRolaPlugin));

    /// <summary>The Avalonia version this host carries.</summary>
    public static Version Avalonia { get; } = Of(typeof(Avalonia.Controls.Control));

    /// <summary>
    /// Why a plugin may not be loaded, or nothing when it may.
    /// </summary>
    /// <param name="assembly">The plugin's assembly.</param>
    /// <param name="manifest">What the plugin says about itself.</param>
    /// <returns>A reason, or nothing when the versions agree.</returns>
    public static string? Mismatch(Assembly assembly, PluginManifest manifest)
    {
        if (!SameContract(manifest.ContractVersion, Contract))
        {
            return $"built against contract {manifest.ContractVersion}, but this host speaks {Contract}";
        }

        foreach (var reference in assembly.GetReferencedAssemblies())
        {
            // A plugin that does not touch Avalonia directly states no reference to it; the contract
            // it does reference already carries the check, since the contract names Avalonia too.
            if (reference.Version is null || !IsAvalonia(reference.Name))
            {
                continue;
            }

            if (
                reference.Version.Major != Avalonia.Major
                || reference.Version.Minor != Avalonia.Minor
            )
            {
                return $"built against Avalonia {reference.Version}, but this host carries {Avalonia}";
            }

            break;
        }

        return null;
    }

    /// <summary>
    /// Whether two contract versions are the same, ignoring the revision.
    /// </summary>
    /// <remarks>
    /// The revision is a build detail of the assembly, not part of the surface a plugin agrees to.
    /// </remarks>
    private static bool SameContract(Version a, Version b) =>
        a.Major == b.Major && a.Minor == b.Minor && a.Build == b.Build;

    /// <summary>Whether an assembly name is one of Avalonia's.</summary>
    private static bool IsAvalonia(string? name) =>
        name is not null && name.StartsWith("Avalonia", StringComparison.Ordinal);

    /// <summary>An assembly's version, or <c>0.0</c> when it carries none.</summary>
    private static Version Of(Type type) => type.Assembly.GetName().Version ?? new Version(0, 0);
}

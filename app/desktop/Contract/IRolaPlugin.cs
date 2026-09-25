namespace RorolalaDesktop.Contract;

/// <summary>
/// What a plugin is: a type implementing this interface, exactly one per plugin assembly.
/// </summary>
/// <remarks>
/// The host finds the one implementing type in an assembly, makes it, reads
/// <see cref="Manifest"/>, and — once the plugin's dependencies are satisfied and the version check
/// has passed — calls <see cref="Initialize"/> with the host services. Registration happens during
/// that call, and a plugin that would register after the window is shown is not supported.
/// </remarks>
public interface IRolaPlugin
{
    /// <summary>What the plugin says about itself.</summary>
    PluginManifest Manifest { get; }

    /// <summary>
    /// Registers everything the plugin contributes, through the host services it is handed.
    /// </summary>
    /// <remarks>
    /// A plugin that throws here is not loaded, and neither are the plugins that depend on it
    /// (Section 14.2). It is safe to fail: a broken plugin does not stop the program.
    /// </remarks>
    /// <param name="host">The host, as this plugin sees it.</param>
    void Initialize(IPluginHost host);
}

using System.Reflection;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Plugins;

/// <summary>
/// One plugin assembly that was found and made, before anything is initialized.
/// </summary>
internal sealed class DiscoveredPlugin
{
    /// <summary>The one plugin the assembly states.</summary>
    public required IRolaPlugin Instance { get; init; }

    /// <summary>What the plugin says about itself.</summary>
    public required PluginManifest Manifest { get; init; }

    /// <summary>The assembly the plugin came from.</summary>
    public required Assembly Assembly { get; init; }

    /// <summary>Where the assembly is.</summary>
    public required string Path { get; init; }

    /// <summary>The plugin's identity.</summary>
    public PluginId Id => Manifest.Id;
}

/// <summary>
/// One plugin as the load order holds it, with what happened to it.
/// </summary>
internal sealed class PlannedPlugin
{
    /// <summary>The plugin itself.</summary>
    public required IRolaPlugin Instance { get; init; }

    /// <summary>What the plugin says about itself.</summary>
    public required PluginManifest Manifest { get; init; }

    /// <summary>
    /// Where the plugin sits in the load order, counting from zero.
    /// </summary>
    /// <remarks>
    /// The position is what attribution uses: hooks and menu items registered by this plugin sort
    /// against those of others by it, which is plugin load order.
    /// </remarks>
    public required int Position { get; init; }

    /// <summary>Whether <c>Initialize</c> was called and returned.</summary>
    public bool Initialized { get; internal set; }

    /// <summary>Whether the plugin was left out, though it was not fatal.</summary>
    public bool Skipped { get; internal set; }

    /// <summary>The plugin's identity.</summary>
    public PluginId Id => Manifest.Id;
}

/// <summary>
/// Something about a plugin the user should know, which did not stop the program.
/// </summary>
/// <param name="Subject">The plugin the problem is about, or the assembly it was reading.</param>
/// <param name="Description">What is wrong, in the words the plugin manager shows.</param>
internal sealed record PluginProblem(string Subject, string Description);

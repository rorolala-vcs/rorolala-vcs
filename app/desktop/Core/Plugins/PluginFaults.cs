using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Plugins;

/// <summary>Why an enabled plugin cannot be loaded, as its own order and switches state it.</summary>
internal enum PluginFaultKind
{
    /// <summary>A dependency is not among the plugins that were discovered.</summary>
    DependencyMissing,

    /// <summary>A dependency was disabled by the user.</summary>
    DependencyDisabled,

    /// <summary>A dependency stands after the plugin, so it would not have been started yet.</summary>
    DependencyLater,

    /// <summary>A dependency cannot be loaded either, so the premise this plugin stands on is gone.</summary>
    DependencyUnavailable,
}

/// <summary>One reason an enabled plugin cannot be loaded, and the dependency it is about.</summary>
/// <param name="Plugin">The plugin that cannot be loaded.</param>
/// <param name="Dependency">The dependency that is not satisfied.</param>
/// <param name="Kind">What is wrong with it.</param>
internal sealed record PluginFault(PluginId Plugin, PluginId Dependency, PluginFaultKind Kind);

/// <summary>
/// Which enabled plugins cannot be loaded, from the order and the dependencies alone.
/// </summary>
/// <remarks>
/// It reads a given sequence rather than the one on disk, so it answers for an order the user is
/// still dragging as well as for the one that was saved. A disabled plugin is never faulted itself:
/// what its choice costs is paid by its dependents, and an error belongs where the missing premise
/// is felt rather than where the choice was made.
/// <para>
/// The kinds are decided in the order below — missing, disabled, later, unavailable — so the reason
/// a card gives is the first one that applies and is the one the user can act on. A dependency that
/// is announced only later is answered by <see cref="PluginFaultKind.DependencyLater"/> before its
/// own fault is known, which is right: moving it earlier is what fixes the order, whatever else is
/// wrong with it.
/// </para>
/// </remarks>
internal static class PluginFaults
{
    /// <summary>Every plugin the order leaves unable to load, with each dependency that stops it.</summary>
    /// <param name="order">The plugins in the user's order, each with whether it is enabled.</param>
    /// <param name="dependencies">
    /// Each discovered plugin's dependencies; a name absent from it was not discovered.
    /// </param>
    /// <param name="broken">
    /// Plugins that cannot load for a reason this cannot see, such as a version mismatch. They seed
    /// the propagation: a plugin depending on one is faulted as if the dependency had been disabled.
    /// </param>
    /// <returns>One entry per dependency that stops a plugin, in the order they were met.</returns>
    public static IReadOnlyList<PluginFault> Of(
        IReadOnlyList<PluginState> order,
        IReadOnlyDictionary<PluginId, IReadOnlyList<PluginId>> dependencies,
        IReadOnlySet<PluginId>? broken = null
    )
    {
        var position = new Dictionary<PluginId, int>();
        var enabled = new Dictionary<PluginId, bool>();

        for (var index = 0; index < order.Count; index++)
        {
            position[order[index].Id] = index;
            enabled[order[index].Id] = order[index].Enabled;
        }

        var faulted = broken is null ? new HashSet<PluginId>() : new HashSet<PluginId>(broken);
        var faults = new List<PluginFault>();

        // Walking the sequence is what makes a dependency's own fault known before its dependents are
        // reached, which is the whole of the propagation here.
        foreach (var state in order)
        {
            if (!state.Enabled || !dependencies.TryGetValue(state.Id, out var declared))
            {
                continue;
            }

            foreach (var dependency in declared)
            {
                var kind = Kind(state.Id, dependency, dependencies, position, enabled, faulted);

                if (kind is null)
                {
                    continue;
                }

                faults.Add(new PluginFault(state.Id, dependency, kind.Value));
                faulted.Add(state.Id);
            }
        }

        return faults;
    }

    /// <summary>What stops one plugin through one dependency, or nothing when it does not.</summary>
    private static PluginFaultKind? Kind(
        PluginId plugin,
        PluginId dependency,
        IReadOnlyDictionary<PluginId, IReadOnlyList<PluginId>> dependencies,
        IReadOnlyDictionary<PluginId, int> position,
        IReadOnlyDictionary<PluginId, bool> enabled,
        IReadOnlySet<PluginId> faulted
    )
    {
        if (!dependencies.ContainsKey(dependency))
        {
            return PluginFaultKind.DependencyMissing;
        }

        if (!enabled.GetValueOrDefault(dependency))
        {
            return PluginFaultKind.DependencyDisabled;
        }

        if (position.GetValueOrDefault(dependency) > position[plugin])
        {
            return PluginFaultKind.DependencyLater;
        }

        return faulted.Contains(dependency) ? PluginFaultKind.DependencyUnavailable : null;
    }
}

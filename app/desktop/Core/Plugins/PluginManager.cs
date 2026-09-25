using System.Reflection;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.Plugins;

/// <summary>
/// Finds the plugins, checks them against the user's configuration, works out the load order, and
/// starts them.
/// </summary>
/// <remarks>
/// The order of the steps is the one Section 4.6 lays down: discover, reconcile with the file, check
/// the versions, check the dependencies, order, and initialize. A version check that fails, or an
/// <c>Initialize</c> that throws, leaves that plugin and the plugins depending on it out and is
/// reported but is not fatal; a dependency that is missing, disabled, or in a cycle is fatal, since
/// the user's configuration asks for something that cannot be honoured.
/// </remarks>
internal sealed class PluginManager
{
    /// <summary>Where the plugins sit, beside the program.</summary>
    private readonly string _directory;

    /// <summary>What went wrong with a plugin, in the order it was noticed.</summary>
    private readonly List<PluginProblem> _problems = [];

    /// <summary>Where the user's ordering contradicts the dependency order.</summary>
    private readonly List<PluginProblem> _orderingNotes = [];

    /// <summary>Every plugin that was loaded and may be started, in load order.</summary>
    private readonly List<PlannedPlugin> _loadOrder = [];

    /// <summary>Plugins left out though it was not fatal, and why.</summary>
    private readonly HashSet<PluginId> _unavailable = [];

    /// <summary>Every plugin assembly that was found.</summary>
    private List<DiscoveredPlugin> _discovered = [];

    /// <summary>The context the plugin assemblies are loaded into.</summary>
    private PluginLoadContext? _context;

    /// <summary>Makes a manager over the directory the plugins sit in.</summary>
    /// <param name="directory">The directory the plugins sit in.</param>
    public PluginManager(string directory) => _directory = directory;

    /// <summary>The completed user state, as written back to <c>plugins.json</c>.</summary>
    public PluginsConfiguration Configuration { get; private set; } = new();

    /// <summary>Every plugin assembly that was found.</summary>
    public IReadOnlyList<DiscoveredPlugin> Discovered => _discovered;

    /// <summary>What went wrong with a plugin, in the order it was noticed.</summary>
    public IReadOnlyList<PluginProblem> Problems => _problems;

    /// <summary>Where the user's ordering contradicts the dependency order.</summary>
    public IReadOnlyList<PluginProblem> OrderingNotes => _orderingNotes;

    /// <summary>Every plugin that may be started, in load order.</summary>
    public IReadOnlyList<PlannedPlugin> LoadOrder => _loadOrder;

    /// <summary>
    /// Discovers the plugins, reconciles them with the file, and works out the load order.
    /// </summary>
    /// <param name="fromFile">What <c>plugins.json</c> stated, or empty when it was not there.</param>
    /// <exception cref="ConfigurationFailure">The configuration cannot be honoured.</exception>
    public void Load(PluginsConfiguration fromFile)
    {
        Discover();

        Configuration = ConfigurationLoader.ReconcilePlugins(
            fromFile,
            _discovered.Select(plugin => plugin.Id).ToArray()
        );

        var loadable = CheckVersions();
        CheckDependencies(loadable);

        var active = ResolveAvailability(loadable);

        _loadOrder.AddRange(Order(active));
    }

    /// <summary>
    /// Calls <c>Initialize</c> on each plugin in load order, through the services it is handed.
    /// </summary>
    /// <param name="services">Everything the host holds, handed to each plugin in its own view.</param>
    public void InitializeAll(HostServices services)
    {
        var failed = new HashSet<PluginId>(_unavailable);

        foreach (var planned in _loadOrder)
        {
            var unmet = planned.Manifest.Dependencies.Where(failed.Contains).ToArray();

            if (unmet.Length > 0)
            {
                planned.Skipped = true;
                failed.Add(planned.Id);
                _problems.Add(
                    new PluginProblem(
                        planned.Id.Value,
                        $"not loaded: {Names(unmet)} could not be loaded"
                    )
                );
                continue;
            }

            try
            {
                planned.Instance.Initialize(services.For(planned.Id, planned.Position));
                planned.Initialized = true;
                services.Log.Record(LogLevel.Debug, Core.Source, $"loaded `{planned.Id}`");
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                // Fail-open: the plugin is left out, the plugins depending on it are left out, and
                // the program carries on. A broken plugin must not stop the user.
                planned.Skipped = true;
                failed.Add(planned.Id);
                _problems.Add(
                    new PluginProblem(
                        planned.Id.Value,
                        $"not loaded: it threw while starting — {error.Message}"
                    )
                );
            }
        }
    }

    /// <summary>Finds every plugin assembly beside the program and makes each one plugin.</summary>
    private void Discover()
    {
        if (!Directory.Exists(_directory))
        {
            _discovered = [];

            return;
        }

        _context = new PluginLoadContext(_directory);

        var discovered = new List<DiscoveredPlugin>();

        foreach (
            var file in Directory
                .EnumerateFiles(_directory, "*.dll")
                .OrderBy(path => path, StringComparer.Ordinal)
        )
        {
            var assembly = Read(file);
            var plugin = assembly is null ? null : Make(assembly, file);

            if (plugin is null)
            {
                continue;
            }

            if (discovered.Any(found => found.Id == plugin.Id))
            {
                _problems.Add(
                    new PluginProblem(plugin.Id.Value, "is claimed by more than one assembly")
                );
                continue;
            }

            discovered.Add(plugin);
        }

        _discovered = discovered;
    }

    /// <summary>Loads one file as an assembly, reporting one that cannot be read.</summary>
    private Assembly? Read(string file)
    {
        try
        {
            return _context!.LoadFromAssemblyPath(Path.GetFullPath(file));
        }
        catch (Exception error)
            when (error
                    is BadImageFormatException
                        or FileLoadException
                        or IOException
                        or UnauthorizedAccessException
            )
        {
            _problems.Add(
                new PluginProblem(
                    Path.GetFileName(file),
                    $"could not be read as an assembly: {error.Message}"
                )
            );

            return null;
        }
    }

    /// <summary>Reads the one plugin an assembly states, or nothing when it states none.</summary>
    /// <remarks>
    /// An assembly with no plugin is left alone rather than reported: it is how a private dependency
    /// of a plugin is laid beside it.
    /// </remarks>
    private DiscoveredPlugin? Make(Assembly assembly, string file)
    {
        Type[] types;

        try
        {
            types = assembly.GetTypes();
        }
        catch (ReflectionTypeLoadException error)
        {
            _problems.Add(
                new PluginProblem(Path.GetFileName(file), $"could not be read: {error.Message}")
            );

            return null;
        }

        var pluginTypes = types
            .Where(type =>
                typeof(IRolaPlugin).IsAssignableFrom(type)
                && type is { IsAbstract: false, IsInterface: false }
                && !type.ContainsGenericParameters
            )
            .ToArray();

        if (pluginTypes.Length == 0)
        {
            return null;
        }

        if (pluginTypes.Length > 1)
        {
            _problems.Add(
                new PluginProblem(Path.GetFileName(file), "states more than one IRolaPlugin")
            );

            return null;
        }

        try
        {
            if (Activator.CreateInstance(pluginTypes[0]) is not IRolaPlugin instance)
            {
                _problems.Add(new PluginProblem(Path.GetFileName(file), "could not be made"));

                return null;
            }

            var manifest = instance.Manifest;

            if (!PluginId.IsWellFormed(manifest.Id.Value))
            {
                _problems.Add(
                    new PluginProblem(
                        Path.GetFileName(file),
                        $"states a malformed plugin id `{manifest.Id}`"
                    )
                );

                return null;
            }

            return new DiscoveredPlugin
            {
                Instance = instance,
                Manifest = manifest,
                Assembly = assembly,
                Path = file,
            };
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _problems.Add(
                new PluginProblem(
                    Path.GetFileName(file),
                    $"threw while being made: {error.Message}"
                )
            );

            return null;
        }
    }

    /// <summary>Leaves out the plugins whose contract or Avalonia version does not agree.</summary>
    private List<DiscoveredPlugin> CheckVersions()
    {
        var loadable = new List<DiscoveredPlugin>();

        foreach (var plugin in _discovered)
        {
            var mismatch = PluginVersions.Mismatch(plugin.Assembly, plugin.Manifest);

            if (mismatch is null)
            {
                loadable.Add(plugin);
                continue;
            }

            _unavailable.Add(plugin.Id);
            _problems.Add(new PluginProblem(plugin.Id.Value, $"not loaded: {mismatch}"));
        }

        return loadable;
    }

    /// <summary>
    /// Refuses the program when a plugin's declared dependencies cannot be honoured.
    /// </summary>
    /// <remarks>
    /// A dependency that is not discovered, or is disabled, stops the program: the configuration
    /// asks for something that cannot be done, and carrying on would load a plugin whose premise is
    /// missing. A dependency that failed its version check is not this: it is handled by leaving the
    /// dependent out, which the user is told about rather than being stopped.
    /// </remarks>
    private void CheckDependencies(List<DiscoveredPlugin> loadable)
    {
        foreach (var plugin in loadable)
        {
            if (!Enabled(plugin.Id))
            {
                continue;
            }

            foreach (var dependency in plugin.Manifest.Dependencies)
            {
                if (dependency == plugin.Id)
                {
                    throw Fatal($"`{plugin.Id}` depends on itself");
                }

                if (_discovered.All(found => found.Id != dependency))
                {
                    throw Fatal($"`{plugin.Id}` depends on `{dependency}`, which is not discovered");
                }

                if (!Enabled(dependency))
                {
                    throw Fatal($"`{plugin.Id}` depends on `{dependency}`, which is disabled");
                }
            }
        }
    }

    /// <summary>
    /// Leaves out the enabled plugins that depend, at any depth, on one that was left out.
    /// </summary>
    private List<DiscoveredPlugin> ResolveAvailability(List<DiscoveredPlugin> loadable)
    {
        var active = loadable
            .Where(plugin => Enabled(plugin.Id) && !_unavailable.Contains(plugin.Id))
            .ToList();

        var changed = true;

        while (changed)
        {
            changed = false;

            foreach (var plugin in active)
            {
                if (
                    _unavailable.Contains(plugin.Id)
                    || !plugin.Manifest.Dependencies.Any(_unavailable.Contains)
                )
                {
                    continue;
                }

                _unavailable.Add(plugin.Id);
                _problems.Add(
                    new PluginProblem(
                        plugin.Id.Value,
                        "not loaded: a plugin it depends on was not loaded"
                    )
                );
                changed = true;
            }
        }

        return active.Where(plugin => !_unavailable.Contains(plugin.Id)).ToList();
    }

    /// <summary>
    /// Sorts the plugins: dependency order first, then the user's order within a tier, then the
    /// identity so the result does not depend on the file system's order.
    /// </summary>
    private List<PlannedPlugin> Order(List<DiscoveredPlugin> active)
    {
        var byId = active.ToDictionary(plugin => plugin.Id);
        var remaining = new Dictionary<PluginId, int>();

        foreach (var plugin in active)
        {
            remaining[plugin.Id] = plugin.Manifest.Dependencies.Count(byId.ContainsKey);
        }

        var ready = new SortedSet<PluginId>(
            Comparer<PluginId>.Create(
                (left, right) =>
                {
                    var byOrder = OrderOf(left).CompareTo(OrderOf(right));

                    return byOrder != 0
                        ? byOrder
                        : string.CompareOrdinal(left.Value, right.Value);
                }
            )
        );

        foreach (var plugin in active)
        {
            if (remaining[plugin.Id] == 0)
            {
                ready.Add(plugin.Id);
            }
        }

        var ordered = new List<PlannedPlugin>();

        while (ready.Count > 0)
        {
            var id = ready.Min;
            ready.Remove(id);
            ordered.Add(
                new PlannedPlugin
                {
                    Instance = byId[id].Instance,
                    Manifest = byId[id].Manifest,
                    Position = ordered.Count,
                }
            );

            foreach (var plugin in active)
            {
                if (!plugin.Manifest.Dependencies.Contains(id))
                {
                    continue;
                }

                remaining[plugin.Id] -= 1;

                if (remaining[plugin.Id] == 0)
                {
                    ready.Add(plugin.Id);
                }
            }
        }

        if (ordered.Count != active.Count)
        {
            var stuck = active
                .Select(plugin => plugin.Id)
                .Except(ordered.Select(plugin => plugin.Id))
                .OrderBy(id => id.Value, StringComparer.Ordinal);

            throw Fatal(
                $"the plugins {Names(stuck)} depend on one another in a cycle"
            );
        }

        // The user's ordering is kept only where it does not contradict the dependency order. Where
        // it does, the dependency order wins and the user is told who must follow whom.
        foreach (var planned in ordered)
        {
            foreach (var dependency in planned.Manifest.Dependencies)
            {
                if (
                    byId.ContainsKey(dependency)
                    && OrderOf(planned.Id) < OrderOf(dependency)
                )
                {
                    _orderingNotes.Add(
                        new PluginProblem(
                            planned.Id.Value,
                            $"must come after `{dependency}`"
                        )
                    );
                }
            }
        }

        return ordered;
    }

    /// <summary>Whether the user left a plugin enabled.</summary>
    private bool Enabled(PluginId id) =>
        Configuration.Plugins.TryGetValue(id, out var state) && state.Enabled;

    /// <summary>The user's ordering for a plugin, or zero when the file states none.</summary>
    private int OrderOf(PluginId id) =>
        Configuration.Plugins.TryGetValue(id, out var state) ? state.Order : 0;

    /// <summary>Names a run of plugin identities as a sentence reads them.</summary>
    private static string Names(IEnumerable<PluginId> ids) =>
        string.Join(", ", ids.Select(id => $"`{id}`"));

    /// <summary>A configuration problem that stops the program.</summary>
    private static ConfigurationFailure Fatal(string reason) =>
        new(ExitCode.Plugins, $"plugins: {reason}");
}

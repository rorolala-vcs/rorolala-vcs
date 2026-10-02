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
/// reported but is not fatal; a cycle is fatal, since no order honors one at all.
/// <para>
/// The load order is the user's own sequence, subject to nothing: a plugin whose dependency is
/// disabled, absent, or placed after it is left out rather than moved, and the fault is shown on
/// the card that belongs to it (Section 14.2). Nothing is reordered behind the user's back, which is
/// what makes the arrangement in the plugin manager the arrangement that is started.
/// </para>
/// </remarks>
internal sealed class PluginManager
{
    /// <summary>Where the plugins sit, beside the program.</summary>
    private readonly string _directory;

    /// <summary>What went wrong with a plugin, in the order it was noticed.</summary>
    private readonly List<PluginProblem> _problems = [];

    /// <summary>Every plugin that was loaded and may be started, in load order.</summary>
    private readonly List<PlannedPlugin> _loadOrder = [];

    /// <summary>Plugins left out though it was not fatal, and why.</summary>
    private readonly HashSet<PluginId> _unavailable = [];

    /// <summary>
    /// Plugins that cannot load for a reason the order cannot state, such as a version mismatch.
    /// </summary>
    /// <remarks>
    /// Kept apart from <see cref="_unavailable"/>, which also collects what the order itself leaves
    /// out: the plugin manager asks for the live answer while the user is dragging, and a set that
    /// already holds the order's own verdict would blame a stale arrangement for the new one.
    /// </remarks>
    private readonly HashSet<PluginId> _broken = [];

    /// <summary>Every plugin assembly that was found.</summary>
    private List<DiscoveredPlugin> _discovered = [];

    /// <summary>Every discovered plugin's declared dependencies, by identity.</summary>
    private IReadOnlyDictionary<PluginId, IReadOnlyList<PluginId>> _dependencies =
        new Dictionary<PluginId, IReadOnlyList<PluginId>>();

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

    /// <summary>Every plugin that may be started, in load order.</summary>
    public IReadOnlyList<PlannedPlugin> LoadOrder => _loadOrder;

    /// <summary>
    /// Which enabled plugins an order leaves unable to load, as they stand in that order.
    /// </summary>
    /// <remarks>
    /// It takes the order rather than reading the one on disk so that the plugin manager can ask for
    /// the arrangement the user is still dragging, which is what lets the cards turn red before the
    /// pointer is let go of.
    /// </remarks>
    /// <param name="order">The plugins in the user's order, each with whether it is enabled.</param>
    /// <returns>One entry per dependency that stops a plugin.</returns>
    public IReadOnlyList<PluginFault> Faults(IReadOnlyList<PluginState> order) =>
        PluginFaults.Of(order, _dependencies, _broken);

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

        _dependencies = _discovered.ToDictionary(
            plugin => plugin.Id,
            plugin => plugin.Manifest.Dependencies
        );

        var loadable = CheckVersions();
        CheckCycles(loadable);
        ResolveAvailability(loadable);

        // The sequence is the user's, so a plugin the order cannot support is left out where it
        // stands rather than moved to where it would load. The plugin manager shows it on its card.
        foreach (var fault in Faults(Configuration.Plugins))
        {
            _unavailable.Add(fault.Plugin);
        }

        var byId = loadable.ToDictionary(plugin => plugin.Id);
        var position = 0;

        foreach (var state in Configuration.Plugins)
        {
            if (
                !state.Enabled
                || _unavailable.Contains(state.Id)
                || !byId.TryGetValue(state.Id, out var plugin)
            )
            {
                continue;
            }

            _loadOrder.Add(
                new PlannedPlugin
                {
                    Instance = plugin.Instance,
                    Manifest = plugin.Manifest,
                    Position = position++,
                }
            );
        }
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
            _broken.Add(plugin.Id);
            _problems.Add(new PluginProblem(plugin.Id.Value, $"not loaded: {mismatch}"));
        }

        return loadable;
    }

    /// <summary>
    /// Refuses the program when enabled plugins depend on one another in a cycle.
    /// </summary>
    /// <remarks>
    /// A cycle is the one arrangement no order can honor: every plugin in it would be left out with a
    /// fault blaming the next, and the user would be told nothing about the shape of the knot. The
    /// plugins in it are named instead.
    /// <para>
    /// Only enabled plugins are followed. A disabled plugin breaks a chain rather than forming one,
    /// since nothing is waiting on a plugin that does not load.
    /// </para>
    /// </remarks>
    private void CheckCycles(List<DiscoveredPlugin> loadable)
    {
        var byId = loadable
            .Where(plugin => Enabled(plugin.Id))
            .ToDictionary(plugin => plugin.Id);

        var done = new HashSet<PluginId>();
        var path = new List<PluginId>();

        foreach (var plugin in byId.Values)
        {
            Visit(plugin);
        }

        return;

        void Visit(DiscoveredPlugin plugin)
        {
            if (done.Contains(plugin.Id))
            {
                return;
            }

            var at = path.IndexOf(plugin.Id);

            if (at >= 0)
            {
                throw Fatal(
                    $"the plugins {Names(path.Skip(at))} depend on one another in a cycle"
                );
            }

            path.Add(plugin.Id);

            foreach (var dependency in plugin.Manifest.Dependencies)
            {
                if (byId.TryGetValue(dependency, out var found))
                {
                    Visit(found);
                }
            }

            path.RemoveAt(path.Count - 1);
            done.Add(plugin.Id);
        }
    }

    /// <summary>
    /// Leaves out the enabled plugins that depend, at any depth, on one that was left out.
    /// </summary>
    private void ResolveAvailability(List<DiscoveredPlugin> loadable)
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
    }

    /// <summary>Whether the user left a plugin enabled.</summary>
    private bool Enabled(PluginId id) => Configuration.Enabled(id);

    /// <summary>Names a run of plugin identities as a sentence reads them.</summary>
    private static string Names(IEnumerable<PluginId> ids) =>
        string.Join(", ", ids.Select(id => $"`{id}`"));

    /// <summary>A configuration problem that stops the program.</summary>
    private static ConfigurationFailure Fatal(string reason) =>
        new(ExitCode.Plugins, $"plugins: {reason}");
}

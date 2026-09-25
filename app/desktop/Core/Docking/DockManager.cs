using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The docks: what is registered, what is open, and where.
/// </summary>
/// <remarks>
/// Registration and the live instances are kept together because the two are read together — the
/// <c>Window</c> menu is built from the registrations, and the area from the instances — and because
/// the layout is written from one and restored into the other.
/// <para>
/// A dock's view factory is plugin code and may throw. That is caught here: the dock does not open,
/// the failure is logged and raised to the user, and nothing else is disturbed.
/// </para>
/// </remarks>
internal sealed class DockManager
{
    /// <summary>The registrations.</summary>
    private readonly DockRegistry _registry;

    /// <summary>The host's translations, for resolving dock titles.</summary>
    private readonly I18nService _i18n;

    /// <summary>The host's log.</summary>
    private readonly LogService _log;

    /// <summary>Where a failure the user must act on is raised.</summary>
    private readonly PopupService _popups;

    /// <summary>How many instances of each dock name have been made.</summary>
    private readonly Dictionary<string, int> _ordinals = new(StringComparer.Ordinal);

    /// <summary>Whether the saved docks are being opened, and so are not all here yet.</summary>
    private bool _restoring;

    /// <summary>What a dock remembers when it is opened for the first time, which is nothing.</summary>
    private static readonly IReadOnlyDictionary<string, string> Nothing =
        new Dictionary<string, string>(StringComparer.Ordinal);

    /// <summary>Every live instance, in the order they were made.</summary>
    private readonly List<DockInstance> _instances = [];

    /// <summary>Makes a manager over the registrations and the services it reports through.</summary>
    /// <param name="registry">The registrations.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="log">The host's log.</param>
    /// <param name="popups">Where a failure the user must act on is raised.</param>
    public DockManager(
        DockRegistry registry,
        I18nService i18n,
        LogService log,
        PopupService popups
    )
    {
        _registry = registry;
        _i18n = i18n;
        _log = log;
        _popups = popups;
    }

    /// <summary>Raised whenever the set of open docks or their placement changes.</summary>
    public event Action? Changed;

    /// <summary>
    /// Raised whenever a dock keeps something about itself.
    /// </summary>
    /// <remarks>
    /// Told apart from <see cref="Changed"/> because the two are answered differently: a change to the
    /// docks is a change to the area, which is rebuilt, where what a dock remembers about itself is
    /// nothing the area draws — only the layout file has to hear about it.
    /// </remarks>
    public event Action? StateChanged;

    /// <summary>The region sizes, read when the area is built and updated as the splitters move.</summary>
    public DockLayout Layout { get; } = new();

    /// <summary>Every live instance, in the order they were made.</summary>
    public IReadOnlyList<DockInstance> Instances => _instances;

    /// <summary>Every registered dock, in registration order.</summary>
    public IReadOnlyList<DockRegistration> Registrations => _registry.Registrations;

    /// <summary>The form a key is written in, for a dock header or the <c>Window</c> menu.</summary>
    /// <param name="key">The key, as the files nest it.</param>
    /// <returns>What the key says, or the key itself when nothing states it.</returns>
    public string Text(string key) => _i18n.Get(key);

    /// <summary>Registers a dock.</summary>
    /// <param name="registration">What the plugin says about the dock.</param>
    public void Register(DockRegistration registration) => _registry.Register(registration);

    /// <summary>
    /// Activates a dock from the <c>Window</c> menu: a toggle is shown or hidden, a factory makes
    /// a new instance.
    /// </summary>
    /// <param name="registration">The dock.</param>
    /// <returns>The instance that was opened, or nothing when one could not be made.</returns>
    public DockInstance? Activate(DockRegistration registration)
    {
        if (registration.OpenMode == DockOpenMode.Toggle)
        {
            return Toggle(registration);
        }

        return Create(registration, registration.DefaultPlacement);
    }

    /// <summary>
    /// Shows or hides a toggle dock, making its one instance the first time.
    /// </summary>
    /// <param name="registration">The dock.</param>
    /// <returns>The dock's instance, or nothing when one could not be made.</returns>
    public DockInstance? Toggle(DockRegistration registration)
    {
        var existing = _instances.FirstOrDefault(instance =>
            instance.DockNameId == registration.DockNameId
        );

        if (existing is not null)
        {
            existing.IsOpen = !existing.IsOpen;
            Raise();

            return existing;
        }

        return Create(registration, registration.DefaultPlacement);
    }

    /// <summary>Makes a new dock instance at a placement.</summary>
    /// <param name="registration">The dock.</param>
    /// <param name="placement">Where to place the new instance.</param>
    /// <returns>The new instance, or nothing when its view could not be made.</returns>
    public DockInstance? Create(DockRegistration registration, DockPlacement placement) =>
        Create(registration, placement, Nothing);

    /// <summary>Makes a new dock instance, told what that dock remembered last time.</summary>
    private DockInstance? Create(
        DockRegistration registration,
        DockPlacement placement,
        IReadOnlyDictionary<string, string> kept
    )
    {
        IDockView view;

        try
        {
            view = registration.Create(placement);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Record(
                LogLevel.Error,
                Core.Source,
                $"`{registration.DockNameId}` could not make its view: {error.Message}"
            );
            _popups.Raise(
                LogLevel.Warn,
                registration.Owner.Value,
                $"The dock `{registration.DockNameId}` could not be opened: {error.Message}"
            );

            return null;
        }

        var instance = new DockInstance
        {
            Registration = registration,
            View = view,
            Ordinal = NextOrdinal(registration.DockNameId),
            Title = _i18n.Get(registration.DisplayNameKey),
            Placement = placement,
            IsOpen = true,
        };

        foreach (var (key, value) in kept)
        {
            instance.Meta[key] = value;
        }

        _instances.Add(instance);

        // Told after it exists and before it is drawn, so that a view which reads what it kept has
        // read it by the time the area draws it, and a view which kept nothing is none the wiser.
        // A view that throws here is a dock that does not open, as one that throws while being made.
        try
        {
            view.Restored(new DockState(instance, this));
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Record(
                LogLevel.Error,
                Core.Source,
                $"`{registration.DockNameId}` could not take what it remembered: {error.Message}"
            );
        }

        Raise();

        return instance;
    }

    /// <summary>
    /// Closes a dock: a toggle is hidden and kept, so showing it again is showing the same instance;
    /// a created instance is discarded.
    /// </summary>
    /// <param name="instance">The dock to close.</param>
    public void Close(DockInstance instance)
    {
        if (instance.OpenMode == DockOpenMode.Toggle)
        {
            instance.IsOpen = false;
        }
        else
        {
            _instances.Remove(instance);
        }

        Raise();
    }

    /// <summary>
    /// Puts a dock in another region, which is what dragging its header does.
    /// </summary>
    /// <remarks>
    /// A placement is the same thing a registration declares for a new dock, so one moved by hand
    /// is written to the layout like any other and comes back where it was left.
    /// </remarks>
    /// <param name="instance">The dock to move.</param>
    /// <param name="placement">The region to move it to.</param>
    public void Move(DockInstance instance, DockPlacement placement)
    {
        if (instance.Placement == placement)
        {
            return;
        }

        instance.Placement = placement;
        Raise();
    }

    /// <summary>
    /// Opens the docks a saved layout names, at the placements it names.
    /// </summary>
    /// <remarks>
    /// A name the layout states that no registration answers to is dropped: a plugin may have been
    /// removed, and the rest of the layout is still worth restoring.
    /// </remarks>
    /// <param name="layout">What was saved.</param>
    public void Restore(DockLayout layout)
    {
        Layout.LeftWidth = layout.LeftWidth;
        Layout.RightWidth = layout.RightWidth;
        Layout.BottomHeight = layout.BottomHeight;
        Layout.TopHeight = layout.TopHeight;

        // Nothing a dock keeps about itself is written down while this runs, and that is not a
        // nicety: what would be written is a snapshot of the docks that happen to be open yet, which
        // is a layout with every dock further down the file dropped from it. A dock that keeps
        // something while it is being restored keeps it in memory, and the next write — of anything —
        // takes it with it.
        _restoring = true;

        try
        {
            foreach (var saved in layout.Docks)
            {
                var registration = _registry.Find(saved.DockNameId);

                if (registration is null)
                {
                    _log.Record(
                        LogLevel.Debug,
                        Core.Source,
                        $"the saved layout names `{saved.DockNameId}`, which no dock registers"
                    );
                    continue;
                }

                Create(registration, saved.Placement, saved.Meta);
            }
        }
        finally
        {
            _restoring = false;
        }
    }

    /// <summary>Everything that is open, as it is written back to the layout file.</summary>
    /// <returns>What to save, with the region sizes as they stand.</returns>
    public DockLayout Snapshot()
    {
        var layout = new DockLayout
        {
            LeftWidth = Layout.LeftWidth,
            RightWidth = Layout.RightWidth,
            BottomHeight = Layout.BottomHeight,
            TopHeight = Layout.TopHeight,
        };

        foreach (
            var instance in _instances
                .Where(instance => instance.IsOpen)
                .OrderBy(instance => instance.DockNameId, StringComparer.Ordinal)
                .ThenBy(instance => instance.Ordinal)
        )
        {
            layout.Docks.Add(
                new LayoutDock
                {
                    DockNameId = instance.DockNameId,
                    Ordinal = instance.Ordinal,
                    Placement = instance.Placement,
                    Meta = new Dictionary<string, string>(instance.Meta, StringComparer.Ordinal),
                }
            );
        }

        return layout;
    }

    /// <summary>The next instance number for a dock name.</summary>
    private int NextOrdinal(string nameId)
    {
        _ordinals.TryGetValue(nameId, out var next);
        _ordinals[nameId] = next + 1;

        return next;
    }

    /// <summary>Tells the area to rebuild.</summary>
    private void Raise() => Changed?.Invoke();

    /// <summary>
    /// One dock's own memory: what it kept, and what it keeps next.
    /// </summary>
    /// <remarks>
    /// The keys are the dock's own business and the keeping is the host's, so nothing here is a
    /// contract with any particular dock: what a dock writes is what it reads back, and a key it never
    /// writes reads as nothing.
    /// </remarks>
    private sealed class DockState : IDockState
    {
        /// <summary>The dock whose memory this is.</summary>
        private readonly DockInstance _instance;

        /// <summary>The manager to tell when the memory changed, so the layout is written.</summary>
        private readonly DockManager _manager;

        /// <summary>Makes a view of one dock's memory.</summary>
        /// <param name="instance">The dock whose memory this is.</param>
        /// <param name="manager">The manager to tell when it changes.</param>
        public DockState(DockInstance instance, DockManager manager)
        {
            _instance = instance;
            _manager = manager;
        }

        /// <inheritdoc />
        public string? Read(string key) =>
            _instance.Meta.TryGetValue(key, out var kept) ? kept : null;

        /// <inheritdoc />
        public void Write(string key, string value)
        {
            if (_instance.Meta.TryGetValue(key, out var kept) && kept == value)
            {
                return;
            }

            _instance.Meta[key] = value;

            if (!_manager._restoring)
            {
                _manager.StateChanged?.Invoke();
            }
        }
    }
}

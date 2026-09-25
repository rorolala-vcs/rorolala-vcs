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
    public DockInstance? Create(DockRegistration registration, DockPlacement placement)
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

        _instances.Add(instance);
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

            Create(registration, saved.Placement);
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
}

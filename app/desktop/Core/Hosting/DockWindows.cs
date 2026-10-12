using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The host's windows: the main one, and the ones docks were moved into.
/// </summary>
/// <remarks>
/// One set for the whole program, and one window class behind it, because a window of a dock's own is the
/// main window's own shell with one dock in it: the same menu row, the same frame, the same exit button, and
/// nothing else. What a window is told is which dock it is for, and everything else follows from that.
/// <para>
/// A window is closed by the user, by the dock it holds being closed, or by the program ending; the three
/// are told apart here. What the program ends on is the set of windows being empty rather than any one
/// window being closed — the main window among them — which is why nothing here ends the program until
/// there is nothing left to draw in.
/// </para>
/// </remarks>
internal sealed class DockWindows : IWindows
{
    /// <summary>The docks and where they are.</summary>
    private readonly DockManager _docks;

    /// <summary>The host's translations.</summary>
    private readonly I18nService _i18n;

    /// <summary>What the host tells the plugins when one of its windows is come back to.</summary>
    private readonly Refocus _refocus;

    /// <summary>Where a failure to open a window is reported.</summary>
    private readonly LogService _log;

    /// <summary>The pictures a tab menu's marks are read from.</summary>
    private readonly IIconLibrary _icons;

    /// <summary>What a dock's own menus were registered with.</summary>
    private readonly DockMenuRegistry _menus;

    /// <summary>The picture the program is known by, or nothing when the platform has none.</summary>
    private readonly WindowIcon? _icon;

    /// <summary>
    /// How a window is made, or nothing where there are no windows to make.
    /// </summary>
    /// <remarks>
    /// Handed over rather than written here so that this service can be exercised without a platform: what
    /// it does is decide which windows there are, and making one is the toolkit's business. A host with no
    /// way of making a window — a test — answers requests as if nothing had been asked for rather than
    /// failing, since what it is being asked for is something it was never given.
    /// <para>
    /// Written after the services are made rather than handed in, because the window it makes is made over
    /// those services: the one thing this returns needs is the object this is held in.
    /// </para>
    /// </remarks>
    private Func<string?, DockInstance?, Window>? _make;

    /// <summary>Every window that is up, by its key.</summary>
    private readonly Dictionary<string, Window> _windows = new(StringComparer.Ordinal);

    /// <summary>The dock each window of its own is for, by the window's key.</summary>
    private readonly Dictionary<string, DockInstance> _held = new(StringComparer.Ordinal);

    /// <summary>
    /// The main window, once it is up.
    /// </summary>
    /// <remarks>
    /// Held beside the table rather than as a row of it, because what a dictionary cannot do is be keyed by
    /// nothing: the main window's key is the absence of one, and the one place that absence is named is
    /// <see cref="Key"/>.
    /// </remarks>
    private Window? _main;

    /// <summary>How many windows have been made, which is what names the next one.</summary>
    private int _made;

    /// <summary>Whether the program is ending, so that the last window closing does not end it twice.</summary>
    private bool _ending;

    /// <summary>Makes the set of windows over what a window is drawn from.</summary>
    /// <param name="docks">The docks and where they are.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="refocus">What a window being come back to is said through.</param>
    /// <param name="log">Where a failure to open a window is reported.</param>
    /// <param name="icons">The pictures a tab menu's marks are read from.</param>
    /// <param name="menus">What a dock's own menus were registered with.</param>
    /// <param name="icon">The picture the program is known by.</param>
    public DockWindows(
        DockManager docks,
        I18nService i18n,
        Refocus refocus,
        LogService log,
        IIconLibrary icons,
        DockMenuRegistry menus,
        WindowIcon? icon
    )
    {
        _docks = docks;
        _i18n = i18n;
        _refocus = refocus;
        _log = log;
        _icons = icons;
        _menus = menus;
        _icon = icon;

        // A detached window whose dock is gone is a window of nothing, so it goes with it. Subscribed here
        // rather than by the window because what a window holds is the docks' business, and a window that
        // watched for its own dock would be every window asking the same question.
        _docks.Changed += Settle;
    }

    /// <summary>
    /// How a window is made.
    /// </summary>
    /// <remarks>
    /// Written by the program once the services exist, since what a window is made over is those services
    /// and the object they are held in is this one. Nothing where there is no program to write it — a test —
    /// and then a request to open a window is answered as nothing having been asked for.
    /// </remarks>
    public Func<string?, DockInstance?, Window>? Make
    {
        get => _make;
        set => _make = value;
    }

    /// <summary>The picture the program is known by, which a window of its own is drawn with.</summary>
    public WindowIcon? Icon => _icon;

    /// <summary>The pictures a tab menu's marks are read from.</summary>
    public IIconLibrary Pictures => _icons;

    /// <summary>
    /// What was registered for a dock's own menu.
    /// </summary>
    /// <param name="target">The place the menu belongs to.</param>
    /// <param name="dockNameId">The dock the menu belongs to.</param>
    /// <returns>The items, in the order they are shown.</returns>
    public IReadOnlyList<ContextMenuItem> DockMenuItems(DockMenuTarget target, string dockNameId) =>
        _menus.Items(target, dockNameId);

    /// <summary>Whether the main window is up.</summary>
    public bool MainIsUp => _main is not null;

    /// <summary>
    /// The main window.
    /// </summary>
    /// <remarks>
    /// Asked for by the program rather than reached for through the shell, because what it is to start the
    /// program is to have a main window: the window is the answer to the startup, and the set of windows is
    /// the thing that knows whether there is one.
    /// </remarks>
    public MainWindow Main =>
        _main as MainWindow
        ?? throw new InvalidOperationException("the main window was asked for before it was opened");

    /// <summary>How many of the host's windows are up, the main one included.</summary>
    public int Count => _windows.Count + (_main is null ? 0 : 1);

    /// <summary>
    /// Closes a dock by its stable name id.
    /// </summary>
    /// <remarks>
    /// How a menu about a dock closes one: what a menu item is handed is the dock's name, and the instance
    /// it names is looked up here rather than carried in the menu, which is made once and used whenever it
    /// is opened. A name nothing answers to is nothing to close rather than a failure.
    /// </remarks>
    /// <param name="dockNameId">The dock's stable name id.</param>
    public void CloseDock(string dockNameId)
    {
        ArgumentNullException.ThrowIfNull(dockNameId);

        if (_docks.InstanceOf(dockNameId) is { } instance)
        {
            _docks.Close(instance);
        }
    }

    /// <summary>
    /// How the program is ended, once there is nothing left to draw in.
    /// </summary>
    /// <remarks>
    /// Handed over by the program rather than reached for here, because what ends a run is the lifetime and
    /// not the windows. Nothing where there is no lifetime — a test — and then no window that closes ends
    /// anything.
    /// </remarks>
    public Action? End { get; set; }

    /// <inheritdoc />
    public void OpenMain()
    {
        if (_main is { } up)
        {
            Bring(up);

            return;
        }

        // Made with nothing to name it by, which is the main window: it is the one window a dock is not
        // what it is for. What is remembered is the window rather than the fact that it is up, because the
        // two questions it answers — is there one, and which one is it — are one question.
        _main = Add(null, null);

        if (_main is { } opened)
        {
            Bring(opened);
        }
    }

    /// <inheritdoc />
    public void Open(DockRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        var registration = Registration(request.DockNameId);

        if (registration is null)
        {
            return;
        }

        Recover();

        // What was asked for is either a window or a second one of a dock that may have several, and the two
        // are one question about the dock: a dock that is already open and is one thing is moved, while a dock
        // that is one of many is made again beside it. What decides is the mode the request states, or the
        // registration's own where it states none.
        var anew = (request.OpenMode ?? registration.OpenMode) == DockOpenMode.New;
        var open = Instance(registration);

        // Already in a window of its own: the same request made twice is the same thing, which is the window
        // that is already there brought forward rather than a second one beside it.
        if (open?.Window is { } already && !anew)
        {
            if (_windows.TryGetValue(already, out var window))
            {
                Bring(window);
            }

            return;
        }

        if (open is not null && !anew)
        {
            ToOwnWindow(open);

            return;
        }

        var key = $"dock-{++_made}";
        var made = _docks.Create(registration, DockPlacement.Center, request, key);

        if (made is null)
        {
            return;
        }

        _held[key] = made;
        LeaveMain();

        if (Add(key, made) is { } opened)
        {
            Bring(opened);
        }
    }

    /// <summary>
    /// Moves a dock into a window of its own, or brings the one it has forward.
    /// </summary>
    /// <param name="instance">The dock to move.</param>
    public void ToOwnWindow(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        Recover();

        if (instance.Window is { } already)
        {
            if (_windows.TryGetValue(already, out var there))
            {
                Bring(there);
            }

            return;
        }

        var key = $"dock-{++_made}";

        _held[key] = instance;

        // The region is the one it already had rather than the centre of its new window: pulling a dock out
        // of a region is about where it is shown, and being in a window of its own is a change of window and
        // not a change of region — a dock dragged out of the bottom band comes back to it when it returns.
        _docks.Move(instance, instance.Placement, key);
        LeaveMain();

        if (Add(key, instance) is { } opened)
        {
            Bring(opened);
        }
    }

    /// <summary>The registration a stable name id names, or nothing.</summary>
    /// <param name="dockNameId">The dock's stable name id.</param>
    /// <returns>The registration, or nothing when no dock answers to that name.</returns>
    private DockRegistration? Registration(string dockNameId)
    {
        var found = _docks.Registrations.FirstOrDefault(dock =>
            string.Equals(dock.DockNameId, dockNameId, StringComparison.Ordinal)
        );

        if (found is null)
        {
            _log.Record(
                LogLevel.Warn,
                Core.Source,
                $"no dock is registered as `{dockNameId}`, so no window was opened for it"
            );
        }

        return found;
    }

    /// <summary>
    /// Shows a dock where it is, making it if it is not there.
    /// </summary>
    /// <remarks>
    /// How a second start of the program reaches the one that is already up: the run that was called hands
    /// over the directory it was made in, and what that means is a browser in the main window looking at it.
    /// The dock is made rather than moved, because nothing in the run that was already up is looking at the
    /// directory the caller was working in.
    /// </remarks>
    /// <param name="dockNameId">The stable name id of the dock to show.</param>
    /// <param name="startAt">What the dock is to look at, or nothing.</param>
    public void Show(string dockNameId, string? startAt)
    {
        OpenMain();

        if (dockNameId is not { Length: > 0 } || Registration(dockNameId) is not { } registration)
        {
            return;
        }

        var instance = _docks.Create(
            registration,
            registration.DefaultPlacement,
            new DockRequest(registration.DockNameId, startAt)
        );

        if (instance is not null)
        {
            _docks.Show(instance);
        }
    }

    /// <summary>
    /// Brings back the docks that are in a window which is no longer there.
    /// </summary>
    /// <remarks>
    /// The belt to the braces of <see cref="Gone"/>: a dock whose window went away without saying so — one
    /// that was made before this service knew about it, or one whose window was taken down another way —
    /// would otherwise be a dock nobody can show. Asked at the two moments a dock is about to be placed,
    /// because that is when a stale window reference first does harm, and asked as a question about the set
    /// of windows rather than by remembering anything, so that what it answers cannot drift from what there
    /// is.
    /// </remarks>
    private void Recover()
    {
        var stranded = _docks
            .Instances.Where(instance => instance.Window is { } key && !_windows.ContainsKey(key))
            .ToArray();

        if (stranded.Length == 0)
        {
            return;
        }

        // The main window is where they came from, so it is what they need: a dock brought back to a window
        // that is not there either would be no better off than it was.
        OpenMain();

        foreach (var instance in stranded)
        {
            var was = instance.Window;

            _docks.Rehome(instance);
            _docks.Show(instance);
            _log.Record(
                LogLevel.Debug,
                Core.Source,
                $"`{instance.DockNameId}` was in `{was}`, which is not there, and came back to the main window"
            );
        }
    }

    /// <summary>
    /// Closes a window by its key.
    /// </summary>
    /// <remarks>
    /// Closing is asked of the window rather than done by forgetting it, so that whatever the toolkit has to
    /// say about a window going away is said once. A window that is not up is nothing to close, which is not
    /// a failure: the set of windows is what is true, and closing one that is already gone says the same
    /// thing as closing one that never was.
    /// </remarks>
    /// <param name="key">The window's key, or nothing for the main one.</param>
    public void Close(string? key)
    {
        if (_main is { } main && key is null)
        {
            main.Close();

            return;
        }

        if (key is not null && _windows.TryGetValue(key, out var window))
        {
            window.Close();
        }
    }

    /// <summary>
    /// Lets go of a window that has closed, and ends the program if it was the last one.
    /// </summary>
    /// <remarks>
    /// A dock that was in the window is brought back to the main one here, whether it was on show or not: a
    /// dock left naming a window that is gone is a dock nothing can draw — the next time it is asked for,
    /// there is no window to ask, and it stays invisible for the rest of the run. Bringing it back is what
    /// keeps a dock a dock, and it is deliberately not said through <see cref="Changed"/>: this runs on the
    /// way out of a window closing, and a rebuild started from there would be work for a window that is
    /// already gone, and would run it for every dock that was in it.
    /// </remarks>
    /// <param name="key">The window's key, or nothing for the main one.</param>
    private void Gone(string? key)
    {
        if (key is null)
        {
            _main = null;
        }
        else
        {
            _windows.Remove(key);

            foreach (var instance in _docks.Instances.Where(one => one.Window == key).ToArray())
            {
                _docks.Rehome(instance);
                _log.Record(
                    LogLevel.Debug,
                    Core.Source,
                    $"`{instance.DockNameId}` came back to the main window from `{key}`, which closed"
                );
            }
        }

        _held.Remove(key ?? string.Empty);

        if (Count > 0 || _ending)
        {
            return;
        }

        _ending = true;
        End?.Invoke();
    }

    /// <summary>An instance of a dock that is open, or nothing.</summary>
    /// <param name="registration">The dock.</param>
    /// <returns>The instance, or nothing when none is open.</returns>
    private DockInstance? Instance(DockRegistration registration) =>
        _docks.InstanceOf(registration.DockNameId);

    /// <summary>
    /// Closes the main window when the last dock left it.
    /// </summary>
    /// <remarks>
    /// The main window is a window of docks and of nothing else, so one that holds none is a window with
    /// nothing to draw in — the same judgement a dock's own window is closed on. Whether it is up is asked
    /// rather than assumed, because the message loop a close is posted through outlives the call that posted
    /// it: the dock being moved may be the second one out of a window that went with the first.
    /// </remarks>
    private void LeaveMain()
    {
        if (_main is not null && !_docks.Instances.Any(instance => instance.Window is null))
        {
            Close(null);
        }
    }

    /// <summary>
    /// Makes a window, has it drawn, and starts showing it.
    /// </summary>
    /// <param name="key">The window's key, or nothing for the main one.</param>
    /// <param name="instance">The dock it is for, or nothing for the main window.</param>
    /// <returns>The window, or nothing when there is no way to make one.</returns>
    private Window? Add(string? key, DockInstance? instance)
    {
        if (_make is null)
        {
            return null;
        }

        var window = _make(key, instance);

        // The key is read back off the window rather than closed over, so that every window of the set is
        // one window's answer to what it is: a list of keys beside a list of windows is a second truth to
        // keep in step, and the one thing that can disagree with it is the window itself.
        window.Closed += (_, _) => Gone(window.Tag as string);

        if (key is not null)
        {
            _windows[key] = window;
        }

        window.Show();

        return window;
    }

    /// <summary>Shows a window and brings it to the front.</summary>
    /// <remarks>
    /// Activated rather than focused, because what is being asked for is the window being come back to
    /// rather than the keyboard being moved into one control of it: a window that is behind another is
    /// brought forward, and a window that is minimized is restored.
    /// </remarks>
    /// <param name="window">The window to bring forward.</param>
    private static void Bring(Window window)
    {
        if (!window.IsVisible)
        {
            window.Show();
        }

        if (window.WindowState == WindowState.Minimized)
        {
            window.WindowState = WindowState.Normal;
        }

        window.Activate();
    }

    /// <summary>
    /// Closes the windows of docks that are gone.
    /// </summary>
    /// <remarks>
    /// A window of a dock's own is a window of that dock: a dock that was closed from its tab, or closed
    /// from the window's own button, leaves a window with nothing to draw in, and the window goes with it.
    /// The main window is not closed here — it is not for any one dock — and neither is a window whose dock
    /// is merely hidden, since a hidden dock is one that is still there.
    /// </remarks>
    private void Settle()
    {
        foreach (var (key, instance) in _held.ToArray())
        {
            if (_docks.Instances.Contains(instance))
            {
                continue;
            }

            _held.Remove(key);
            Close(key);
        }
    }
}

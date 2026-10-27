using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Input;
using Avalonia.Interactivity;
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

    /// <summary>
    /// Where the window the host's own commands act on is named.
    /// </summary>
    /// <remarks>
    /// Named here rather than by whoever makes a window, because a window going away is this set's to hear
    /// about: a name left on a window that has closed would have every question asked nowhere.
    /// </remarks>
    private readonly Shell _shell;

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
    /// <param name="shell">Where the window the host's own commands act on is named.</param>
    /// <param name="icon">The picture the program is known by.</param>
    public DockWindows(
        DockManager docks,
        I18nService i18n,
        Refocus refocus,
        LogService log,
        IIconLibrary icons,
        DockMenuRegistry menus,
        Shell shell,
        WindowIcon? icon
    )
    {
        _docks = docks;
        _i18n = i18n;
        _refocus = refocus;
        _log = log;
        _icons = icons;
        _menus = menus;
        _shell = shell;
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
    /// Whether the drag in flight was over a dock area of one of the host's windows the last time an area
    /// heard about it.
    /// </summary>
    /// <remarks>
    /// What a drag that ended in no drop is read as: a drag the host last heard of over a dock area was one that
    /// ended without a drop there, which is a drag called off rather than let go of, while one it last heard of
    /// nowhere was let go of in the open — and a dock let go of in the open is a dock the user pulled out of
    /// the window it was in. The platform says the same thing about both, since neither landed anywhere, so
    /// the last thing an area heard is what tells them apart. Written by whichever area hears the drag, which
    /// is why it is here and not in the drag itself.
    /// </remarks>
    public bool OverArea { get; set; }

    /// <summary>
    /// Whether the drag in flight has been called off with Esc.
    /// </summary>
    /// <remarks>
    /// The platform reports a drag called off with Esc as one that applied no effect, which is the same thing it
    /// reports for a drag let go of where there was nothing to take it — so the two cannot be told apart by what
    /// the drag returns. This is the other half: while a drag is in flight the window it was picked up in is heard
    /// for Esc, and an area that sees this set offers no place to land and takes none.
    /// </remarks>
    public bool Cancelled { get; private set; }

    /// <summary>
    /// Whether a drag that is in flight was let go of over a dock area.
    /// </summary>
    /// <remarks>
    /// Told apart from <see cref="OverArea"/>, which is where the pointer was last heard of: the platform says a drop
    /// was delivered by delivering it, so an area that took a drop — even one with no place to put the dock — says
    /// the release was there. That matters because a released drag is followed by the platform's own
    /// <c>DragLeave</c>, which would otherwise leave the record reading "last heard of nowhere" for a drag that was
    /// very much let go of over a dock area.
    /// </remarks>
    public bool Dropped { get; set; }

    /// <summary>
    /// Raised when a drag that was picked up has ended, however it ended.
    /// </summary>
    /// <remarks>
    /// What an area hears instead of a drop: a drag called off with Esc, or let go of where it was over nothing,
    /// has no drop to be told about, and an area that had drawn the points would otherwise leave them up.
    /// </remarks>
    public event Action? DragEnded;

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
        var made = _docks.CreateInWindow(registration, request, key);

        if (made is null)
        {
            return;
        }

        _held[key] = made;

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

        // The group it came from is kept as its home rather than replaced by the centre of its new window:
        // pulling a dock out of a group is about where it is shown, and being in a window of its own is a
        // change of window and not of group — a dock taken out of the bottom band comes back to it. The main
        // window is left standing even where this was the last dock in it (see `Settle`).
        _docks.DetachToWindow(instance, key);

        if (Add(key, instance) is { } opened)
        {
            Bring(opened);
        }
    }

    /// <summary>
    /// Takes a dock by the pointer: hands it to the platform, and lands what comes back.
    /// </summary>
    /// <remarks>
    /// A drag that no dock area took is a dock the user pulled out of the window it was in, so it becomes a window
    /// of its own — which is the whole of what letting go of a header away from every area means. A dock in a
    /// window of its own has nothing to tear out of: the window it is in is already its own, so a drag of it that
    /// lands nowhere leaves it where it is.
    /// <para>
    /// A drag whose pointer was over a dock area when it ended, and which no drop took, is one that was called off
    /// rather than let go of, and a dock that was never let go of stays where it was.
    /// </para>
    /// <para>
    /// Esc is the third way a drag can end, and the one the platform does not report differently: a drag called off
    /// with Esc is returned as having applied no effect, exactly as a drag let go of where there is nothing to take
    /// it. So the window the dock was picked up in is heard for Esc for as long as the platform owns the gesture,
    /// and a drag that heard one is not a drag let go of in the open — it is one the user called off.
    /// </para>
    /// <para>
    /// A platform that will not carry a drag at all — a toolkit given no drag of its own — leaves the dock where it
    /// was as well: the gesture never reached anything, and a window nobody asked for is worse than a drag that did
    /// nothing.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock the pointer took hold of.</param>
    /// <param name="from">The press the drag begins at.</param>
    /// <param name="window">The window it was picked up in, or nothing for the main one.</param>
    public async void PickUp(DockInstance instance, PointerPressedEventArgs from, string? window)
    {
        ArgumentNullException.ThrowIfNull(instance);
        ArgumentNullException.ThrowIfNull(from);

        OverArea = false;
        Dropped = false;
        Cancelled = false;

        var host = Host(window);
        void Called(object? sender, KeyEventArgs e)
        {
            if (e.Key == Key.Escape)
            {
                Cancelled = true;
            }
        }

        host?.AddHandler(
            InputElement.KeyDownEvent,
            Called,
            RoutingStrategies.Tunnel,
            handledEventsToo: true
        );

        DragDropEffects applied;

        try
        {
            applied = await DockDrag.Away(from, instance);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Record(
                LogLevel.Warn,
                Core.Source,
                $"`{instance.DockNameId}` could not be dragged: {error.Message}"
            );

            return;
        }
        finally
        {
            host?.RemoveHandler(InputElement.KeyDownEvent, Called);
        }

        var over = OverArea;
        var dropped = Dropped;
        var cancelled = Cancelled;

        OverArea = false;
        Dropped = false;
        Cancelled = false;
        DragEnded?.Invoke();

        if (!TearsOut(applied, over || dropped, cancelled, window))
        {
            return;
        }

        // A dock that is no longer one of the manager's — closed from its tab, or by its view, while the
        // pointer was still holding it — has no window to be torn into: what would be made is a window of a
        // dock that is gone.
        if (!_docks.Instances.Contains(instance))
        {
            return;
        }

        ToOwnWindow(instance);
    }

    /// <summary>
    /// Whether a drag that has ended with no drop is a dock the user pulled out of the window it was in.
    /// </summary>
    /// <remarks>
    /// The one reading of what can only be guessed at from the outside, kept as a function of its own so that it can
    /// be said what it answers rather than only how it is used: a drag that applied an effect landed somewhere; one
    /// that was called off with Esc was the user's own; one over a dock area when it ended, or which a dock area took
    /// a drop for, was let go of over that area and so was called off; and one picked up in a window of its own has
    /// no window to be torn into. What is left is a drag let go of in the open, which is the dock becoming a window
    /// of its own.
    /// </remarks>
    /// <param name="applied">What the platform said the drag came to.</param>
    /// <param name="over">Whether the drag ended over a dock area, or was let go of into one.</param>
    /// <param name="cancelled">Whether the drag was called off with Esc.</param>
    /// <param name="window">The window it was picked up in, or nothing for the main one.</param>
    /// <returns>Whether it is to become a window of its own.</returns>
    internal static bool TearsOut(
        DragDropEffects applied,
        bool over,
        bool cancelled,
        string? window
    ) => applied == DragDropEffects.None && !over && !cancelled && window is null;

    /// <summary>The window a dock was picked up in, or nothing when that window is not up.</summary>
    /// <param name="key">The window's key, or nothing for the main one.</param>
    /// <returns>The window, or nothing.</returns>
    private Window? Host(string? key) =>
        key is null
            ? _main
            : _windows.TryGetValue(key, out var found)
                ? found
                : null;

    /// <summary>
    /// Lands a dragged dock where a drop of it in one of the host's windows said.
    /// </summary>
    /// <remarks>
    /// The same judgement <see cref="MoveToMain"/> makes, because a dock dropped on the main window is the same
    /// thing asked for with the hand: a dock that keeps one instance of its own, where that window already
    /// shows it, is not moved in — the dock is already there, so what is let go of is the instance that was
    /// being dragged, and the window it came from goes with it.
    /// <para>
    /// A drag let go of where it already was is not a move at all, which is <see cref="DockManager.Move"/>'s own
    /// answer rather than a second one here. Only the main window accepts a drop, since a window of one dock has
    /// nothing to put a second one beside.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock that was dragged.</param>
    /// <param name="target">Where the drop said it goes.</param>
    /// <param name="window">The window it was let go of in, or nothing for the main one.</param>
    public void Land(DockInstance instance, DockTarget target, string? window)
    {
        ArgumentNullException.ThrowIfNull(instance);

        if (WouldDouble(instance, window))
        {
            _docks.Discard(instance);

            return;
        }

        _docks.Move(instance, target);
    }

    /// <summary>
    /// Brings a dock back to the main window, which is what a window of its own offers.
    /// </summary>
    /// <remarks>
    /// The main window is where a dock of its own came from, so it is what it goes back to — and it is opened
    /// first where it is not up, because a dock brought back to a window that is not there would be no better
    /// off than it was.
    /// <para>
    /// A dock that keeps one instance of its own, where the main window already shows that dock, is the one
    /// thing this cannot do: the two are one dock, and moving this instance in would leave two of a name that
    /// may have one. What is let go of is the instance in the window of its own, and that window with it —
    /// what the user asked for is already the case, and a window of a dock the main window already holds is
    /// the only thing left over.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock to bring back.</param>
    public void MoveToMain(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        if (instance.Window is null)
        {
            return;
        }

        OpenMain();

        if (WouldDouble(instance, null))
        {
            _docks.Discard(instance);

            return;
        }

        // The group it came from is the group it goes back to, which is what a dock taken out of a band and
        // brought back finds again — or the active group where that one has been pruned since (see
        // `DockManager.Rehome`).
        _docks.ReturnToMain(instance);
    }

    /// <summary>
    /// Whether a dock may not be shown where it is going, because it is already shown there.
    /// </summary>
    /// <remarks>
    /// A dock that keeps one instance of its own is one dock wherever its instances are, so a second one of
    /// those in the main window is one too many: what a move would leave is two docks of a name that may have
    /// one, and the <c>Window</c> menu's toggle would find the wrong one of them.
    /// </remarks>
    /// <param name="instance">The dock that is going somewhere.</param>
    /// <param name="window">Where it is going, or nothing for the main window.</param>
    /// <returns>Whether that window already shows it.</returns>
    private bool WouldDouble(DockInstance instance, string? window)
    {
        if (instance.OpenMode != DockOpenMode.Toggle)
        {
            return false;
        }

        // A window of one dock is not a place two docks are put in: what it holds is the dock it was made
        // for, so nothing can double there.
        if (window is not null)
        {
            return false;
        }

        return _docks.Instances.Any(one =>
            !ReferenceEquals(one, instance)
            && one.IsOpen
            && one.Window is null
            && string.Equals(one.DockNameId, instance.DockNameId, StringComparison.Ordinal)
        );
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
            registration.DefaultSide,
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
    /// A dock that was moved into the window is brought back to the main one here, whether it was on show or
    /// not: a dock left naming a window that is gone is a dock nothing can draw — the next time it is asked
    /// for, there is no window to ask, and it stays invisible for the rest of the run. Bringing it back is
    /// what keeps a dock a dock, and it is deliberately not said through <see cref="Changed"/>: this runs on
    /// the way out of a window closing, and a rebuild started from there would be work for a window that is
    /// already gone, and would run it for every dock that was in it.
    /// <para>
    /// A dock that was <em>opened</em> in the window is the other case, and it is let go of instead: it was
    /// asked for as a window of its own — a directory in a window of its own, which is what the File System's
    /// own item asks for — so it was never in the main window, and a dock appearing there when the window it
    /// was asked for is closed is a dock nobody asked for. A toggle is rehomed though it was opened there,
    /// since a toggle is one dock looked for by name and the main window's <c>Window</c> menu is where it is
    /// looked for from.
    /// </para>
    /// </remarks>
    /// <param name="window">The window that closed.</param>
    private void Gone(Window window)
    {
        var key = window.Tag as string;

        if (key is null)
        {
            _main = null;
        }
        else
        {
            _windows.Remove(key);

            foreach (var instance in _docks.Instances.Where(one => one.Window == key).ToArray())
            {
                if (instance.OpenedHere && instance.OpenMode != DockOpenMode.Toggle)
                {
                    _log.Record(
                        LogLevel.Debug,
                        Core.Source,
                        $"`{instance.DockNameId}` was opened in `{key}`, which closed, and is let go of with it"
                    );
                    _docks.Discard(instance);

                    continue;
                }

                _docks.Rehome(instance);
                _log.Record(
                    LogLevel.Debug,
                    Core.Source,
                    $"`{instance.DockNameId}` came back to the main window from `{key}`, which closed"
                );
            }
        }

        _held.Remove(key ?? string.Empty);

        // The shell must not keep naming a window that is gone: a question put over one is a question asked
        // nowhere. The main window is where a dock comes back to, so it is named first when it is up, and any
        // window still drawn is a window to ask over when it is not. Nothing is named only when none is left.
        if (ReferenceEquals(_shell.Window, window))
        {
            _shell.Window =
                _main is { IsVisible: true } main ? main
                : _windows.Values.FirstOrDefault(one => one.IsVisible);
        }

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
        window.Closed += (_, _) => Gone(window);

        if (key is not null)
        {
            _windows[key] = window;
        }

        // The shell names the window the host's own commands act on, which is where a question or a notice
        // is put: a window that is made is one that is up, and one the user comes back to is the one they
        // are working in. A window going away is heard about below, so the name never outlives it.
        _shell.Window = window;
        window.Activated += (_, _) => _shell.Window = window;

        window.Show();

        return window;
    }

    /// <summary>Shows a window and brings it to the front.</summary>
    /// <remarks>
    /// Activated rather than focused, because what is being asked for is the window being come back to
    /// rather than the keyboard being moved into one control of it: a window that is behind another is
    /// brought forward, and a window that is minimized is restored. It is named as the shell's window as
    /// well, since what the user asked for is where the next question is to be put.
    /// </remarks>
    /// <param name="window">The window to bring forward.</param>
    private void Bring(Window window)
    {
        if (!window.IsVisible)
        {
            window.Show();
        }

        if (window.WindowState == WindowState.Minimized)
        {
            window.WindowState = WindowState.Normal;
        }

        _shell.Window = window;
        window.Activate();
    }

    /// <summary>
    /// Closes the windows of docks that are gone, or that have moved into another window.
    /// </summary>
    /// <remarks>
    /// A window of a dock's own is a window of that dock: a dock that was closed from its tab, or closed
    /// from the window's own button, leaves a window with nothing to draw in, and the window goes with it.
    /// So does a dock that left for another window — one brought back to the main window, or landed in a
    /// region of it — since what a window of one dock draws is that dock and nothing else, and a window left
    /// standing would be a frame around a dock that is drawn somewhere else. A window whose dock is merely
    /// hidden is not closed either, since a hidden dock is one that is still there.
    /// <para>
    /// The main window is not closed here when it holds nothing, and that is deliberate rather than left out:
    /// it is not a window of any one dock but the program's own window — its <c>Window</c> menu is where a dock
    /// is asked for again, and it is what a dock in a window of its own is brought back to — and it is the one
    /// window a first run has, holding the one dock the starting layout names. Closing it as that dock is torn
    /// out would end the run, because what ends a run is every window being gone: the drag would take the
    /// program with it, and the dock with the window it was being moved out of. An empty main window is what is
    /// left instead, and it is a window a dock can be opened in again and closed when the user is done with it.
    /// </para>
    /// </remarks>
    private void Settle()
    {
        foreach (var (key, instance) in _held.ToArray())
        {
            if (
                _docks.Instances.Contains(instance)
                && string.Equals(instance.Window, key, StringComparison.Ordinal)
            )
            {
                continue;
            }

            _held.Remove(key);
            Close(key);
        }
    }
}

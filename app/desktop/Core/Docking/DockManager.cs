using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.Docking;

/// <summary>
/// The docks: what is registered, what is open, and where in the main window's tree each one is.
/// </summary>
/// <remarks>
/// Registration and the live instances are kept together because the two are read together — the
/// <c>Window</c> menu is built from the registrations, and the area from the instances — and because the layout
/// is written from one and restored into the other.
/// <para>
/// The manager owns one <see cref="DockLayout"/>, which is the single truth about where a dock in the main
/// window is. A dock's <see cref="DockInstance.GroupId"/> is a reading of that tree rather than a second copy of
/// it: every operation here changes the tree and then writes the id the tree gives back.
/// </para>
/// <para>
/// A dock's view factory is plugin code and may throw. That is caught here: the dock does not open, the failure
/// is logged and raised to the user, and nothing else is disturbed.
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

    /// <summary>
    /// Which dock of each name the user worked in last, which is what a caller asking for a kind of dock gets.
    /// </summary>
    /// <remarks>
    /// The focus says where the user is now; this says where they were in a dock of a kind. A press inside a dock
    /// moves the focus to it, so a plugin that acts from another dock — the Overview asking for a file to be shown
    /// in a browser — needs the browser the user left behind rather than the one that asked. One entry per name,
    /// because that is the question a caller asks: a browser, a tree, a graph.
    /// </remarks>
    private readonly Dictionary<string, DockInstance> _recent = new(StringComparer.Ordinal);

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

    /// <summary>Which dock the user is working in.</summary>
    /// <remarks>
    /// The record is the docks' own rather than a service beside them, because it is the docks that move it: a
    /// dock closing is a dock that has to hand it on, and the manager is what knows which docks are there.
    /// </remarks>
    public DockFocus Focus { get; } = new();

    /// <summary>The dock of a name the user worked in last, if it is still open.</summary>
    /// <param name="dockNameId">The dock's stable name id.</param>
    /// <returns>The dock, or nothing when no instance of that name has been worked in or it is gone.</returns>
    public DockInstance? Recent(string dockNameId)
    {
        ArgumentNullException.ThrowIfNull(dockNameId);

        return _recent.TryGetValue(dockNameId, out var dock)
            && _instances.Contains(dock)
            && dock.IsOpen
                ? dock
                : null;
    }

    /// <summary>Makes a dock the one the user is working in.</summary>
    /// <remarks>
    /// What an area says when a press lands in a dock, when the keyboard goes into one, and when its tab is
    /// chosen; and what the windows say when one of them comes forward. Saying it for a dock that is not open is
    /// the caller's mistake rather than something to correct here: the manager is what passes the focus on when
    /// the dock it named goes.
    /// </remarks>
    /// <param name="instance">The dock being worked in.</param>
    public void WorkedIn(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        Remember(instance);
    }

    /// <summary>
    /// Says that one of the host's windows has come forward, so the dock it is for is the one being worked in.
    /// </summary>
    /// <remarks>
    /// A window of its own holds one dock, and it is that dock. The main window holds many, so what it answers is
    /// the dock it was already working in where that one is still open, and otherwise the one its active group is
    /// showing: a window coming back is not a reason to move the work to another dock of it, and it is a reason to
    /// name one when nothing was named or the named one is elsewhere.
    /// </remarks>
    /// <param name="sole">
    /// The dock a window of one dock holds, or nothing for the main window.
    /// </param>
    public void ActivateWindow(DockInstance? sole)
    {
        if (sole is not null)
        {
            if (sole.IsOpen)
            {
                Remember(sole);
            }

            return;
        }

        if (
            Focus.Instance is { } current
            && current.Window is null
            && current.IsOpen
            && _instances.Contains(current)
        )
        {
            return;
        }

        if (MainWindowDock() is { } dock)
        {
            Remember(dock);
        }
    }

    /// <summary>Raised whenever the set of open docks or the shape of the main window's tree changes.</summary>
    public event Action? Changed;

    /// <summary>
    /// Raised whenever a dock keeps something about itself, or the arrangement changes without its shape
    /// changing.
    /// </summary>
    /// <remarks>
    /// Told apart from <see cref="Changed"/> because the two are answered differently: a change to the docks is
    /// a change to the area, which is rebuilt, where what a dock remembers about itself and which tab or group
    /// is active are nothing the area redraws from scratch — only the layout file has to hear about them.
    /// </remarks>
    public event Action? StateChanged;

    /// <summary>The tree of the main window: which groups there are, where, and which docks they hold.</summary>
    public DockLayout Layout { get; } = new();

    /// <summary>
    /// Raised when a dock says it is looking at something else, which is a change to what it is called.
    /// </summary>
    /// <remarks>
    /// Told apart from <see cref="Changed"/> and <see cref="StateChanged"/> because the three are answered by different
    /// work: a change to the docks rebuilds the area, and what a dock keeps about itself or which tab is chosen is
    /// only written down — while a dock's name is a word in the tab, the window it is in and the top of its menu, and
    /// what has to happen is those three being said again. A rebuild for it would be a window rearranged for a word.
    /// </remarks>
    public event Action<DockInstance>? Renamed;

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
    /// Activates a dock from the <c>Window</c> menu: a toggle is shown or hidden, a factory makes a new
    /// instance.
    /// </summary>
    /// <param name="registration">The dock.</param>
    /// <returns>The instance that was opened, or nothing when one could not be made.</returns>
    public DockInstance? Activate(DockRegistration registration)
    {
        ArgumentNullException.ThrowIfNull(registration);

        if (registration.OpenMode == DockOpenMode.Toggle)
        {
            return Toggle(registration);
        }

        return Create(registration, registration.DefaultSide, Request(registration, null));
    }

    /// <summary>
    /// Shows or hides a toggle dock, making its one instance the first time.
    /// </summary>
    /// <param name="registration">The dock.</param>
    /// <returns>The dock's instance, or nothing when one could not be made.</returns>
    public DockInstance? Toggle(DockRegistration registration)
    {
        ArgumentNullException.ThrowIfNull(registration);

        var existing = InstanceOf(registration.DockNameId);

        if (existing is not null)
        {
            existing.IsOpen = !existing.IsOpen;
            Raise();

            if (existing.IsOpen)
            {
                Remember(existing);
            }
            else
            {
                Replaced(existing);
            }

            return existing;
        }

        return Create(registration, registration.DefaultSide, Request(registration, null));
    }

    /// <summary>
    /// Makes a new dock instance in the main window, put into the tree at the side it asks for.
    /// </summary>
    /// <remarks>
    /// The side is applied to the active group: <see cref="DockSide.Center"/> puts the dock among that group's
    /// tabs and an edge splits it. A main window holding no docks has no active group, so the dock makes the
    /// root group and the side says nothing — there is no group to be beside.
    /// </remarks>
    /// <param name="registration">The dock.</param>
    /// <param name="side">Where in the active group to put it.</param>
    /// <param name="request">What the dock is being opened with.</param>
    /// <returns>The new instance, or nothing when its view could not be made.</returns>
    public DockInstance? Create(DockRegistration registration, DockSide side, DockRequest request)
    {
        ArgumentNullException.ThrowIfNull(registration);
        ArgumentNullException.ThrowIfNull(request);

        var made = Make(registration, Nothing, request, window: null, groupId: "");

        if (made is null)
        {
            return null;
        }

        var group = ActiveOrFirst();
        var target = group is null
            ? new DockTarget("", DockSide.Center, 0)
            : new DockTarget(group.Id, side, group.Docks.Count);

        made.GroupId = DockTree.Insert(Layout, target, Record(made));
        Layout.ActiveGroup = made.GroupId;
        Raise();
        Remember(made);

        return made;
    }

    /// <summary>
    /// Makes a new dock instance in a window of its own, over what a caller asked for.
    /// </summary>
    /// <remarks>
    /// The window is stated here rather than assigned afterwards, because there is no moment at which a dock
    /// opened in a window of its own is in the main window's tree: a dock made at the centre and then moved
    /// would be drawn in a group of the main window for as long as that took, and the point of opening it in a
    /// window of its own is that it never is. It has no group, because it was never in the tree.
    /// </remarks>
    /// <param name="registration">The dock.</param>
    /// <param name="request">What the dock is being opened with.</param>
    /// <param name="window">The window it is opened in.</param>
    /// <returns>The new instance, or nothing when its view could not be made.</returns>
    public DockInstance? CreateInWindow(
        DockRegistration registration,
        DockRequest request,
        string window
    )
    {
        ArgumentNullException.ThrowIfNull(registration);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(window);

        var made = Make(registration, Nothing, request, window, groupId: "");

        if (made is null)
        {
            return null;
        }

        Raise();
        Remember(made);

        return made;
    }

    /// <summary>Makes a new dock instance, told what that dock remembered last time.</summary>
    /// <param name="registration">The dock.</param>
    /// <param name="kept">What the dock kept about itself, by key.</param>
    /// <param name="request">What the dock is being opened with.</param>
    /// <param name="window">The window it is in, or nothing for the main one.</param>
    /// <param name="groupId">The group of the main window's tree it is in, or empty.</param>
    /// <returns>The new instance, or nothing when its view could not be made.</returns>
    private DockInstance? Make(
        DockRegistration registration,
        IReadOnlyDictionary<string, string> kept,
        DockRequest request,
        string? window,
        string groupId
    )
    {
        IDockView view;

        try
        {
            view = registration.Create();
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
            Detail = _i18n.Get(registration.DisplayNameKey),
            Request = request,
            Window = window,
            // A dock made in a window was made for it, and one made with no window is the main window's.
            OpenedHere = window is not null,
            GroupId = groupId,
            IsOpen = true,
        };

        foreach (var (key, value) in kept)
        {
            instance.Meta[key] = value;
        }

        _instances.Add(instance);

        // A dock that names itself by where it is says so as the place it is opened onto arrives, which is why the
        // listening is put on before it is told what it was opened with: what a dock says is said by what it does.
        var titled = view as IDockTitle;

        if (titled is not null)
        {
            titled.TitleChanged += () => Retitled(instance, titled);
        }

        // Told after it exists and before it is drawn, so that a view which reads what it kept has read it by
        // the time the area draws it, and a view which kept nothing is none the wiser. A view that throws here
        // is a dock that does not open, as one that throws while being made.
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

        // And what it was opened with comes after, so that a dock which was asked for at a directory of the
        // caller's choosing is drawn there rather than being restored somewhere else and moved under the first
        // frame. A dock with nothing to do about it is one that does not implement this.
        if (view is IDockOpened told)
        {
            try
            {
                told.Opened(request);
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Record(
                    LogLevel.Error,
                    Core.Source,
                    $"`{registration.DockNameId}` could not take what it was opened with: {error.Message}"
                );
            }
        }

        if (titled is not null)
        {
            // What it says once it has been restored and told where to look: a dock that took a place of its own as it
            // was opened has said nothing about it until then.
            Retitled(instance, titled);
        }

        return instance;
    }

    /// <summary>
    /// Takes what a dock says it is looking at, where it says anything, and says the dock's name changed.
    /// </summary>
    /// <remarks>
    /// The registered name is what a dock that says nothing is called, which is every dock with one instance: a name
    /// that is a kind and not a place is the whole of what such a dock has to say. What a dock does say is kept here
    /// rather than asked of it each time something draws it, because a tab is drawn again for reasons the dock knows
    /// nothing about — a window resize, a splitter, a group rearranged — and asking a view for its name on each of
    /// those would be asking it on every layout pass.
    /// </remarks>
    /// <param name="instance">The dock.</param>
    /// <param name="titled">What the dock says about itself.</param>
    private void Retitled(DockInstance instance, IDockTitle titled)
    {
        var name = _i18n.Get(instance.Registration.DisplayNameKey);
        var title = titled.Title is { Length: > 0 } said ? said : name;
        var detail = titled.Detail is { Length: > 0 } where ? where : title;

        if (
            string.Equals(instance.Title, title, StringComparison.Ordinal)
            && string.Equals(instance.Detail, detail, StringComparison.Ordinal)
        )
        {
            return;
        }

        instance.Title = title;
        instance.Detail = detail;
        Renamed?.Invoke(instance);
    }

    /// <summary>
    /// An instance of a dock that is already open, or nothing.
    /// </summary>
    /// <remarks>
    /// One instance of every name, which is what a dock with a single instance is: what a caller asking for one
    /// of those wants is the one it already has rather than a second one beside it.
    /// </remarks>
    /// <param name="dockNameId">The dock's stable name id.</param>
    /// <returns>The instance, or nothing when none is open.</returns>
    public DockInstance? InstanceOf(string dockNameId) =>
        _instances.FirstOrDefault(instance =>
            string.Equals(instance.DockNameId, dockNameId, StringComparison.Ordinal)
        );

    /// <summary>
    /// One instance of a dock, told apart from its namesakes by its ordinal.
    /// </summary>
    /// <remarks>
    /// How a drag names the instance it is carrying: a dock may be made again, so its name says which dock it
    /// is and not which of them, and the ordinal is what the layout writes beside the name for the same reason.
    /// </remarks>
    /// <param name="dockNameId">The dock's stable name id.</param>
    /// <param name="ordinal">Which instance of that name.</param>
    /// <returns>The instance, or nothing when that one is not open.</returns>
    public DockInstance? InstanceOf(string dockNameId, int ordinal) =>
        _instances.FirstOrDefault(instance =>
            instance.Ordinal == ordinal
            && string.Equals(instance.DockNameId, dockNameId, StringComparison.Ordinal)
        );

    /// <summary>
    /// Closes a dock: a toggle is hidden and kept, so showing it again is showing the same instance; a created
    /// instance is discarded.
    /// </summary>
    /// <param name="instance">The dock to close.</param>
    public void Close(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        if (instance.OpenMode == DockOpenMode.Toggle)
        {
            instance.IsOpen = false;
            LetGo(instance);
            Replaced(instance);

            return;
        }

        Discard(instance);
    }

    /// <summary>
    /// Lets go of an instance for good, whether or not its dock is one that is kept.
    /// </summary>
    /// <remarks>
    /// What hides a toggle is <see cref="Close"/>, which leaves the instance where it is so that showing it
    /// again is showing the same one. This is for an instance that must not be there at all: a second one of a
    /// name that keeps only one, which is what a window of its own can come to hold when a dock is asked for
    /// anew. The tree is told it is gone too, since a recorded dock nothing is open as is a place in the
    /// arrangement that can never be drawn.
    /// </remarks>
    /// <param name="instance">The dock to let go of.</param>
    public void Discard(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        if (!_instances.Remove(instance))
        {
            return;
        }

        var home = DockTree.GroupOf(Layout, instance.DockNameId, instance.Ordinal);

        DockTree.Remove(Layout, instance.DockNameId, instance.Ordinal);
        LetGo(instance);
        Replaced(instance, home?.Id);
    }

    /// <summary>
    /// Lets go of what a dock's view holds, and tells the windows it is gone.
    /// </summary>
    /// <remarks>
    /// The view is let go of here, which is the moment a dock stops being a dock: it is held apart from every
    /// window it was drawn in — a dock closed from a window of its own is one whose control has already left
    /// the tree — and what it holds of the plugin's would otherwise outlive it.
    /// </remarks>
    /// <param name="instance">The dock that has stopped being one.</param>
    private void LetGo(DockInstance instance)
    {
        (instance.View as IDisposable)?.Dispose();
        Raise();
    }

    /// <summary>
    /// Brings a dock back to the main window's tree, which is what happens when the window it was in goes
    /// away.
    /// </summary>
    /// <remarks>
    /// It goes back to the group it came from, since that is what a window of its own was a leave of absence
    /// from; if that group has been pruned in the meantime — its last dock may have been the one torn out —
    /// it goes into the active group, or makes the root group when there is none.
    /// <para>
    /// Nothing is raised: this is said by the one that is closing a window, in the middle of closing it, and a
    /// rebuild started from there would be asking the windows to draw for a window that is already on its way
    /// out. A dock that was closed stays closed, so one that is asked for again is shown and one that was
    /// hidden is not.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock to bring back.</param>
    public void Rehome(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        // A dock the tree already holds is only being told its window is gone; there is nowhere to insert it.
        if (DockTree.GroupOf(Layout, instance.DockNameId, instance.Ordinal) is not null)
        {
            instance.Window = null;
            instance.OpenedHere = false;

            return;
        }

        var home = DockTree.Find(Layout, instance.GroupId) ?? ActiveOrFirst();
        var target = home is null
            ? new DockTarget("", DockSide.Center, 0)
            : new DockTarget(home.Id, DockSide.Center, home.Docks.Count);

        instance.GroupId = DockTree.Insert(Layout, target, Record(instance));
        instance.Window = null;
        instance.OpenedHere = false;
    }

    /// <summary>
    /// Brings a dock back to the main window, which is what a window of its own offers.
    /// </summary>
    /// <param name="instance">The dock to bring back.</param>
    public void ReturnToMain(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        Rehome(instance);
        Raise();
    }

    /// <summary>
    /// Brings a dock forward where it is: its tab chosen in the main window, or it named as the one worked in.
    /// </summary>
    /// <remarks>
    /// The chosen tab is written into the arrangement and the area rebuilt, because that is what the area settles a
    /// region from: saying it anywhere else would leave two answers to which tab is shown. A dock in a window of its
    /// own has no tab to choose — the window that holds it is what is brought forward, and the caller is what knows
    /// which of the host's windows that is — so what is done here is naming it.
    /// </remarks>
    /// <param name="instance">The dock to bring forward.</param>
    public void Bring(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        if (!_instances.Contains(instance))
        {
            return;
        }

        if (instance.Window is not null)
        {
            Remember(instance);

            return;
        }

        if (DockTree.GroupOf(Layout, instance.DockNameId, instance.Ordinal) is not { } group)
        {
            return;
        }

        var at = group.Docks.FindIndex(dock =>
            dock.Ordinal == instance.Ordinal
            && string.Equals(dock.DockNameId, instance.DockNameId, StringComparison.Ordinal)
        );

        if (at < 0)
        {
            return;
        }

        group.Active = at;
        Layout.ActiveGroup = group.Id;
        Remember(instance);
        Raise();
        StateChanged?.Invoke();
    }

    /// <summary>
    /// Shows a dock that was hidden.
    /// </summary>
    /// <remarks>
    /// A hidden dock is one that is still there, so showing it again is showing the same instance rather than
    /// making another — which is what a dock with a single instance of its own is, and why asking for one of
    /// those to be shown where it is already open gives back the one there is.
    /// </remarks>
    /// <param name="instance">The dock to show.</param>
    public void Show(DockInstance instance)
    {
        ArgumentNullException.ThrowIfNull(instance);

        if (instance.IsOpen)
        {
            return;
        }

        instance.IsOpen = true;
        Raise();
        Remember(instance);
    }

    /// <summary>
    /// Puts a dock where a drop said, whether it came from the main window or from a window of its own.
    /// </summary>
    /// <remarks>
    /// The one place a dock's window is ever written, so that being moved is one change and not two: the window
    /// it is leaving and the one it is arriving in would otherwise both draw it for as long as it took to say
    /// both, and a view belongs to one parent at a time. A move that would put a dock where it already is says
    /// nothing, because a drag that ended where it started is not a move.
    /// <para>
    /// A dock dragged within its own group is a reordering rather than a split: the index a drop names is where
    /// the tab goes before the dock is taken out, so it is one place further along once the dock is gone.
    /// </para>
    /// </remarks>
    /// <param name="instance">The dock to move.</param>
    /// <param name="target">Where the drop said it goes.</param>
    public void Move(DockInstance instance, DockTarget target)
    {
        ArgumentNullException.ThrowIfNull(instance);

        var group = DockTree.GroupOf(Layout, instance.DockNameId, instance.Ordinal);
        var wanted = target;

        if (group is not null)
        {
            var at = group.Docks.FindIndex(dock =>
                dock.Ordinal == instance.Ordinal
                && string.Equals(dock.DockNameId, instance.DockNameId, StringComparison.Ordinal)
            );

            if (
                target.Side == DockSide.Center
                && string.Equals(target.GroupId, group.Id, StringComparison.Ordinal)
            )
            {
                var index = Math.Clamp(target.Index, 0, group.Docks.Count);
                var adjusted = index > at ? index - 1 : index;

                if (adjusted == at)
                {
                    return;
                }

                wanted = target with { Index = adjusted };
            }
            else if (
                target.Side != DockSide.Center
                && string.Equals(target.GroupId, group.Id, StringComparison.Ordinal)
                && group.Docks.Count == 1
            )
            {
                // The only dock of a group has no second place in it to be put: splitting it would mean
                // splitting a group that the removal leaves empty, which is a group that does not exist.
                return;
            }

            DockTree.Remove(Layout, instance.DockNameId, instance.Ordinal);
        }

        instance.GroupId = DockTree.Insert(Layout, wanted, Record(instance));
        instance.Window = null;
        instance.OpenedHere = false;
        Raise();
    }

    /// <summary>
    /// Moves a dock into a window of its own, keeping the group it came from as its home.
    /// </summary>
    /// <remarks>
    /// The group is remembered rather than forgotten, so that the window closing gives the dock back to where
    /// it was taken from. It may not still be there when it returns — taking the last dock of a group out
    /// prunes the group — and then the active group is the fallback (see <see cref="Rehome"/>).
    /// </remarks>
    /// <param name="instance">The dock to move.</param>
    /// <param name="key">The window it is moved into.</param>
    public void DetachToWindow(DockInstance instance, string key)
    {
        ArgumentNullException.ThrowIfNull(instance);
        ArgumentNullException.ThrowIfNull(key);

        var group = DockTree.GroupOf(Layout, instance.DockNameId, instance.Ordinal);

        if (group is not null)
        {
            DockTree.Remove(Layout, instance.DockNameId, instance.Ordinal);
            instance.GroupId = group.Id;
        }

        instance.Window = key;
        instance.OpenedHere = false;
        Raise();
    }

    /// <summary>
    /// Writes which group a click or a focus landed in, so that a dock opened without saying where goes there.
    /// </summary>
    /// <param name="groupId">The group that is now active.</param>
    public void ActivateGroup(string groupId)
    {
        ArgumentNullException.ThrowIfNull(groupId);

        if (string.Equals(Layout.ActiveGroup, groupId, StringComparison.Ordinal))
        {
            return;
        }

        if (DockTree.Find(Layout, groupId) is null)
        {
            return;
        }

        Layout.ActiveGroup = groupId;
        StateChanged?.Invoke();

        if (DockTree.Find(Layout, groupId) is { } group && GroupDock(group) is { } dock)
        {
            Remember(dock);
        }
    }

    /// <summary>
    /// Writes which tab of a group is being shown, and makes that group the active one.
    /// </summary>
    /// <remarks>
    /// The selected tab is part of the arrangement that survives a restart — which view of a place a person
    /// left open is the same kind of fact as where the place is — so it is written where the layout is, not
    /// only where the area draws it.
    /// </remarks>
    /// <param name="groupId">The group whose tab was chosen.</param>
    /// <param name="index">The chosen tab's index among the group's docks.</param>
    public void SelectTab(string groupId, int index)
    {
        ArgumentNullException.ThrowIfNull(groupId);

        var group = DockTree.Find(Layout, groupId);

        if (group is null)
        {
            return;
        }

        var active = Math.Clamp(index, 0, Math.Max(0, group.Docks.Count - 1));

        if (
            group.Active == active
            && string.Equals(Layout.ActiveGroup, groupId, StringComparison.Ordinal)
        )
        {
            return;
        }

        group.Active = active;
        Layout.ActiveGroup = group.Id;
        StateChanged?.Invoke();

        if (DockAt(group, active) is { } dock)
        {
            Remember(dock);
        }
    }

    /// <summary>
    /// Writes the share a split was dragged to.
    /// </summary>
    /// <remarks>
    /// Raised through <see cref="Changed"/> rather than kept quiet because the star weights the area renders with
    /// are read from these ratios: the grid a drag has just resized holds pixel widths until it is built again,
    /// and a window resized afterwards would then not move the splitter. The rebuild is what puts the split back
    /// under the ratio the drag chose. It is raised even where the ratio is the one it already was, since that is
    /// the case where the pixel widths most need the rebuild: a drag that ends where it started is still a drag.
    /// </remarks>
    /// <param name="split">The split that was dragged.</param>
    /// <param name="ratio">The share of its first child the drag left it at.</param>
    public void SetRatio(LayoutSplit split, double ratio)
    {
        ArgumentNullException.ThrowIfNull(split);

        split.Ratio = DockTree.Share(ratio);
        Raise();
    }

    /// <summary>Says what several splits were dragged to at once, which is one layout change rather than several.</summary>
    /// <remarks>
    /// A crossing grip moves a split on each of its axes, and a rebuild apiece would throw the structure away
    /// between two halves of one gesture: what a drag leaves behind is one arrangement, so it is written down
    /// and said once.
    /// </remarks>
    /// <param name="ratios">What each split is left at.</param>
    internal void SetRatios(IReadOnlyList<(LayoutSplit Split, double Ratio)> ratios)
    {
        foreach (var (split, ratio) in ratios)
        {
            split.Ratio = DockTree.Share(ratio);
        }

        Raise();
    }

    /// <summary>
    /// Opens the docks a saved layout names, in the groups and tab orders it states.
    /// </summary>
    /// <remarks>
    /// A name the layout states that no registration answers to is dropped: a plugin may have been removed,
    /// and the rest of the layout is still worth restoring. What is left is normalized afterwards, since a
    /// group whose docks were all dropped is a group that must not be there.
    /// </remarks>
    /// <param name="layout">What was saved.</param>
    public void Restore(DockLayout layout)
    {
        ArgumentNullException.ThrowIfNull(layout);

        Layout.Root = layout.Root;
        Layout.ActiveGroup = layout.ActiveGroup;
        Layout.GroupIdHighWater = 0;

        // Nothing a dock keeps about itself is written down while this runs, and that is not a nicety: what
        // would be written is a snapshot of the docks that happen to be open yet, which is a layout with every
        // dock further down the file dropped from it. A dock that keeps something while it is being restored
        // keeps it in memory, and the next write — of anything — takes it with it.
        _restoring = true;

        try
        {
            foreach (var group in DockTree.Groups(Layout).ToArray())
            {
                var gone = new List<LayoutDock>();

                foreach (var saved in group.Docks)
                {
                    var registration = _registry.Find(saved.DockNameId);

                    if (registration is null)
                    {
                        _log.Record(
                            LogLevel.Debug,
                            Core.Source,
                            $"the saved layout names `{saved.DockNameId}`, which no dock registers"
                        );
                        gone.Add(saved);

                        continue;
                    }

                    if (Make(registration, saved.Meta, Request(registration, null), null, group.Id) is null)
                    {
                        gone.Add(saved);
                    }
                }

                foreach (var dropped in gone)
                {
                    group.Docks.Remove(dropped);
                }
            }

            DockTree.Normalize(Layout);
        }
        finally
        {
            _restoring = false;
        }

        Raise();
    }

    /// <summary>
    /// Everything open in the main window, as it is written back to the layout file.
    /// </summary>
    /// <remarks>
    /// Walked in document order, groups as the tree states them and docks as their tabs are ordered, because
    /// that order is what comes back: a snapshot sorted by name lost both the tab order and the chosen tab. A
    /// dock in a window of its own is left out — the layout states what the main window's tree is, and a dock
    /// that is somewhere else has no group in it. A hidden toggle is left out as well, since what a run starts
    /// with is the docks that were open.
    /// </remarks>
    /// <returns>What to save.</returns>
    public DockLayout Snapshot()
    {
        var saved = new DockLayout { ActiveGroup = Layout.ActiveGroup };

        if (Layout.Root is not null)
        {
            saved.Root = Save(Layout.Root);
        }

        if (saved.Root is null)
        {
            saved.ActiveGroup = null;
        }
        else if (
            saved.ActiveGroup is not { } active
            || DockTree.Find(saved, active) is null
        )
        {
            saved.ActiveGroup = DockTree.Groups(saved).First().Id;
        }

        return saved;
    }

    /// <summary>
    /// A saved group or split, with a dock written only where one is open in the main window.
    /// </summary>
    /// <param name="node">The live node.</param>
    /// <returns>The copy, or nothing when nothing under it is open.</returns>
    private LayoutNode? Save(LayoutNode node)
    {
        if (node is LayoutSplit split)
        {
            var first = Save(split.First);
            var second = Save(split.Second);

            if (first is null)
            {
                return second;
            }

            if (second is null)
            {
                return first;
            }

            return new LayoutSplit
            {
                Axis = split.Axis,
                Ratio = split.Ratio,
                First = first,
                Second = second,
            };
        }

        var group = (LayoutGroup)node;
        var docks = new List<LayoutDock>();
        var active = 0;

        for (var at = 0; at < group.Docks.Count; at++)
        {
            var instance = OpenInMain(group.Docks[at]);

            if (instance is null)
            {
                continue;
            }

            if (at <= group.Active)
            {
                active = docks.Count;
            }

            docks.Add(
                new LayoutDock
                {
                    DockNameId = instance.DockNameId,
                    Ordinal = instance.Ordinal,
                    Meta = new Dictionary<string, string>(instance.Meta, StringComparer.Ordinal),
                }
            );
        }

        if (docks.Count == 0)
        {
            return null;
        }

        return new LayoutGroup
        {
            Id = group.Id,
            Active = Math.Min(active, docks.Count - 1),
            Docks = docks,
        };
    }

    /// <summary>The instance a recorded dock is open as in the main window, or nothing.</summary>
    /// <param name="dock">The recorded dock.</param>
    /// <returns>The instance, or nothing when it is closed or in a window of its own.</returns>
    private DockInstance? OpenInMain(LayoutDock dock)
    {
        var instance = InstanceOf(dock.DockNameId, dock.Ordinal);

        return instance is { IsOpen: true, Window: null } ? instance : null;
    }

    /// <summary>
    /// What a dock is opened with when nothing asked for anything in particular.
    /// </summary>
    /// <remarks>
    /// The name is the registration's rather than a caller's, so that a dock opened from the Window menu and one
    /// restored from the layout are the same request: what a dock reads of it is what it was told, and a dock
    /// that was told nothing reads its own name back.
    /// </remarks>
    /// <param name="registration">The dock being opened.</param>
    /// <param name="startAt">Where it is to look, or nothing.</param>
    /// <returns>What to open it with.</returns>
    private static DockRequest Request(DockRegistration registration, string? startAt) =>
        new(registration.DockNameId, startAt);

    /// <summary>A recorded dock for a live instance, which is how an operation names it to the tree.</summary>
    /// <param name="instance">The live dock.</param>
    /// <returns>The record to insert.</returns>
    private static LayoutDock Record(DockInstance instance) =>
        new() { DockNameId = instance.DockNameId, Ordinal = instance.Ordinal };

    /// <summary>The group a side is applied to: the active one, or the first the tree states.</summary>
    /// <returns>The group, or nothing when the tree holds none.</returns>
    private LayoutGroup? ActiveOrFirst()
    {
        if (Layout.ActiveGroup is { } active && DockTree.Find(Layout, active) is { } found)
        {
            return found;
        }

        return DockTree.Groups(Layout).FirstOrDefault();
    }

    /// <summary>Names a dock as the one being worked in, and remembers it as the last of its kind.</summary>
    /// <param name="instance">The dock.</param>
    private void Remember(DockInstance instance)
    {
        _recent[instance.DockNameId] = instance;
        Focus.Set(instance);
    }

    /// <summary>
    /// Passes the focus on when the dock that had it is gone, and lets go of it where nothing is left.
    /// </summary>
    /// <remarks>
    /// The dock that takes its place is the one beside it first — the sibling tab of its group, or the dock its
    /// group was split with — and otherwise any dock still open in the window it was in. The remembered dock of a
    /// kind is moved the same way, because a caller asking for a browser is asking for one that is there rather than
    /// a name whose only instance has just been closed.
    /// </remarks>
    /// <param name="gone">The dock that is gone.</param>
    /// <param name="groupId">The group it was in, when the caller knows it; otherwise it is read from the tree.</param>
    private void Replaced(DockInstance gone, string? groupId = null)
    {
        var home = groupId ?? DockTree.GroupOf(Layout, gone.DockNameId, gone.Ordinal)?.Id;
        var replacement = Neighbour(home, gone);

        if (ReferenceEquals(Focus.Instance, gone))
        {
            Focus.Set(replacement);
        }

        foreach (var (name, dock) in _recent.ToArray())
        {
            if (!ReferenceEquals(dock, gone))
            {
                continue;
            }

            var next =
                replacement is not null
                && string.Equals(replacement.DockNameId, name, StringComparison.Ordinal)
                    ? replacement
                    : _instances.LastOrDefault(one =>
                        one.IsOpen && string.Equals(one.DockNameId, name, StringComparison.Ordinal)
                    );

            if (next is null)
            {
                _ = _recent.Remove(name);
            }
            else
            {
                _recent[name] = next;
            }
        }
    }

    /// <summary>The dock that takes the focus when another goes: one beside it first, then one in its window.</summary>
    /// <param name="groupId">The group the dock was in, or nothing.</param>
    /// <param name="gone">The dock that is gone.</param>
    /// <returns>The dock, or nothing when none is left.</returns>
    private DockInstance? Neighbour(string? groupId, DockInstance gone)
    {
        if (groupId is { } id && DockTree.Find(Layout, id) is { } group)
        {
            if (GroupDock(group) is { } beside)
            {
                return beside;
            }

            foreach (var recorded in group.Docks)
            {
                if (InstanceOf(recorded.DockNameId, recorded.Ordinal) is { IsOpen: true } dock)
                {
                    return dock;
                }
            }
        }

        return _instances.FirstOrDefault(one =>
            one.IsOpen && string.Equals(one.Window, gone.Window, StringComparison.Ordinal)
        );
    }

    /// <summary>The dock a group is showing, or the first of its docks that is open.</summary>
    /// <param name="group">The group.</param>
    /// <returns>The dock, or nothing when none of its docks is open.</returns>
    private DockInstance? GroupDock(LayoutGroup group) =>
        DockAt(group, group.Active)
        ?? group
            .Docks.Select(recorded => InstanceOf(recorded.DockNameId, recorded.Ordinal))
            .OfType<DockInstance>()
            .FirstOrDefault(one => one.IsOpen);

    /// <summary>One of a group's docks, by its place in the group's tabs.</summary>
    /// <param name="group">The group.</param>
    /// <param name="index">The place.</param>
    /// <returns>The dock, or nothing when it is not open.</returns>
    private DockInstance? DockAt(LayoutGroup group, int index) =>
        index >= 0
        && index < group.Docks.Count
        && InstanceOf(group.Docks[index].DockNameId, group.Docks[index].Ordinal) is { IsOpen: true } dock
            ? dock
            : null;

    /// <summary>The dock the main window is working in, or the first one it has.</summary>
    /// <returns>The dock, or nothing when it holds none.</returns>
    private DockInstance? MainWindowDock() =>
        (ActiveOrFirst() is { } group ? GroupDock(group) : null)
        ?? _instances.FirstOrDefault(one => one.IsOpen && one.Window is null);

    /// <summary>The next instance number for a dock name.</summary>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <returns>How many instances of that name have been made.</returns>
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
    /// The keys are the dock's own business and the keeping is the host's, so nothing here is a contract with
    /// any particular dock: what a dock writes is what it reads back, and a key it never writes reads as
    /// nothing.
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

        /// <inheritdoc />
        public void Close() => _manager.Close(_instance);
    }
}

using Avalonia.Controls;

namespace RorolalaDesktop.Contract;

/// <summary>
/// What to open a dock with when a host window is asked for one of its own.
/// </summary>
/// <param name="DockNameId">The stable name id of the dock to open.</param>
/// <param name="StartAt">
/// The directory the dock is to look at, or nothing for the one it opens at by itself: what a window was
/// opened onto is not the same as where a dock it holds happens to start, and a browser told where to look
/// is the whole point of opening one from a directory's own menu.
/// </param>
/// <param name="OpenMode">
/// Whether an existing instance is to be shown instead of a new one being made, or nothing for what the
/// registration itself declares. A dock with one instance of its own — a tree, the log — is shown rather
/// than made twice, while a browser is made again: the two answers follow the registration, so nothing is
/// asked for here unless a caller has a reason to disagree with it.
/// </param>
/// <remarks>
/// The dock is named by its stable id rather than by the registration, because this is what a plugin
/// reaches the host's windows through and a registration is the host's: what is asked for is a dock, and
/// which registration answers to that name is looked up where the registrations are.
/// </remarks>
public sealed record DockRequest(
    string DockNameId,
    string? StartAt = null,
    DockOpenMode? OpenMode = null
);

/// <summary>
/// The host's windows, as a plugin sees them: where a dock is asked to be shown outside the window it
/// happens to sit in.
/// </summary>
/// <remarks>
/// It is read-only on purpose. Nothing here makes a window, moves one, or closes one: what a plugin says is
/// that a dock should be shown — which the host answers with the window it already has or one of its own —
/// and the set of windows the program is made of stays the host's to decide.
/// </remarks>
public interface IWindows
{
    /// <summary>
    /// Shows a dock in a window of its own, or focuses the window it already has.
    /// </summary>
    /// <remarks>
    /// A dock that is already in a window of its own is not moved or made again: the window is brought
    /// forward, because asking twice for the same thing means the same thing the second time.
    /// <para>
    /// A dock the registration declares as one instance is moved into a window of its own when it is already
    /// open, and one the registration lets be made again is made again. The two answers follow the dock
    /// rather than the caller: what a plugin knows about its own dock is whether the thing being shown is
    /// one thing or many, and what a caller knows is only that it wants it in a window.
    /// </para>
    /// </remarks>
    /// <param name="request">The dock to show, and what to open it with.</param>
    void Open(DockRequest request);

    /// <summary>
    /// Shows a dock where it is, making it if it is not there.
    /// </summary>
    /// <remarks>
    /// What a plugin reaches for when what it has to show is a directory or an address: the dock is opened
    /// in the main window at where it was asked for, rather than in a window of its own. Naming the dock is
    /// the plugin's own business, and the host only answers whether a registration holds that name.
    /// </remarks>
    /// <param name="dockNameId">The stable name id of the dock to show.</param>
    /// <param name="startAt">What the dock is to look at, or nothing.</param>
    void Show(string dockNameId, string? startAt);

    /// <summary>Opens the main window, or brings it forward where it is already up.</summary>
    /// <remarks>
    /// The main window is one of the host's windows like any other — it can be closed and opened again —
    /// which is why this is a question rather than a control: what it answers is that there is a main
    /// window now, not that one was made.
    /// </remarks>
    void OpenMain();
}

/// <summary>
/// Which place of a dock's own chrome a menu item belongs to.
/// </summary>
public enum DockMenuTarget
{
    /// <summary>Right-click on a dock's tab in a region's strip.</summary>
    Tab,
}

/// <summary>One item added to a dock's own context menu.</summary>
/// <param name="DockNameId">
/// The stable name id of the dock the item is offered for, or nothing for every dock: an action about
/// browsing belongs to a browser, and one about a tree belongs to a tree.
/// </param>
/// <param name="Item">
/// The item itself. Its <c>Command</c> is handed the one contextual shape the program has —
/// <see cref="ContextTarget"/> — which for a tab states the dock's stable name id as the directory and
/// carries no entries: a tab is a dock and not a file, and the shape is shared so that an item is written
/// once for both.
/// </param>
public sealed record DockMenuItem(string? DockNameId, ContextMenuItem Item);

/// <summary>Where a plugin adds items to the menus a dock's own chrome opens.</summary>
public interface IDockMenuRegistry
{
    /// <summary>
    /// Adds an item to one of a dock's own menus.
    /// </summary>
    /// <remarks>
    /// Items from different plugins are ordered by plugin load order and then by the item's own order
    /// value, as the file menus are.
    /// </remarks>
    /// <param name="target">The place the item belongs to.</param>
    /// <param name="item">The item to add.</param>
    void Add(DockMenuTarget target, DockMenuItem item);

    /// <summary>
    /// Every item offered for one place and one dock, in the order they are shown.
    /// </summary>
    /// <param name="target">The place.</param>
    /// <param name="dockNameId">The dock the menu belongs to.</param>
    /// <returns>The items, or nothing when that place has none for that dock.</returns>
    IReadOnlyList<ContextMenuItem> Items(DockMenuTarget target, string dockNameId);
}

/// <summary>
/// A dock's view, once the host has opened it and knows what it asked for.
/// </summary>
/// <remarks>
/// Kept apart from <see cref="IDockView"/> because this is what a dock may be told rather than what it
/// must be: a dock with nothing to do about where it was opened does not implement it, and one that is
/// opened at a directory of the caller's choosing does.
/// </remarks>
public interface IDockOpened
{
    /// <summary>
    /// Takes what the dock was opened with.
    /// </summary>
    /// <remarks>
    /// Called after the view is made and after <see cref="IDockView.Restored"/>, so that a dock opened in a
    /// window of its own is drawn at the directory it was asked for rather than starting where it last was
    /// and being moved under the first frame.
    /// </remarks>
    /// <param name="request">What it was opened with.</param>
    void Opened(DockRequest request);
}

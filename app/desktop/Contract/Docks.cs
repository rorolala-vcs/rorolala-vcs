using Avalonia.Controls;

namespace RorolalaDesktop.Contract;

/// <summary>Whether a dock is a toggle or a factory of instances.</summary>
public enum DockOpenMode
{
    /// <summary>
    /// At most one instance exists. Activating the dock shows or hides it.
    /// </summary>
    Toggle,

    /// <summary>
    /// Every activation creates a new instance. This is what "copyable" means.
    /// </summary>
    New,
}

/// <summary>Where a dock is asked to go when it is opened or dropped.</summary>
/// <remarks>
/// No longer a place a dock sits — a dock's place is its group in the layout tree, which the host keeps and
/// the contract never names. This is the intent of a request: <see cref="Center"/> means "into the tabs of
/// the target group", and an edge means "split the target group on that side". Which group is the target is
/// the host's answer, so a plugin says what it wants done and not where the result ends up.
/// </remarks>
public enum DockSide
{
    /// <summary>Split the target group, the new one above it.</summary>
    Top,

    /// <summary>Split the target group, the new one to its left.</summary>
    Left,

    /// <summary>Split the target group, the new one to its right.</summary>
    Right,

    /// <summary>Split the target group, the new one below it.</summary>
    Bottom,

    /// <summary>Put the dock among the target group's tabs.</summary>
    Center,
}

/// <summary>
/// What a dock remembers about itself between runs.
/// </summary>
/// <remarks>
/// A dock's own state — which layout it is in, what it has open — belongs to the plugin that made the
/// dock and is kept by the host: it is written into the dock layout beside the dock's placement, so a
/// dock comes back where it was and as it was.
/// <para>
/// What is kept is text, because the layout is a file a person may read and edit; a plugin with a
/// structure to keep writes it down as text and reads it back.
/// </para>
/// </remarks>
public interface IDockState
{
    /// <summary>Reads what was kept under a key.</summary>
    /// <param name="key">The key it was kept under.</param>
    /// <returns>What was kept, or nothing when nothing was.</returns>
    string? Read(string key);

    /// <summary>Keeps a value under a key, to be read back when the dock is made again.</summary>
    /// <param name="key">The key to keep it under.</param>
    /// <param name="value">What to keep.</param>
    void Write(string key, string value);

    /// <summary>
    /// Closes the dock, as its own close button does.
    /// </summary>
    /// <remarks>
    /// What a dock that is over reaches for: a process that ended, a connection that dropped, a job with
    /// nothing left to show. A window of its own is the host's to close with it, since what it holds is the
    /// dock and nothing else.
    /// </remarks>
    void Close();
}

/// <summary>
/// One thing a dock's header carries.
/// </summary>
/// <remarks>
/// A dock's header is the strip a person reaches for what the dock is showing rather than for what it shows:
/// a command is drawn there as a button by the host, and a strip a plugin draws itself is put there as it is.
/// The two are one list so that their order is the plugin's — what stands beside what is the plugin's answer,
/// and a plugin that wanted its own thing between two commands could not say so with two lists.
/// <para>
/// A strip is a control of the plugin's rather than a run of commands because what a plugin wants there is
/// sometimes not a button: a group of buttons that act as one, a button whose picture changes with a key the
/// user is holding, a dropdown of choices. The host places it and asks nothing about what is inside it, which
/// is the same trade the view itself makes.
/// </para>
/// <para>
/// A header drawn as a menu — a dock in a window of its own, and a tab's own menu — carries the commands and
/// not the strips: a menu entry is a word and an action, and a strip is neither.
/// </para>
/// </remarks>
public abstract record DockHeaderItem;

/// <summary>
/// A command shown in a dock's header, drawn as a button by the host.
/// </summary>
/// <param name="LabelKey">An i18n key for the command's label.</param>
/// <param name="Command">What the command does.</param>
/// <param name="IconKey">
/// An optional key naming the command's icon. A command that names one is drawn as the mark alone in the
/// dock's strip, because that is what a strip is for, and its label is what the mark says when the pointer
/// rests on it.
/// </param>
public sealed record DockHeaderCommand(string LabelKey, Action Command, string? IconKey = null)
    : DockHeaderItem;

/// <summary>
/// A strip a plugin draws itself, put in a dock's header where a command would be.
/// </summary>
/// <remarks>
/// The control is the plugin's own and the host neither reads nor rearranges it, so what a plugin needs of a
/// header — a group, a modifier, a dropdown, a bar — it draws. Two things follow from that and are the
/// plugin's to keep: the control is inserted into the host's tree, so it must come from the host's Avalonia
/// copy like the view, and it may be taken out and put back when the header is rebuilt, so it holds no state
/// that a parent's removing it would lose.
/// </remarks>
/// <param name="View">What to stand in the header.</param>
/// <param name="Command">
/// What a header drawn as a menu carries in its place, or nothing for a strip that has no one thing to lead
/// with. A menu entry is a word and an action and a strip is neither, so a host with menus — a dock in a window
/// of its own — would otherwise lose everything the strip holds: what it is given here is the one command the
/// strip's own first button is, so that the reader still reaches it there.
/// </param>
public sealed record DockHeaderView(Control View, DockHeaderCommand? Command = null)
    : DockHeaderItem;

/// <summary>
/// One dock's view, as the host holds it.
/// </summary>
/// <remarks>
/// A plugin may build the view out of Avalonia controls directly, which is the deliberate trade
/// that keeps the contract small: the host inserts <see cref="View"/> into its own visual tree, so
/// the control must come from the host's Avalonia copy.
/// </remarks>
public interface IDockView
{
    /// <summary>The control the dock shows.</summary>
    Control View { get; }

    /// <summary>What the dock's header carries, in the order it is drawn.</summary>
    IReadOnlyList<DockHeaderItem> Header { get; }

    /// <summary>
    /// Takes what the dock remembered about itself, and does nothing with it by default.
    /// </summary>
    /// <remarks>
    /// A view is made before it is told what it was, so that a dock with nothing to remember — every
    /// dock the first time it is opened — is made the same way as one that has. A view that keeps
    /// nothing about itself therefore does not override this, and one that does reads its own keys.
    /// </remarks>
    /// <param name="state">What the dock kept, and where the next of it is kept.</param>
    void Restored(IDockState state) { }
}

/// <summary>
/// What a plugin says about a dock it registers.
/// </summary>
/// <param name="Owner">The registering plugin, or the kernel for a core dock.</param>
/// <param name="DockNameId">
/// A globally unique stable id, by convention <c>&lt;plugin-id&gt;.&lt;dock&gt;</c>. It is what layout
/// persistence and the <c>Window</c> menu address the dock by, so it must not change once shipped.
/// </param>
/// <param name="DisplayNameKey">An i18n key for the dock's title.</param>
/// <param name="OpenMode">Whether the dock toggles or is created anew each time.</param>
/// <param name="DefaultSide">
/// How the dock is put into the active group when no drop says otherwise: among its tabs for
/// <see cref="DockSide.Center"/>, or beside it for an edge.
/// </param>
/// <param name="Create">
/// The factory making a view. It is told nothing about where the dock will be, because where it ends up is
/// the layout tree's answer and is not known when the view is made: a dock moved by hand is the same dock.
/// </param>
public sealed record DockRegistration(
    PluginId Owner,
    string DockNameId,
    string DisplayNameKey,
    DockOpenMode OpenMode,
    DockSide DefaultSide,
    Func<IDockView> Create
);

/// <summary>Where a plugin registers docks.</summary>
public interface IDockRegistry
{
    /// <summary>
    /// Registers a dock.
    /// </summary>
    /// <param name="registration">What the plugin says about the dock.</param>
    void Register(DockRegistration registration);
}

/// <summary>
/// One dock, named the way the program names it: the stable name it was registered under and which of its
/// instances it is.
/// </summary>
/// <remarks>
/// The two are needed together because a dock may be made again: a browser is one dock looked at in several
/// windows, and which of them is being worked in is part of the answer. The ordinal is the one the layout
/// writes beside the name, so a plugin that keeps a dock's identity has a value it can compare rather than a
/// control it would have to hold.
/// </remarks>
/// <param name="DockNameId">The dock's stable name id, as it was registered.</param>
/// <param name="Ordinal">Which instance of that name.</param>
public sealed record FocusedDock(string DockNameId, int Ordinal);

/// <summary>
/// Which dock the user is working in, as a plugin reads it.
/// </summary>
/// <remarks>
/// One dock at a time for the whole program, and it is what the host draws its mark around: the dock a press
/// landed in, the dock the keyboard went into, the tab that was chosen, or the dock of the window that came
/// forward. A dock that is closed passes it on to the one that takes its place, and a dock that is moved into
/// another group or window keeps it, so what is named is a dock that is actually open.
/// <para>
/// Nothing is named before the user has worked in a dock, and nothing again once the last dock that had it is
/// gone: an answer here is always something the user can act from, and there is nothing to act from in a
/// window that holds no dock.
/// </para>
/// </remarks>
public interface IDockFocus
{
    /// <summary>The dock the user is working in, or nothing before one has been worked in.</summary>
    FocusedDock? Current { get; }

    /// <summary>Raised when <see cref="Current"/> says something else.</summary>
    event Action? Changed;
}

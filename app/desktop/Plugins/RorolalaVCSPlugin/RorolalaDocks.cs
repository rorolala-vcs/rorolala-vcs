using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The three docks this plugin shows: what the work is looked at through.
/// </summary>
/// <remarks>
/// The Overview is the view of the work that is there — how it stands beside the Layout it works from,
/// which is the reading `rola status` makes (see <see cref="OverviewDock"/>). The other two are empty
/// for now, and what they carry while they are is the one thing that is already known about them:
/// where the File System plugin is rooted. What Rorolala works on is a Workspace, and a base directory
/// that no Workspace holds has nothing under it for any of them to look at, which is worth saying
/// rather than leaving a blank view of the work to be read as a work with nothing in it.
/// <para>
/// The base is read through <see cref="BaseDirectory"/>, the File System plugin's published answer,
/// because that plugin is the one that owns where the browser works: a dock of this one that went
/// looking for it itself would be a second answer to the same question, kept in step by hand.
/// </para>
/// </remarks>
internal static class RorolalaDocks
{
    /// <summary>
    /// The stable name of the Overview dock.
    /// </summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string OverviewNameId = "rorolala.vcs.overview";

    /// <summary>The stable name of the Workspace dock.</summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string WorkspaceNameId = "rorolala.vcs.workspace";

    /// <summary>The stable name of the Version Graph dock.</summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string VersionGraphNameId = "rorolala.vcs.version_graph";

    /// <summary>What the Window menu lists the Overview dock as.</summary>
    private const string OverviewKey = "rorolala_vcs.dock.overview";

    /// <summary>What the Window menu lists the Workspace dock as.</summary>
    private const string WorkspaceKey = "rorolala_vcs.dock.workspace";

    /// <summary>What the Window menu lists the Version Graph dock as.</summary>
    private const string VersionGraphKey = "rorolala_vcs.dock.version_graph";

    /// <summary>
    /// Registers the three, each under its own title.
    /// </summary>
    /// <remarks>
    /// Two of the three are still a line saying that the base is not a Workspace, since what each will
    /// hold is not written yet; the Overview is the view of the work that is there.
    /// </remarks>
    /// <param name="host">The host, whose dock registry they are registered with.</param>
    /// <param name="owner">This plugin, which is what the registrations are attributed to.</param>
    public static void Declare(IPluginHost host, PluginId owner)
    {
        ArgumentNullException.ThrowIfNull(host);

        Add(host, owner, OverviewNameId, OverviewKey, _ => new OverviewDock(host, host.Log, host.Files, host.Refocus));
        Add(host, owner, WorkspaceNameId, WorkspaceKey, _ => new RorolalaDock(host.Log));
        Add(host, owner, VersionGraphNameId, VersionGraphKey, _ => new RorolalaDock(host.Log));
    }

    /// <summary>
    /// Registers one of the three.
    /// </summary>
    /// <remarks>
    /// A toggle, because what a view of the work is, is one view of it: asking for one that is already
    /// open brings that one forward rather than making a second beside it, and the same answer is what
    /// lets one be moved into a window of its own and found again from the <c>Window</c> menu.
    /// </remarks>
    /// <param name="host">The host, whose dock registry it is registered with.</param>
    /// <param name="owner">This plugin, which is what the registration is attributed to.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <param name="labelKey">An i18n key for the dock's title.</param>
    /// <param name="make">What the dock shows, made once per request for it.</param>
    private static void Add(
        IPluginHost host,
        PluginId owner,
        string nameId,
        string labelKey,
        Func<DockPlacement, IDockView> make
    ) =>
        host.Docks.Register(
            new DockRegistration(owner, nameId, labelKey, DockOpenMode.Toggle, DockPlacement.Center, make)
        );
}

/// <summary>
/// The dock itself: the view, and nothing the header offers.
/// </summary>
internal sealed class RorolalaDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly WorkspaceNotice _view;

    /// <summary>Makes one of the two views that are still a line about where the tree is rooted.</summary>
    /// <param name="log">Where a base that could not be read is said.</param>
    public RorolalaDock(ILog log) => _view = new WorkspaceNotice(log);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>
/// What one of the views that is not the Overview shows: nothing at all while the work is a Workspace,
/// and the one thing there is to say while it is not.
/// </summary>
/// <remarks>
/// The base is read where it changes rather than once, and only while this control is on screen: a
/// dock that was closed is not a view of anything, and a dock that is on screen has to say the same
/// thing after the user roots the tree elsewhere as it did before.
/// </remarks>
internal sealed class WorkspaceNotice : UserControl
{
    /// <summary>
    /// How wide the line is allowed to read.
    /// </summary>
    /// <remarks>
    /// A view of the work is as wide as the user made its window, and a sentence left to run the whole
    /// width of a wide one is harder to read than the same sentence wrapped: this is where it wraps.
    /// </remarks>
    private const double Widest = 420;

    /// <summary>The line shown while the base is not a Workspace.</summary>
    private readonly TextBlock _words;

    /// <summary>Where a base that could not be read is said.</summary>
    private readonly ILog _log;

    /// <summary>Makes the control.</summary>
    /// <param name="log">Where a base that could not be read is said.</param>
    public WorkspaceNotice(ILog log)
    {
        _log = log;

        _words = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            TextAlignment = TextAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            MaxWidth = Widest,
            Classes = { "muted" },
        };

        // The line is the dock's only content and stays so: while a Workspace holds the base there
        // is nothing to say, and what is said then is said by the line not being drawn.
        Content = _words;

        // Listening while it is on screen rather than for as long as it exists: the catalogue outlives
        // every dock, and a dock that was closed has no reason to be told where the work moved to.
        AttachedToVisualTree += (_, _) => { BaseDirectory.Changed += Draw; Draw(); };
        DetachedFromVisualTree += (_, _) => BaseDirectory.Changed -= Draw;

        Draw();
    }

    /// <summary>
    /// What one of these docks has to say about a directory the browser is rooted at.
    /// </summary>
    /// <remarks>
    /// A base that has not been said yet is answered as nothing holding it, which a run cannot reach:
    /// the docks are registered by a plugin the host starts after the File System plugin, and that one
    /// publishes its base as it is made. Answering it as a Workspace would be the one claim this
    /// cannot make, since nothing has been found.
    /// <para>
    /// A read that failed is answered the same way and said in the log, so that a run without the
    /// native library beside it can be told from one working outside every Workspace: the two read the
    /// same from the window, and only one of them is a reason to go looking.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory the tree is rooted at.</param>
    /// <param name="log">Where a read that failed is said.</param>
    /// <returns>The words to show, or nothing while there is nothing to say.</returns>
    public static string? Notice(string? directory, ILog log)
    {
        ArgumentNullException.ThrowIfNull(log);

        if (Holds(directory, log))
        {
            return null;
        }

        return RolaI18N.Get("rorolala_vcs.not_a_workspace");
    }

    /// <summary>Says what there is to say about where the tree is rooted.</summary>
    private void Draw()
    {
        _words.Text = Notice(BaseDirectory.Current, _log);
        _words.IsVisible = _words.Text is { Length: > 0 };
    }

    /// <summary>Whether a Rorolala Workspace holds a directory.</summary>
    /// <remarks>
    /// The judgement is RolaSharp's and not this plugin's: whether a directory is inside a Workspace is
    /// a question about the C ABI's own notion of one, and asking it here would be a second answer to
    /// keep in step with the first (see <see cref="RolaWorkspace.IsWorkspace"/>). What is this plugin's
    /// is what to do when the question cannot be asked at all.
    /// <para>
    /// It is asked by the Overview as well, which is why it is not private to this view: a reading that
    /// comes back with nothing is a different thing depending on whether a Workspace holds the base.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory to ask about.</param>
    /// <param name="log">Where a read that failed is said.</param>
    /// <returns>Whether the directory is inside a Workspace.</returns>
    internal static bool Holds(string? directory, ILog log)
    {
        // Nothing is not a path, and the computer is not a directory: neither is somewhere a Workspace
        // can be found from, so neither is read.
        if (directory is not { Length: > 0 } at)
        {
            return false;
        }

        try
        {
            return RolaWorkspace.IsWorkspace(at);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            log.Warn($"whether a Rorolala Workspace holds `{at}` could not be read: {error.Message}");

            return false;
        }
    }
}

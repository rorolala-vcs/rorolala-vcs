using Avalonia.Controls;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The docks this plugin shows: what the work is looked at through, and what the accounts are.
/// </summary>
/// <remarks>
/// The Overview is the view of the work that is there, how it stands beside the Layout it works from,
/// which is the reading `rola status` makes (see <see cref="OverviewDock"/>). The Version Graph is the
/// work's variants: a card per variant, a lane per file and a column per version depth, laid out from
/// the index's own reading of them (see <see cref="VersionGraphDock"/>). The Tree dock is one tree's
/// staging area and the description of its next version (see <see cref="TreePanelDock"/>), and it is
/// the only work it does: recording the tree is the Version Graph's, and how the work stands is the
/// Overview's. The Workspace dock is the work as a project: the ignore rules it is managed by and the
/// operations it last recorded (see <see cref="WorkspaceView"/>). The Accounts
/// dock is the machine's own key pairs, which account the work acts as, and what may be done about
/// them (see <see cref="AccountsDock"/>), which is not about one work but about the person, and so is
/// a dock of its own rather than a part of the Overview.
/// <para>
/// The base is read through <see cref="FileSystemPlugin.BaseDirectory"/>, the File System plugin's published answer,
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

    /// <summary>The stable name of the Tree dock.</summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string TreeNameId = "rorolala.vcs.tree";

    /// <summary>The stable name of the Accounts dock.</summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string AccountsNameId = "rorolala.vcs.accounts";

    /// <summary>What the Window menu lists the Overview dock as.</summary>
    private const string OverviewKey = "rorolala_vcs.dock.overview";

    /// <summary>What the Window menu lists the Workspace dock as.</summary>
    private const string WorkspaceKey = "rorolala_vcs.dock.workspace";

    /// <summary>What the Window menu lists the Version Graph dock as.</summary>
    private const string VersionGraphKey = "rorolala_vcs.dock.version_graph";

    /// <summary>What the Window menu lists the Tree dock as.</summary>
    private const string TreeKey = "rorolala_vcs.dock.tree";

    /// <summary>What the Window menu lists the Accounts dock as.</summary>
    private const string AccountsKey = "rorolala_vcs.dock.accounts";

    /// <summary>
    /// Registers them, each under its own title.
    /// </summary>
    /// <remarks>
    /// The Workspace reads the rules and the log of the Workspace holding the base, and says so while
    /// the base is not one; the Overview is the view of the work that is there, the Version Graph is
    /// one file's chain, and the Accounts is the machine's own key pairs.
    /// </remarks>
    /// <param name="host">The host, whose dock registry they are registered with.</param>
    /// <param name="owner">This plugin, which is what the registrations are attributed to.</param>
    /// <param name="hub">What every command this plugin runs reports itself to, which the Overview draws.</param>
    /// <param name="statuses">What the work says about a file, which the graph reads an edit from.</param>
    /// <param name="trees">What each tree stands as, which the Tree dock reads its staging from.</param>
    /// <param name="sync">What a sync would carry, read by both views of the work.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout, asked for by both views.</param>
    public static void Declare(
        IPluginHost host,
        PluginId owner,
        IProgressHub hub,
        StatusReadings statuses,
        TreeReadings trees,
        SyncWatcher sync,
        VaultFetch fetch
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(statuses);
        ArgumentNullException.ThrowIfNull(trees);
        ArgumentNullException.ThrowIfNull(sync);
        ArgumentNullException.ThrowIfNull(fetch);

        Add(
            host,
            owner,
            OverviewNameId,
            OverviewKey,
            Icons.ListChecks,
            () => new OverviewDock(host, host.Log, host.Files, hub, sync, fetch)
        );
        Add(host, owner, WorkspaceNameId, WorkspaceKey, Icons.Briefcase, () => new RorolalaDock(host));
        Add(
            host,
            owner,
            VersionGraphNameId,
            VersionGraphKey,
            Icons.ArrowsSplit,
            () => new VersionGraphDock(host, statuses, sync, fetch, hub)
        );
        Add(
            host,
            owner,
            TreeNameId,
            TreeKey,
            Icons.Tree,
            () => new TreePanelDock(host, hub, trees)
        );
        Add(host, owner, AccountsNameId, AccountsKey, Icons.UserCircleGear, () => new AccountsDock(host));
    }

    /// <summary>
    /// Registers one of them.
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
    /// <param name="iconKey">The picture the dock is drawn by, in the Window menu and at the left of its tab.</param>
    /// <param name="make">What the dock shows, made once per request for it.</param>
    private static void Add(
        IPluginHost host,
        PluginId owner,
        string nameId,
        string labelKey,
        string iconKey,
        Func<IDockView> make
    ) =>
        host.Docks.Register(
            new DockRegistration(owner, nameId, labelKey, DockOpenMode.Toggle, DockSide.Center, make, iconKey)
        );
}

/// <summary>
/// The Workspace dock: the rules the work is managed by and the operations it last recorded.
/// </summary>
internal sealed class RorolalaDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly WorkspaceView _view;

    /// <summary>Makes the Workspace view.</summary>
    /// <param name="host">The host, which is what the view reads and reports through.</param>
    public RorolalaDock(IPluginHost host) => _view = new WorkspaceView(host, host.Log);

    /// <inheritdoc />
    public Control View => _view;

    /// <summary>
    /// Reading the rules and the log again, which is the one thing a reader asks of this view as a
    /// whole: the two panes have answers of their own, and what this offers is both of them at once.
    /// </summary>
    public IReadOnlyList<DockHeaderItem> Header =>
        [
            new DockHeaderCommand(
                "rorolala_vcs.workspace.refresh",
                _view.Refresh,
                IconKey: Icons.ArrowClockwise
            ),
        ];
}

/// <summary>
/// The accounts dock: the accounts on this machine, and what may be done about them.
/// </summary>
/// <remarks>
/// A view of the person rather than of one work: which account the work acts as is Rorolala's own file for the
/// user rather than any one Workspace's, so it is a dock of its own rather than a part of the Overview, which is
/// about one tree. Everything about it is read through the command line, which is where the accounts are.
/// </remarks>
internal sealed class AccountsDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly AccountsPanel _view;

    /// <summary>Makes the dock.</summary>
    /// <param name="host">The host, which is what a row's buttons reach to run and to report.</param>
    public AccountsDock(IPluginHost host) => _view = new AccountsPanel(host, host.Log);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header =>
        [
            new DockHeaderCommand("rorolala_vcs.accounts.refresh", _view.Refresh, IconKey: Icons.ArrowClockwise),
        ];
}

/// <summary>
/// What a view of the work has to say about a directory the browser is rooted at.
/// </summary>
/// <remarks>
/// Nothing at all while a Workspace holds the base, and the one thing there is to say while none
/// does. The judgement is shared rather than made by each dock, since every view of the work is read
/// against the same base and a second answer would be a second thing to keep in step.
/// </remarks>
internal static class WorkspaceNotice
{
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

    /// <summary>Whether a Rorolala Workspace holds a directory.</summary>
    /// <remarks>
    /// The judgement is RolaSharp's and not this plugin's: whether a directory is inside a Workspace is
    /// a question about the C ABI's own notion of one, and asking it here would be a second answer to
    /// keep in step with the first (see <see cref="WorkspaceScope.Open(string)"/> and
    /// <see cref="Scope.Read()"/>). What is this plugin's
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
            using var workspace = WorkspaceScope.Open(at);
            workspace.Read();

            return true;
        }
        catch (RolaReadError error)
            when (error.Kind is RolaReadErrorKind.Unavailable or RolaReadErrorKind.NoWorkspace)
        {
            // No Workspace holding the directory is the absence the V1 call answered with false
            // rather than a failure, and nothing is said about it here for the same reason.
            return false;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            log.Warn($"whether a Rorolala Workspace holds `{at}` could not be read: {error.Message}");

            return false;
        }
    }
}

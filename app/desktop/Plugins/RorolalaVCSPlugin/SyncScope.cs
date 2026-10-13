namespace RorolalaVCSPlugin;

/// <summary>
/// Which way a sync carries work.
/// </summary>
/// <remarks>
/// The three the command line offers, in the order the buttons are read: up carries this side's work to the
/// Vault, down brings the Vault's here, and both carries whichever of the two there is to carry.
/// </remarks>
internal enum SyncWay
{
    /// <summary>This side's work goes to the Vault: <c>--up-only</c>.</summary>
    Up,

    /// <summary>The Vault's work comes here: <c>--down-only</c>.</summary>
    Down,

    /// <summary>Whichever of the two there is to carry: <c>--both</c>.</summary>
    Both,
}

/// <summary>
/// What a sync is told to carry, and what it is told to leave alone.
/// </summary>
/// <remarks>
/// The three parts of a sync are separate on purpose — the command line's own three flags — because a side may
/// be behind on one and not on another, and because what is left out changes what the run means rather than
/// only how much it does. What a scope leaves out is both the flags a run is given and what warns about them.
/// </remarks>
/// <param name="Index">Whether the index is carried.</param>
/// <param name="Layout">Whether the Layout is carried.</param>
/// <param name="Storage">Whether the storage is carried.</param>
internal readonly record struct SyncScope(bool Index, bool Layout, bool Storage)
{
    /// <summary>Everything is carried, which is what a sync that leaves nothing out is.</summary>
    public static SyncScope All => new(true, true, true);

    /// <summary>Whether everything is carried, which is the one scope with nothing to warn about.</summary>
    public bool Everything => Index && Layout && Storage;

    /// <summary>What the command line is told to leave alone, in the order the settings are read in.</summary>
    /// <returns>The flags a run is given, which is nothing at all when everything is carried.</returns>
    public IReadOnlyList<string> Flags
    {
        get
        {
            var flags = new List<string>(3);

            if (!Index)
            {
                flags.Add("--no-index");
            }

            if (!Layout)
            {
                flags.Add("--no-layout");
            }

            if (!Storage)
            {
                flags.Add("--no-storage");
            }

            return flags;
        }
    }
}

/// <summary>
/// Which of the three parts a scope leaves out, which is what a warning is chosen by.
/// </summary>
/// <remarks>
/// One value per way of leaving something out rather than the bits themselves, because the ways that leave two
/// out are not two warnings but their own: a run that carries only the index and a run that carries only the
/// storage are two things to say, and neither is what either of the single omissions says.
/// </remarks>
internal enum SyncLeft
{
    /// <summary>The index alone is left out.</summary>
    Index,

    /// <summary>The storage alone is left out.</summary>
    Storage,

    /// <summary>The Layout alone is left out.</summary>
    Layout,

    /// <summary>Only the Layout is carried: the index and the storage are left out.</summary>
    LayoutOnly,

    /// <summary>Only the index is carried: the Layout and the storage are left out.</summary>
    IndexOnly,

    /// <summary>Only the storage is carried: the index and the Layout are left out.</summary>
    StorageOnly,

    /// <summary>All three are left out, so the run does nothing at all.</summary>
    Nothing,
}

/// <summary>
/// What is said before a sync that leaves something out, which is the user's own table of what it costs.
/// </summary>
/// <remarks>
/// A sync that leaves a part behind still writes what it does carry, and the part it left can be the one that
/// made the write safe: the index is what a later reader finds a version by, the storage is what its content
/// is in, and the Layout is what says which version either belongs to. A run that leaves one out is therefore
/// not a smaller run but a different one, and the user is told which before it starts.
/// <para>
/// What is said depends on what is left out and which way the run goes, because the two together decide what
/// becomes inconsistent: leaving the storage out of an upload leaves the Vault recording a version whose
/// content it does not hold, and leaving it out of a download leaves this side with an entry whose content is
/// not here. The ways that differ most are the two single omissions; the ways that leave the Layout out say
/// one thing whichever way the run goes, since nothing about the Vault's Layout moves either way.
/// </para>
/// </remarks>
internal static class SyncWarnings
{
    /// <summary>Which of the three a scope leaves out.</summary>
    /// <param name="scope">The scope.</param>
    /// <returns>What is left out.</returns>
    /// <exception cref="ArgumentException">The scope leaves nothing out, so there is no answer to give.</exception>
    public static SyncLeft Left(SyncScope scope) =>
        (scope.Index, scope.Layout, scope.Storage) switch
        {
            (false, true, true) => SyncLeft.Index,
            (true, true, false) => SyncLeft.Storage,
            (true, false, true) => SyncLeft.Layout,

            // What is carried rather than what is left: only the Layout is a scope with the index and the
            // storage off, and the two others are read the same way round.
            (false, true, false) => SyncLeft.LayoutOnly,
            (true, false, false) => SyncLeft.IndexOnly,
            (false, false, true) => SyncLeft.StorageOnly,
            (true, true, true) => throw new ArgumentException(
                "everything is carried, so nothing is left out",
                nameof(scope)
            ),
            _ => SyncLeft.Nothing,
        };

    /// <summary>
    /// The i18n key of the warning a run is to be confirmed with, or nothing when there is none to give.
    /// </summary>
    /// <remarks>
    /// The keys are named by literals here rather than built from the scope, so that the gate which checks the
    /// catalogue can see every key this program can say: a key assembled out of parts is a key that can go
    /// missing without anything noticing.
    /// </remarks>
    /// <param name="scope">What the run is to carry.</param>
    /// <param name="way">Which way it is to carry it.</param>
    /// <returns>The key, or nothing when everything is carried.</returns>
    public static string? Key(SyncScope scope, SyncWay way)
    {
        if (scope.Everything)
        {
            return null;
        }

        return Left(scope) switch
        {
            SyncLeft.Index => way switch
            {
                SyncWay.Up => "rorolala_vcs.sync.warn_no_index_up",
                SyncWay.Down => "rorolala_vcs.sync.warn_no_index_down",
                _ => "rorolala_vcs.sync.warn_no_index_both",
            },
            SyncLeft.Storage => way switch
            {
                SyncWay.Up => "rorolala_vcs.sync.warn_no_storage_up",
                SyncWay.Down => "rorolala_vcs.sync.warn_no_storage_down",
                _ => "rorolala_vcs.sync.warn_no_storage_both",
            },
            SyncLeft.Layout => "rorolala_vcs.sync.warn_no_layout",
            SyncLeft.LayoutOnly => way switch
            {
                SyncWay.Up => "rorolala_vcs.sync.warn_layout_only_up",
                SyncWay.Down => "rorolala_vcs.sync.warn_layout_only_down",
                _ => "rorolala_vcs.sync.warn_layout_only_both",
            },
            SyncLeft.IndexOnly => "rorolala_vcs.sync.warn_index_only",
            SyncLeft.StorageOnly => "rorolala_vcs.sync.warn_storage_only",
            _ => "rorolala_vcs.sync.warn_nothing",
        };
    }
}

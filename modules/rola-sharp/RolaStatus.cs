using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>One path that moved.</summary>
/// <param name="From">The path the Layout names it by.</param>
/// <param name="To">The path the tree holds it at.</param>
/// <param name="Modified">
/// Whether it was edited as well as moved. A move whose destination no longer holds what the Layout
/// agreed with at the old path is a change as well, and one that is recorded is recorded as both.
/// </param>
/// <param name="Strong">
/// Whether the move is one the two being the same bytes makes rather than one only their likeness
/// suggests. The first is a fact to be confirmed and the second a guess to be confirmed or taken
/// back, so a caller that acts on one of them says which it acts on.
/// </param>
public sealed record Rename(string From, string To, bool Modified, bool Strong);

/// <summary>One variant waiting to be joined into one file.</summary>
/// <param name="Target">The file it is to be joined into.</param>
/// <param name="Variant">The variant that was checked in, as the hex it is named by.</param>
/// <param name="Path">The path its file lies at, or lay at before a move.</param>
/// <param name="Moved">
/// Where the tree holds its file now, when a move was found. A variant file that is not where the
/// record puts it is found again by what it holds, since what it holds is what was checked in.
/// </param>
public sealed record Merge(string Target, string Variant, string Path, string? Moved);

/// <summary>Why a merge in progress cannot go on.</summary>
/// <remarks>
/// The reasons are told apart because what is to be done about them differs: a variant file that
/// moved is confirmed, one that is gone is looked for, and a file that changed underneath is not
/// the merge that was checked in.
/// </remarks>
public enum BrokenKind
{
    /// <summary>Nothing was read: there is no such merge.</summary>
    None,

    /// <summary>The file the variant is for is no longer named by the Layout.</summary>
    NoTarget,

    /// <summary>Nowhere here can hold its variant file.</summary>
    NoPlace,

    /// <summary>The variant file is gone from where the record says it lies.</summary>
    FileGone,

    /// <summary>This reading has no index to read the variant from.</summary>
    NoIndex,

    /// <summary>The index does not hold that variant.</summary>
    NoVariant,

    /// <summary>The variant file could not be read.</summary>
    Unreadable,

    /// <summary>The variant file no longer holds the variant's content.</summary>
    Changed,
}

/// <summary>What one path of the work is, as a reading says it in a word.</summary>
/// <remarks>
/// A reading reports lists of paths; a caller looking at one path — a file in a directory, a row of a table
/// — has one question about it rather than six, and this is that question answered. Where a path is more
/// than one of these at once, what is answered is the state a reader acts on first, so the answer is the one
/// worth acting on rather than a summary of them all.
/// </remarks>
public enum PathState
{
    /// <summary>The reading says nothing about the path.</summary>
    Unknown,

    /// <summary>The Layout names it and the tree does not hold it, which a path a file moved away from is.</summary>
    Lost,

    /// <summary>The tree holds it and the Layout does not name it.</summary>
    Untagged,

    /// <summary>Its content is not what the Layout agreed with, and it is this account's to record.</summary>
    Modified,

    /// <summary>
    /// Its content is not what the Layout agreed with, and it is not this account's to record: another
    /// account holds it, or the version it is based on is one upstream has moved past.
    /// </summary>
    Wrong,

    /// <summary>It is where a file the Layout names at another path now sits.</summary>
    Renamed,

    /// <summary>The same, where the move is one their likeness suggests rather than one their bytes make.</summary>
    RenamedLikely,

    /// <summary>A variant is waiting to be joined into it.</summary>
    Merging,
}

/// <summary>A merge that cannot go on, and what the reason is about.</summary>
/// <param name="Kind">Which reason it is.</param>
/// <param name="What">
/// The variant, as the hex it is named by, or the path the variant file lies at — whichever the
/// reason is about. What the reason is *said* as is the caller's: the words belong to the program
/// that shows them, and this side has no language.
/// </param>
/// <param name="Target">
/// The file the variant is for, or an empty string when the Layout names none any more — which is one
/// of the reasons a merge cannot go on, and the only one where there is nothing to name.
/// </param>
public sealed record BrokenReason(BrokenKind Kind, string What, string Target);

/// <summary>
/// How the version recorded for a path stands beside the one upstream records.
/// </summary>
/// <remarks>
/// It is <see cref="PathState"/>'s shape and not its subject: a path's <em>state</em> is what is wrong with
/// it, and its <em>standing</em> is how far it is from what the Vault has. A path may be in either without
/// the other — one that is in step and one that was never recorded are both nothing to act on — so the two
/// are asked separately.
/// </remarks>
public enum StandingRelation
{
    /// <summary>The two are the same version, so there is nothing between them and nothing to tell.</summary>
    Same,

    /// <summary>This side records work the Vault has not been told of.</summary>
    Ahead,

    /// <summary>The Vault has moved on, so what is here is based on a version that has.</summary>
    /// <remarks>
    /// This is also the answer when the version the Vault records is not in the index, which is the ordinary
    /// case: a fetch brings the Vault's Layout and never its versions. The direction is known from the version
    /// having come from elsewhere, and how far is not — see <see cref="Standing.Distance"/>.
    /// </remarks>
    Behind,

    /// <summary>Both sides have moved on, so neither is based on the other.</summary>
    Apart,

    /// <summary>
    /// How the two stand is not known here: the version this side records is not in the index, the reading was
    /// made without the Vault's copy, or the path is one this reading is not about at all.
    /// </summary>
    /// <remarks>
    /// A caller that has to tell those apart asks the distance beside it: zero is the two being the same
    /// version, and nothing at all is a standing that is not known.
    /// </remarks>
    Unknown,
}

/// <summary>How one path stands beside upstream, and how many versions apart the two are.</summary>
/// <param name="Relation">Where the two stand.</param>
/// <param name="Distance">
/// How many versions apart they are, or nothing when the chain does not say — which includes both a standing
/// that is <see cref="StandingRelation.Unknown"/> and one that is <see cref="StandingRelation.Apart"/>, where
/// the two have moved on from each other and neither is a distance from the other. It is also nothing for the
/// ordinary way of being behind: what the Vault is at is a version this side never fetched, so the direction
/// is known and the chain that would count the versions between is the Vault's own.
/// </param>
public sealed record Standing(StandingRelation Relation, ulong? Distance);

/// <summary>
/// How the working tree stands beside the Layout it works from.
/// </summary>
/// <remarks>
/// The same reading `rola status` makes with nothing named, and the same shape as
/// <see cref="RolaOwnership"/>: absence — no Workspace holds the directory, or the Workspace works
/// in no Layout — is <see langword="null"/> rather than a handle to remember to check, and the handle
/// is given back on <see cref="Dispose"/>.
/// <para>
/// It is a snapshot rather than a live view: a reading is what the tree held at one moment, so asking
/// it again cannot answer anything newer. What it holds is read out in full when it is made, since a
/// caller that wants the lists wants all of them; what one path is — a file of a listing, a row of a
/// table — is asked of the reading itself, which is the one question the native side answers better
/// than a copy of its lists can.
/// </para>
/// </remarks>
public sealed class RolaStatus : IDisposable
{
    /// <summary>The native reading, until it is given back.</summary>
    private nint _handle;

    /// <summary>Makes a reading from a native status, which is read out and kept.</summary>
    /// <param name="handle">The native status, which must not be null.</param>
    private RolaStatus(nint handle)
    {
        _handle = handle;

        List<string> lost;
        List<string> untagged;
        List<string> modified;
        List<string> unowned;
        List<string> wrongBase;
        List<Rename> renamed;
        List<Merge> merging;
        List<BrokenReason> broken;
        string root;

        try
        {
            root = NativeText.Taken(RorolalaBinding.rola_status_root(handle));
            lost = Paths(
                handle,
                RorolalaBinding.rola_status_lost_count,
                RorolalaBinding.rola_status_lost_at
            );
            untagged = Paths(
                handle,
                RorolalaBinding.rola_status_untagged_count,
                RorolalaBinding.rola_status_untagged_at
            );
            modified = Paths(
                handle,
                RorolalaBinding.rola_status_modified_count,
                RorolalaBinding.rola_status_modified_at
            );
            unowned = Paths(
                handle,
                RorolalaBinding.rola_status_unowned_count,
                RorolalaBinding.rola_status_unowned_at
            );
            wrongBase = Paths(
                handle,
                RorolalaBinding.rola_status_wrong_base_count,
                RorolalaBinding.rola_status_wrong_base_at
            );
            renamed = Renames(handle);
            merging = Merges(handle);
            broken = BrokenReasons(handle);
        }
        catch
        {
            // A reading that could not be taken out whole is not one to keep the native side of: what is
            // left would be a handle nobody can reach.
            RorolalaBinding.free_rola_status(handle);
            _handle = 0;

            throw;
        }

        Root = root;
        Lost = lost;
        Untagged = untagged;
        Modified = modified;
        Unowned = unowned;
        WrongBase = wrongBase;
        Renamed = renamed;
        Merging = merging;
        Broken = broken;
    }

    /// <summary>
    /// Where the Workspace this reading was made of is rooted, which the paths it reports are relative to.
    /// </summary>
    /// <remarks>
    /// A caller that has paths of its own — a listing, which names files the way the system does — joins
    /// them to this before <see cref="StateOf"/> can answer about one.
    /// </remarks>
    public string Root { get; }

    /// <summary>Paths the Layout names that the tree does not hold.</summary>
    public IReadOnlyList<string> Lost { get; }

    /// <summary>Paths the tree holds that the Layout does not name.</summary>
    public IReadOnlyList<string> Untagged { get; }

    /// <summary>
    /// Paths that changed where they were, including the destinations of moves that were edited as
    /// well.
    /// </summary>
    public IReadOnlyList<string> Modified { get; }

    /// <summary>
    /// Paths the fetched copy of the Vault's Layout says another account holds.
    /// </summary>
    /// <remarks>
    /// These are changes all the same, and they are held apart rather than counted among the ones
    /// above: recording one would make a version this side cannot push, so a caller that meant to
    /// act on the work must not act on these.
    /// </remarks>
    public IReadOnlyList<string> Unowned { get; }

    /// <summary>
    /// Paths whose content changed while the version recorded for them is not the one upstream is at.
    /// </summary>
    /// <remarks>
    /// What is recorded for one of these would be based on a version the other side has moved past —
    /// behind it, or apart from it — rather than on something both sides have. A version this side is
    /// <em>ahead</em> on is not one of them: holding work the other has not been told of is what working
    /// looks like. It is a subset of <see cref="Modified"/>, and a path is in it only where the index
    /// could say how the two versions stand.
    /// </remarks>
    public IReadOnlyList<string> WrongBase { get; }

    /// <summary>Paths that moved.</summary>
    public IReadOnlyList<Rename> Renamed { get; }

    /// <summary>Files a variant is waiting to be joined into, and the variant waiting.</summary>
    public IReadOnlyList<Merge> Merging { get; }

    /// <summary>Merges that cannot go on, one reason each.</summary>
    public IReadOnlyList<BrokenReason> Broken { get; }

    /// <summary>Whether the work stands as it was recorded, with nothing to confirm or record.</summary>
    public bool Clean =>
        Lost.Count == 0
        && Untagged.Count == 0
        && Modified.Count == 0
        && Unowned.Count == 0
        && Renamed.Count == 0
        && Merging.Count == 0
        && Broken.Count == 0;

    /// <summary>
    /// Reads how the work under a directory stands.
    /// </summary>
    /// <param name="directory">The directory to search upwards from.</param>
    /// <returns>The reading, or nothing when there is nothing to read.</returns>
    public static RolaStatus? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        // A path crosses as bytes, and the platform's own encoding is UTF-8 — which is what this
        // writes. The buffer is this side's to free, which is why it is in a `finally`: the reading
        // raising is no reason to lose it.
        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var handle = RorolalaBinding.locate_rola_status(path);

            return handle == 0 ? null : new RolaStatus(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>
    /// What the reading says about the entry at a path, in one word.
    /// </summary>
    /// <remarks>
    /// The path is one the caller names — a file of a listing, as the system spells it — and what the
    /// reading names is relative to <see cref="Root"/>, so a path outside the Workspace it was made of is
    /// nothing this reading is about.
    /// </remarks>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>What the reading says about it, which is <see cref="PathState.Unknown"/> for a path it is
    /// not about.</returns>
    public PathState StateOf(string path)
    {
        ArgumentNullException.ThrowIfNull(path);
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        var utf8 = Marshal.StringToCoTaskMemUTF8(path);

        try
        {
            return (PathState)RorolalaBinding.rola_status_state_of(_handle, utf8);
        }
        finally
        {
            Marshal.FreeCoTaskMem(utf8);
        }
    }

    /// <summary>
    /// How the version recorded for the entry at a path stands beside the one upstream records.
    /// </summary>
    /// <remarks>
    /// A path outside the Workspace, one nothing was ever recorded for, and one upstream does not name are
    /// all answered <see cref="StandingRelation.Unknown"/>: what tells those apart from a path in step is the
    /// distance, which is zero for the two being the same and nothing otherwise.
    /// <para>
    /// A path the Vault is ahead on is answered <see cref="StandingRelation.Behind"/> whether or not a distance
    /// comes with it, because what the Vault is at is ordinarily a version this side never fetched: the
    /// direction is read from the version having come from elsewhere, and the count from a chain that is not
    /// here.
    /// </para>
    /// </remarks>
    /// <param name="path">The path of the entry, as the program names it.</param>
    /// <returns>Where the two stand, and how many versions apart they are when the chain says.</returns>
    public Standing StandingOf(string path)
    {
        ArgumentNullException.ThrowIfNull(path);
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        var utf8 = Marshal.StringToCoTaskMemUTF8(path);

        try
        {
            // The distance is a signed number on the other side, where a negative one is "nothing says how
            // far": C has no `Option`, so the two answers are told apart by the sign rather than by a second
            // value. Zero is kept as a distance rather than read as "none", since it is what says the two are
            // the same version where the relation alone would have said the standing is not known.
            var distance = RorolalaBinding.rola_status_standing_distance_of(_handle, utf8);

            return new Standing(
                (StandingRelation)RorolalaBinding.rola_status_standing_of(_handle, utf8),
                distance < 0 ? null : (ulong)distance
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(utf8);
        }
    }

/// <summary>Gives the native reading back.</summary>
    public void Dispose()
    {
        // Nothing to give back the second time, and nothing was lost the first: the handle is forgotten as
        // soon as it is released, so a double `Dispose` releases nothing twice.
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_status(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    /// <summary>Reads one list of paths out of a native status.</summary>
    /// <param name="handle">The native status.</param>
    /// <param name="count">How many the list holds.</param>
    /// <param name="at">The path at one index, as an owned C string.</param>
    /// <returns>The paths, in the order the reading gives them.</returns>
    private static List<string> Paths(
        nint handle,
        Func<nint, nuint> count,
        Func<nint, nuint, nint> at
    )
    {
        var many = count(handle);
        var paths = new List<string>();

        for (nuint index = 0; index < many; index++)
        {
            paths.Add(NativeText.Taken(at(handle, index)));
        }

        return paths;
    }

    /// <summary>Reads the moves out of a native status.</summary>
    /// <param name="handle">The native status.</param>
    /// <returns>The moves, in the order the reading gives them.</returns>
    private static List<Rename> Renames(nint handle)
    {
        var many = RorolalaBinding.rola_status_renamed_count(handle);
        var renames = new List<Rename>();

        for (nuint index = 0; index < many; index++)
        {
            renames.Add(
                new Rename(
                    NativeText.Taken(RorolalaBinding.rola_status_renamed_from(handle, index)),
                    NativeText.Taken(RorolalaBinding.rola_status_renamed_to(handle, index)),
                    RorolalaBinding.rola_status_renamed_modified(handle, index),
                    RorolalaBinding.rola_status_renamed_strong(handle, index)
                )
            );
        }

        return renames;
    }

    /// <summary>Reads the merges in progress out of a native status.</summary>
    /// <param name="handle">The native status.</param>
    /// <returns>The merges, in the order the reading gives them.</returns>
    private static List<Merge> Merges(nint handle)
    {
        var many = RorolalaBinding.rola_status_merging_count(handle);
        var merges = new List<Merge>();

        for (nuint index = 0; index < many; index++)
        {
            // A move is said by the path being there and not said at all by it being absent, so an
            // empty answer is read as the absence it is rather than as a path of no characters —
            // which is not a path a Layout could name in the first place.
            var moved = NativeText.Taken(RorolalaBinding.rola_status_merging_moved(handle, index));

            merges.Add(
                new Merge(
                    NativeText.Taken(RorolalaBinding.rola_status_merging_target(handle, index)),
                    NativeText.Taken(RorolalaBinding.rola_status_merging_variant(handle, index)),
                    NativeText.Taken(RorolalaBinding.rola_status_merging_path(handle, index)),
                    moved.Length > 0 ? moved : null
                )
            );
        }

        return merges;
    }

    /// <summary>Reads the merges that cannot go on out of a native status.</summary>
    /// <param name="handle">The native status.</param>
    /// <returns>The reasons, in the order the reading gives them.</returns>
    private static List<BrokenReason> BrokenReasons(nint handle)
    {
        var many = RorolalaBinding.rola_status_broken_count(handle);
        var reasons = new List<BrokenReason>();

        for (nuint index = 0; index < many; index++)
        {
            reasons.Add(
                new BrokenReason(
                    Kind(RorolalaBinding.rola_status_broken_kind(handle, index)),
                    NativeText.Taken(RorolalaBinding.rola_status_broken_what(handle, index)),
                    NativeText.Taken(RorolalaBinding.rola_status_broken_target(handle, index))
                )
            );
        }

        return reasons;
    }

    /// <summary>Reads the kind of a reason as a caller names it.</summary>
    /// <param name="kind">The kind as it crossed.</param>
    /// <returns>The kind.</returns>
    // A kind this build does not know is answered as nothing read, which is the one answer that
    // cannot be mistaken for a reason: a switch that threw here would take an Overview down for a
    // reason it could have said nothing about.
    private static BrokenKind Kind(RolaStatusBroken kind) =>
        kind switch
        {
            RolaStatusBroken.RolaStatusBroken_NoTarget => BrokenKind.NoTarget,
            RolaStatusBroken.RolaStatusBroken_NoPlace => BrokenKind.NoPlace,
            RolaStatusBroken.RolaStatusBroken_FileGone => BrokenKind.FileGone,
            RolaStatusBroken.RolaStatusBroken_NoIndex => BrokenKind.NoIndex,
            RolaStatusBroken.RolaStatusBroken_NoVariant => BrokenKind.NoVariant,
            RolaStatusBroken.RolaStatusBroken_Unreadable => BrokenKind.Unreadable,
            RolaStatusBroken.RolaStatusBroken_Changed => BrokenKind.Changed,
            _ => BrokenKind.None,
        };
}

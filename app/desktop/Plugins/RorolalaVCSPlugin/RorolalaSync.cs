using RolaSharp;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a sync would do about the file at a path, which is what the Overview's sync rows are made of.
/// </summary>
/// <remarks>
/// It is the plan a sync would make, read one path at a time: <c>rola sync</c> sends a version this side is
/// further on than, brings down one the Vault is further on than, and sends one whose two chains have each moved
/// on only when it is forced. The four are the plan's own, and are told apart the way the plan tells
/// them apart — see <see cref="RolaStatus.SyncOf"/> — so a row of the Overview and the run a reader presses are one
/// answer about one file rather than two that have to agree.
/// </remarks>
internal enum SyncMove
{
    /// <summary>Nothing to move: the file is in step, or upstream does not name it.</summary>
    None,

    /// <summary>This side's version goes up, so a sync sends it.</summary>
    Send,

    /// <summary>The Vault's version comes down, so a sync receives it.</summary>
    Receive,

    /// <summary>The two chains have each moved on, so only a forced sync sends it.</summary>
    Diverged,
}

/// <summary>
/// What a sync would do about the paths of the work, read once each.
/// </summary>
/// <remarks>
/// It is read from the same <see cref="RolaStatus"/> every other answer about the work is read from, and it sits
/// here rather than in the Overview because the comparison is the reading's: what a path's version is beside the
/// one upstream records is a question about the index, and no view has any business walking a chain.
/// </remarks>
internal static class RorolalaSync
{
    /// <summary>What a sync would do about one path of the work.</summary>
    /// <param name="reading">The reading.</param>
    /// <param name="path">The path, as the program names it.</param>
    /// <returns>What a sync would do, which is nothing for a path the reading is not about.</returns>
    public static SyncMove Of(RolaStatus reading, string path)
    {
        ArgumentNullException.ThrowIfNull(reading);
        ArgumentNullException.ThrowIfNull(path);

        return reading.SyncOf(path) switch
        {
            SyncStanding.Send => SyncMove.Send,
            SyncStanding.Receive => SyncMove.Receive,
            SyncStanding.Diverged => SyncMove.Diverged,
            _ => SyncMove.None,
        };
    }

    /// <summary>
    /// How far apart the two versions of one path are, or nothing when the chain does not say.
    /// </summary>
    /// <remarks>
    /// What is read is the distance the column says, and not the standing it is read beside: being behind and being
    /// apart are what a sync row is about, and how far is what the local index can add to it. What the Vault is at
    /// is ordinarily a version this side never fetched, so a legend without a count is the ordinary answer rather
    /// than a failure.
    /// </remarks>
    /// <param name="reading">The reading.</param>
    /// <param name="path">The path, as the program names it.</param>
    /// <returns>How many versions apart they are, or nothing.</returns>
    public static ulong? Distance(RolaStatus reading, string path)
    {
        ArgumentNullException.ThrowIfNull(reading);
        ArgumentNullException.ThrowIfNull(path);

        return reading.StandingOf(path).Distance;
    }

    /// <summary>Whether a path is one a sync would move, which is every one of the four but <see cref="SyncMove.None"/>.</summary>
    /// <param name="move">What a sync would do.</param>
    /// <returns>Whether the path is a row of its own.</returns>
    public static bool Moves(SyncMove move) => move != SyncMove.None;
}

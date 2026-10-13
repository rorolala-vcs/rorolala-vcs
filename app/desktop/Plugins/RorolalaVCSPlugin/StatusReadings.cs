using RolaSharp;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a reading of the work says about the directories being listed, read once each and kept.
/// </summary>
/// <remarks>
/// It is shared by the two columns read from the work — the State mark and the column saying how far a path
/// is from upstream — because a reading is not a cheap question: it walks the tree beside the Layout and,
/// for every path that records a version, walks the version chain as well. One reading per directory between
/// the two of them is the difference between a listing that costs what it costs and one that costs it twice.
/// <para>
/// A directory both providers were asked about is read the first time and the same reading is handed over
/// after that, which is what makes the sharing worth having. Nothing is reached for: a reading is a snapshot
/// of what the tree held at one moment, and a snapshot is what a file operation makes stale — which is what
/// <see cref="Forget"/> is for.
/// </para>
/// </remarks>
internal sealed class StatusReadings
{
    /// <summary>What was read for each directory, so a listing costs one read rather than one per row.</summary>
    private readonly Dictionary<string, RolaStatus?> _readings = new(StringComparer.Ordinal);

    /// <summary>Where what could not be read is said, once.</summary>
    private readonly ILog _log;

    /// <summary>Whether what could not be read has already been said.</summary>
    private bool _warned;

    /// <summary>Makes the set of readings.</summary>
    /// <param name="log">Where what could not be read is said.</param>
    public StatusReadings(ILog log) => _log = log;

    /// <summary>The reading of the work a directory sits in, read once and kept.</summary>
    /// <param name="directory">The directory being listed.</param>
    /// <returns>The reading, or nothing when the directory is not inside a Workspace that works in a Layout.</returns>
    public RolaStatus? Reading(string directory)
    {
        if (_readings.TryGetValue(directory, out var known))
        {
            return known;
        }

        RolaStatus? status = null;

        try
        {
            status = RolaStatus.Locate(directory);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            Warn($"how the work under `{directory}` stands", error);
        }

        _readings[directory] = status;

        return status;
    }

    /// <summary>
    /// Lets go of what was read, so the next answer is read from the tree as it is now.
    /// </summary>
    /// <remarks>
    /// The readings are shared by both columns, so this may be the second letting go of the same set: what
    /// is already let go of is nothing to let go of again.
    /// </remarks>
    public void Forget()
    {
        foreach (var status in _readings.Values)
        {
            status?.Dispose();
        }

        _readings.Clear();
    }

    /// <summary>
    /// Says once that something could not be read.
    /// </summary>
    /// <remarks>
    /// A listing that will not appear because the work could not be read is a worse answer than one that
    /// appears without the columns, and it is said once because a directory of a thousand entries would
    /// otherwise say the same thing a thousand times.
    /// </remarks>
    /// <param name="what">What could not be read.</param>
    /// <param name="error">Why it could not be read.</param>
    private void Warn(string what, Exception error)
    {
        if (_warned)
        {
            return;
        }

        _warned = true;
        _log.Warn($"{what} could not be read: {error.Message}");
    }
}

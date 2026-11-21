namespace RorolalaVCSPlugin;

/// <summary>
/// Bringing the content a version is made of here, through the store's own command.
/// </summary>
/// <remarks>
/// A node of the graph is a variant, and a variant names its content by one store key. What a reader
/// who asks for a version's blobs wants is that key on both ends, and nothing written into the work:
/// the store's sync is the command that does exactly that, and it expands a content kept as chunks
/// from the side holding it, so one key carries the manifest and every chunk behind it.
/// </remarks>
internal static class BlobFetch
{
    /// <summary>The arguments a content fetch is run with, after the program's name.</summary>
    /// <param name="keys">The store keys to make the two ends hold.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(IReadOnlyList<string> keys)
    {
        ArgumentNullException.ThrowIfNull(keys);

        var arguments = new List<string>
        {
            "--theme-choice=" + Sync.Theme,
            "storage",
            "sync-hashes",
        };
        arguments.AddRange(keys);

        return arguments;
    }

    /// <summary>
    /// The keys of the versions of one lane whose content is not all here, each key once.
    /// </summary>
    /// <remarks>
    /// A lane is one file's chain, so what the item offers is that file's history rather than every
    /// version on screen. A version whose objects are all here is left out: the fetch would carry
    /// nothing, and the run that follows a press is worth a round trip only for what is missing.
    /// </remarks>
    /// <param name="versions">The versions the drawing holds, with the lane and the state of each.</param>
    /// <param name="lane">The lane the menu was opened in.</param>
    /// <returns>The keys, in the order the drawing holds them.</returns>
    internal static IReadOnlyList<string> MissingInLane(
        IEnumerable<(int Lane, string Storage, bool Offline)> versions,
        int lane
    )
    {
        ArgumentNullException.ThrowIfNull(versions);

        var keys = new List<string>();

        foreach (var (at, storage, offline) in versions)
        {
            if (
                at == lane
                && !offline
                && storage.Length > 0
                && !keys.Contains(storage, StringComparer.Ordinal)
            )
            {
                keys.Add(storage);
            }
        }

        return keys;
    }

    /// <summary>Runs a content fetch, answering whether it ended well and what it said.</summary>
    /// <remarks>Run off the window's thread: the exchange takes as long as the network takes.</remarks>
    /// <param name="program">The program the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="keys">The store keys to make the two ends hold.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string program,
        string directory,
        IReadOnlyList<string> keys
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(keys);

        var (done, said, _) = Sync.Asked(program, Arguments(keys), directory);

        return (done, said);
    }
}

namespace RorolalaVCSPlugin;

/// <summary>
/// Bringing a version's content here through the command line's fetch.
/// </summary>
/// <remarks>
/// A node of the graph is a variant, and a variant is a name the command line's fetch reads: the
/// content it points at is brought for the variant as it stands, rather than for the version an
/// entry happens to record now. A recursive run follows the version the variant is based on back to
/// its root, which is every version the node stands on. The command also expands a tree's structure
/// into the files it names, which is what the node's own mark requires before it can say the content
/// is all here.
/// </remarks>
internal static class BlobFetch
{
    /// <summary>The arguments a content fetch is run with, after the program's name.</summary>
    /// <param name="variant">The variant whose content is to come, as its whole hash.</param>
    /// <param name="recursive">Whether the versions the variant stands on come with it.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(string variant, bool recursive)
    {
        ArgumentNullException.ThrowIfNull(variant);

        var arguments = new List<string> { "--theme-choice=" + Sync.Theme, "fetch" };

        if (recursive)
        {
            arguments.Add("--recursive");
        }

        arguments.Add("--with");
        arguments.Add(variant);

        return arguments;
    }

    /// <summary>
    /// Whether any version from `hash` back along its chain is not all here.
    /// </summary>
    /// <remarks>
    /// A recursive fetch follows the base version the variant points at, so the item that offers it
    /// is worth pressing when the node or any version below it is missing content. A chain that
    /// reaches a base the drawing does not hold stops there: nothing below it can be named.
    /// </remarks>
    /// <param name="chain">Each variant the drawing holds, with its base and whether it is all here.</param>
    /// <param name="hash">The variant the menu was opened on.</param>
    /// <returns>Whether the fetch would carry anything.</returns>
    internal static bool AnyMissing(
        IReadOnlyDictionary<string, (string? Base, bool Offline)> chain,
        string hash
    )
    {
        ArgumentNullException.ThrowIfNull(chain);
        ArgumentNullException.ThrowIfNull(hash);

        var seen = new HashSet<string>(StringComparer.Ordinal);
        var at = hash;

        while (seen.Add(at) && chain.TryGetValue(at, out var version))
        {
            if (!version.Offline)
            {
                return true;
            }

            if (version.Base is not { Length: > 0 } below)
            {
                return false;
            }

            at = below;
        }

        return false;
    }

    /// <summary>Runs a content fetch, answering whether it ended well and what it said.</summary>
    /// <remarks>Run off the window's thread: the exchange takes as long as the network takes.</remarks>
    /// <param name="program">The program the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="variant">The variant whose content is to come, as its whole hash.</param>
    /// <param name="recursive">Whether the versions the variant stands on come with it.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string program,
        string directory,
        string variant,
        bool recursive
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(variant);

        var (done, said, _) = Sync.Asked(program, Arguments(variant, recursive), directory);

        return (done, said);
    }
}

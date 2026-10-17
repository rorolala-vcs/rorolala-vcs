namespace RorolalaVCSPlugin;

/// <summary>
/// Running the command line's fetch: a Vault's Layout, brought here to read.
/// </summary>
/// <remarks>
/// The exchange with the Vault is the command line's, so what this owns is the one word that asks for it:
/// <c>rola fetch</c> is <c>rola layout fetch</c> under a shorter name, and what it writes is the copy every
/// reading of the work is made from. The command says nothing of how far it has got, so a run of it is drawn as a
/// card that is still moving rather than as a bar to fill.
/// </remarks>
internal static class Fetch
{
    /// <summary>The arguments a fetch is run with, after the program's name.</summary>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments() => ["--theme-choice=" + Sync.Theme, "fetch"];

    /// <summary>Runs a fetch, answering whether it ended well and what it said.</summary>
    /// <param name="program">The program to run, which is the one the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(string program, string directory)
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(directory);

        var (done, said, _) = Sync.Asked(program, Arguments(), directory);

        return (done, said);
    }
}

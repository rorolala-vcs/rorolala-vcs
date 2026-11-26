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
    /// <summary>
    /// The status <c>rola fetch</c> ends with when the leg to the Vault could not be reached.
    /// </summary>
    /// <remarks>
    /// It is the command line's <c>EC_ERR_ACTION_TRANSPORT</c>, stated here because the two programs share no
    /// header: the number is the contract across the boundary, so it is written where it is used. A failure
    /// the Vault answered with, an account that is not here or a configuration that does not read, is a
    /// different status and says nothing about whether the Vault may be reached.
    /// </remarks>
    public const int Transport = 152;

    /// <summary>The arguments a fetch is run with, after the program's name.</summary>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments() => ["--theme-choice=" + Sync.Theme, "fetch"];

    /// <summary>Runs a fetch, answering whether it ended well, what it said, and what status it ended with.</summary>
    /// <param name="program">The program to run, which is the one the window was opened by.</param>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <returns>Whether it ended well, what to say when it did not, and the status it ended with.</returns>
    public static (bool Done, string Said, int Code) Run(string program, string directory)
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(directory);

        var (code, said, _) = Sync.Coded(program, Arguments(), directory);

        return (code == 0, said, code);
    }
}

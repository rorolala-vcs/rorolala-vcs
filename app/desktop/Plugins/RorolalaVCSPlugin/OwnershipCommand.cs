using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// Running the Vault's own ownership doors: <c>rola layout req-ownership</c> and
/// <c>rola layout giveup-ownership</c>.
/// </summary>
/// <remarks>
/// These are the doors that move ownership without making a version: the Vault holds the entry, the two
/// commands ask it to put this account's name on it or take it off, and nothing is checked in. The plugin
/// owns only the words a run is made with, since the exchange and the account it is spoken as are the
/// command line's.
/// </remarks>
internal static class OwnershipCommand
{
    /// <summary>The arguments one ownership exchange is run with.</summary>
    /// <param name="vault">The Vault whose entries are moved.</param>
    /// <param name="ids">The entries, by `Uuid`.</param>
    /// <param name="take">Whether they are taken or let go.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> Arguments(
        string vault,
        IReadOnlyList<string> ids,
        bool take
    )
    {
        ArgumentNullException.ThrowIfNull(vault);
        ArgumentNullException.ThrowIfNull(ids);

        var arguments = new List<string>
        {
            "--theme-choice=" + Sync.Theme,
            "--json",
            "layout",
            take ? "req-ownership" : "giveup-ownership",
            vault,
        };

        arguments.AddRange(ids);

        return arguments;
    }

    /// <summary>Runs one ownership exchange, answering whether it ended well and what it said.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="vault">The Vault whose entries are moved.</param>
    /// <param name="ids">The entries, by `Uuid`.</param>
    /// <param name="take">Whether they are taken or let go.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public static (bool Done, string Said) Run(
        string directory,
        string vault,
        IReadOnlyList<string> ids,
        bool take
    )
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(vault);
        ArgumentNullException.ThrowIfNull(ids);

        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            return (false, RolaI18N.Get("rorolala_vcs.sync.no_program"));
        }

        var (done, said, _) = Sync.Asked(program, Arguments(vault, ids, take), directory);

        return (done, said);
    }
}

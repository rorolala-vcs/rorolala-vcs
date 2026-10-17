using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The fetch: what a run of it is told, and what its card is drawn from when it says nothing of itself.
/// </summary>
/// <remarks>
/// The exchange itself is the command line's and is not run here — a run needs a Vault and a network — so what is
/// checked is everything this plugin decides: the command a refresh runs, and that a fetch asks for no progress
/// records, which is what has its card drawn as one still moving rather than as a bar to fill.
/// </remarks>
public sealed class FetchTests
{
    /// <summary>A fetch is the command the line names, asked for the theme this window reads.</summary>
    /// <remarks>
    /// The word is <c>fetch</c> rather than <c>layout fetch</c>: the two are the same command, and the shorter name
    /// is what the line was given. The theme is the one every run from this window asks for, so that what it says
    /// is what the window can draw.
    /// </remarks>
    [Fact]
    public void AFetchIsTheCommandTheLineNames()
    {
        var arguments = Fetch.Arguments();

        Assert.Contains("fetch", arguments);
        Assert.DoesNotContain("layout", arguments);
        Assert.Single(arguments, argument => argument.StartsWith("--theme-choice=", StringComparison.Ordinal));
    }
}

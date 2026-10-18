using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The retrack: what a run of it is told.
/// </summary>
/// <remarks>
/// The movement itself is the command line's and is not run here — it needs a Workspace and an index — so
/// what is checked is what this plugin decides: which version the record is moved to, which file it is moved
/// for, and whether the work on disk follows it.
/// </remarks>
public sealed class RetrackTests
{
    /// <summary>A retrack names the version, and says so when the file on disk is to be left alone.</summary>
    [Fact]
    public void ARetrackNamesTheVersionAndWhetherTheWorkFollows()
    {
        var moved = RetrackCommand.Arguments("/work/logo.psd", "abc123", onlyIndex: false);

        Assert.Contains("retrack", moved);
        Assert.Contains("/work/logo.psd", moved);
        Assert.Contains("--until", moved);
        Assert.Contains("abc123", moved);
        Assert.DoesNotContain("--only-index", moved);

        // The two items differ in one flag, which is the whole of what tells them apart.
        var indexOnly = RetrackCommand.Arguments("/work/logo.psd", "abc123", onlyIndex: true);

        Assert.Contains("--only-index", indexOnly);
        Assert.Equal(moved.Where(argument => argument != "--only-index"), indexOnly.Where(argument => argument != "--only-index"));
    }
}

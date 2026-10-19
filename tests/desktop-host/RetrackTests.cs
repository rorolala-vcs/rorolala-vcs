using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The retrack: what a run of it is told.
/// </summary>
/// <remarks>
/// The movement itself is the command line's and is not run here — it needs a Workspace and an index — so
/// what is checked is what this plugin decides: which version the record is moved to, which file it is moved
/// for, whether the work on disk follows it or the record is allowed to jump to it, and whether unrecorded
/// changes are discarded.
/// </remarks>
public sealed class RetrackTests
{
    /// <summary>A retrack names the version, and says whether the work follows or the record jumps.</summary>
    [Fact]
    public void ARetrackNamesTheVersionAndWhetherTheWorkFollows()
    {
        var moved = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            allowJump: false,
            force: false
        );

        Assert.Contains("retrack", moved);
        Assert.Contains("/work/logo.psd", moved);
        Assert.Contains("--until", moved);
        Assert.Contains("abc123", moved);
        Assert.DoesNotContain("--only-index", moved);
        Assert.DoesNotContain("--allow-jump", moved);
        Assert.DoesNotContain("--force", moved);

        // The two items differ in one flag, which is the whole of what tells them apart.
        var indexOnly = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: true,
            allowJump: false,
            force: false
        );

        Assert.Contains("--only-index", indexOnly);

        // A Vault version the work's own chain does not hold is a jump, and says so.
        var jump = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            allowJump: true,
            force: false
        );

        Assert.Contains("--allow-jump", jump);

        // Unrecorded changes are discarded only where the reader agreed to it.
        var forced = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            allowJump: false,
            force: true
        );

        Assert.Contains("--force", forced);

        // The file following the record and the record moving alone are opposite instructions, so the
        // two are never given together.
        var indexOnlyForced = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: true,
            allowJump: false,
            force: true
        );

        Assert.DoesNotContain("--force", indexOnlyForced);
    }
}

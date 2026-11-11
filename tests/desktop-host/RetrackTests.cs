using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The retrack: what a run of it is told.
/// </summary>
/// <remarks>
/// The movement itself is the command line's and is not run here, it needs a Workspace and an index, so
/// what is checked is what this plugin decides: which version the record is moved to, which file it is moved
/// for, whether the work on disk follows it, and whether unrecorded changes are discarded. Whether the version
/// is one the work's chain holds is the command line's to judge: a jump is what a retrack does by default, so
/// the plugin asks for none and refuses none.
/// </remarks>
public sealed class RetrackTests
{
    /// <summary>A retrack names the file and its target, and says whether the work follows it.</summary>
    [Fact]
    public void ARetrackNamesTheVersionAndWhetherTheWorkFollows()
    {
        var moved = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            force: false
        );

        Assert.Contains("retrack", moved);
        Assert.Contains("/work/logo.psd", moved);
        Assert.Contains("abc123", moved);

        // The file and what the path is to record are the command's two words, in that order: the
        // one it is about, then what it is to record.
        var words = moved.ToList();
        Assert.True(
            words.IndexOf("/work/logo.psd") < words.IndexOf("abc123"),
            "the file is not named before its target"
        );

        // The target is a word of the command, not a flag's value, and how far it may be from the
        // version the file records is the command line's own default.
        Assert.DoesNotContain("--until", moved);
        Assert.DoesNotContain("--only-index", moved);
        Assert.DoesNotContain("--deny-jump", moved);
        Assert.DoesNotContain("--allow-jump", moved);
        Assert.DoesNotContain("--force", moved);

        // The two items differ in one flag, which is the whole of what tells them apart.
        var indexOnly = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: true,
            force: false
        );

        Assert.Contains("--only-index", indexOnly);

        // Unrecorded changes are discarded only where the reader agreed to it.
        var forced = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            force: true
        );

        Assert.Contains("--force", forced);

        // The file following the record and the record moving alone are opposite instructions, so the
        // two are never given together.
        var indexOnlyForced = RetrackCommand.Arguments(
            "/work/logo.psd",
            "abc123",
            onlyIndex: true,
            force: true
        );

        Assert.DoesNotContain("--force", indexOnlyForced);
    }

    /// <summary>A fork names the path and asks for a fresh entry, with no version target.</summary>
    [Fact]
    public void AForkNamesThePathAndNoTarget()
    {
        var fork = RetrackCommand.ForkArguments("/work/tree");

        Assert.Contains("retrack", fork);
        Assert.Contains("/work/tree", fork);
        Assert.Contains("--fork", fork);

        // A fork is what the path records now, so there is no second word for a version.
        Assert.DoesNotContain("~1", fork);
        Assert.DoesNotContain("--only-index", fork);
        Assert.DoesNotContain("--force", fork);
    }
}

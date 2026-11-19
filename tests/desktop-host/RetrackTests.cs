using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The retrack: what a run of it is asked for.
/// </summary>
/// <remarks>
/// The movement itself is the writer's and is not run here, it needs a Workspace and an index, so
/// what is checked is what this plugin decides: which version the record is moved to, which file it is moved
/// for, whether the work on disk follows it, and whether unrecorded changes are discarded. Whether the version
/// is one the work's chain holds is the writer's to judge: a jump is what a retrack does by default, so
/// the plugin asks for none and refuses none.
/// </remarks>
public sealed class RetrackTests
{
    /// <summary>A retrack names the file and its target, and says whether the work follows it.</summary>
    [Fact]
    public void ARetrackNamesTheVersionAndWhetherTheWorkFollows()
    {
        var moved = RetrackCommand.Request(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            force: false
        );

        Assert.Equal("/work/logo.psd", moved.File);
        Assert.Equal("abc123", moved.Record);

        // The target is what the path records, not a limit on how far it may move, and the plugin
        // neither follows a jump rule nor gives one.
        Assert.False(moved.DenyJump);
        Assert.False(moved.Fork);
        Assert.False(moved.OnlyIndex);
        Assert.False(moved.Force);

        // The two items differ in one switch, which is the whole of what tells them apart.
        var indexOnly = RetrackCommand.Request(
            "/work/logo.psd",
            "abc123",
            onlyIndex: true,
            force: false
        );

        Assert.True(indexOnly.OnlyIndex);
        Assert.False(indexOnly.Force);

        // Unrecorded changes are discarded only where the reader agreed to it.
        var forced = RetrackCommand.Request(
            "/work/logo.psd",
            "abc123",
            onlyIndex: false,
            force: true
        );

        Assert.True(forced.Force);
        Assert.False(forced.OnlyIndex);

        // The file following the record and the record moving alone are opposite instructions, so the
        // two are never asked for together.
        var indexOnlyForced = RetrackCommand.Request(
            "/work/logo.psd",
            "abc123",
            onlyIndex: true,
            force: true
        );

        Assert.True(indexOnlyForced.OnlyIndex);
        Assert.False(indexOnlyForced.Force);
    }

    /// <summary>A fork names the path and asks for a fresh entry, with no version target.</summary>
    [Fact]
    public void AForkNamesThePathAndNoTarget()
    {
        var fork = RetrackCommand.ForkRequest("/work/tree");

        Assert.Equal("/work/tree", fork.File);
        Assert.True(fork.Fork);

        // A fork is what the path records now, so there is no second word for a version.
        Assert.Equal(string.Empty, fork.Record);
        Assert.False(fork.OnlyIndex);
        Assert.False(fork.Force);
    }
}

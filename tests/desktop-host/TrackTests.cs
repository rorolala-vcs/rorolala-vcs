using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The track: what a run of it is told.
/// </summary>
/// <remarks>
/// The recording itself is the command line's and is not run here. What is checked is the one thing
/// this plugin decides about a forced run: whether the command line is asked to record what a plain
/// track would refuse.
/// </remarks>
public sealed class TrackTests
{
    /// <summary>A forced track carries the force flag, and a plain one does not.</summary>
    [Fact]
    public void AForcedTrackAsksTheCommandLineToForceIt()
    {
        var plain = TrackCommand.Arguments(["/work/logo.psd"], "logo", force: false);

        Assert.Contains("track", plain);
        Assert.Contains("--no-editor", plain);
        Assert.DoesNotContain("--force", plain);

        var forced = TrackCommand.Arguments(["/work/logo.psd"], "logo", force: true);

        Assert.Contains("--force", forced);
        Assert.Contains("/work/logo.psd", forced);
    }
}

using RorolalaLauncher.Rola;

namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// What a start of the Desktop program is taken to have come to.
/// </summary>
/// <remarks>
/// The two programs agree on two things by hand, the line the window writes and the exit code a start that
/// found the program already going ends with, so what is checked here is that reading of them, which is the
/// whole of the wait this program does that can be practiced without starting a program.
/// </remarks>
public sealed class ReadinessTests
{
    /// <summary>The line the window writes is a window to look at.</summary>
    /// <returns>A task that ends when the check does.</returns>
    [Fact]
    public async Task AWindowSaysItIsUpWithOneLine()
    {
        var readiness = new Readiness();

        readiness.Said(Readiness.Line);

        var outcome = await readiness.WaitAsync(TimeSpan.FromSeconds(1));

        Assert.True(outcome.Ready);
        Assert.Null(outcome.Ended);
    }

    /// <summary>A line is read as what it says without the room around it.</summary>
    /// <returns>A task that ends when the check does.</returns>
    [Fact]
    public async Task TheLineIsReadWithoutTheRoomAroundIt()
    {
        var readiness = new Readiness();

        readiness.Said($"  {Readiness.Line}  ");

        Assert.True((await readiness.WaitAsync(TimeSpan.FromSeconds(1))).Ready);
    }

    /// <summary>A start that found the program already going is a window to look at as well.</summary>
    /// <returns>A task that ends when the check does.</returns>
    [Fact]
    public async Task AStartThatFoundTheProgramAlreadyGoingIsAWindow()
    {
        var readiness = new Readiness();

        readiness.Ended(Readiness.DesktopCalled);

        var outcome = await readiness.WaitAsync(TimeSpan.FromSeconds(1));

        Assert.True(outcome.Ready);
        Assert.Equal(Readiness.DesktopCalled, outcome.Ended);
    }

    /// <summary>A start that ended another way is nothing to look at, and says what it ended with.</summary>
    /// <returns>A task that ends when the check does.</returns>
    [Fact]
    public async Task AStartThatEndedAnotherWayIsNoWindow()
    {
        var readiness = new Readiness();

        readiness.Ended(171);

        var outcome = await readiness.WaitAsync(TimeSpan.FromSeconds(1));

        Assert.False(outcome.Ready);
        Assert.Equal(171, outcome.Ended);
    }

    /// <summary>A line that is not the one the window writes is not a window.</summary>
    /// <returns>A task that ends when the check does.</returns>
    [Fact]
    public async Task AnotherLineIsNotAWindow()
    {
        var readiness = new Readiness();

        readiness.Said("rola-desktop-starting");

        var outcome = await readiness.WaitAsync(TimeSpan.FromMilliseconds(1));

        Assert.False(outcome.Ready);
        Assert.Null(outcome.Ended);
    }

    /// <summary>
    /// Waiting past the time this window is willing to gives the window back rather than a failure.
    /// </summary>
    /// <remarks>
    /// The start may still arrive, so what a wait that ended with nothing said must not be taken for is a start
    /// that did not work: the process is left running for exactly that reason.
    /// </remarks>
    /// <returns>A task that ends when the check does.</returns>
    [Fact]
    public async Task AWaitThatEndsWithNothingSaidIsAWindowStillComing()
    {
        var readiness = new Readiness();

        var outcome = await readiness.WaitAsync(TimeSpan.Zero);

        Assert.False(outcome.Ready);
        Assert.Null(outcome.Ended);
    }
}

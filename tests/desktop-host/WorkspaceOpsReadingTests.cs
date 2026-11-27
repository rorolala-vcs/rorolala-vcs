using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The words a recorded operation is read by: its command, what became of each change, and how long
/// ago it was.
/// </summary>
/// <remarks>
/// The catalogue here states nothing, so a key reads as itself and what is checked is which key a
/// row is sent to. The time is the part with a rule of its own: days are counted by the calendar,
/// so a run late last night is yesterday in the morning rather than a dozen hours ago.
/// </remarks>
public sealed class WorkspaceOpsReadingTests
{
    /// <summary>Names the catalogue, so a key is read as a key rather than failing to resolve.</summary>
    public WorkspaceOpsReadingTests() => _ = Host.Services();

    /// <summary>The moment a run is shown as is the interval between it and now.</summary>
    [Fact]
    public void ARunIsShownAsHowLongAgoItWas()
    {
        var now = new DateTimeOffset(2026, 11, 27, 12, 0, 0, TimeSpan.Zero);

        Assert.Equal(
            "rorolala_vcs.workspace.when_now",
            WorkspaceOpsReading.When("2026-11-27T11:59:40Z", now)
        );
        Assert.Equal(
            "rorolala_vcs.workspace.when_minutes",
            WorkspaceOpsReading.When("2026-11-27T11:55:00Z", now)
        );
        Assert.Equal(
            "rorolala_vcs.workspace.when_hours",
            WorkspaceOpsReading.When("2026-11-27T09:00:00Z", now)
        );

        // The calendar rather than elapsed hours: nineteen hours ago was still yesterday, and ten
        // days is a week and a half rather than ten days once a week has passed.
        Assert.Equal(
            "rorolala_vcs.workspace.when_yesterday",
            WorkspaceOpsReading.When("2026-11-26T17:00:00Z", now)
        );
        Assert.Equal(
            "rorolala_vcs.workspace.when_days",
            WorkspaceOpsReading.When("2026-11-24T12:00:00Z", now)
        );
        Assert.Equal(
            "rorolala_vcs.workspace.when_weeks",
            WorkspaceOpsReading.When("2026-11-17T12:00:00Z", now)
        );
    }

    /// <summary>A stamp from after now, or one that does not read, is not made into a wrong interval.</summary>
    [Fact]
    public void AStampThatDoesNotReadIsShownAsItWasWritten()
    {
        var now = new DateTimeOffset(2026, 11, 27, 12, 0, 0, TimeSpan.Zero);

        Assert.Equal(
            "rorolala_vcs.workspace.when_now",
            WorkspaceOpsReading.When("2026-11-27T12:00:30Z", now)
        );
        Assert.Equal("not a time", WorkspaceOpsReading.When("not a time", now));
        Assert.Equal("not a time", WorkspaceOpsReading.Exact("not a time"));
    }

    /// <summary>The row's tooltip holds the exact moment, in UTC.</summary>
    [Fact]
    public void TheExactMomentIsSaidInUtc() =>
        Assert.Equal(
            "27 Nov 2026, 10:46 UTC",
            WorkspaceOpsReading.Exact("2026-11-27T10:46:03Z")
        );

    /// <summary>A command is named in the window's words, and one this build does not know as itself.</summary>
    [Fact]
    public void ACommandIsNamedByTheWordsThisWindowUses()
    {
        Assert.Equal(
            "rorolala_vcs.workspace.cmd_fs_mv",
            WorkspaceOpsReading.Command("fs-ops mv")
        );
        Assert.Equal("something else", WorkspaceOpsReading.Command("something else"));
    }

    /// <summary>What became of a change is a word, and a state that says nothing is no word at all.</summary>
    [Fact]
    public void AChangesStateIsAWordOrNothing()
    {
        Assert.Equal(
            "rorolala_vcs.workspace.state_added",
            WorkspaceOpsReading.State("added")
        );
        Assert.Null(WorkspaceOpsReading.State(string.Empty));
        Assert.Null(WorkspaceOpsReading.State("something else"));
    }
}

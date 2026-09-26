using RorolalaDesktop.Logging;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The host's one log, and the deduplication it shares with popups.
/// </summary>
public sealed class LoggingTests
{
    /// <summary>An identical line is counted rather than added again.</summary>
    [Fact]
    public void AnIdenticalLineIsCountedRatherThanAddedAgain()
    {
        var log = new LogService(new NotificationService());

        log.Record(LogLevel.Info, "kernel", "the same words");
        log.Record(LogLevel.Info, "kernel", "the same words");

        var entry = Assert.Single(log.Entries);
        Assert.Equal(2, entry.Count);
    }

    /// <summary>Content that differs by even a little is a line of its own.</summary>
    [Fact]
    public void ContentThatDiffersByALittleIsALineOfItsOwn()
    {
        var log = new LogService(new NotificationService());

        log.Record(LogLevel.Info, "kernel", "the same words");
        log.Record(LogLevel.Info, "kernel", "the same words!");
        log.Record(LogLevel.Warn, "kernel", "the same words");

        Assert.Equal(3, log.Entries.Count);
    }

    /// <summary>A popup and the log share one rule, so a popup is shown once.</summary>
    [Fact]
    public void APopupIsShownOnceHoweverOftenItIsRaised()
    {
        var notifications = new NotificationService();
        var log = new LogService(notifications);
        var popups = new PopupService(notifications);

        log.Record(LogLevel.Warn, "kernel", "something to see");
        popups.Raise(LogLevel.Warn, "kernel", "something to see");
        log.Record(LogLevel.Warn, "kernel", "something to see");

        Assert.Single(popups.Take());
        Assert.Empty(popups.Take());
        Assert.Equal(3, log.Entries[0].Count);
    }

    /// <summary>A log for one plugin names that plugin on every line it writes.</summary>
    [Fact]
    public void ALogForOnePluginNamesItOnEveryLine()
    {
        var log = new LogService(new NotificationService());

        log.For("it.alpha").Error("something went wrong");

        Assert.Equal("it.alpha", log.Entries[0].Source);
        Assert.Equal(LogLevel.Error, log.Entries[0].Level);
    }
}

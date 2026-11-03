using FileSystemPlugin;

using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The place a click landed on, as the catalogue the Version Graph's track reads it from.
/// </summary>
/// <remarks>
/// A directory is not a version chain, so the file catalogues leave it out; this is the answer that
/// lets an action over the place be asked about the place rather than about some file clicked before it.
/// </remarks>
public sealed class ChosenDirectoryTests
{
    /// <summary>A directory is the place, and a file clicked after it takes the place away.</summary>
    [Fact]
    public void ADirectoryIsThePlaceAndAFileTakesItAway()
    {
        Reset();

        var directory = new Entry("D:\\work\\art", EntryKind.Directory);
        var file = new Entry("D:\\work\\art\\one.psd", EntryKind.File);

        ChosenDirectory.Publish(directory);

        Assert.Same(directory, ChosenDirectory.Current);

        ChosenDirectory.Publish(file);

        Assert.Null(ChosenDirectory.Current);
    }

    /// <summary>A directory chosen in the listing is published beside the files the choice leaves out.</summary>
    [Fact]
    public void ChoosingADirectoryInAListingPublishesThePlace()
    {
        Reset();

        var directory = new Entry("D:\\work\\art", EntryKind.Directory);

        ChosenFiles.Publish([directory]);

        Assert.Equal(directory.Path, ChosenDirectory.Current?.Path);
        Assert.Empty(ChosenFiles.Current);

        // A file choice afterwards is the newer answer, and the place is no longer what is meant.
        ChosenFiles.Publish([new Entry("D:\\work\\art\\one.psd", EntryKind.File)]);

        Assert.Null(ChosenDirectory.Current);
    }

    /// <summary>Takes every answer back, so a test does not begin with another test's choice.</summary>
    private static void Reset()
    {
        ChosenFiles.Clear();
        ChosenEntry.Clear();
        ChosenDirectory.Clear();
    }
}

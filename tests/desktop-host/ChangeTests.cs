using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What the program says when something has changed, told apart from what it says when a reading is at hand.
/// </summary>
/// <remarks>
/// The two are different calls on purpose: a change is what makes every reader let go of what it answered, and a
/// reading that has landed is what makes the views draw it. Saying the first for the second is a loop — each reader's
/// landing empties the other's answer, which reads it again, which lands, and so on for as long as the window is open.
/// </remarks>
public sealed class ChangeTests
{
    /// <summary>Begins each test from no configuration and from no hide rule registered.</summary>
    /// <remarks>
    /// Both are the run's rather than a host's: a rule another check registered would hide what this one writes, and
    /// what this side of the program says is not about any rule of the user's.
    /// </remarks>
    public ChangeTests()
    {
        DataHome.Clean();
        HideRegistry.Clear();
    }

    /// <summary>A reading that is at hand draws again and lets go of nothing.</summary>
    [Fact]
    public void AReadingAtHandIsDrawnAndForgottenByNobody()
    {
        var directory = Scratch.New("redraw-channel");
        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config, host.Files);

        var redrawn = 0;
        var restaged = 0;
        var touched = 0;

        shared.Redrawn += () => redrawn++;
        shared.Changed += () => restaged++;
        shared.Touched += () => touched++;

        host.Files.Redraw();

        // Drawn, and said of nothing else: a changed base or a changed hiding restages every listing, and the files
        // having changed reads every directory again — neither is what a landed reading means.
        Assert.Equal(1, redrawn);
        Assert.Equal(0, restaged);
        Assert.Equal(0, touched);
    }

    /// <summary>
    /// A location told that a reading is at hand draws its rows again, out of what it already read.
    /// </summary>
    /// <remarks>
    /// The staged list has to be a new list, because that is what the dock compares to tell a listing to build again
    /// from one only to repaint: a column's value is read as its row is built, so a repaint would leave the answer
    /// that was there before the reading landed.
    /// </remarks>
    [Fact]
    public void ALocationStagesItsEntriesAgainWhenAReadingIsAtHand()
    {
        var directory = Scratch.New("redraw-location");
        File.WriteAllText(Path.Combine(directory, "one.txt"), "x");

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);

        var before = browser.Shown;
        var drawn = 0;

        browser.Changed += () => drawn++;

        host.Files.Redraw();

        Assert.Equal(1, drawn);
        Assert.NotSame(before, browser.Shown);
        Assert.Equal(before.Select(entry => entry.Path), browser.Shown.Select(entry => entry.Path));
    }
}

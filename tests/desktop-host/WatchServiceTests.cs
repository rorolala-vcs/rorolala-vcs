using Avalonia.Headless;
using Avalonia.Threading;
using FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The directories on screen, watched for what another program does to them.
/// </summary>
/// <remarks>
/// The platform raises its own events on a thread of its own and a save is several of them, so what is checked
/// is what is raised once a burst has settled: a directory whose set of entries may differ, and an entry whose
/// content may differ, each named rather than summed. A watch is reference counted, because a location and a
/// tree step can show one directory, and the last of them leaving is what ends it.
/// </remarks>
public sealed class WatchServiceTests
{
    /// <summary>A change is said of the directory it happened in, and of no other that is watched.</summary>
    [Fact]
    public void AChangeIsSaidOfTheDirectoryItHappenedIn()
    {
        var one = Scratch.New("watch-one");
        var other = Scratch.New("watch-other");

        using var watch = new WatchService(TimeSpan.FromMilliseconds(20));

        var sets = new List<string>();

        watch.Set += sets.Add;
        watch.Show(one);
        watch.Show(other);

        File.WriteAllText(Path.Combine(one, "made.txt"), "one");

        Wait(watch, () => sets.Count > 0);

        Assert.Contains(sets, said => Same(said, one));
        Assert.DoesNotContain(sets, said => Same(said, other));
    }

    /// <summary>A change of one entry's content names that entry.</summary>
    [Fact]
    public void AContentChangeIsSaidOfTheEntry()
    {
        var directory = Scratch.New("watch-entry");
        var file = Path.Combine(directory, "written.txt");

        File.WriteAllText(file, "one");

        using var watch = new WatchService(TimeSpan.FromMilliseconds(20));

        var entries = new List<string>();

        watch.Entry += entries.Add;
        watch.Show(directory);

        // The first write may have been gathered before the watch was made, so what is written for this is a
        // second write of the same file, which is the shape of a program saving one.
        File.WriteAllText(file, "two");

        Wait(watch, () => entries.Count > 0);

        Assert.Contains(entries, said => Same(said, file));
    }

    /// <summary>A hidden directory is no longer watched, and a directory shown twice is watched once.</summary>
    [Fact]
    public void AWatchIsHeldUntilEveryShowingOfItIsHidden()
    {
        var directory = Scratch.New("watch-count");

        using var watch = new WatchService(TimeSpan.FromMilliseconds(20));

        var sets = new List<string>();

        watch.Set += sets.Add;
        watch.Show(directory);
        watch.Show(directory);
        watch.Hide(directory);

        File.WriteAllText(Path.Combine(directory, "kept.txt"), "one");

        Wait(watch, () => sets.Count > 0);

        sets.Clear();
        watch.Hide(directory);

        File.WriteAllText(Path.Combine(directory, "gone.txt"), "one");

        // The watch is over, so nothing is raised for a change of a directory nobody shows.
        Thread.Sleep(500);
        Pump();
        watch.Flush();

        Assert.Empty(sets);
    }

    /// <summary>Runs the window's loop until what is being waited for happens, raising what was gathered.</summary>
    /// <param name="watch">The watch whose burst is raised, rather than waited out.</param>
    /// <param name="until">What is being waited for.</param>
    private static void Wait(WatchService watch, Func<bool> until)
    {
        var started = DateTime.UtcNow;

        while (!until())
        {
            if (DateTime.UtcNow > started.AddSeconds(10))
            {
                throw new InvalidOperationException("what was waited for never happened");
            }

            Thread.Sleep(20);
            Pump();
            watch.Flush();
        }
    }

    /// <summary>Runs the window's loop for a few turns.</summary>
    private static void Pump()
    {
        Dispatcher.UIThread.RunJobs();
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
    }

    /// <summary>Whether two paths name the same directory.</summary>
    /// <param name="left">One path.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether they are the same place.</returns>
    private static bool Same(string left, string right) =>
        string.Equals(Full(left), Full(right), OperatingSystem.IsWindows()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal);

    /// <summary>A path as it is compared.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The path, made full and without a trailing separator.</returns>
    private static string Full(string path) => Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
}

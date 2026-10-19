using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The base directory the File System plugin publishes for the plugins that depend on it.
/// </summary>
/// <remarks>
/// What a dependent plugin reaches this plugin through is a catalogue, and this is the one that says
/// where the work is: what a run starts on has to be there from the first read, and a tree rooted
/// elsewhere has to move it.
/// </remarks>
public sealed class BaseDirectoryTests
{
    /// <summary>The base is published as the run is made, and follows where the tree is rooted.</summary>
    /// <remarks>
    /// The two halves are one answer: a plugin that reads the base before anything has moved it has to
    /// be told where the run started, and one that follows it has to be told when the user roots the
    /// tree somewhere else — once, since rooting it where it already is asks for nothing.
    /// </remarks>
    [Fact]
    public void TheBaseDirectoryIsPublishedAndFollowsWhereTheTreeIsRooted()
    {
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);

        var first = Scratch.New("root");
        var second = Scratch.New("elsewhere");

        // Making the shared answers is the run rooting its tree: everything a location reads sits on
        // them, and the base they are made with is the one the run starts on.
        var shared = new Shared(first, new HideRegistry(host.Config), host.Config);

        Assert.Equal(first, BaseDirectory.Current);

        using var browser = new Browser(host.Log, shared, first, host.LogicalViews);

        var said = 0;
        Action listener = () => said++;

        BaseDirectory.Changed += listener;

        try
        {
            browser.SetBase(second);
            browser.SetBase(second);
        }
        finally
        {
            BaseDirectory.Changed -= listener;
        }

        Assert.Equal(second, BaseDirectory.Current);
        Assert.Equal(1, said);
    }
}

using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;
using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Rorolala plugin's contribution to what the browser leaves out.
/// </summary>
/// <remarks>
/// What is checked here is the registering and the shape of the answer — that the option is offered in
/// the File System plugin's catalogue, labelled with this plugin's own key, and that a place no Workspace
/// holds is one under which nothing is hidden. The matching itself is the native loader's and is checked
/// where the loader is: the C ABI suite.
/// </remarks>
public sealed class RolaIgnoreTests
{
    /// <summary>Begins each test from no configuration at all, and from no provider registered.</summary>
    public RolaIgnoreTests()
    {
        DataHome.Clean();
        HideRegistry.Clear();
    }

    /// <summary>
    /// Starting the Rorolala plugin offers its hide provider beside the ones the File System plugin
    /// brings itself, labelled with this plugin's own key.
    /// </summary>
    [Fact]
    public void TheRorolalaPluginOffersItsHideProvider()
    {
        var services = Host.Services();

        new FileSystem().Initialize(services.For(new PluginId(FileSystem.Identity), 0));
        new Vcs().Initialize(services.For(new PluginId("rorolala.vcs"), 1));

        // The plugin's own label key is what names it — the File System plugin knows nothing about
        // Rorolala's rules, only that a provider was added to the catalogue it offered.
        var rola = Assert.Single(HideRegistry.Providers, option => option.Value == "rola_ignored");
        Assert.Equal("rorolala_vcs.hides.rola_ignored", rola.LabelKey);

        Assert.Contains(
            HideRegistry.Providers,
            option => option.Value == "dot_file"
        );
    }

    /// <summary>
    /// A place no Workspace holds has no rules to meet, so nothing there is hidden.
    /// </summary>
    /// <remarks>
    /// The Workspace is looked for upwards from the root a view is rooted at, so a root outside every
    /// Workspace is an answer before any rules are read: the walk finds the data directory that says a
    /// Workspace is there, and finds none.
    /// </remarks>
    [Fact]
    public void NothingIsHiddenWhereNoWorkspaceIs()
    {
        var outside = Scratch.New("rola-ignore-outside");
        var kept = Path.Combine(outside, "kept.mb");
        File.WriteAllText(kept, "x");

        var entry = new Entry(kept, EntryKind.File);

        Assert.False(new RolaIgnored().Hides(entry));
        Assert.False(new RolaIgnored().Hides(entry, outside));
        Assert.True(new RolaIgnored().Ready(outside));
    }

    /// <summary>
    /// Inside a Workspace the answer is read before it is asked for, and asking for it again is asking
    /// the same reading.
    /// </summary>
    /// <remarks>
    /// A view primes a Workspace off the thread its rows are drawn on, which is what <c>Prepare</c> is:
    /// what is asked afterwards finds the answer here rather than reading it while a row is drawn. What
    /// the rules say is the native loader's business; what is checked is that the reading is had, and
    /// that letting it go makes the next answer be read again.
    /// </remarks>
    [Fact]
    public void AWorkspacesRulesAreReadBeforeTheyAreAsked()
    {
        var workspace = Scratch.New("rola-ignore-workspace");
        Directory.CreateDirectory(Path.Combine(workspace, ".rola"));
        var kept = Path.Combine(workspace, "kept.txt");
        File.WriteAllText(kept, "x");

        var provider = new RolaIgnored();

        Assert.False(provider.Ready(workspace));

        provider.Prepare(workspace);
        Assert.True(provider.Ready(workspace));

        // A path no rule can govern is answered as shown either way — the reading being had is the
        // answer, not a hide that came of it.
        Assert.False(provider.Hides(new Entry(kept, EntryKind.File), workspace));

        provider.Forget();

        Assert.False(provider.Ready(workspace));
    }
}

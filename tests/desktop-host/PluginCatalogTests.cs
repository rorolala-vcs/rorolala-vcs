using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;
using Git = global::GitVCSPlugin.GitVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The plugins that ship with the program: what one contributes to another's extension point.
/// </summary>
/// <remarks>
/// The two real plugins are started here, in process, rather than through the load context the program
/// uses. What that leaves out is the loading; what it checks is the part the load context exists to make
/// work — that a plugin reaching into another's static catalogue reaches the same one, and that what it
/// adds is offered by the setting the other declared (Section 6.2).
/// </remarks>
public sealed class PluginCatalogTests
{
    /// <summary>Begins each test from no configuration at all.</summary>
    public PluginCatalogTests() => DataHome.Clean();

    /// <summary>
    /// The File System plugin offers its own three hide providers, and the Git plugin's is offered beside
    /// them even though the setting was declared before the Git plugin started.
    /// </summary>
    /// <remarks>
    /// The catalogue is static and the two plugins are started in process, so it holds what every test in
    /// this run registered; what is checked is that the three this plugin brings are there and in the order
    /// it brings them, and that Git's is added after them rather than instead of one.
    /// </remarks>
    [Fact]
    public void ADependentPluginAddsItsHideProviderToTheFileSystemsCatalogue()
    {
        var services = Host.Services();

        new FileSystem().Initialize(
            services.For(new PluginId(FileSystem.Identity), 0)
        );

        Assert.Equal(
            ["dot_file", "dot_dir", "windows_hidden"],
            HideRegistry.Providers.Select(option => option.Value).Take(3).ToArray()
        );

        new Git().Initialize(
            services.For(new PluginId(Git.Identity), 1)
        );

        // The Git plugin's provider is in the catalogue the setting was declared with, and the plugin's own
        // label key is what names it — the File System plugin knows nothing about Git.
        var git = Assert.Single(HideRegistry.Providers, option => option.Value == "git_ignored");
        Assert.Equal("rorolala_file_system.hides.git_ignored", git.LabelKey);
    }

    /// <summary>
    /// Every registered provider is in force for a run that has stored nothing, and one taken out of the
    /// setting stops hiding.
    /// </summary>
    /// <remarks>
    /// The catalogue is static and the two plugins are started in process, so it holds what every test in
    /// this run registered; what is checked is that the selection is the catalogue, however many providers
    /// that turns out to be, rather than a number this test would pin.
    /// </remarks>
    [Fact]
    public void EveryProviderIsInForceUntilTheSettingTakesItOut()
    {
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 0);
        var hides = new HideRegistry(host.Config);

        global::FileSystemPlugin.HideRegistry.Declare(host);

        new Git().Initialize(services.For(new PluginId(Git.Identity), 1));

        // Nothing stored: the selection is the whole catalogue. The rule that rules on files hides a
        // dot-file, and the one that rules on directories has nothing to say about a plain file.
        Assert.Equal(
            HideRegistry.Providers.Select(option => option.Value).ToHashSet(StringComparer.Ordinal),
            hides.Selection()
        );
        Assert.Single(hides.Provider(new Entry("/tmp/.gitignore", EntryKind.File)), provider => provider.Id == "dot_file");
        Assert.Empty(hides.Provider(new Entry("/tmp/kept.txt", EntryKind.File)));

        // Taken out: nothing is hidden, which is not the default coming back but a choice of no rules.
        host.Config.Keep(HideRegistry.Setting, string.Empty);

        Assert.Empty(hides.Selection());
        Assert.Empty(hides.Provider(new Entry("/tmp/.gitignore", EntryKind.File)));
    }
}

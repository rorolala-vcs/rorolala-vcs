using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using FileSystemPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a listing of a directory shows and what it leaves out, which is what a user sees.
/// </summary>
/// <remarks>
/// The plugin's own location is made over a scratch directory and the listing it staged is read: what a rule
/// does to what is on screen is not visible from outside the plugin, and it is the one thing a wrong default
/// gets wrong. A fresh run has every rule in force and the switch that shows what is hidden on, so the first
/// thing a user sees is the whole directory with the hidden entries faded, a rule that hides with no switch
/// to bring the entries back is what would leave them with an empty browser.
/// </remarks>
public sealed class ListingTests
{
    /// <summary>Begins each test from no configuration at all, and from no provider registered.</summary>
    public ListingTests()
    {
        DataHome.Clean();
        global::FileSystemPlugin.HideRegistry.Clear();
    }

    /// <summary>
    /// A directory with a plain file, a dot-file and a dot-directory: every entry is listed to begin with,
    /// the switch takes the hidden ones out and puts them back, and a rule that is turned off stops leaving
    /// anything out, unless it is the last one, whose empty choice hides nothing at all.
    /// </summary>
    [Fact]
    public void AListingShowsEverythingUntilTheSwitchIsTurnedOff()
    {
        var directory = Scratch.New("listing");
        File.WriteAllText(Path.Combine(directory, "kept.txt"), "x");
        File.WriteAllText(Path.Combine(directory, ".hidden.txt"), "x");
        Directory.CreateDirectory(Path.Combine(directory, ".hidden-dir"));

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);

        // One settings view, and the one this host would hand the plugin: a second view over the same
        // registry is a second subscription, so a write through it would reach neither the other view nor
        // this test.
        var config = host.Config;

        var hides = new global::FileSystemPlugin.HideRegistry(config);
        global::FileSystemPlugin.HideRegistry.Declare(host);

        var shared = new global::FileSystemPlugin.Shared(directory, hides, config, new FileChanges());
        var browser = new global::FileSystemPlugin.Browser(host.Log, shared, directory, host.LogicalViews);

        // The plugin's own one-liner, repeated here because this is the location alone rather than the
        // plugin: a change to what is shown is a listing staged again out of what was read.
        config.Changed += setting =>
        {
            if (
                string.Equals(setting, global::FileSystemPlugin.HideRegistry.Setting, StringComparison.Ordinal)
                || string.Equals(setting, global::FileSystemPlugin.Shared.ShowSetting, StringComparison.Ordinal)
            )
            {
                shared.Hidden();
            }
        };

        // A fresh run: every rule is in force and the switch is on, so every entry is listed, the hidden
        // ones faded rather than left out. This is the answer that must not be an empty listing.
        Assert.True(shared.ShowHidden);
        Assert.Equal([".hidden-dir", ".hidden.txt", "kept.txt"], Names(browser));

        // Turning a rule off is a staging with the entries exactly as they were, the dot-file is shown
        // either way while the switch is on, and the list is a new one all the same, which is what a dock
        // holds to tell a listing staged again from one it has not drawn again.
        var staged = browser.Shown;
        var names = Names(browser);

        config.Keep(global::FileSystemPlugin.HideRegistry.Setting, "dot_file,dot_dir,git_ignored");

        Assert.NotSame(staged, browser.Shown);
        Assert.Equal(names, Names(browser));

        // The switch off is the whole directory with the hidden entries taken out.
        shared.ShowHidden = false;
        Assert.Equal(["kept.txt"], Names(browser));

        // And back on, from what was read rather than by reading the directory again.
        shared.ShowHidden = true;
        Assert.Equal([".hidden-dir", ".hidden.txt", "kept.txt"], Names(browser));

        // With only the file rule in force, the switch off leaves the dot-directory in and the dot-file out.
        config.Keep(global::FileSystemPlugin.HideRegistry.Setting, "dot_file");

        shared.ShowHidden = false;
        Assert.Equal([".hidden-dir", "kept.txt"], Names(browser));

        // Every rule off hides nothing, and reads as that rather than as the default coming back: the empty
        // list is what the user chose, not the absence of a choice.
        config.Keep(global::FileSystemPlugin.HideRegistry.Setting, string.Empty);

        Assert.Empty(hides.Selection());
        Assert.Equal([".hidden-dir", ".hidden.txt", "kept.txt"], Names(browser));
    }

    /// <summary>
    /// A rule in the faded state stays in the listing, and the eye turns the entries a hiding rule takes out
    /// into the same faded rows rather than changing that rule.
    /// </summary>
    [Fact]
    public void AGrayRuleStaysAndTheEyeFadesWhatAHidingRuleTakesOut()
    {
        var directory = Scratch.New("listing-gray");
        File.WriteAllText(Path.Combine(directory, "kept.txt"), "x");
        File.WriteAllText(Path.Combine(directory, ".hidden.txt"), "x");
        Directory.CreateDirectory(Path.Combine(directory, ".hidden-dir"));

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var config = host.Config;
        var hides = new global::FileSystemPlugin.HideRegistry(config);

        global::FileSystemPlugin.HideRegistry.Declare(host);

        var shared = new global::FileSystemPlugin.Shared(directory, hides, config, new FileChanges());
        var browser = new global::FileSystemPlugin.Browser(host.Log, shared, directory, host.LogicalViews);

        config.Changed += setting =>
        {
            if (
                string.Equals(setting, global::FileSystemPlugin.HideRegistry.Setting, StringComparison.Ordinal)
                || string.Equals(setting, global::FileSystemPlugin.Shared.ShowSetting, StringComparison.Ordinal)
            )
            {
                shared.Hidden();
            }
        };

        var hidden = Path.Combine(directory, ".hidden.txt");
        var hiddenDirectory = Path.Combine(directory, ".hidden-dir");

        Assert.True(hides.SetState("dot_dir", HideState.Show));
        Assert.True(hides.SetState("dot_file", HideState.Gray));

        shared.ShowHidden = false;

        Assert.Equal([".hidden-dir", ".hidden.txt", "kept.txt"], Names(browser));
        Assert.True(shared.Dims(new Entry(hidden, EntryKind.File)));
        Assert.False(shared.Hides(new Entry(hidden, EntryKind.File)));
        Assert.False(shared.Dims(new Entry(hiddenDirectory, EntryKind.Directory)));

        // A rule moved from shown to faded does not write the hiding setting, so the caller says the listing
        // is to be staged again. A caller that forgot would leave the row drawn as it was.
        Assert.False(hides.SetState("dot_dir", HideState.Gray));
        shared.Hidden();
        Assert.True(shared.Dims(new Entry(hiddenDirectory, EntryKind.Directory)));

        // A rule moved back to hiding is taken out while the eye is off, and is drawn faded while it is on.
        Assert.True(hides.SetState("dot_file", HideState.Hide));

        Assert.Equal([".hidden-dir", "kept.txt"], Names(browser));

        shared.ShowHidden = true;

        Assert.Equal([".hidden-dir", ".hidden.txt", "kept.txt"], Names(browser));
        Assert.True(shared.Dims(new Entry(hidden, EntryKind.File)));

        // Shown wins over the eye: the rule is not one of the rules the eye is showing.
        Assert.True(hides.SetState("dot_file", HideState.Show));

        Assert.False(shared.Dims(new Entry(hidden, EntryKind.File)));
        Assert.True(shared.Dims(new Entry(hiddenDirectory, EntryKind.Directory)));
    }

    /// <summary>What a listing shows, by name, in the order it shows it.</summary>
    /// <param name="browser">The location whose listing is read.</param>
    private static string[] Names(global::FileSystemPlugin.Browser browser) =>
        browser.Shown.Select(entry => Path.GetFileName(entry.Path)).ToArray();
}

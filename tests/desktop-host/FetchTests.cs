using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.VisualTree;
using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The fetch: what a run of it is told, and what its card is drawn from when it says nothing of itself.
/// </summary>
/// <remarks>
/// The exchange itself is the command line's and is not run here, a run needs a Vault and a network, so what is
/// checked is everything this plugin decides: the command a refresh runs, and that a fetch asks for no progress
/// records, which is what has its card drawn as one still moving rather than as a bar to fill.
/// </remarks>
public sealed class FetchTests
{
    /// <summary>A fetch is the command the line names, asked for the theme this window reads.</summary>
    /// <remarks>
    /// The word is <c>fetch</c> rather than <c>layout fetch</c>: the two are the same command, and the shorter name
    /// is what the line was given. The theme is the one every run from this window asks for, so that what it says
    /// is what the window can draw.
    /// </remarks>
    [Fact]
    public void AFetchIsTheCommandTheLineNames()
    {
        var arguments = Fetch.Arguments();

        Assert.Contains("fetch", arguments);
        Assert.DoesNotContain("layout", arguments);
        Assert.Single(arguments, argument => argument.StartsWith("--theme-choice=", StringComparison.Ordinal));
    }

    /// <summary>A foreground change marks the copy behind, and does not start a fetch of its own.</summary>
    [Fact]
    public void AForegroundChangeMarksTheCopyBehind()
    {
        var services = Host.Services();
        var fetch = new VaultFetch(
            services.For(new PluginId("it.fetch"), 1),
            services.Progress
        );

        Assert.Equal(FetchState.Never, fetch.State);
        Assert.False(fetch.Dirty);

        var read = false;

        fetch.MarkDirty("/work", () => read = true);

        Assert.True(fetch.Dirty);
        Assert.Equal(FetchState.Never, fetch.State);
        Assert.False(read);
    }

    /// <summary>Only the leg to the Vault failing is taken as the Vault being unreachable.</summary>
    /// <remarks>
    /// A fetch that failed because no account is here or no Vault is tracked says nothing about the leg, so a
    /// write is not opened offline on it: that would hide a failure the reader can act on.
    /// </remarks>
    [Fact]
    public void OnlyTheLegToTheVaultIsTakenAsUnreachable()
    {
        Assert.Equal(Reachability.Reached, VaultFetch.Reach(done: true, code: 0));
        Assert.Equal(
            Reachability.Unreachable,
            VaultFetch.Reach(done: false, Fetch.Transport)
        );
        Assert.Equal(Reachability.Unknown, VaultFetch.Reach(done: false, code: 0));
        Assert.Equal(Reachability.Unknown, VaultFetch.Reach(done: false, code: 1));
    }

    /// <summary>A fetch that has said nothing yet leaves runs that may reach the Vault online.</summary>
    [Fact]
    public void NothingIsOfflineBeforeTheFirstFetch()
    {
        var services = Host.Services();
        var fetch = new VaultFetch(
            services.For(new PluginId("it.fetch.offline"), 1),
            services.Progress
        );

        Assert.Equal(FetchState.Never, fetch.State);
        Assert.False(fetch.Offline);
    }

    /// <summary>An unchanged copy fingerprints the same, and a changed rule or pointer does not.</summary>
    /// <remarks>
    /// The rules are written whole on every fetch, so their bytes are what says whether they changed; the
    /// Layout's copy moves by moving its pointer, so the pointer's own line is what says the copy became
    /// another one.
    /// </remarks>
    [Fact]
    public void AnUnchangedCopyFingerprintsTheSame()
    {
        var layouts = Scratch.New("fetch-layouts");
        var rules = Scratch.New("fetch-rules");
        var pointer = Path.Combine(layouts, "origin", "truth", "CURRENT");
        var rule = Path.Combine(rules, "vaults", "origin", "maya");

        Directory.CreateDirectory(Path.GetDirectoryName(pointer)!);
        Directory.CreateDirectory(Path.GetDirectoryName(rule)!);
        File.WriteAllText(pointer, "g-1\n");
        File.WriteAllText(rule, "*.mb\n");

        var before = VaultFetch.Fingerprint(layouts, rules);

        Assert.Equal(before, VaultFetch.Fingerprint(layouts, rules));

        File.WriteAllText(rule, "*.ma\n");
        Assert.NotEqual(before, VaultFetch.Fingerprint(layouts, rules));

        var afterRules = VaultFetch.Fingerprint(layouts, rules);

        File.WriteAllText(pointer, "g-2\n");
        Assert.NotEqual(afterRules, VaultFetch.Fingerprint(layouts, rules));
    }

    /// <summary>The status bar names the Vault in capitals, in bold, and shows the address beside it.</summary>
    [Fact]
    public void TheStatusBarNamesTheVaultAndItsAddress()
    {
        var bar = new FetchStatusBar();

        bar.Show(FetchState.Ready, "origin", "rola://127.0.0.1/rainbow");

        Assert.Equal(35, bar.Height);
        Assert.Equal(new Thickness(0, 1, 0, 0), bar.BorderThickness);

        var texts = bar.GetVisualDescendants().OfType<TextBlock>().ToList();
        var name = Assert.Single(texts, text => text.Text == "ORIGIN");

        Assert.Equal(FontWeight.Bold, name.FontWeight);
        Assert.Single(texts, text => text.Text == "|");
        Assert.Single(texts, text => text.Text == "rola://127.0.0.1/rainbow");
    }
}

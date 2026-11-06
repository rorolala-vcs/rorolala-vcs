using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Taking an entry out of a Vault's Layout: what the place offers to remove, and what the command is run with.
/// </summary>
/// <remarks>
/// An entry of a Vault's copy is a file the Vault holds, and the account that holds it may let it go, which is the one
/// row of a place that is a deletion rather than a suggestion about the work. What is checked here is the judgement the
/// place draws from what it read, and the run it makes: neither the command line nor a Vault is here to be run against.
/// </remarks>
public sealed class UpstreamRemoveTests
{
    /// <summary>A row of a Vault's copy is removable by the account that holds it, and by no other.</summary>
    [Fact]
    public void AFileTheVaultHoldsForThisAccountMayBeRemoved()
    {
        var place = Place();
        var before = Acting("alice");

        try
        {
            Assert.True(
                place.Removable(
                    new UpstreamCarry(
                        Origin,
                        "Assets/hero.psd",
                        new LayoutEntry("Assets/hero.psd", "a", "v", "alice", string.Empty, false)
                    )
                )
            );

            // An entry another account holds is that account's: the command refuses the removal, and the place offers
            // nothing it knows will be refused.
            Assert.False(
                place.Removable(
                    new UpstreamCarry(
                        Origin,
                        "Assets/other.psd",
                        new LayoutEntry("Assets/other.psd", "b", "v", "bob", string.Empty, false)
                    )
                )
            );

            // A row of the work's own Layout is not the Vault's, whatever an owner says: the work itself is not removed
            // by a suggestion about the upstream.
            Assert.False(
                place.Removable(
                    new UpstreamCarry(
                        new UpstreamSource(UpstreamPlace.Layouts, "work"),
                        "Assets/hero.psd",
                        new LayoutEntry("Assets/hero.psd", "a", "v", "alice", string.Empty, false)
                    )
                )
            );

            // A row that names no entry is a folder of the listing, and nothing of the Vault's is there to take away.
            Assert.False(place.Removable(new UpstreamCarry(Origin, "Assets")));
        }
        finally
        {
            _ = Acting(before);
        }
    }

    /// <summary>A removal is run against the Layout the place names, not against the one being worked in.</summary>
    /// <remarks>
    /// The copy is named so that the run writes the Vault's own Layout: a run without the name would unbind a path of the
    /// Layout being worked in, which is a different write to a different file.
    /// </remarks>
    [Fact]
    public void ARemovalNamesTheLayoutItIsIn()
    {
        var named = LayoutEntryRemoveCommand.Arguments(
            LayoutPathMoveCommand.Named("origin"),
            "Assets/hero.psd"
        );

        Assert.Equal(
            ["--theme-choice=" + Sync.Theme, "--json", "layout", "path", "remove", "--layout", "truth@origin", "Assets/hero.psd"],
            named
        );

        var local = LayoutEntryRemoveCommand.Arguments(null, "Assets/hero.psd");

        Assert.Equal(
            ["--theme-choice=" + Sync.Theme, "--json", "layout", "path", "remove", "Assets/hero.psd"],
            local
        );
    }

    /// <summary>Names the account the work acts as, and answers with the one it named before.</summary>
    /// <param name="name">The account to name.</param>
    /// <returns>The account named before, or an empty string when none was.</returns>
    private static string Acting(string name)
    {
        var directory = Path.Combine(DataHome.Root, "rola");
        var file = Path.Combine(directory, "user");
        var before = File.Exists(file) ? File.ReadAllText(file).Trim() : string.Empty;

        Directory.CreateDirectory(directory);
        File.WriteAllText(file, name);

        return before;
    }

    /// <summary>A place to check, which reads nothing and starts nothing.</summary>
    /// <returns>The place.</returns>
    private static UpstreamPlace Place()
    {
        var services = Host.Services();
        var host = services.For(new PluginId("rorolala.vcs"), 1);
        var hub = new ProgressHub();

        return new UpstreamPlace(host, host.Log, hub, new VaultFetch(host, hub));
    }

    /// <summary>The Vault `origin`'s copy, for these checks to plan removals of.</summary>
    private static UpstreamSource Origin => new(UpstreamPlace.Vaults, "origin");
}

using RorolalaDesktop.Progress;
using FileSystemPlugin;
using Rorolala;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Taking an entry out of a Vault's Layout: what the place offers to archive, and what it offers to remove for good.
/// </summary>
/// <remarks>
/// An entry of a Vault's copy is a file the Vault holds. Archiving it keeps the entry and needs no holder, and what
/// it is asked for below is the other removal: letting it go is the holder's, which is the one row of a place that
/// is a loss rather than a suggestion about the work. What is checked here is the judgement the place draws from what
/// it read. Neither the command line nor a Vault is here to be run against, and the write the judgement leads to goes
/// through the writer, which <see cref="LayoutWriterCommandTests"/> checks.
/// </remarks>
public sealed class UpstreamRemoveTests
{
    /// <summary>A row of a Vault's copy is archived by anyone, and removed for good by the account that holds it.</summary>
    [Fact]
    public void AFileTheVaultHoldsForThisAccountMayBeRemoved()
    {
        var place = Place();
        var before = Acting("alice");

        try
        {
            var mine = new UpstreamCarry(
                Origin,
                "Assets/hero.psd",
                new LayoutEntry("Assets/hero.psd", "a", "v", "alice", string.Empty, false)
            );

            Assert.True(place.Removable(mine));
            Assert.True(place.Erasable(mine));

            // The holder's alone is the removal that cannot be taken back. The archive keeps the entry, so it asks
            // for no holder, and the command allows it for any account that may write the Vault's Layout.
            var theirs = new UpstreamCarry(
                Origin,
                "Assets/other.psd",
                new LayoutEntry("Assets/other.psd", "b", "v", "bob", string.Empty, false)
            );

            Assert.True(place.Removable(theirs));
            Assert.False(place.Erasable(theirs));

            // A row of the work's own Layout is not the Vault's, whatever an owner says: the work itself is not removed
            // by a suggestion about the upstream.
            var work = new UpstreamCarry(
                new UpstreamSource(UpstreamPlace.Layouts, "work"),
                "Assets/hero.psd",
                new LayoutEntry("Assets/hero.psd", "a", "v", "alice", string.Empty, false)
            );

            Assert.False(place.Removable(work));
            Assert.False(place.Erasable(work));

            // A row that names no entry is a folder of the listing, and nothing of the Vault's is there to take away.
            Assert.False(place.Removable(new UpstreamCarry(Origin, "Assets")));
        }
        finally
        {
            _ = Acting(before);
        }
    }

    /// <summary>Names the account the work acts as, and answers with the one it named before.</summary>
    /// <param name="name">The account to name.</param>
    /// <returns>The account named before, or an empty string when none was.</returns>
    private static string Acting(string name)
    {
        var directory = UserDataPaths.Root;
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

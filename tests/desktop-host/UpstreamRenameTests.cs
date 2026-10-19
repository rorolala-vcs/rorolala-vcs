using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What may be renamed in the upstream place, and what a rename of it comes to.
/// </summary>
/// <remarks>
/// A name of the work's own is the account's that holds it, so the two questions are one: who holds every entry at the path
/// and under it, and what the paths become. Both are questions about the copy the place reads, answered without a run.
/// </remarks>
public sealed class UpstreamRenameTests
{
    /// <summary>A folder keeps what is under it when it is renamed, and only the prefix changes.</summary>
    [Fact]
    public void ARenamedFolderCarriesWhatIsUnderIt()
    {
        var tree = Tree();

        var moves = UpstreamPlace.Renaming(tree, new UpstreamCarry(Origin, "A"), "B");

        // The folder's own entry and everything under it, each keeping what it has below the folder. A level reads folders
        // first, so the folder of a folder is walked before the file beside it.
        Assert.Equal(["A", "A/sub/d.fbx", "A/c.psd"], moves.Select(move => move.From));
        Assert.Equal(["B", "B/sub/d.fbx", "B/c.psd"], moves.Select(move => move.To));
        Assert.Equal(["me", "me", "me"], moves.Select(move => move.Holder));
    }

    /// <summary>A file renamed takes a name in its own folder, and nothing beside it moves.</summary>
    [Fact]
    public void ARenamedFileChangesOnlyItsName()
    {
        var tree = Tree();

        var moves = UpstreamPlace.Renaming(tree, new UpstreamCarry(Origin, "A/c.psd"), "A/e.psd");

        var move = Assert.Single(moves);

        Assert.Equal("A/c.psd", move.From);
        Assert.Equal("A/e.psd", move.To);
    }

    /// <summary>A name already taken is not a move, which is what the copy says rather than what the run says.</summary>
    [Fact]
    public void ARenameOntoANameThatIsTakenMovesNothing()
    {
        var tree = Tree();

        // The name it has is not a name to be given: what a rename to the same name comes to is no move at all.
        Assert.Empty(UpstreamPlace.Renaming(tree, new UpstreamCarry(Origin, "A"), "A"));
        Assert.Empty(UpstreamPlace.Renaming(tree, new UpstreamCarry(Origin, "A/c.psd"), "A/c.psd"));
    }

    /// <summary>A name another folder of this side's already has is refused, and both keep the names they had.</summary>
    [Fact]
    public void ANameAnotherFolderHasIsRefused()
    {
        var place = Place();
        var inNew = UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "new");

        Assert.True(place.NewFolder(inNew));
        Assert.True(place.NewFolder(inNew));

        var names = place.DraftPaths.OrderBy(path => path, StringComparer.Ordinal).ToArray();
        var first = names[0];
        var second = names[1];

        Assert.False(
            place.Rename(
                inNew,
                new UpstreamCarry(Origin, first),
                second[(second.LastIndexOf('/') + 1)..]
            )
        );

        Assert.Equal(names, place.DraftPaths.OrderBy(path => path, StringComparer.Ordinal));
    }

    /// <summary>Only what the account holds is its to name: the whole of it, and never what nobody holds.</summary>
    [Fact]
    public void OnlyWhatTheAccountHoldsIsItsToName()
    {
        var tree = Tree();

        Assert.True(UpstreamPlace.Held(tree, new UpstreamCarry(Origin, "A"), "me"));
        Assert.True(UpstreamPlace.Held(tree, new UpstreamCarry(Origin, "A/c.psd"), "me"));

        // One entry of the folder is another's, and a folder is only what stands under it: the Vault would refuse the folder
        // whole, so it is not offered.
        Assert.False(UpstreamPlace.Held(tree, new UpstreamCarry(Origin, "B"), "me"));

        // An entry nobody holds has no owner to match, and an account that is not known yet names nothing.
        Assert.False(UpstreamPlace.Held(tree, new UpstreamCarry(Origin, "Loose.psd"), "me"));
        Assert.False(UpstreamPlace.Held(tree, new UpstreamCarry(Origin, "A"), string.Empty));

        // A path the copy does not name is not a row, and holds nothing.
        Assert.False(UpstreamPlace.Held(tree, new UpstreamCarry(Origin, "gone.psd"), "me"));
    }

    /// <summary>The copy these checks rename in.</summary>
    /// <returns>The tree.</returns>
    private static UpstreamEntriesTree Tree() =>
        UpstreamEntriesTree.Build(
            [
                Entry("A", "a", "me"),
                Entry("A/c.psd", "b", "me"),
                Entry("A/sub/d.fbx", "c", "me"),
                Entry("B/e.psd", "d", "you"),
                Entry("B/f.psd", "e", "me"),
                Entry("Loose.psd", "f", string.Empty),
            ],
            []
        );

    /// <summary>An entry as the copy names it.</summary>
    /// <param name="path">The path.</param>
    /// <param name="uuid">The uuid it is known by.</param>
    /// <param name="owner">The account that holds it, empty when none does.</param>
    /// <returns>The entry.</returns>
    private static LayoutEntry Entry(string path, string uuid, string owner) =>
        new(path, uuid, string.Empty, owner, string.Empty, false);

    /// <summary>The Vault `origin`'s copy.</summary>
    private static UpstreamSource Origin => new(UpstreamPlace.Vaults, "origin");

    /// <summary>A place to check, which reads nothing and starts nothing.</summary>
    /// <returns>The place.</returns>
    private static UpstreamPlace Place()
    {
        var services = Host.Services();
        var host = services.For(new PluginId("rorolala.vcs"), 1);
        var hub = new ProgressHub();

        return new UpstreamPlace(host, host.Log, hub, new VaultFetch(host, hub));
    }
}

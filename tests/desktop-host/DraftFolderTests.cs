using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The empty folders a place holds: made, named, moved and removed, and gone when nothing was put in them.
/// </summary>
/// <remarks>
/// A place reads a copy of the Vault's Layout, so a folder this side made is not in anything: what is checked here is
/// what the place does with one — a name it gives, a name it takes, a path that carries what is under it, and a folder
/// that is let go of whole. Nothing is written anywhere, which is the point: the folder is a suggestion about the work.
/// </remarks>
public sealed class DraftFolderTests
{
    /// <summary>A folder made in a place is the place's own, and only its own rows are the place's to name.</summary>
    [Fact]
    public void AFolderMadeInAPlaceIsThePlacesOwn()
    {
        var place = Place();
        var source = Origin;
        var inNew = UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "new");

        Assert.True(place.NewFolder(inNew));

        var made = Assert.Single(place.DraftPaths);

        Assert.StartsWith("new/", made, StringComparison.Ordinal);
        Assert.True(place.Renamable(new UpstreamCarry(source, made)));
        Assert.True(place.Removable(new UpstreamCarry(source, made)));

        // A row the work names is not something the place may name: it is the Vault's, and a place that edited it would
        // be saying the work had a name it does not have.
        Assert.False(place.Renamable(new UpstreamCarry(source, "Assets/hero.psd", new LayoutEntry("Assets/hero.psd", "a", string.Empty, string.Empty, string.Empty, false))));
        Assert.False(place.Removable(new UpstreamCarry(source, "Assets/hero.psd", new LayoutEntry("Assets/hero.psd", "a", string.Empty, string.Empty, string.Empty, false))));
    }

    /// <summary>A folder keeps what is under it when it is renamed, and lets go of it when it is removed.</summary>
    [Fact]
    public void ARenamedFolderCarriesWhatIsUnderIt()
    {
        var place = Place();
        var source = Origin;

        Assert.True(place.NewFolder(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "new")));

        var outer = Assert.Single(place.DraftPaths);

        // A folder made inside the one just made, which is where nesting comes from: a path under another path.
        Assert.True(
            place.NewFolder(
                UpstreamPlace.Of(
                    UpstreamPlace.Vaults,
                    "origin",
                    "new",
                    outer[(outer.LastIndexOf('/') + 1)..]
                )
            )
        );

        Assert.Equal(2, place.DraftPaths.Count);

        var inner = place.DraftPaths.Single(path => !string.Equals(path, outer, StringComparison.Ordinal));

        Assert.True(
            place.Rename(
                UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "new"),
                new UpstreamCarry(source, outer),
                "Shots"
            )
        );

        // The folder took the name it was given, and the one under it moved with it.
        Assert.Contains("new/Shots", place.DraftPaths);
        Assert.Contains("new/Shots/" + inner[(inner.LastIndexOf('/') + 1)..], place.DraftPaths);
        Assert.DoesNotContain(outer, place.DraftPaths);
        Assert.DoesNotContain(inner, place.DraftPaths);

        // What it holds goes with it: a folder taken away with its children left standing would be children of nothing.
        Assert.True(
            place.Remove(
                UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "new"),
                new UpstreamCarry(source, "new/Shots")
            )
        );
        Assert.Empty(place.DraftPaths);
    }

    /// <summary>A name the place cannot give is refused, and the folder keeps the one it had.</summary>
    [Fact]
    public void ANameAPathCannotTakeIsRefused()
    {
        var place = Place();
        var source = Origin;

        Assert.True(place.NewFolder(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin")));

        var made = Assert.Single(place.DraftPaths);
        var inLayout = UpstreamPlace.Of(UpstreamPlace.Vaults, "origin");

        foreach (var name in new[] { string.Empty, " ", "a/b", ".", ".." })
        {
            Assert.False(
                place.Rename(inLayout, new UpstreamCarry(source, made), name),
                $"`{name}` was taken as a name"
            );
        }

        Assert.Equal(made, Assert.Single(place.DraftPaths));
    }

    /// <summary>A place over the work's own Layouts has no folders of its own, and makes none.</summary>
    [Fact]
    public void ALayoutOfTheWorkHasNoFoldersOfItsOwn()
    {
        var place = Place();

        Assert.False(place.NewFolder(UpstreamPlace.Of(UpstreamPlace.Layouts, "work")));
        Assert.Empty(place.DraftPaths);
    }

    /// <summary>A folder moved in a place moves as the tree it is, and keeps the paths it has below it.</summary>
    [Fact]
    public void AFolderMovedInAPlaceMovesWhatIsUnderIt()
    {
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("A", "a"),
                Entry("A/c.psd", "b"),
                Entry("A/sub/d.fbx", "c"),
                Entry("Elsewhere.psd", "d"),
            ],
            []
        );

        var moves = UpstreamPlace.Moving(tree, [new UpstreamCarry(Origin, "A")], "B");

        // The folder's own entry and everything under it, each keeping what it has below the folder: a folder cut and
        // pasted is a folder moved, which is what the file browser does with one.
        // A level reads folders first, so the folder of a folder is walked before the file beside it.
        Assert.Equal(["A", "A/sub/d.fbx", "A/c.psd"], moves.Select(move => move.From));
        Assert.Equal(["B/A", "B/A/sub/d.fbx", "B/A/c.psd"], moves.Select(move => move.To));

    }

    /// <summary>An entry as the copy names it, with a path and the `Uuid` it is known by.</summary>
    /// <param name="path">The path.</param>
    /// <param name="uuid">The `Uuid`.</param>
    /// <returns>The entry.</returns>
    private static LayoutEntry Entry(string path, string uuid) =>
        new(path, uuid, string.Empty, string.Empty, string.Empty, false);

    /// <summary>A place to check, which reads nothing and starts nothing.</summary>
    /// <returns>The place.</returns>
    private static UpstreamPlace Place()
    {
        var services = Host.Services();
        var host = services.For(new PluginId("rorolala.vcs"), 1);
        var hub = new ProgressHub();

        return new UpstreamPlace(host, host.Log, hub, new VaultFetch(host, hub));
    }

    /// <summary>The Vault `origin`'s copy, for these checks to plan drags of.</summary>
    private static UpstreamSource Origin => new(UpstreamPlace.Vaults, "origin");
}

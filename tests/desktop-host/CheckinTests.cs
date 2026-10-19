using FileSystemPlugin;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A checkin worked out before anything is run: which entries can come in, which are already here, which are here
/// under another name, and which cannot land at all.
/// </summary>
/// <remarks>
/// The command line refuses a run whole for the first reference it will not take, so a batch that half fits is a batch
/// that would do nothing at all. What is checked here is the judgement made before the run — where each entry would
/// land, and what is already in the way — because that judgement is what the question to the user is built from.
/// The runs themselves are checked beside this, as the arguments they are given.
/// </remarks>
public sealed class CheckinTests
{
    /// <summary>Every entry of a folder is placed against what this side holds, at its own path or another.</summary>
    [Fact]
    public void ACheckinIsPlannedAgainstWhatIsAlreadyHere()
    {
        LayoutEntry[] local =
        [
            Entry("imported.fbx", "b"),
            Entry("A/sub/c.fbx", "c"),
            Entry("A/d.fbx", "x"),
        ];
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("A/d.fbx", "d"),
                Entry("A/sub/b.fbx", "b"),
                Entry("A/sub/c.fbx", "c"),
                Entry("A/sub/e.psd", "e"),
            ],
            local
        );

        var plan = CheckinPlanning.Of(
            tree,
            local,
            Path.GetTempPath(),
            destination: null,
            "origin",
            [new UpstreamCarry(Origin, "A")]
        );

        // The one nothing is in the way of is clean, the one this side checked in under an older name can be moved
        // over, the one named where the Vault names it is already here, and the path another entry occupies cannot land
        // at all.
        Assert.Equal(["A/sub/e.psd"], plan.Clean.Select(step => step.Landing));
        Assert.Equal(["A/sub/b.fbx"], plan.Elsewhere.Select(step => step.Landing));
        Assert.Equal("imported.fbx", plan.Elsewhere[0].Local);
        Assert.Equal(1, plan.Here);
        Assert.Equal(["A/d.fbx"], plan.Taken.Select(step => step.Landing));

        // Three of the four are left out of a run as it stands, which is what the question says and what a partial
        // checkin is counted by.
        Assert.True(plan.Blocks);
        Assert.Equal(3, plan.Left);
        Assert.Equal(["A/sub/e.psd"], plan.References());
        Assert.Equal("origin", plan.Vault);
    }

    /// <summary>A plan that fits asks nothing: everything is clean and a run is made of the lot.</summary>
    [Fact]
    public void ACheckinThatFitsAsksNothing()
    {
        LayoutEntry[] local = [];
        var tree = UpstreamEntriesTree.Build(
            [Entry("Assets/hero.psd", "b"), Entry("Assets/bg.psd", "c")],
            local
        );

        var plan = CheckinPlanning.Of(
            tree,
            local,
            Path.GetTempPath(),
            destination: null,
            "origin",
            [new UpstreamCarry(Origin, "Assets")]
        );

        Assert.False(plan.Blocks);
        Assert.Equal(0, plan.Left);
        Assert.Equal(
            ["Assets/bg.psd", "Assets/hero.psd"],
            plan.References()
        );
    }

    /// <summary>A drop on a directory keeps the Vault's paths under it, so the folder structure comes with it.</summary>
    [Fact]
    public void AnEntryKeepsItsPathUnderTheDirectoryItWasDroppedOn()
    {
        var root = Path.GetTempPath();
        var under = Path.Combine(root, "art");

        LayoutEntry[] local = [];
        var tree = UpstreamEntriesTree.Build([Entry("Assets/hero.psd", "b")], local);
        var carrying = new[] { new UpstreamCarry(Origin, "Assets/hero.psd") };

        var landed = CheckinPlanning.Of(tree, local, root, under, "origin", carrying);

        // The destination is a prefix: `Assets/` is remade under it, which is what makes a folder dragged in the tree
        // it is rather than its files alone.
        Assert.Equal("art/Assets/hero.psd", landed.Clean[0].Landing);
        Assert.Equal(under, landed.Destination);

        // A directory that is the root itself adds no segment, which is the same place the Vault's own path is relative to.
        var atRoot = CheckinPlanning.Of(tree, local, root, root, "origin", carrying);

        Assert.Equal("Assets/hero.psd", atRoot.Clean[0].Landing);
    }

    /// <summary>A file already on disk where an entry would land is what stops it landing.</summary>
    [Fact]
    public void AFileOnDiskTakesThePlaceAnEntryWouldLandIn()
    {
        var root = Path.Combine(Path.GetTempPath(), "rorolala-checkin-" + Guid.NewGuid());
        var held = Path.Combine(root, "Assets");

        Directory.CreateDirectory(held);
        File.WriteAllText(Path.Combine(held, "hero.psd"), "already drawn");

        try
        {
            LayoutEntry[] local = [];
            var tree = UpstreamEntriesTree.Build([Entry("Assets/hero.psd", "b")], local);

            var plan = CheckinPlanning.Of(
                tree,
                local,
                root,
                destination: null,
                "origin",
                [new UpstreamCarry(Origin, "Assets/hero.psd")]
            );

            // The command line would refuse the run for the file being there, and moving a local file out of the way is
            // not what would settle it: what the user is offered is leaving it out or giving the checkin up.
            Assert.Equal(["Assets/hero.psd"], plan.Taken.Select(step => step.Landing));
            Assert.Empty(plan.Clean);
            Assert.True(plan.Blocks);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    /// <summary>A path that is a file and a directory at once is one of the things a checkin cannot land.</summary>
    [Fact]
    public void AnEntryThatWouldBeAFileAndAFolderAtOnceIsTaken()
    {
        LayoutEntry[] local = [];
        var tree = UpstreamEntriesTree.Build(
            [Entry("A", "a"), Entry("A/b.fbx", "b")],
            local
        );

        var plan = CheckinPlanning.Of(
            tree,
            local,
            Path.GetTempPath(),
            destination: null,
            "origin",
            [new UpstreamCarry(Origin, "A")]
        );

        // `A` cannot be the file the entry is and the directory the other entry is under, so neither lands by itself: the
        // run is asked about rather than started and left half written.
        Assert.All(plan.Steps, step => Assert.Equal(CheckinBlocker.Taken, step.Blocker));
        Assert.Empty(plan.Clean);
        Assert.True(plan.Blocks);
    }

    /// <summary>A folder cut out of the upstream and pasted into the work comes back as the folder it was.</summary>
    /// <remarks>
    /// A cut is a drag that waits: the clipboard carries the folder's row, and the paste is the same plan a drop of it
    /// makes. What it must not be is the folder's files alone — cutting `A` and pasting into the root is the root gaining
    /// an `A`, which is the tree under it rather than three names beside each other.
    /// </remarks>
    [Fact]
    public void ACutFolderPastedAtTheRootBringsTheFolderItself()
    {
        var root = Path.GetTempPath();

        LayoutEntry[] local = [];
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("A/c.psd", "c"),
                Entry("A/sub/b.fbx", "b"),
            ],
            local
        );

        var plan = CheckinPlanning.Of(
            tree,
            local,
            root,
            root,
            "origin",
            [new UpstreamCarry(Origin, "A")]
        );

        Assert.Equal(["A/sub/b.fbx", "A/c.psd"], plan.Clean.Select(step => step.Landing));

        // Pasted into a directory of the work rather than at its root, the same folder is made under it.
        var under = Path.Combine(root, "art");
        var inside = CheckinPlanning.Of(
            tree,
            local,
            root,
            under,
            "origin",
            [new UpstreamCarry(Origin, "A")]
        );

        Assert.Equal(
            ["art/A/sub/b.fbx", "art/A/c.psd"],
            inside.Clean.Select(step => step.Landing)
        );
    }

    /// <summary>What the Vault holds under its unplaced mark lands by the name below the mark.</summary>
    /// <remarks>
    /// `@/new/a.fbx` is an entry the Vault holds no path for yet, so pasting `@/new` into the work makes `new/`, not
    /// `@/new/`: the mark is the Vault's bookkeeping and a directory called `@` would be a decision this side made for
    /// it. An entry already placed here under the name below the mark is recognised as the same entry, not a new one.
    /// </remarks>
    [Fact]
    public void AnEntryUnderTheUnplacedMarkLandsByItsNameBelowIt()
    {
        var root = Path.GetTempPath();

        LayoutEntry[] local = [Entry("new/sub/b.fbx", "b")];
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("@/new/a.fbx", "a"),
                Entry("@/new/sub/b.fbx", "b"),
            ],
            local
        );

        var plan = CheckinPlanning.Of(
            tree,
            local,
            root,
            root,
            "origin",
            [new UpstreamCarry(Origin, "@/new")]
        );

        Assert.Equal(
            ["new/a.fbx"],
            plan.Clean.Select(step => step.Landing).Order(StringComparer.Ordinal)
        );
        Assert.Equal(1, plan.Here);

        var under = Path.Combine(root, "art");
        var inside = CheckinPlanning.Of(
            tree,
            local,
            root,
            under,
            "origin",
            [new UpstreamCarry(Origin, "@/new")]
        );

        Assert.Equal(
            ["art/new/a.fbx"],
            inside.Clean.Select(step => step.Landing).Order(StringComparer.Ordinal)
        );
    }

    /// <summary>A move of a local copy is made in the Layout being worked in unless another one is named.</summary>
    [Fact]
    public void ALocalMoveNamesNoLayoutAndOneOfTheWorksNamesOne()
    {
        var local = LayoutPathMoveCommand.Arguments(null, "imported.fbx", "A/b.fbx");

        Assert.Contains("move", local);
        Assert.DoesNotContain("--layout", local);
        Assert.Equal("imported.fbx", local[^2]);
        Assert.Equal("A/b.fbx", local[^1]);

        // A Layout the place is a view of is named, which is the same run against a Layout that is not the one on screen.
        var named = LayoutPathMoveCommand.Arguments("work", "Assets/hero.psd", "Assets/hero-moved.psd");

        Assert.Contains("--layout", named);
        Assert.Contains("work", named);
        Assert.Equal("Assets/hero-moved.psd", named[^1]);
    }

    /// <summary>An entry as the copy names it, with a path and the `Uuid` it is known by.</summary>
    /// <param name="path">The path.</param>
    /// <param name="uuid">The `Uuid`.</param>
    /// <returns>The entry.</returns>
    private static LayoutEntry Entry(string path, string uuid) =>
        new(path, uuid, string.Empty, string.Empty, string.Empty, false);

    /// <summary>The Vault `origin`'s copy, for these checks to plan drags of.</summary>
    private static UpstreamSource Origin => new(UpstreamPlace.Vaults, "origin");
}

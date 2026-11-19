using System.Diagnostics;

using FileSystemPlugin;
using RolaSharp;
using RorolalaVCSPlugin;
using LayoutEntry = RorolalaVCSPlugin.LayoutEntry;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A bring worked out before anything is run: which entries can come in, which are already here, which are here
/// under another name, and which cannot land at all.
/// </summary>
/// <remarks>
/// The command line refuses a run whole for the first reference it will not take, so a batch that half fits is a batch
/// that would do nothing at all. What is checked here is the judgement made before the run, where each entry would
/// land, and what is already in the way, because that judgement is what the question to the user is built from.
/// The move of a local copy is run here as well, since it is the writer's and needs nothing but a Workspace.
/// </remarks>
public sealed class BringTests
{
    /// <summary>Every entry of a folder is placed against what this side holds, at its own path or another.</summary>
    [Fact]
    public void ABringIsPlannedAgainstWhatIsAlreadyHere()
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

        var plan = BringPlanning.Of(
            tree,
            local,
            Path.GetTempPath(),
            destination: null,
            "origin",
            [new UpstreamCarry(Origin, "A")]
        );

        // The one nothing is in the way of is clean, the one this side brought in under an older name can be moved
        // over, the one named where the Vault names it is already here, and the path another entry occupies cannot land
        // at all.
        Assert.Equal(["A/sub/e.psd"], plan.Clean.Select(step => step.Landing));
        Assert.Equal(["A/sub/b.fbx"], plan.Elsewhere.Select(step => step.Landing));
        Assert.Equal("imported.fbx", plan.Elsewhere[0].Local);
        Assert.Equal(1, plan.Here);
        Assert.Equal(["A/d.fbx"], plan.Taken.Select(step => step.Landing));

        // Three of the four are left out of a run as it stands, which is what the question says and what a partial
        // bring is counted by.
        Assert.True(plan.Blocks);
        Assert.Equal(3, plan.Left);
        Assert.Equal(["A/sub/e.psd"], plan.References());
        Assert.Equal("origin", plan.Vault);
    }

    /// <summary>A plan that fits asks nothing: everything is clean and a run is made of the lot.</summary>
    [Fact]
    public void ABringThatFitsAsksNothing()
    {
        LayoutEntry[] local = [];
        var tree = UpstreamEntriesTree.Build(
            [Entry("Assets/hero.psd", "b"), Entry("Assets/bg.psd", "c")],
            local
        );

        var plan = BringPlanning.Of(
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

        // What is carried is the folder, not the file: a folder dragged in is the folder, remade where
        // it was put, with the tree it holds under it.
        var carrying = new[] { new UpstreamCarry(Origin, "Assets") };

        var landed = BringPlanning.Of(tree, local, root, under, "origin", carrying);

        // The destination is a prefix: `Assets/` is remade under it, which is what makes a folder dragged in the tree
        // it is rather than its files alone.
        Assert.Equal("art/Assets/hero.psd", landed.Clean[0].Landing);
        Assert.Equal(under, landed.Destination);

        // A directory that is the root itself adds no segment, which is the same place the Vault's own path is relative to.
        var atRoot = BringPlanning.Of(tree, local, root, root, "origin", carrying);

        Assert.Equal("Assets/hero.psd", atRoot.Clean[0].Landing);
    }

    /// <summary>A file already on disk where an entry would land is what stops it landing.</summary>
    [Fact]
    public void AFileOnDiskTakesThePlaceAnEntryWouldLandIn()
    {
        var root = Path.Combine(Path.GetTempPath(), "rorolala-bring-" + Guid.NewGuid());
        var held = Path.Combine(root, "Assets");

        Directory.CreateDirectory(held);
        File.WriteAllText(Path.Combine(held, "hero.psd"), "already drawn");

        try
        {
            LayoutEntry[] local = [];
            var tree = UpstreamEntriesTree.Build([Entry("Assets/hero.psd", "b")], local);

            var plan = BringPlanning.Of(
                tree,
                local,
                root,
                destination: null,
                "origin",
                [new UpstreamCarry(Origin, "Assets/hero.psd")]
            );

            // The command line would refuse the run for the file being there, and moving a local file out of the way is
            // not what would settle it: what the user is offered is leaving it out or giving the bring up.
            Assert.Equal(["Assets/hero.psd"], plan.Taken.Select(step => step.Landing));
            Assert.Empty(plan.Clean);
            Assert.True(plan.Blocks);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    /// <summary>A path that is a file and a directory at once is one of the things a bring cannot land.</summary>
    [Fact]
    public void AnEntryThatWouldBeAFileAndAFolderAtOnceIsTaken()
    {
        LayoutEntry[] local = [];
        var tree = UpstreamEntriesTree.Build(
            [Entry("A", "a"), Entry("A/b.fbx", "b")],
            local
        );

        var plan = BringPlanning.Of(
            tree,
            local,
            Path.GetTempPath(),
            destination: null,
            "origin",
            [new UpstreamCarry(Origin, "A")]
        );

        // `A` cannot be the file the entry is and the directory the other entry is under, so neither lands by itself: the
        // run is asked about rather than started and left half written.
        Assert.All(plan.Steps, step => Assert.Equal(BringBlocker.Taken, step.Blocker));
        Assert.Empty(plan.Clean);
        Assert.True(plan.Blocks);
    }

    /// <summary>A folder cut out of the upstream and pasted into the work comes back as the folder it was.</summary>
    /// <remarks>
    /// A cut is a drag that waits: the clipboard carries the folder's row, and the paste is the same plan a drop of it
    /// makes. What it must not be is the folder's files alone, cutting `A` and pasting into the root is the root gaining
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

        var plan = BringPlanning.Of(
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
        var inside = BringPlanning.Of(
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

        var plan = BringPlanning.Of(
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
        var inside = BringPlanning.Of(
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

    /// <summary>A move of a local copy is made in the Layout being worked in, through the writer.</summary>
    /// <remarks>
    /// The record the local file is kept under is the writer's, so nothing of the command line is run for a move within
    /// the work: what changes is where the Layout being worked in names the entry, and the content stays where it is.
    /// </remarks>
    [Fact]
    public void ALocalMoveIsMadeInTheLayoutBeingWorkedIn()
    {
        var directory = Scratch.New("bring-move");

        Given(directory, "init");

        var id = Guid.NewGuid().ToString();
        Given(directory, "layout", "entry", "create", id, $"blake3:{new string('0', 64)}");
        Given(directory, "layout", "path", "create", "imported.fbx", id);

        var (done, said) = LayoutPathMoveCommand.Move(
            directory,
            layout: null,
            "imported.fbx",
            "A/b.fbx"
        );

        Assert.True(done, said);

        // A place of the Workspace's own names the one Layout it works in, which is the same write under the name the
        // listing gives it.
        var (named, namedSaid) = LayoutPathMoveCommand.Move(
            directory,
            layout: "main",
            "A/b.fbx",
            "A/c.fbx"
        );

        Assert.True(named, namedSaid);

        using var workspace = new WorkspaceScope(directory);
        workspace.Read();
        var layout = workspace.Layout;
        layout.Read();

        Assert.Null(layout.Find("imported.fbx"));
        Assert.Null(layout.Find("A/b.fbx"));
        Assert.Equal(id, layout.Find("A/c.fbx")?.Uuid);
    }

    /// <summary>An entry as the copy names it, with a path and the `Uuid` it is known by.</summary>
    /// <param name="path">The path.</param>
    /// <param name="uuid">The `Uuid`.</param>
    /// <returns>The entry.</returns>
    private static LayoutEntry Entry(string path, string uuid) =>
        new(path, uuid, string.Empty, string.Empty, string.Empty, false);

    /// <summary>Runs one command in a directory, failing the test when it does not end successfully.</summary>
    /// <param name="directory">Where to run it.</param>
    /// <param name="arguments">What the command is told.</param>
    private static void Given(string directory, params string[] arguments)
    {
        // What the program says is left on the run's own streams: a run that fails shows why in the
        // test output, and a pipe nobody drains cannot fill and stop the run.
        var start = new ProcessStartInfo(Program()) { WorkingDirectory = directory };

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        using var command = Process.Start(start)!;

        command.WaitForExit();

        Assert.Equal(0, command.ExitCode);
    }

    /// <summary>The built command line, whose directory the suites are told.</summary>
    /// <returns>The program to run.</returns>
    private static string Program() =>
        Path.Combine(
            Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
                ?? throw new InvalidOperationException("the suites are run with ROLA_BIN_DIR"),
            OperatingSystem.IsWindows() ? "rola.exe" : "rola"
        );

    /// <summary>The Vault `origin`'s copy, for these checks to plan drags of.</summary>
    private static UpstreamSource Origin => new(UpstreamPlace.Vaults, "origin");
}

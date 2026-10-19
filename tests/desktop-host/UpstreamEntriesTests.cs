using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The upstream place: what one level of it holds, where a drop puts an entry, and the commands it runs.
/// </summary>
/// <remarks>
/// The tree and the level are the pure half of the place — they read nothing and start nothing — so what is checked here
/// is the judgement they make: which folders a Layout's paths come to, what stands at a prefix, and where a dropped
/// entry would land. The commands are checked beside them as the arguments they are run with, which is the same
/// division the retrack's own test makes.
/// </remarks>
public sealed class UpstreamEntriesTests
{
    /// <summary>A path that is an entry and the folder of another is drawn as both.</summary>
    [Fact]
    public void APathThatIsAlsoAFolderIsBoth()
    {
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("Assets", "a"),
                Entry("Assets/hero.psd", "b"),
            ],
            []
        );

        // UNWRAP: the path was named above, so the tree stands for it.
        var assets = tree.Find("Assets")!;

        Assert.NotNull(assets.Entry);
        Assert.True(assets.HasChildren);

        var top = tree.At(string.Empty);

        Assert.Equal(["Assets"], top.Select(node => node.Path));

        var inside = tree.At("Assets");

        Assert.Equal(["Assets/hero.psd"], inside.Select(node => node.Path));
    }

    /// <summary>A level reads folders first, and each of the two by name.</summary>
    [Fact]
    public void ALevelReadsFoldersThenNames()
    {
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("zebra.psd", "a"),
                Entry("Drafts/cover.psd", "b"),
                Entry("apple.psd", "c"),
                Entry("Assets/hero.psd", "d"),
            ],
            []
        );

        Assert.Equal(
            ["Assets", "Drafts", "apple.psd", "zebra.psd"],
            tree.At(string.Empty).Select(node => node.Path)
        );
    }

    /// <summary>A level carries a row for the way up, which leads out of the place.</summary>
    [Fact]
    public void ALevelCarriesTheWayUp()
    {
        var tree = UpstreamEntriesTree.Build(
            [Entry("Assets/sub/hero.psd", "a"), Entry("Assets/sub/backup.psd", "b")],
            []
        );

        var top = UpstreamSpots.Of(tree, string.Empty);

        Assert.DoesNotContain(top, spot => spot.Name == "..");

        var level = UpstreamSpots.Of(tree, "Assets/sub");

        Assert.Equal("..", level[0].Name);
        Assert.Equal("Assets", level[0].Path);
        Assert.True(level[0].Folder);
        Assert.Equal(["Assets/sub/backup.psd", "Assets/sub/hero.psd"], level.Skip(1).Select(spot => spot.Path));
    }

    /// <summary>A prefix's parent is one segment up, and nothing at the top.</summary>
    [Fact]
    public void APrefixIsOneStepUnderItsParent()
    {
        Assert.Null(UpstreamEntriesTree.Parent(string.Empty));
        Assert.Equal(string.Empty, UpstreamEntriesTree.Parent("Assets"));
        Assert.Equal("Assets", UpstreamEntriesTree.Parent("Assets/sub"));
        Assert.Equal("Assets/sub", UpstreamEntriesTree.Parent("Assets/sub/hero.psd"));
    }

    /// <summary>An entry the Vault names at no path is not shown, but one this side holds is still held.</summary>
    [Fact]
    public void AnEntryAtNoPathIsNotShownButIsStillHeld()
    {
        var tree = UpstreamEntriesTree.Build(
            [Entry(string.Empty, "a")],
            [Entry(string.Empty, "a")]
        );

        Assert.Empty(tree.Roots);
        Assert.True(tree.IsHeld(Entry(string.Empty, "a")));

        // A half-made entry of this Layout still holds its uuid, which is what keeps it from being brought in twice.
        var named = UpstreamEntriesTree.Build(
            [Entry("hero.psd", "b")],
            [Entry(string.Empty, "b")]
        );

        Assert.True(named.IsHeld(Entry("hero.psd", "b")));
        Assert.Equal(string.Empty, named.LocalPath("b"));
    }

    /// <summary>A dropped entry is put at the prefix it was dropped on, under its own name.</summary>
    [Fact]
    public void ADropPutsAnEntryUnderThePrefixItLandedOn()
    {
        // Dropped on a folder's row: it keeps its own name and takes the folder's path.
        Assert.Equal("Drafts/hero.psd", UpstreamPlace.Landing("Assets/hero.psd", "Drafts"));

        // Dropped on the top of the place: the top is a prefix of its own, and the name is all it keeps.
        Assert.Equal("hero.psd", UpstreamPlace.Landing("Assets/hero.psd", string.Empty));

        // Let go where it already is: nothing to do, and nothing for the Vault to refuse.
        Assert.Null(UpstreamPlace.Landing("Assets/hero.psd", "Assets"));
        Assert.Null(UpstreamPlace.Landing("hero.psd", string.Empty));
    }

    /// <summary>An address carries the prefix, and reads back as the same address.</summary>
    [Fact]
    public void AnAddressIsAPathThroughTheWork()
    {
        Assert.Empty(UpstreamPlace.Segments(UpstreamPlace.Of()));

        // `rola@vaults/origin/@/new` is the unplaced entries of the Vault `origin`: two segments name it, and the rest is
        // the path inside its Layout.
        var address = UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "@", "new");

        Assert.Equal(["vaults", "origin", "@", "new"], UpstreamPlace.Segments(address));
        Assert.Equal("rola@vaults/origin/@/new", address.ToString());
        Assert.Equal(
            (new UpstreamSource(UpstreamPlace.Vaults, "origin"), "@/new"),
            UpstreamPlace.Target(address)
        );

        // The domain on its own names no copy: what is under it is a list of names rather than a Vault.
        Assert.Null(UpstreamPlace.Target(UpstreamPlace.Of(UpstreamPlace.Vaults)));

        // A Layout of the Workspace's own is a level of its own, and what a run is told differs by domain: the Vault's is
        // named `truth@VAULT` and the Workspace's by its name alone.
        var layout = UpstreamPlace.Target(UpstreamPlace.Of(UpstreamPlace.Layouts, "work", "Assets"));

        Assert.Equal((new UpstreamSource(UpstreamPlace.Layouts, "work"), "Assets"), layout);
        Assert.Equal("truth@origin", new UpstreamSource(UpstreamPlace.Vaults, "origin").Layout);
        Assert.Equal("work", new UpstreamSource(UpstreamPlace.Layouts, "work").Layout);

        // The slashes a path is written with are not part of the segments.
        Assert.Equal(["vaults", "origin"], UpstreamPlace.Segments(UpstreamPlace.Of("/vaults/origin/")));
    }

    /// <summary>
    /// A row's picture is the one the icon set in force draws for what it is called.
    /// </summary>
    /// <remarks>
    /// What a place holds is not on disk, so the picture is chosen from the name — the extension, and whether it is a folder
    /// — by the same look a directory listing uses, the system's own pictures included where the set says nothing.
    /// </remarks>
    [Fact]
    public void ARowsPictureIsChosenFromWhatItIsCalled()
    {
        _ = Host.Services();

        Assert.NotNull(EntryIcons.For("Assets/hero.psd", EntryKind.File, 16));
        Assert.NotNull(EntryIcons.For("Assets", EntryKind.Directory, 16));
        Assert.NotNull(EntryIcons.Folder(16));
    }

    /// <summary>A checkin names each entry by its path, and by its uuid where the path reads as a pattern.</summary>
    [Fact]
    public void ACheckinNamesWhatWasChosenAndNothingElse()
    {
        var plain = CheckinCommand.Arguments(["Assets/hero.psd"]);

        Assert.Contains("checkin", plain);
        Assert.Contains("--json", plain);
        Assert.Contains("Assets/hero.psd", plain);

        // Not offline, and no destination: a run fetches the Vault's copy and lands what it brings at the path the
        // Vault names it by.
        Assert.DoesNotContain("--offline", plain);
        Assert.DoesNotContain("--force", plain);

        // A path a glob would read is handed over as the uuid it stands for, which no pattern can be read from.
        Assert.Equal("a", CheckinCommand.Reference(Entry("Assets/hero[1].psd", "a")));
        Assert.Equal("b", CheckinCommand.Reference(Entry("Assets/{draft}.psd", "b")));
        Assert.Equal("c", CheckinCommand.Reference(Entry("Assets/why?.psd", "c")));
        Assert.Equal("Assets/hero.psd", CheckinCommand.Reference(Entry("Assets/hero.psd", "d")));
    }

    /// <summary>
    /// A drop of a folder brings in everything that stands under it, and a choice that names an entry twice names it once.
    /// </summary>
    /// <remarks>
    /// A folder is not an entry, so what a drop of one means is what its paths come to; and a run that named the same entry
    /// twice is one the command line refuses, which is why what is carried is put in order and deduplicated here.
    /// </remarks>
    [Fact]
    public void ADropOfAFolderBringsInWhatStandsUnderIt()
    {
        var tree = UpstreamEntriesTree.Build(
            [
                Entry("Assets", "a"),
                Entry("Assets/hero.psd", "b"),
                Entry("Assets/bg.psd", "c"),
                Entry("Drafts", "d"),
            ],
            []
        );

        // The folder's own entry first, then what stands under it in the order a listing reads it.
        Assert.Equal(
            ["Assets", "Assets/bg.psd", "Assets/hero.psd"],
            UpstreamBringing.Plan(tree, [new UpstreamCarry(Origin, "Assets")]).References
        );

        // A choice of a folder and one of its entries is the folder's answer, and the entry is named once.
        Assert.Equal(
            ["Assets", "Assets/bg.psd", "Assets/hero.psd"],
            UpstreamBringing.Plan(
                tree,
                [new UpstreamCarry(Origin, "Assets"), new UpstreamCarry(Origin, "Assets/hero.psd")]
            ).References
        );

        // One entry is one reference, and a path the copy no longer names is not a reference at all.
        Assert.Equal(
            ["Assets/hero.psd"],
            UpstreamBringing.Plan(tree, [new UpstreamCarry(Origin, "Assets/hero.psd")]).References
        );
        Assert.Empty(UpstreamBringing.Plan(tree, [new UpstreamCarry(Origin, "Gone/hero.psd")]).References);

        // What this Layout already holds is what a run is not given, and is counted so that the drop can say so.
        var held = UpstreamEntriesTree.Build([Entry("Assets", "a"), Entry("Assets/hero.psd", "b")], [Entry("Assets/hero.psd", "b")]);
        var plan = UpstreamBringing.Plan(held, [new UpstreamCarry(Origin, "Assets")]);

        Assert.Equal(["Assets"], plan.References);
        Assert.Equal(1, plan.Held);
    }

    /// <summary>A checkin landed under a directory names it last, because that is how the operands are read.</summary>
    [Fact]
    public void ACheckinLandedUnderADirectoryNamesItLast()
    {
        var landed = CheckinCommand.Arguments(["Assets/hero.psd", "Assets/bg.psd"], @"C:\work\art");

        Assert.Equal("checkin", landed[^4]);
        Assert.Equal("Assets/hero.psd", landed[^3]);
        Assert.Equal("Assets/bg.psd", landed[^2]);
        Assert.Equal(@"C:\work\art", landed[^1]);

        // No destination is no operand: a run that lands where upstream names it has nothing asserted after the references.
        Assert.Equal("Assets/hero.psd", CheckinCommand.Arguments(["Assets/hero.psd"])[^1]);
    }

    /// <summary>A move names the Layout the place is a view of, and nothing else is touched.</summary>
    /// <remarks>
    /// The Vault's own Layout is named <c>truth@VAULT</c> and one of the Workspace's by its name alone: the flag is what
    /// keeps a move from being taken for one in the Layout being worked in, which is a different file.
    /// </remarks>
    [Fact]
    public void AMoveNamesTheLayoutItIsIn()
    {
        var moved = LayoutPathMoveCommand.Arguments(
            LayoutPathMoveCommand.Named("origin"),
            "@/new/hero.psd",
            "Assets/hero.psd"
        );

        Assert.Contains("layout", moved);
        Assert.Contains("path", moved);
        Assert.Contains("move", moved);
        Assert.Contains("--layout", moved);
        Assert.Contains("truth@origin", moved);
        Assert.Contains("@/new/hero.psd", moved);
        Assert.Contains("Assets/hero.psd", moved);
        Assert.DoesNotContain("--offline", moved);

        // A Layout of the Workspace's own is passed as the name it is known by, which is what makes the same command
        // write that Layout rather than a Vault's.
        var own = LayoutPathMoveCommand.Arguments("work", "Drafts/hero.psd", "Assets/hero.psd");

        Assert.Contains("work", own);
        Assert.DoesNotContain(own, argument => argument.Contains('@'));
    }

    /// <summary>A Layout of the Workspace's own is listed by the name the place opened it under.</summary>
    [Fact]
    public void ALayoutOfTheWorkspaceIsListedByName()
    {
        var listed = LayoutEntriesCommand.LayoutArguments("work");

        Assert.Contains("layout", listed);
        Assert.Contains("entries", listed);
        Assert.Contains("--layout", listed);
        Assert.Contains("work", listed);
    }

    /// <summary>A list of entries is read for what it states, and a row that states no uuid is not one.</summary>
    [Fact]
    public void AListingOfEntriesIsReadForWhatItStates()
    {
        var entries = LayoutEntriesCommand.ParseEntries(
            """
            {
              "entries": [
                { "path": "Assets/hero.psd", "uuid": "a", "version": "0123", "owner": "catilgrass", "description": "the hero", "deprecated": false },
                { "path": "", "uuid": "b", "version": "4567", "owner": "", "description": "", "deprecated": false },
                { "path": "Assets/bg.psd", "uuid": "c", "version": "", "owner": "", "description": "" },
                { "path": "Assets/gone.psd" },
                "not an entry"
              ]
            }
            """
        );

        Assert.Equal(3, entries.Count);
        Assert.Equal("the hero", entries[0].Description);
        Assert.Equal("hero.psd", entries[0].Name);
        Assert.Equal(string.Empty, entries[1].Path);
        Assert.False(entries[2].Deprecated);

        // A member an answer does not state is read as nothing rather than as a failure.
        Assert.Equal(string.Empty, entries[2].Owner);
    }

    /// <summary>A result brings in some and refuses some, and the refusals are read from the result.</summary>
    [Fact]
    public void AResultSaysWhatCameInAndWhatDidNot()
    {
        var answer = CheckinCommand.Parse(
            """
            {
              "layout": "work",
              "vault": "origin",
              "entries": [
                { "uuid": "a", "remote_path": "Assets/hero.psd", "local_path": "Assets/hero.psd" }
              ],
              "failed": ["b: something is already at `Assets/bg.psd`"]
            }
            """
        );

        // The run ended well and brought one of the two in: with a structural renderer the refusal is a line in the
        // result, and a status alone would call this a success with nothing to say.
        Assert.True(answer.Done);
        Assert.Single(answer.Landed);
        Assert.Equal("Assets/hero.psd", answer.Landed[0].LocalPath);
        Assert.Single(answer.Failed);
    }

    /// <summary>The place begins with the two lists the work holds, and each is one step under it.</summary>
    /// <remarks>
    /// The one level of the address that is answered without reading anything: <c>rola@</c> is the two domains, and what
    /// is over each of them is the root. What is under them needs a Workspace to read, which is what the checks above
    /// are made without.
    /// </remarks>
    [Fact]
    public void TheRootIsTheTwoListsTheWorkHolds()
    {
        var services = Host.Services();
        var host = services.For(new PluginId("rorolala.vcs"), 1);
        var hub = new ProgressHub();
        var place = new UpstreamPlace(host, host.Log, hub, new VaultFetch(host, hub));

        var root = UpstreamPlace.Of();

        Assert.Equal(["vaults", "layouts"], place.Entries(root).Select(entry => entry.Name));
        Assert.All(place.Entries(root), entry => Assert.False(entry.Draggable));
        Assert.All(place.Entries(root), entry => Assert.NotNull(entry.Opens));
        Assert.Null(place.Parent(root));

        // A list is a step of its own with nothing over it: what leads back to the root from there is the row the list
        // carries, not the toolbar's way up.
        Assert.Null(place.Parent(UpstreamPlace.Of(UpstreamPlace.Vaults)));
        Assert.NotNull(place.Parent(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "@", "new")));
        Assert.Equal(
            "rola@vaults/origin/@",
            place.Parent(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "@", "new"))?.ToString()
        );
        Assert.Equal(
            "rola@vaults/origin",
            place.Parent(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin", "@"))?.ToString()
        );
    }

    /// <summary>A drop the place will not take says why, in the place's own words.</summary>
    /// <remarks>
    /// A refusal with no word is a drop the user cannot tell from one the drag never reached, so every way the place can
    /// refuse one is answered with a reason: a drag that is not the place's own, a list that is not a folder to drop into,
    /// and a copy that is still being read. The words are the place's keys — nothing here states translations — which is
    /// what says the reason is the place's rather than the browser's.
    /// </remarks>
    [Fact]
    public void ARefusedDropSaysWhy()
    {
        var services = Host.Services();
        var host = services.For(new PluginId("rorolala.vcs"), 1);
        var hub = new ProgressHub();
        var place = new UpstreamPlace(host, host.Log, hub, new VaultFetch(host, hub));
        var source = new UpstreamSource(UpstreamPlace.Vaults, "origin");
        var entry = new LayoutEntry("Assets/hero.psd", "a", "v1", "me", "the hero", false);
        var carrying = new LogicalDrag(new UpstreamCarry(source, entry.Path, entry));

        // A drag of files carries paths, which is not something a Layout can name.
        Assert.Equal(
            "rorolala_vcs.place.refused_not_a_drag",
            place.Refusal(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin"), new LogicalDrag("C:\\work\\hero.psd"))
        );

        // A list is a level of the address rather than a folder of a Layout.
        Assert.Equal(
            "rorolala_vcs.place.refused_not_a_folder",
            place.Refusal(UpstreamPlace.Of(UpstreamPlace.Vaults), carrying)
        );

        // Nothing has been read here — there is no `rola` to read it with — so there is nothing to move in.
        Assert.Equal(
            "rorolala_vcs.place.refused_reading",
            place.Refusal(UpstreamPlace.Of(UpstreamPlace.Vaults, "origin"), carrying)
        );
    }

    /// <summary>The upstream root is the Vault the work tracks, or the default one, or nothing.</summary>
    /// <remarks>
    /// What the root opens is one Vault, and which it is is the answer of a program: the one the Layout being worked in
    /// tracks, the Workspace's default where it tracks none, and nothing at all where it has neither — which is said in a
    /// notification rather than opened as a list of Vaults, the list not being what the item is named for.
    /// </remarks>
    [Fact]
    public void ARootIsTheTrackedVaultOrTheDefaultOne()
    {
        LayoutListing tracked = new("work", Current: true, Tracking: "origin");
        LayoutListing untracked = new("work", Current: true, Tracking: string.Empty);
        VaultListing bound = new("origin", Default: false);
        VaultListing fallback = new("home", Default: true);

        // The Vault the Layout being worked in tracks is what this work is the upstream of.
        Assert.Equal("origin", UpstreamPlace.RootOf([tracked], [bound, fallback]));

        // Where it tracks none, the Workspace's own default Vault is the answer.
        Assert.Equal("home", UpstreamPlace.RootOf([untracked], [bound, fallback]));

        // A Workspace with no default and no tracking has no upstream to open.
        Assert.Null(UpstreamPlace.RootOf([untracked], [bound]));
        Assert.Null(UpstreamPlace.RootOf(null, null));
    }

    /// <summary>A listing of Vaults says which of them the Workspace works against by default.</summary>
    [Fact]
    public void AListingOfVaultsSaysWhichIsTheDefault()
    {
        var vaults = LayoutEntriesCommand.ParseVaults(
            """
            {
              "vaults": [
                { "name": "origin", "address": "https://one", "is_default": false },
                { "name": "home", "address": "https://two", "is_default": true },
                { "address": "https://three" }
              ]
            }
            """
        );

        // A row with no name is not a Vault, and one that states no default is not the default.
        Assert.Equal(2, vaults.Count);
        Assert.Equal(("origin", false), (vaults[0].Name, vaults[0].Default));
        Assert.Equal(("home", true), (vaults[1].Name, vaults[1].Default));
    }

    /// <summary>An entry of a Layout, for these checks to build trees from.</summary>
    /// <param name="path">The path it is at.</param>
    /// <param name="uuid">The uuid it is known by.</param>
    /// <param name="deprecated">Whether the Vault has deprecated it.</param>
    /// <returns>The entry.</returns>
    private static LayoutEntry Entry(string path, string uuid, bool deprecated = false) =>
        new(path, uuid, string.Empty, string.Empty, string.Empty, deprecated);

    /// <summary>The Vault `origin`'s copy, for these checks to build drags of.</summary>
    private static UpstreamSource Origin => new(UpstreamPlace.Vaults, "origin");
}

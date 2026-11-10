using System.Diagnostics;

using RolaSharp;
using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The variant graph read through the native library: what a graph of the work names, and what a
/// window of it holds.
/// </summary>
/// <remarks>
/// The graph is the one reading this suite cannot stand in for: it is walked over the inverse index,
/// which only a workspace with a recorded history has, and the entries, the merges and the texts are
/// read beside it by the module that owns them. So the workspace is made by running the command line
/// here (a file recorded twice, which is a chain of two variants) and what is checked is the reading
/// of it through the C ABI and the wrapper above it.
/// </remarks>
public sealed class VariantGraphTests
{
    /// <summary>A place offers every changed or untracked file under it, as a chosen file would.</summary>
    [Fact]
    public void APlaceOffersTheChangedAndUntrackedFilesUnderIt()
    {
        var workspace = Recorded("place");

        // The recording's own file is changed, and a directory beside it holds one file the Layout
        // has never named.
        File.WriteAllText(Path.Combine(workspace, "art.txt"), "changed\n");
        Directory.CreateDirectory(Path.Combine(workspace, "art"));
        File.WriteAllText(Path.Combine(workspace, "art", "one.txt"), "one\n");

        using var status = RolaStatus.Locate(workspace);

        Assert.NotNull(status);

        var under = VersionGraphTrackBar.RecordableUnder(status, workspace);

        Assert.Contains(Path.Combine(workspace, "art.txt"), under);
        Assert.Contains(Path.Combine(workspace, "art", "one.txt"), under);
    }

    /// <summary>A tree is one entry, and a file inside it is read as that entry.</summary>
    [Fact]
    public void ATreeAndAFileInsideItAreCentredOnTheTree()
    {
        var workspace = TreeRecorded("tree-focus");
        var tree = Path.Combine(workspace, "tree");

        Assert.Equal(tree, RolaTrees.RootOf(tree, workspace));
        Assert.Equal(tree, RolaTrees.RootOf(Path.Combine(tree, "a.txt"), workspace));

        var graph = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "tree")],
            }
        );

        Assert.NotNull(graph);

        var focus = Assert.Single(graph.Focus);
        Assert.Equal("local", focus.Side);
        Assert.Equal("tree", focus.Path);
        Assert.NotNull(focus.Version);
        Assert.NotNull(focus.Variant);

        var entry = Assert.Single(graph.Entries, one => one.Path == "tree");
        Assert.Equal(focus.Version, entry.Version);
    }

    /// <summary>A tree's staging area is read as a change, even though no file in it is modified.</summary>
    [Fact]
    public void ATreesStagingAreaIsAChange()
    {
        var workspace = TreeRecorded("tree-staging");
        var tree = Path.Combine(workspace, "tree");

        Assert.True(RolaTreeStatus.Locate(tree)?.Clean);

        File.WriteAllText(Path.Combine(tree, "a.txt"), "two\n");
        Given(workspace, "tree", "add", "tree");

        var status = RolaTreeStatus.Locate(tree);

        Assert.NotNull(status);
        Assert.False(status.Clean);
        Assert.Equal(1, status.Staged);
    }

    /// <summary>The tree's own entry, and not the file inside it, is what the graph is centred on.</summary>
    [Fact]
    public void AFileInsideATreeFocusesTheTree()
    {
        var workspace = TreeRecorded("tree-focus-file");
        var tree = Path.Combine(workspace, "tree");

        var focus = VersionGraphView.FocusOf(
            [Path.Combine(tree, "a.txt")],
            workspace
        );

        var one = Assert.Single(focus);
        Assert.Equal("local", one.Side);
        Assert.Equal("tree", one.Path);
    }

    /// <summary>A staged tree is a draft in the graph, even when the tree's files are unchanged.</summary>
    [Fact]
    public void AStagedTreeDrawsADraft()
    {
        var workspace = TreeRecorded("tree-draft");
        var tree = Path.Combine(workspace, "tree");

        File.WriteAllText(Path.Combine(tree, "a.txt"), "two\n");
        Given(workspace, "tree", "add", "tree");

        var graph = RolaVariantGraph.Locate(workspace);
        using var status = RolaStatus.Locate(workspace);

        Assert.NotNull(graph);
        Assert.NotNull(status);

        var drafts = VersionGraphView.DraftsOf(graph, status, workspace);
        var draft = Assert.Single(drafts, one => one.Path == "tree");

        Assert.True(draft.Tree);
    }

    /// <summary>A track with a tree's directory records the tree, not a path inside it.</summary>
    [Fact]
    public void ATrackOnATreeRecordsTheTree()
    {
        var workspace = TreeRecorded("tree-track");
        var tree = Path.Combine(workspace, "tree");

        File.WriteAllText(Path.Combine(tree, "a.txt"), "two\n");
        Given(workspace, "tree", "add", "tree");

        RolaI18N.SetTranslationDirectory(Scratch.New("i18n"));
        Environment.SetEnvironmentVariable(Sync.Program, Program());

        var (done, said) = TrackCommand.Run(tree, [tree], "second tree");

        Assert.True(done, said);
        Assert.True(RolaTreeStatus.Locate(tree)?.Clean);

        var graph = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "tree")],
            }
        );

        Assert.NotNull(graph);
        Assert.NotNull(Assert.Single(graph.Focus).Version);
        Assert.True(graph.Nodes.Count >= 2);
    }

    /// <summary>A chain of two versions is drawn with its entries, its words and its versions.</summary>
    [Fact]
    public void AGraphOfTheWorkNamesItsChainAndItsEntries()
    {
        var workspace = Recorded("graph");

        var graph = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(graph);

        // The focus is the entry the Layout names, which records the second version and resolves to
        // the variant that version fixes.
        var focus = Assert.Single(graph.Focus);
        Assert.Equal("local", focus.Side);
        Assert.Equal("art.txt", focus.Path);
        Assert.NotNull(focus.Version);
        Assert.NotNull(focus.Variant);

        // The chain behind it: two variants, the three versions that fix them and the root, and one
        // base edge between the two variants.
        Assert.Equal(2, graph.Nodes.Count);
        Assert.Equal(3, graph.Versions.Count);
        var edge = Assert.Single(graph.Edges);
        Assert.Equal("base", edge.Kind);
        Assert.False(graph.Truncated);
        Assert.Empty(graph.Missing);

        // The first version is numbered from nothing and the second one past it; the root is named as
        // the root rather than numbered.
        Assert.Contains(graph.Versions, version => version is { Number: 0, IsRoot: false });
        Assert.Contains(graph.Versions, version => version is { Number: 1, IsRoot: false });
        Assert.Contains(
            graph.Versions,
            version => version is { Number: null, IsRoot: true }
        );

        // The entry is named beside the graph, held by the account the work was recorded as.
        var entry = Assert.Single(graph.Entries);
        Assert.Equal("local", entry.Side);
        Assert.Equal("art.txt", entry.Path);
        Assert.NotNull(entry.Version);
        Assert.Equal("alice", entry.Owner);
        Assert.False(entry.Deprecated);

        // The words of both variants are read from the index, once each.
        Assert.Contains("alice", graph.Texts.Creators.Values);
        Assert.Contains("first", graph.Texts.Messages.Values);
        Assert.Contains("second", graph.Texts.Messages.Values);
    }

    /// <summary>A window holds what the request asked for, and says where it stopped.</summary>
    [Fact]
    public void AWindowHoldsWhatTheRequestAskedFor()
    {
        var workspace = Recorded("window");

        // A second file is another chain, and the reading of the whole work names both its entries.
        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var whole = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(whole);
        Assert.Equal(2, whole.Focus.Count);
        Assert.Equal(2, whole.Entries.Count);

        var graph = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest { MaxNodes = 1 }
        );

        Assert.NotNull(graph);
        Assert.True(graph.Truncated);

        var node = Assert.Single(graph.Nodes);
        Assert.True(node.Frontier.Parents);

        // A window that stands on one file carries the entry that file is, and not the other chain's.
        var one = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "art.txt")],
            }
        );

        Assert.NotNull(one);
        Assert.Equal("art.txt", Assert.Single(one.Entries).Path);

        // A focus the reading does not hold is answered as an entry recording no version, and the
        // window is empty rather than centred somewhere else.
        var unknown = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "nowhere.txt")],
            }
        );

        Assert.NotNull(unknown);
        var focus = Assert.Single(unknown.Focus);
        Assert.Null(focus.Version);
        Assert.Null(focus.Variant);
        Assert.Empty(unknown.Nodes);
        Assert.Empty(unknown.Entries);

        // A side this reading does not hold is the same answer: the work may have moved on since the
        // request was written, and it is not answered from somewhere else.
        var unknownSide = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("vault", "art.txt")],
            }
        );

        Assert.NotNull(unknownSide);
        Assert.Null(unknownSide.Focus[0].Version);
        Assert.Empty(unknownSide.Nodes);
    }

    /// <summary>A directory no workspace holds has no graph, which is nothing rather than a failure.</summary>
    [Fact]
    public void ADirectoryWithoutAWorkspaceHasNoGraph()
    {
        var plain = Scratch.New("plain");

        Assert.Null(RolaVariantGraph.Locate(plain));
    }

    /// <summary>A reading is laid out as lanes of files and columns of version depths.</summary>
    [Fact]
    public void AReadingIsLaidOutInLanesAndColumns()
    {
        var workspace = Recorded("layout");

        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var graph = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(graph);

        var art = graph.Entries.Single(entry => entry.Path == "art.txt");
        var otherEntry = graph.Entries.Single(entry => entry.Path == "other.txt");
        var words = new GraphVocabulary(
            "DRAFT",
            "unrecorded",
            "unrecorded tree",
            "wrong",
            "unowned",
            "draft",
            "wrong-draft",
            "unowned-draft",
            "merging into %{file}",
            "no entries",
            "unknown base"
        );
        var drawing = VariantGraphDrawing.From(
            graph,
            [
                new DraftFact(
                    "art.txt",
                    art.Version!,
                    Wrong: false,
                    Unowned: false,
                    // The other side has moved on, which is what the draft's edge names; it does not
                    // decide the column the draft stands in.
                    ShouldBeVersionHash: otherEntry.Version
                ),
            ],
            words,
            new GraphMeasure(),
            emphasized: false
        );

        // One card per variant, and one more for the draft the work holds no variant of.
        Assert.Equal(graph.Nodes.Count + 1, drawing.Cards.Count);
        Assert.Equal(graph.Nodes.Count, drawing.VariantCount);
        Assert.Equal(2, drawing.EntryCount);
        Assert.Equal(1, drawing.DraftCount);

        // Every card of one file is on one lane, and the two files are two lanes.
        var artCard = drawing.Cards.Single(card =>
            card.Entries.Any(entry => entry.Path == "art.txt")
        );
        var artCards = drawing.Cards.Where(card => card.Lane == artCard.Lane).ToArray();
        var other = drawing.Cards.Single(card =>
            card.Entries.Any(entry => entry.Path == "other.txt")
        );

        Assert.Equal(3, artCards.Length);
        Assert.DoesNotContain(other.Lane, artCards.Select(card => card.Lane));

        // The first variant of each chain is version 0; the second version is column 1; the draft
        // stands one past the version it is recorded at and has no version of its own.
        Assert.Equal(0, artCards.Single(card => card.Number == 0).Column);
        Assert.Equal(1, artCards.Single(card => card.Number == 1).Column);
        var draft = artCards.Single(card => card.Kind == GraphCardKind.Draft);
        Assert.Equal(2, draft.Column);
        Assert.False(draft.HasVersion);
        Assert.Equal("DRAFT", VariantGraphDrawing.VersionLabel(draft));
        Assert.Contains(draft.Chips, chip => chip.Text == words.DraftBadge);

        // The file is being edited, so its local pill is on the draft rather than on the version the
        // draft is based on: one file is one place in the drawing.
        Assert.Contains(
            draft.Chips,
            chip => chip.Kind == GraphChipKind.File && chip.Tag == "LOCAL"
        );
        Assert.DoesNotContain(
            artCards.Single(card => card.Number == 1).Chips,
            chip => chip.Kind == GraphChipKind.Entry && chip.Entry?.Side == "local"
        );

        // A chain edge joins each version to the one before it: one for the two versions of art.txt,
        // and none for other.txt's only one.
        Assert.Equal(
            1,
            drawing.Links.Count(link => link.Kind == GraphLinkKind.Base)
        );
        var placed = drawing.Links.Single(link => link.Kind == GraphLinkKind.Draft);
        Assert.Equal("art.txt", drawing.Cards[placed.To].Draft?.Path);
        Assert.Contains(
            drawing.Cards[placed.From].Entries,
            entry => entry.Path == "art.txt"
        );

        // A reading that does not stand on chosen entries has no chains to emphasise.
        Assert.Empty(drawing.FocusChain);
        Assert.Empty(drawing.FocusRelated);
    }

    /// <summary>Every chosen file's local pill is marked, and a pill for nothing chosen is not.</summary>
    [Fact]
    public void ChosenFilesWearTheInk()
    {
        var workspace = Recorded("chosen-pill");

        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var words = new GraphVocabulary(
            "DRAFT",
            "unrecorded",
            "unrecorded tree",
            "wrong",
            "unowned",
            "draft",
            "wrong-draft",
            "unowned-draft",
            "merging into %{file}",
            "no entries",
            "unknown base"
        );

        // A reading that stands on no entry has nothing marked.
        var whole = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(whole);
        var unmarked = VariantGraphDrawing.From(whole, [], words, new GraphMeasure(), false);

        Assert.All(
            LocalChips(unmarked),
            chip => Assert.False(chip.Chosen, $"{chip.Name} was marked with nothing chosen")
        );

        // Standing on both files marks both of their local pills, so a several-file choice is read
        // as several pills rather than one.
        var both = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus =
                [
                    new VariantGraphFocus("local", "art.txt"),
                    new VariantGraphFocus("local", "other.txt"),
                ],
            }
        );

        Assert.NotNull(both);
        var marked = VariantGraphDrawing.From(both, [], words, new GraphMeasure(), true);
        var chosen = LocalChips(marked).ToList();

        Assert.Equal(2, chosen.Count);
        Assert.All(chosen, chip => Assert.True(chip.Chosen, $"{chip.Name} was not marked"));
    }

    /// <summary>The entry pills a drawing has for the work's own side.</summary>
    /// <param name="drawing">The drawing.</param>
    /// <returns>The pills.</returns>
    private static IEnumerable<GraphChip> LocalChips(VariantGraphDrawing drawing) =>
        drawing
            .Cards.SelectMany(card => card.Chips)
            .Where(chip =>
                chip.Kind is GraphChipKind.Entry or GraphChipKind.File
                && chip.Entry is not { Side: not "local" }
            );

    /// <summary>A merge in progress is named on the variant it was checked in as, and drawn as pending.</summary>
    [Fact]
    public void AMergeInProgressIsDrawnAsAPendingCard()
    {
        var workspace = Recorded("pending-merge");
        var graph = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(graph);

        var recorded = graph.Entries.Single(entry => entry.Side == "local");
        var variant = graph.Nodes[0].Hash;

        File.WriteAllText(
            Path.Combine(workspace, ".rola", "MERGING"),
            $"{recorded.Uuid} {variant}\n"
        );

        var merging = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(merging);
        Assert.Single(merging.Merging);
        Assert.Equal(variant, merging.Merging[0].Variant);

        var drawing = VariantGraphDrawing.From(
            merging,
            [],
            new GraphVocabulary(
                "DRAFT",
                "unrecorded",
                "unrecorded tree",
                "wrong",
                "unowned",
                "draft",
                "wrong-draft",
                "unowned-draft",
                "merging into %{file}",
                "no entries",
                "unknown base"
            ),
            new GraphMeasure(),
            emphasized: false
        );

        var pending = Assert.Single(drawing.Cards, card => card.Pending);

        Assert.Equal(1, drawing.PendingCount);
        Assert.Equal(variant, pending.Hash);
        Assert.Contains("merging into ", pending.Message);
        Assert.DoesNotContain("%{", pending.Message);
    }

    /// <summary>A file may be made to record the entry another side names, its content kept or not.</summary>
    [Fact]
    public void AFileMaySwitchTheEntryItRecords()
    {
        var workspace = Recorded("switch-uuid");

        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var graph = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(graph);

        var other = graph.Entries.Single(entry => entry.Path == "other.txt");

        // The entry given must hold no path of its own: unbinding the other file's path leaves its
        // entry in the Layout, which is what a switch takes as its target.
        Given(workspace, "layout", "path", "remove", "other.txt");

        Environment.SetEnvironmentVariable(Sync.Program, Program());
        var (done, said) = RetrackCommand.Run(
            workspace,
            Path.Combine(workspace, "art.txt"),
            other.Uuid,
            onlyIndex: false,
            force: false
        );

        Assert.True(done, said);

        var after = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(after);
        var switched = after.Entries.Single(entry =>
            entry.Side == "local" && entry.Path == "art.txt"
        );

        Assert.Equal(other.Uuid, switched.Uuid);
    }

    /// <summary>A path's entry may be forked into a Uuid of its own, holding what it held.</summary>
    [Fact]
    public void AFileMayForkItsEntry()
    {
        var workspace = Recorded("fork-uuid");

        var graph = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(graph);
        var art = graph.Entries.Single(entry => entry.Side == "local" && entry.Path == "art.txt");

        Environment.SetEnvironmentVariable(Sync.Program, Program());
        var (done, said) = LayoutPathNewUuidCommand.Run(workspace, "art.txt");

        Assert.True(done, said);

        var after = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(after);
        var forked = after.Entries.Single(entry =>
            entry.Side == "local" && entry.Path == "art.txt"
        );

        Assert.NotEqual(art.Uuid, forked.Uuid);
        Assert.Equal(art.Version, forked.Version);
    }

    /// <summary>What another Uuid of the same variant went on to record is followed to its head.</summary>
    /// <remarks>
    /// Two entries may hold the same version and go on from it separately: the file's record is forked
    /// into a second Uuid, both are advanced, and a reader who chose the first still has to find the
    /// second's versions. The window walks each branch on, so what was built on it is in the answer
    /// rather than only the variant the branch forked from.
    /// </remarks>
    [Fact]
    public void TheOtherUuidOfTheSameVariantIsFollowedToItsHead()
    {
        var workspace = Recorded("two-branch");
        var before = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(before);
        var art = before.Entries.Single(entry => entry.Path == "art.txt");

        // Both Uuids are made to hold the same version: the second is written into the Layout and a
        // path of its own names it, so the two stand on one variant and go on from it separately.
        var other = Guid.NewGuid().ToString();
        Given(workspace, "layout", "entry", "create", other, art.Version!, "--owner", "alice");
        Given(workspace, "layout", "path", "create", "copy.txt", other);

        // The path's first track adopts the file it holds; the versions of the branch are the two
        // after it.
        File.WriteAllText(Path.Combine(workspace, "copy.txt"), "branch zero\n");
        Given(workspace, "track", "copy.txt", "--message", "branch adopt", "--no-editor");
        File.WriteAllText(Path.Combine(workspace, "copy.txt"), "branch one\n");
        Given(workspace, "track", "copy.txt", "--message", "branch first", "--no-editor");
        File.WriteAllText(Path.Combine(workspace, "copy.txt"), "branch two\n");
        Given(workspace, "track", "copy.txt", "--message", "branch second", "--no-editor");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "main one\n");
        Given(workspace, "track", "art.txt", "--message", "main first", "--no-editor");
        File.WriteAllText(Path.Combine(workspace, "art.txt"), "main two\n");
        Given(workspace, "track", "art.txt", "--message", "main second", "--no-editor");

        var graph = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "art.txt")],
            }
        );

        Assert.NotNull(graph);

        // The main chain is two ahead of the forked version, and the other Uuid's two are in the
        // window as well: the fork itself, what was built on it, and that version's own child.
        Assert.Equal(6, graph.Nodes.Count);
        Assert.False(graph.Truncated);
    }

    /// <summary>
    /// A workspace holding one file with two recorded versions, and the account the work is recorded
    /// as.
    /// </summary>
    /// <param name="label">What the workspace is for, for a person reading the path.</param>
    /// <returns>The workspace's directory.</returns>
    private static string Recorded(string label)
    {
        var parent = Scratch.New($"variant-graph-{label}");
        var workspace = Path.Combine(parent, "ws");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "one\n");
        Given(workspace, "track", "art.txt", "--message", "first", "--no-editor");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "two\n");
        Given(workspace, "track", "art.txt", "--message", "second", "--no-editor");

        return workspace;
    }

    /// <summary>A workspace whose one recorded entry is a tree.</summary>
    /// <param name="label">The scratch directory's label.</param>
    /// <returns>The workspace.</returns>
    private static string TreeRecorded(string label)
    {
        var parent = Scratch.New($"variant-graph-{label}");
        var workspace = Path.Combine(parent, "ws");
        var tree = Path.Combine(workspace, "tree");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        Directory.CreateDirectory(tree);
        File.WriteAllText(Path.Combine(tree, "a.txt"), "one\n");

        Given(workspace, "tree", "init", "tree");
        Given(workspace, "tree", "add", "tree");
        Given(workspace, "track", "tree", "--message", "first tree", "--no-editor");

        return workspace;
    }

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
}

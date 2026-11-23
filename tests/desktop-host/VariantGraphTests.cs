using System.Diagnostics;

using Avalonia;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
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

        using var status = Read(workspace);

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

        var graph = Graph(
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

        Assert.True(TreeReading(tree)?.Clean);

        File.WriteAllText(Path.Combine(tree, "a.txt"), "two\n");
        Given(workspace, "tree", "add", "tree");

        var status = TreeReading(tree);

        Assert.NotNull(status);
        Assert.False(status.Clean);
        Assert.True(status.HasWork);
        Assert.Single(status.Staged);
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

        var graph = Graph(workspace);
        using var status = Read(workspace);

        Assert.NotNull(graph);
        Assert.NotNull(status);

        var drafts = VersionGraphView.DraftsOf(graph, status, workspace);
        var draft = Assert.Single(drafts, one => one.Path == "tree");

        Assert.True(draft.Tree);
    }

    /// <summary>A tree with nothing to record is not a draft in the graph.</summary>
    [Fact]
    public void ACleanTreeDrawsNoDraft()
    {
        var workspace = TreeRecorded("tree-clean");
        var tree = Path.Combine(workspace, "tree");

        var graph = Graph(workspace);
        using var status = Read(workspace);

        Assert.NotNull(graph);
        Assert.NotNull(status);

        var drafts = VersionGraphView.DraftsOf(graph, status, workspace);
        var treeStatus = TreeReading(tree);

        Assert.DoesNotContain(drafts, one => one.Path == "tree");
        Assert.NotNull(treeStatus);
        Assert.False(treeStatus.HasWork);
        Assert.True(treeStatus.Clean);
    }

    /// <summary>A directory in a tree is what the changes under it add up to.</summary>
    [Fact]
    public void ATreesChangesRollUpToItsDirectory()
    {
        var workspace = TreeRecorded("tree-rollup");
        var tree = Path.Combine(workspace, "tree");
        var sub = Directory.CreateDirectory(Path.Combine(tree, "sub")).FullName;

        File.WriteAllText(Path.Combine(sub, "new.txt"), "one\n");
        Given(workspace, "tree", "add", "tree");

        var entry = new Entry(sub, EntryKind.Directory);
        var status = TreeReading(tree);

        Assert.NotNull(status);

        var staged = RolaTreeWork.Of(status, tree, entry);

        Assert.NotNull(staged);
        Assert.Equal(TreeWorkStatus.Added, staged.Status);
        Assert.Equal(TreeWorkStage.Staged, staged.Stage);

        var rootWork = RolaTreeWork.Of(status, tree, new Entry(tree, EntryKind.Directory));

        Assert.NotNull(rootWork);
        Assert.Equal(TreeWorkStatus.Added, rootWork.Status);
        Assert.Equal(TreeWorkStage.Staged, rootWork.Stage);

        File.WriteAllText(Path.Combine(sub, "new.txt"), "two\n");

        status = TreeReading(tree);

        Assert.NotNull(status);

        var partial = RolaTreeWork.Of(status, tree, entry);

        Assert.NotNull(partial);
        Assert.Equal(TreeWorkStatus.Modified, partial.Status);
        Assert.Equal(TreeWorkStage.Partial, partial.Stage);

        var rootPartial = RolaTreeWork.Of(status, tree, new Entry(tree, EntryKind.Directory));

        Assert.NotNull(rootPartial);
        Assert.Equal(TreeWorkStatus.Modified, rootPartial.Status);
        Assert.Equal(TreeWorkStage.Partial, rootPartial.Stage);
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

        var (done, said) = TrackCommand.Run(tree, [tree], "second tree");

        Assert.True(done, said);
        Assert.True(TreeReading(tree)?.Clean);

        var graph = Graph(
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

        var graph = Graph(workspace);

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

    /// <summary>A fingerprint says what the drawing was made from, and nothing else.</summary>
    [Fact]
    public void AFingerprintChangesOnlyWhenTheReadingDoes()
    {
        var workspace = Recorded("fingerprint");
        var graph = Graph(workspace);

        Assert.NotNull(graph);

        var first = VariantGraphDrawing.Fingerprint(graph, [], false);

        Assert.Equal(first, VariantGraphDrawing.Fingerprint(graph, [], false));

        var draft = new DraftFact("art.txt", graph.Nodes[0].Hash, false, false, null);

        Assert.NotEqual(first, VariantGraphDrawing.Fingerprint(graph, [draft], false));
        Assert.NotEqual(first, VariantGraphDrawing.Fingerprint(graph, [], true));
    }

    /// <summary>A window holds what the request asked for, and says where it stopped.</summary>
    [Fact]
    public void AWindowHoldsWhatTheRequestAskedFor()
    {
        var workspace = Recorded("window");

        // A second file is another chain, and the reading of the whole work names both its entries.
        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var whole = Graph(workspace);

        Assert.NotNull(whole);
        Assert.Equal(2, whole.Focus.Count);
        Assert.Equal(2, whole.Entries.Count);

        var graph = Graph(
            workspace,
            new VariantGraphRequest { MaxNodes = 1 }
        );

        Assert.NotNull(graph);
        Assert.True(graph.Truncated);

        var node = Assert.Single(graph.Nodes);
        Assert.True(node.Frontier.Parents);

        // A window that stands on one file carries the entry that file is, and not the other chain's.
        var one = Graph(
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
        var unknown = Graph(
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
        var unknownSide = Graph(
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

        Assert.Null(Graph(plain));
    }

    /// <summary>A reading is laid out as lanes of files and columns of version depths.</summary>
    [Fact]
    public void AReadingIsLaidOutInLanesAndColumns()
    {
        var workspace = Recorded("layout");

        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var graph = Graph(workspace);

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
        var whole = Graph(workspace);

        Assert.NotNull(whole);
        var unmarked = VariantGraphDrawing.From(whole, [], words, new GraphMeasure(), false);

        Assert.All(
            LocalChips(unmarked),
            chip => Assert.False(chip.Chosen, $"{chip.Name} was marked with nothing chosen")
        );

        // Standing on both files marks both of their local pills, so a several-file choice is read
        // as several pills rather than one.
        var both = Graph(
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

    /// <summary>A merge in progress is named on the variant it was brought in as, and drawn as pending.</summary>
    [Fact]
    public void AMergeInProgressIsDrawnAsAPendingCard()
    {
        var workspace = Recorded("pending-merge");
        var graph = Graph(workspace);

        Assert.NotNull(graph);

        var recorded = graph.Entries.Single(entry => entry.Side == "local");
        var variant = graph.Nodes[0].Hash;

        File.WriteAllText(
            Path.Combine(workspace, ".rola", "MERGING"),
            $"{recorded.Uuid} {variant}\n"
        );

        var merging = Graph(workspace);

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

        var graph = Graph(workspace);

        Assert.NotNull(graph);

        var other = graph.Entries.Single(entry => entry.Path == "other.txt");

        // The entry given must hold no path of its own: unbinding the other file's path leaves its
        // entry in the Layout, which is what a switch takes as its target.
        Given(workspace, "layout", "path", "remove", "other.txt");

        var (done, said) = RetrackCommand.Run(
            workspace,
            Path.Combine(workspace, "art.txt"),
            other.Uuid,
            onlyIndex: false,
            force: false
        );

        Assert.True(done, said);

        var after = Graph(workspace);

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

        var graph = Graph(workspace);

        Assert.NotNull(graph);
        var art = graph.Entries.Single(entry => entry.Side == "local" && entry.Path == "art.txt");

        var (done, said) = LayoutPathNewUuidCommand.Run(workspace, "art.txt");

        Assert.True(done, said);

        var after = Graph(workspace);

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
        var before = Graph(workspace);

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

        var graph = Graph(
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

    /// <summary>The grid finds the same thing under a point as walking the whole drawing does.</summary>
    /// <remarks>
    /// The points are fixed by a seed, and a third of them are placed on a card's box, a third beside
    /// an edge's curve within a pointer's reach, and the rest anywhere around the drawing: the first
    /// two are where an answer is found, the third is where it is not, and every one of them must be
    /// answered by the cells alone as a whole walk answers it.
    /// </remarks>
    [Fact]
    public void TheHitIndexAgreesWithAWholeWalk()
    {
        var drawing = RecordedDrawing("hit-index");

        // The reach of a pointer on an edge at the closest zoom a drawing is read at, in content units:
        // seven screen pixels over the least scale.
        const double reach = 7 / 0.18;
        var random = new Random(20261124);
        var left = drawing.Left - 40;
        var top = drawing.Top - 40;
        var width = drawing.Right - drawing.Left + 80;
        var height = drawing.Bottom - drawing.Top + 80;
        int[] allCards = [.. Enumerable.Range(0, drawing.Cards.Count)];
        int[] allGhosts = [.. Enumerable.Range(0, drawing.Ghosts.Count)];
        int[] allLinks = [.. Enumerable.Range(0, drawing.Links.Count)];

        for (var turn = 0; turn < 500; turn++)
        {
            var (x, y) = (turn % 3) switch
            {
                0 => (left + (random.NextDouble() * width), top + (random.NextDouble() * height)),
                1 => OverCard(drawing.Cards[random.Next(drawing.Cards.Count)], random),
                _ => OverLink(
                    drawing,
                    drawing.Links[random.Next(drawing.Links.Count)],
                    random,
                    reach
                ),
            };

            var (cards, ghosts, links) = drawing.Hits.Near(x, y, reach);
            var found = VersionGraphView.Resolve(drawing, cards, ghosts, links, x, y, reach);
            var whole = VersionGraphView.Resolve(
                drawing,
                allCards,
                allGhosts,
                allLinks,
                x,
                y,
                reach
            );

            Assert.True(
                Summary(whole) == Summary(found),
                $"the cells and a whole walk differ at ({x}, {y})"
            );
        }
    }

    /// <summary>A card, an edge and the empty ground are answered as they always were.</summary>
    [Fact]
    public void TheHitIndexAnswersACardAnEdgeAndTheGround()
    {
        var drawing = RecordedDrawing("hit-answer");
        const double reach = 7 / 0.18;

        var card = drawing.Cards[0];
        var onCard = HitAt(drawing, card.X, card.Y, reach);

        Assert.Equal(card.Index, onCard?.Card?.Index);

        var edge = drawing.Links.First(link => link.Kind == GraphLinkKind.Base);
        var at = Halfway(drawing, edge);
        var onEdge = HitAt(drawing, at.X, at.Y, reach);

        Assert.Equal(HoverPart.Edge, onEdge?.Part);
        Assert.Equal(drawing.Links.ToList().IndexOf(edge), onEdge?.EdgeIndex);

        Assert.Null(HitAt(drawing, drawing.Left - 5000, drawing.Bottom + 5000, reach));
    }

    /// <summary>Lays out the graph of a workspace holding two files, one of them being edited.</summary>
    /// <param name="label">What the workspace is for, for a person reading the path.</param>
    /// <returns>The drawing.</returns>
    private static VariantGraphDrawing RecordedDrawing(string label)
    {
        var workspace = Recorded(label);

        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var graph = Graph(workspace);

        Assert.NotNull(graph);

        var art = graph.Entries.Single(entry => entry.Path == "art.txt");
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

        return VariantGraphDrawing.From(
            graph,
            [new DraftFact("art.txt", art.Version!, false, false, null)],
            words,
            new GraphMeasure(),
            emphasized: false
        );
    }

    /// <summary>What a hover is, as a value two walks can be compared by.</summary>
    /// <param name="hover">The answer.</param>
    /// <returns>The answer's parts.</returns>
    private static (int? Card, HoverPart Part, string? Chip, int Ghost, int Link) Summary(
        Hover? hover
    ) =>
        hover is null
            ? (null, HoverPart.Card, null, -1, -1)
            : (hover.Card?.Index, hover.Part, hover.Chip?.Text, hover.GhostIndex, hover.EdgeIndex);

    /// <summary>The answer the grid gives at a content point.</summary>
    /// <param name="drawing">The graph.</param>
    /// <param name="x">The point's x.</param>
    /// <param name="y">The point's y.</param>
    /// <param name="reach">How far a point is still on an edge.</param>
    /// <returns>The hover answer.</returns>
    private static Hover? HitAt(VariantGraphDrawing drawing, double x, double y, double reach)
    {
        var (cards, ghosts, links) = drawing.Hits.Near(x, y, reach);

        return VersionGraphView.Resolve(drawing, cards, ghosts, links, x, y, reach);
    }

    /// <summary>A point anywhere inside a card's box, pills and all.</summary>
    /// <param name="card">The card.</param>
    /// <param name="random">The seeded source of places.</param>
    /// <returns>The point.</returns>
    private static (double X, double Y) OverCard(GraphCard card, Random random) =>
        (
            card.CardLeft + (random.NextDouble() * (card.CardRight - card.CardLeft)),
            card.CardTop + (random.NextDouble() * (card.ContentBottom - card.CardTop))
        );

    /// <summary>A point within a pointer's reach of an edge's curve.</summary>
    /// <param name="drawing">The graph.</param>
    /// <param name="link">The edge.</param>
    /// <param name="random">The seeded source of places.</param>
    /// <param name="reach">How far the point may be placed from the curve.</param>
    /// <returns>The point.</returns>
    private static (double X, double Y) OverLink(
        VariantGraphDrawing drawing,
        GraphLink link,
        Random random,
        double reach
    )
    {
        var (x, y) = Halfway(drawing, link);

        return (
            x + ((random.NextDouble() - 0.5) * 2 * reach),
            y + ((random.NextDouble() - 0.5) * 2 * reach)
        );
    }

    /// <summary>The middle of an edge's curve, in content units.</summary>
    /// <param name="drawing">The graph.</param>
    /// <param name="link">The edge.</param>
    /// <returns>The point halfway along it.</returns>
    private static (double X, double Y) Halfway(VariantGraphDrawing drawing, GraphLink link)
    {
        var frame = new VariantGraphFrame(
            1,
            new Point(0, 0),
            null,
            null,
            string.Empty,
            false,
            false,
            null
        );
        var (start, end, control1, control2, _) = VariantGraphRenderer.GeometryOf(
            drawing.Cards[link.From],
            drawing.Cards[link.To],
            link.Kind,
            frame
        );
        const double t = 0.5;
        const double rest = 0.5;
        var x =
            (rest * rest * rest * start.X)
            + (3 * rest * rest * t * control1.X)
            + (3 * rest * t * t * control2.X)
            + (t * t * t * end.X);
        var y =
            (rest * rest * rest * start.Y)
            + (3 * rest * rest * t * control1.Y)
            + (3 * rest * t * t * control2.Y)
            + (t * t * t * end.Y);

        return (x, y);
    }

    /// <summary>Reads a window of the variant graph, or nothing when there is none to draw.</summary>
    /// <param name="directory">The directory the Workspace is searched upwards from.</param>
    /// <param name="request">What to read, or nothing for the work's own entries and their chains.</param>
    /// <returns>The reading, or nothing when there is nothing to draw.</returns>
    private static VariantGraphReading? Graph(
        string directory,
        VariantGraphRequest? request = null
    )
    {
        try
        {
            using var scope = new VariantGraphScope(directory, request);
            scope.Read();

            return scope.Reading;
        }
        catch (RolaReadError error)
            when (
                error.Kind
                is RolaReadErrorKind.InvalidArgument or RolaReadErrorKind.Unavailable
            )
        {
            // No Workspace, no Layout, no index, or a directory that is gone is the absence V1's
            // Locate answered with nothing rather than a failure.
            return null;
        }
    }

    /// <summary>Reads how the work under a directory stands, for the checks that need a reading.</summary>
    /// <param name="directory">The directory the read is of.</param>
    /// <returns>The reading, which the caller gives back.</returns>
    private static StatusReading Read(string directory)
    {
        var scope = new StatusScope(directory);
        scope.Read();

        return scope.Status;
    }

    /// <summary>Reads how a tree stands, for the checks that need a reading.</summary>
    /// <param name="tree">The tree's directory.</param>
    /// <returns>The reading, or nothing when the directory holds no tree.</returns>
    private static TreeStatus? TreeReading(string tree)
    {
        try
        {
            var scope = new TreeScope(tree);
            scope.Read();

            return scope.Status;
        }
        catch (RolaReadError error)
            when (
                error.Kind
                is RolaReadErrorKind.InvalidArgument
                    or RolaReadErrorKind.Unavailable
            )
        {
            return null;
        }
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

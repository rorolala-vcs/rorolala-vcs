using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;
using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;
using FileIcons = FileSystemPlugin.Icons;
using VcsIcons = RorolalaVCSPlugin.Icons;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The contributed column extension point: which providers a listing asks, where their columns sit, and
/// when what they answered is let go of.
/// </summary>
/// <remarks>
/// Nothing here draws — that needs a running Avalonia, and a column and its cells are built out of
/// controls — so what is exercised is the catalogue and what the plugins contribute to it.
/// </remarks>
public sealed class ColumnTests
{
    /// <summary>Begins each test from no provider at all.</summary>
    public ColumnTests() => EntryColumns.Clear();

    /// <summary>A mark says its literal first, and nothing at all when it has neither.</summary>
    /// <remarks>
    /// What a mark says is what the cell is written as, so it comes from one place. The literal wins because
    /// it is data: a path is not a key, however much it looks like one.
    /// </remarks>
    [Fact]
    public void AMarkSaysItsLiteralBeforeItsKey()
    {
        Assert.Equal("art/hero.psd", new EntryColumnMark(Text: "art/hero.psd").Said());
        Assert.Equal(
            "art/hero.psd",
            new EntryColumnMark(TextKey: "it.column.upstream", Text: "art/hero.psd").Said()
        );
        Assert.Null(new EntryColumnMark().Said());
        Assert.Null(new EntryColumnMark(Text: "", TextKey: "").Said());
    }

    /// <summary>A provider is asked only for the directory it speaks for.</summary>
    [Fact]
    public void AProviderIsAskedOnlyForTheDirectoryItSpeaksFor()
    {
        var provider = new FakeColumnProvider("it.column.one", "/work", ColumnPlacement.BesideName);

        EntryColumns.Register(provider);

        Assert.Equal([provider], EntryColumns.For("/work"));
        Assert.Empty(EntryColumns.For("/elsewhere"));
    }

    /// <summary>
    /// Every provider that speaks for a directory is a column of its own, in the order they were offered.
    /// </summary>
    /// <remarks>
    /// This is where a contributed column is not like a contributed lock: the columns are told apart by the
    /// key each is headed by, so two providers speaking for one directory are two columns rather than two
    /// answers to one question.
    /// </remarks>
    [Fact]
    public void EveryProviderThatSpeaksIsAColumnOfItsOwn()
    {
        var first = new FakeColumnProvider("it.column.first", "/work", ColumnPlacement.BesideName);
        var second = new FakeColumnProvider("it.column.second", "/work", ColumnPlacement.AfterValues);

        EntryColumns.Register(first);
        EntryColumns.Register(second);

        Assert.Equal([first, second], EntryColumns.For("/work"));
    }

    /// <summary>A provider of an identity already offered is ignored rather than replacing it.</summary>
    [Fact]
    public void AProviderOfAnIdentityAlreadyOfferedIsIgnored()
    {
        var first = new FakeColumnProvider("it.column.same", "/work", ColumnPlacement.BesideName);
        var second = new FakeColumnProvider("it.column.same", "/other", ColumnPlacement.BesideName);

        EntryColumns.Register(first);
        EntryColumns.Register(second);

        Assert.Equal([first], EntryColumns.All);
        Assert.Equal([first], EntryColumns.For("/work"));

        // The one that was ignored is not a column for the directory it spoke for either.
        Assert.Empty(EntryColumns.For("/other"));
    }

    /// <summary>A listing asks for no column at all until one is contributed.</summary>
    [Fact]
    public void NothingIsAColumnUntilSomethingIsRegistered()
    {
        Assert.Empty(EntryColumns.All);
        Assert.Empty(EntryColumns.For("/work"));
    }

    /// <summary>What the providers answered is let go of when the files may have changed.</summary>
    /// <remarks>
    /// What a provider answers is read from a tree a file operation changes — a Layout names each path by a
    /// `Uuid`, and a move renames one — and a listing is read again for the same change. A listing read from
    /// an answer that was not let go of is drawn from a tree that is gone, which is what this says cannot
    /// happen again.
    /// </remarks>
    [Fact]
    public void WhatTheProvidersAnsweredIsLetGoOfWhenTheFilesMayHaveChanged()
    {
        var provider = new FakeColumnProvider("it.column.forget", "/work", ColumnPlacement.BesideName);
        EntryColumns.Register(provider);

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared("/work", new HideRegistry(host.Config), host.Config);

        var readAgain = 0;
        shared.Touched += () => readAgain++;

        shared.FilesChanged();

        Assert.Equal(1, provider.Forgotten);
        Assert.Equal(1, readAgain);
    }

    /// <summary>
    /// Rorolala contributes the Upstream column beside the name, which is where a second name is read.
    /// </summary>
    /// <remarks>
    /// What the plugin contributes is a column of its own rather than only the lock it already had: the two
    /// are answers to different questions, and the one saying where upstream names an entry is read with the
    /// name rather than after what the entry is.
    /// </remarks>
    [Fact]
    public void ThePluginContributesTheUpstreamColumnBesideTheName()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        var upstream = Assert.Single(EntryColumns.All, column => column.Id == "rorolala.upstream");

        Assert.Equal("rorolala_vcs.column_upstream", upstream.ColumnKey);
        Assert.Equal(ColumnPlacement.BesideName, upstream.Placement);
        Assert.True(upstream.Width > 0);
    }

    /// <summary>Rorolala contributes the State mark at the left, with no heading of its own.</summary>
    /// <remarks>
    /// The mark is about the entry rather than a value of it, so it stands before everything, the picture
    /// included, where it is read first. It is headed by nothing: the listing heads a column from a key, and
    /// there is no word that would fit over a column of dots — what the mark means is said on the mark.
    /// </remarks>
    [Fact]
    public void ThePluginContributesTheStateMarkAtTheLeft()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        var state = Assert.Single(EntryColumns.All, column => column.Id == "rorolala.states");

        Assert.Empty(state.ColumnKey);
        Assert.Equal(ColumnPlacement.Leftmost, state.Placement);
        Assert.True(state.Width <= 24, $"{state.Width}");

        var upstream = Assert.Single(EntryColumns.All, column => column.Id == "rorolala.upstream");

        Assert.Equal(ColumnPlacement.BesideName, upstream.Placement);
    }

    /// <summary>
    /// Every state a file of the work can be in is a dot in a colour with its word on the dot, and the two a
    /// listing has no entry for are no mark at all.
    /// </summary>
    /// <remarks>
    /// What each state is drawn as is a role the listing resolves to the theme's resource for it: green for
    /// what appeared, the user's own colour for what changed, the error red for what must not be recorded,
    /// the second colour for what moved, and the one amber for what is waiting. A dot says nothing in a
    /// column, since a cell holding both a dot and a word would be a dot drawn for nothing — what it means is
    /// the word it is hovered for.
    /// </remarks>
    [Fact]
    public void EveryStateIsADotInAColourWithItsWordOnIt()
    {
        _ = Host.Services();

        Assert.Equal(RolaI18N.Get("rorolala_vcs.state.untagged"), RorolalaStates.Say(PathState.Untagged).Hint);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.state.modified"), RorolalaStates.Say(PathState.Modified).Hint);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.state.wrong"), RorolalaStates.Say(PathState.Wrong).Hint);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.state.renamed"), RorolalaStates.Say(PathState.Renamed).Hint);
        Assert.Equal(
            RolaI18N.Get("rorolala_vcs.state.renamed_likely"),
            RorolalaStates.Say(PathState.RenamedLikely).Hint
        );
        Assert.Equal(RolaI18N.Get("rorolala_vcs.state.merging"), RorolalaStates.Say(PathState.Merging).Hint);

        Assert.Equal(RorolalaColours.Untagged, RorolalaStates.Say(PathState.Untagged).Colour);
        Assert.Equal(RorolalaColours.UnderWay, RorolalaStates.Say(PathState.Modified).Colour);
        Assert.Equal(RorolalaColours.Wrong, RorolalaStates.Say(PathState.Wrong).Colour);
        Assert.Equal(RorolalaColours.Moved, RorolalaStates.Say(PathState.Renamed).Colour);
        Assert.Equal(RorolalaColours.Moved, RorolalaStates.Say(PathState.RenamedLikely).Colour);
        Assert.Equal(RorolalaColours.UnderWay, RorolalaStates.Say(PathState.Merging).Colour);

        // Every one of them is a dot with a line of its own, and every dot says its word on itself rather than
        // in its cell.
        foreach (var state in new[]
        {
            PathState.Untagged,
            PathState.Modified,
            PathState.Wrong,
            PathState.Renamed,
            PathState.RenamedLikely,
            PathState.Merging,
        })
        {
            var mark = RorolalaStates.Say(state);

            Assert.True(mark.Dot, $"{state}");
            Assert.Null(mark.Said());

            // The line around it is a shade of the mark rather than the mark itself, so that the mark reads
            // against the ground it is drawn on.
            Assert.NotNull(mark.Edge);
            Assert.NotEqual(mark.Colour, mark.Edge);
        }

        Assert.False(RorolalaStates.Say(PathState.Lost).Dot);
        Assert.False(RorolalaStates.Say(PathState.Unknown).Dot);
    }

    /// <summary>The Standing column writes a count where there is one, and says so where there is not.</summary>
    /// <remarks>
    /// What the column is for is how far apart the two sides are, and that is a number only while the chain
    /// that counts is here: a fetch brings the Vault's Layout and never its versions, so being behind is
    /// ordinarily a direction without a count, and two sides that are not on one chain have no count at all.
    /// The three are told apart because what a reader does about them differs — one is work to fetch, one is
    /// work to fetch by an unknown amount, and one cannot be settled by moving anything.
    /// </remarks>
    [Fact]
    public void TheStandingColumnWritesACountWhereThereIsOne()
    {
        _ = Host.Services();

        // Ahead and behind are the count the chain gave, with the sign saying which side holds what.
        var ahead = RorolalaStanding.Say(new Standing(StandingRelation.Ahead, 2));
        Assert.Equal("+2", ahead.Text);
        Assert.Equal(RorolalaColours.Untagged, ahead.Colour);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.standing.ahead", 2UL), ahead.Hint);

        var behind = RorolalaStanding.Say(new Standing(StandingRelation.Behind, 1));
        Assert.Equal("-1", behind.Text);
        Assert.Equal(RorolalaColours.Wrong, behind.Colour);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.standing.behind", 1UL), behind.Hint);

        // Behind with no count is the dash in the red of what must not be recorded, hovered for which side
        // holds what — said on itself, since only the reading knows which of the two it is.
        var untold = RorolalaStanding.Say(new Standing(StandingRelation.Behind, null));
        Assert.Equal("-", untold.Text);
        Assert.Equal(RorolalaColours.Wrong, untold.Colour);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.standing.behind_hover"), untold.Hint);

        // Two chains that parted have no number between them at all, which is what the question mark says.
        var apart = RorolalaStanding.Say(new Standing(StandingRelation.Apart, null));
        Assert.Equal("?", apart.Text);
        Assert.Equal(RorolalaColours.Wrong, apart.Colour);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.standing.apart"), apart.Hint);

        // In step, and a standing nothing says: the listing's own dash, and no ink of its own.
        Assert.Equal("-", RorolalaStanding.Say(new Standing(StandingRelation.Same, 0)).Text);
        Assert.Equal("-", RorolalaStanding.Say(new Standing(StandingRelation.Unknown, null)).Text);
        Assert.Null(RorolalaStanding.Say(new Standing(StandingRelation.Same, 0)).Colour);
    }

    /// <summary>A tile wears an arrow for the direction, and a fork where there is no direction.</summary>
    /// <remarks>
    /// The badge is the one thing a tile has room for, so what it says is the direction the work would have to
    /// move: up for work waiting to be sent, down for work waiting to be fetched, and the fork where the two
    /// sides have each moved on — which is the one standing no arrow can say.
    /// </remarks>
    [Fact]
    public void ATileWearsAnArrowForTheDirectionAndAForkWhereThereIsNone()
    {
        _ = Host.Services();

        var ahead = RorolalaStandingBadge.Say(new Standing(StandingRelation.Ahead, 2));
        Assert.Equal(VcsIcons.Upload, ahead.IconKey);
        Assert.Equal(MarkInk.Add, ahead.Ink);

        // Behind is one answer with the count and without it: what a reader does about either is fetch.
        var behind = RorolalaStandingBadge.Say(new Standing(StandingRelation.Behind, 1));
        Assert.Equal(VcsIcons.Download, behind.IconKey);
        Assert.Equal(MarkInk.Error, behind.Ink);

        var untold = RorolalaStandingBadge.Say(new Standing(StandingRelation.Behind, null));
        Assert.Equal(VcsIcons.Download, untold.IconKey);
        Assert.Equal(MarkInk.Error, untold.Ink);

        var apart = RorolalaStandingBadge.Say(new Standing(StandingRelation.Apart, null));
        Assert.Equal(VcsIcons.CallSplit, apart.IconKey);
        Assert.Equal(MarkInk.Error, apart.Ink);

        // Every one of them is worn on the badge's own ground, since a tile carries marks of several kinds at
        // once and a bare picture and a badge would be two of them.
        foreach (var mark in new[] { ahead, behind, untold, apart })
        {
            Assert.True(mark.Tagged);
            Assert.NotNull(mark.HoverKey);
        }

        // In step, and a standing nothing says: no badge at all.
        Assert.Null(RorolalaStandingBadge.Say(new Standing(StandingRelation.Same, 0)).IconKey);
        Assert.Null(RorolalaStandingBadge.Say(new Standing(StandingRelation.Unknown, null)).IconKey);
    }

    /// <summary>A column of marks says nothing about an entry by having no mark at all.</summary>
    /// <remarks>
    /// The dash is how the listing says what it does not know, and a dash where a mark would be is a mark of
    /// its own: a file that stands as it was recorded is a file the column has nothing to say about, and that
    /// is said by drawing nothing — which is also the room the name beside it is read in.
    /// </remarks>
    [Fact]
    public void AColumnOfMarksDrawsNothingWhereTheEntryHasNoMark()
    {
        var marks = new FakeColumnProvider("it.column.marks", "/work", ColumnPlacement.Leftmost, marks: true);
        var dots = new FakeColumnProvider(
            "it.column.dots",
            "/work",
            ColumnPlacement.Leftmost,
            marks: true,
            dot: true
        );
        var values = new FakeColumnProvider("it.column.values", "/work", ColumnPlacement.BesideName);
        var entry = new Entry("hero.psd", EntryKind.File);

        // A mark for the entry is a mark, and no mark at all is nothing drawn.
        Assert.IsType<Border>(ListBrowser.Contributed(dots, entry));
        Assert.Empty(Assert.IsType<Panel>(ListBrowser.Contributed(marks, entry)).Children);

        // A column of values says the same thing the way it says everything it does not know.
        Assert.Equal("-", Assert.IsType<TextBlock>(ListBrowser.Contributed(values, entry)).Text);
    }

    /// <summary>A state is worn on a tile as a picture in the colour the dot beside a name is.</summary>
    /// <remarks>
    /// A tile has no column to draw a dot in, so the state is a badge there — and what a state is coloured is
    /// the state, so the badge and the dot are the same colour. Two states share a picture where what a reader
    /// does about them is the same thing: a file that is only possibly renamed is renamed as far as that goes,
    /// and one that changed and one waiting to be joined are both content on its way to a version. What tells
    /// those apart is the word on the badge, which is the word the dot is hovered for.
    /// </remarks>
    [Fact]
    public void AStateIsWornOnATileAsAPictureInItsColour()
    {
        _ = Host.Services();

        Assert.Equal(VcsIcons.Add, RorolalaStates.Badge(PathState.Untagged).IconKey);
        Assert.Equal(VcsIcons.EditNote, RorolalaStates.Badge(PathState.Modified).IconKey);
        Assert.Equal(VcsIcons.PriorityHigh, RorolalaStates.Badge(PathState.Wrong).IconKey);
        Assert.Equal(VcsIcons.ArrowRightAlt, RorolalaStates.Badge(PathState.Renamed).IconKey);
        Assert.Equal(VcsIcons.ArrowRightAlt, RorolalaStates.Badge(PathState.RenamedLikely).IconKey);
        Assert.Equal(VcsIcons.EditNote, RorolalaStates.Badge(PathState.Merging).IconKey);

        // The colours are the ones the dots are, since what a state is coloured is the state and not the shape
        // it is drawn in.
        foreach (var state in new[] { PathState.Untagged, PathState.Modified, PathState.Wrong, PathState.Renamed })
        {
            Assert.Equal(RorolalaStates.Say(state).Colour, RorolalaStates.Badge(state).Colour);
        }

        // Every one of them is worn on the badge's own ground, and says on itself what the dot says on itself.
        foreach (var state in new[]
        {
            PathState.Untagged,
            PathState.Modified,
            PathState.Wrong,
            PathState.Renamed,
            PathState.RenamedLikely,
            PathState.Merging,
        })
        {
            var worn = RorolalaStates.Badge(state);

            Assert.True(worn.Tagged, $"{state}");
            Assert.Null(worn.HoverKey);
            Assert.Equal(RorolalaStates.Say(state).Hint, worn.Hover());
        }

        // A path the Layout names and the tree has lost is not an entry of any listing, and one the reading
        // says nothing about stands as it was recorded: neither wears a badge.
        Assert.Null(RorolalaStates.Badge(PathState.Lost).IconKey);
        Assert.Null(RorolalaStates.Badge(PathState.Unknown).IconKey);
    }

    /// <summary>A provider that speaks for one directory and says one thing about an entry.</summary>
    /// <param name="id">The identity it is offered under.</param>
    /// <param name="directory">The directory it answers for.</param>
    /// <param name="placement">Where it asks its column to sit.</param>
    /// <param name="marks">Whether its column is one of marks rather than of values.</param>
    /// <param name="dot">Whether it answers a mark for an entry rather than nothing at all.</param>
    private sealed class FakeColumnProvider(
        string id,
        string directory,
        ColumnPlacement placement,
        bool marks = false,
        bool dot = false
    ) : IEntryColumnProvider
    {
        /// <inheritdoc />
        public string Id => id;

        /// <inheritdoc />
        public string ColumnKey => "it.column";

        /// <inheritdoc />
        public double Width => 120;

        /// <inheritdoc />
        public ColumnPlacement Placement => placement;

        /// <inheritdoc />
        public bool Marks => marks;

        /// <summary>How many times it was told what it answered may have changed.</summary>
        public int Forgotten { get; private set; }

        /// <inheritdoc />
        public bool Applies(string at) => string.Equals(at, directory, StringComparison.Ordinal);

        /// <inheritdoc />
        public EntryColumnMark Value(Entry entry) =>
            dot
                ? new EntryColumnMark(Ink: MarkInk.Add, Dot: true, Hint: "it.column.dot")
                : new EntryColumnMark();

        /// <inheritdoc />
        public void Forget() => Forgotten++;
    }
}

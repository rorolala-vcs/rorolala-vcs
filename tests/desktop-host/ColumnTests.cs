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

        Assert.Equal(MarkInk.Add, RorolalaStates.Say(PathState.Untagged).Ink);
        Assert.Equal(MarkInk.Primary, RorolalaStates.Say(PathState.Modified).Ink);
        Assert.Equal(MarkInk.Error, RorolalaStates.Say(PathState.Wrong).Ink);
        Assert.Equal(MarkInk.Accent, RorolalaStates.Say(PathState.Renamed).Ink);
        Assert.Equal(MarkInk.Accent, RorolalaStates.Say(PathState.RenamedLikely).Ink);
        Assert.Equal(MarkInk.Warn, RorolalaStates.Say(PathState.Merging).Ink);

        // Every one of them is a dot, and every dot says its word on itself rather than in its cell.
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
        }

        Assert.False(RorolalaStates.Say(PathState.Lost).Dot);
        Assert.False(RorolalaStates.Say(PathState.Unknown).Dot);
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

    /// <summary>A mark that is a dot is a round mark drawn against whatever it is put beside.</summary>
    /// <remarks>
    /// Where a dot stands is the view's — beside a name in a tile's caption, in a column in a list — so what
    /// the plugin draws is the mark itself and the room around it is the view's. What it means is the dot's,
    /// and it is said at once when the pointer rests on it.
    /// </remarks>
    [Fact]
    public void AMarkIsADotDrawnAgainstWhatItStandsBeside()
    {
        var dot = Assert.IsType<Border>(
            FileIcons.Dot(new EntryColumnMark(Ink: MarkInk.Add, Dot: true, Hint: "it"), 64)
        );

        Assert.Equal(VerticalAlignment.Center, dot.VerticalAlignment);
        Assert.Equal(dot.Width, dot.Height);
        Assert.Equal(new CornerRadius(dot.Width / 2), dot.CornerRadius);
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

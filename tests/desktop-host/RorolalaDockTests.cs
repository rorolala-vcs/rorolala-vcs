using Avalonia.Controls;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;
using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;
// Both plugins publish an `Operation` — theirs are the file operations — so what a row of the Overview
// offers is named by the one its buttons are.
using Operation = RorolalaVCSPlugin.Operation;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Rorolala plugin's own docks: which there are, and what one says about where the work is.
/// </summary>
/// <remarks>
/// Nothing here draws, and nothing here makes a window. What is exercised is what the <c>Window</c> menu
/// is built from, the judgement a dock's view is filled by, and the control that judgement is put into —
/// which is what a run with no Workspace under its base directory comes to.
/// </remarks>
public sealed class RorolalaDockTests
{
    /// <summary>Begins each test from no lock provider at all, since starting the plugin adds one.</summary>
    public RorolalaDockTests()
    {
        EntryLockProviders.Clear();
        EntryMarkProviders.Clear();
    }

    /// <summary>The plugin adds one dock for each view it shows, named and titled apart.</summary>
    [Fact]
    public void ThePluginRegistersADockForEachView()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        Assert.Equal(
            [
                ("rorolala.vcs.overview", "rorolala_vcs.dock.overview"),
                ("rorolala.vcs.workspace", "rorolala_vcs.dock.workspace"),
                ("rorolala.vcs.version_graph", "rorolala_vcs.dock.version_graph"),
                ("rorolala.vcs.accounts", "rorolala_vcs.dock.accounts"),
            ],
            services.Docks.Registrations.Select(dock => (dock.DockNameId, dock.DisplayNameKey))
        );

        // One view of the work is one dock: asking for one that is open shows it rather than making a
        // second, which is what the Window menu's list of them is read against.
        Assert.All(
            services.Docks.Registrations,
            dock =>
            {
                Assert.Equal(plugin.Manifest.Id, dock.Owner);
                Assert.Equal(DockOpenMode.Toggle, dock.OpenMode);
            }
        );
    }

    /// <summary>The plugin offers the work's upstream as a place the browser opens.</summary>
    /// <remarks>
    /// A place rather than a dock: what is read at <c>rola@</c> is drawn by the browser's own listing, so what this
    /// checks is that the prefix is answered at all and that a directory's menu offers the way there.
    /// </remarks>
    [Fact]
    public void ThePluginOffersTheUpstreamAsAPlace()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        Assert.NotNull(services.LogicalViews.Find(UpstreamPlace.Scheme));

        Assert.Contains(
            services.ContextMenus.Items(ContextMenuTarget.Directory),
            item => item.LabelKey == "rorolala_vcs.menu.upstream"
        );
    }

    /// <summary>A base directory no Workspace holds is said so rather than left blank.</summary>
    /// <remarks>
    /// A scratch directory is under no Workspace, and a directory that was never said — nothing, or the
    /// computer, which is not a path at all — is under none either. What a dock does with the answer is
    /// that it shows these words, so what is checked is that there are words to show.
    /// </remarks>
    [Fact]
    public void ABaseDirectoryNoWorkspaceHoldsIsSaidSo()
    {
        var services = Host.Services();
        var plugin = new Vcs();
        var host = services.For(plugin.Manifest.Id, 1);

        string?[] bases = [Scratch.New("outside"), null, string.Empty];

        foreach (var directory in bases)
        {
            var said = WorkspaceNotice.Notice(directory, host.Log);

            Assert.False(string.IsNullOrEmpty(said));
        }
    }

    /// <summary>
    /// A dock made while no Workspace holds the base is a control holding something, not an empty one.
    /// </summary>
    /// <remarks>
    /// The judgement and the control are two halves, and a dock that decides rightly and carries nothing
    /// is a window the user reads as an empty work: what is checked here is that what the registration
    /// makes is a control with content in it. The two views that are not the Overview say the words as
    /// their only line; the Overview draws the work when there is work to draw, so what it carries is
    /// checked by <see cref="TheOverviewSaysABaseOutsideEveryWorkspaceIsNotOne"/>.
    /// <para>
    /// Nothing about a Workspace is asked, since the base published by an earlier test of this run is no
    /// more a Workspace than a fresh one is — and the line is what a base outside every Workspace comes
    /// to either way.
    /// </para>
    /// </remarks>
    [Fact]
    public void ADockOutsideEveryWorkspaceCarriesSomethingItIsMadeWith()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        // A base nothing holds, which is the state a run outside every Workspace is in.
        BaseDirectory.Publish(Scratch.New("outside"));

        foreach (var registration in services.Docks.Registrations)
        {
            var view = registration.Create().View;

            Assert.NotNull(view);

            // The three docks that show something of their own rather than a line about the base: the
            // Overview's table, the Accounts dock, which is about the machine rather than about a base
            // at all, and the Version Graph, which draws one file's chain. What they show is their own
            // control rather than a `UserControl` holding one line, so what is checked about them is
            // that the registration made something; the words are checked for the docks made of a line.
            if (
                registration.DockNameId == RorolalaDocks.OverviewNameId
                || registration.DockNameId == RorolalaDocks.AccountsNameId
                || registration.DockNameId == RorolalaDocks.VersionGraphNameId
            )
            {
                continue;
            }

            var dock = Assert.IsAssignableFrom<UserControl>(view);
            var line = Assert.IsType<TextBlock>(dock.Content);

            Assert.True(line.IsVisible);
            Assert.False(string.IsNullOrEmpty(line.Text));
        }
    }

    /// <summary>
    /// The Overview has nothing to draw for a base no Workspace holds, and says so in words.
    /// </summary>
    /// <remarks>
    /// What is exercised is the judgement the view is filled by, which is the same one the command line
    /// makes: there is no reading, and there is something to say instead. A base that was never said,
    /// the empty string and the computer are all somewhere no Workspace can be found from.
    /// </remarks>
    [Fact]
    public void TheOverviewSaysABaseOutsideEveryWorkspaceIsNotOne()
    {
        // A plugin's own view of the host's log, which is what a view is handed: the one log is the
        // host's and what it hands a plugin is that log attributed to the plugin.
        ILog log = Host.Services().For(new PluginId("it.overview"), 1).Log;

        string?[] bases = [Scratch.New("outside"), null, string.Empty];

        foreach (var directory in bases)
        {
            var reading = OverviewView.Of(directory, log);

            Assert.Null(reading.Facts);
            Assert.False(string.IsNullOrEmpty(reading.Said), $"{directory}");
        }
    }

    /// <summary>The Overview's header carries the strip that holds the reading and the sync together.</summary>
    /// <remarks>
    /// It is one strip rather than a run of commands, because what stands there is more than buttons: the three
    /// sync buttons belong together, their colour answers a key the user is holding, and what they are to leave
    /// out is a dropdown. The host puts the control in its header and asks nothing about what is inside it, so
    /// what this pins is that the dock hands one over at all — which is the whole of what the header says — and
    /// that reading the work again is still in it, drawn from a picture the plugin contributes: the library is
    /// the shell's, and a plugin's pictures are found in it by key.
    /// </remarks>
    [Fact]
    public void TheOverviewOffersToReadTheWorkAgain()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        var overview = services.Docks.Registrations.Single(dock =>
            dock.DockNameId == RorolalaDocks.OverviewNameId
        );
        var view = overview.Create();
        var strip = Assert.IsType<DockHeaderView>(Assert.Single(view.Header));

        Assert.IsType<SyncStrip>(strip.View);

        Assert.Contains(
            "rorolala_vcs.refresh",
            global::RorolalaVCSPlugin.Icons.Pictures.Select(picture => picture.Key)
        );
    }

    /// <summary>A line in the middle of the dock is said only where the table has nothing to show.</summary>
    /// <remarks>
    /// The two say different things at once: a work with changes is the table, and the words for a work without
    /// them are what stands in its place. A line left over a table is a dock saying both at once — that there
    /// is nothing to confirm, over the things to confirm.
    /// </remarks>
    [Fact]
    public void ALineIsSaidOnlyWhereTheTableHasNothingToShow()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: ["gone.txt"],
            Untagged: [],
            Renamed: [],
            Modified: [],
            Unowned: [],
            WrongBase: [],
            Merging: [],
            Broken: [],
            Sends: [],
            Receives: [],
            Apart: []
        );

        OverviewReading read = new(facts, null);
        OverviewReading unread = new(null, "it.overview.unreadable");

        // A reading with nothing in it says so, and a reading that could not be made says why.
        Assert.Equal(RolaI18N.Get("rorolala_vcs.status.clean"), OverviewView.Line(read, 0));
        Assert.Equal("it.overview.unreadable", OverviewView.Line(unread, 0));

        // A reading with a row in it says nothing in the middle: the row is the whole of what is said.
        Assert.Null(OverviewView.Line(read, 1));
        Assert.Null(OverviewView.Line(unread, 1));
    }

    /// <summary>Every change a reading holds becomes one row, and no more.</summary>
    /// <remarks>
    /// The table is the whole of what the Overview shows, so what it is built from is the one thing worth
    /// checking without a window: a change of each kind becomes a row of its own, the paths land in the
    /// columns they belong in, and the state a row is in is the pills it wears — the kind of change it is,
    /// and what became of its content.
    /// </remarks>
    [Fact]
    public void EveryChangeBecomesOneRowOfTheTable()
    {
        // The translations are named before anything is asked for them, which is what a run does: what a
        // key reads as here is the key, since the catalogue is a scratch directory that states nothing.
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: ["gone.txt"],
            Untagged: ["new.txt"],
            Renamed: [new Rename("a.txt", "b.txt", Modified: false, Strong: true)],
            Modified: ["kept.txt", "stale.psd"],
            Unowned: ["theirs.psd"],
            WrongBase: ["stale.psd"],
            Merging: [new Merge("merge/file.txt", "1a2b3c4d5e6f", "merge/file.1a2b3c4.txt", null)],
            Broken: [],
            Sends: [],
            Receives: [],
            Apart: []
        );

        var rows = OverviewView.Rows(facts);

        Assert.Equal(7, rows.Count);

        // What is wrong is read first — another account's before a base upstream has moved past — then what
        // is under way, then what the Layout and the tree disagree about, and last what is to be recorded.
        // A pill is read by its letters, which is what a column of states is scanned by.
        Assert.Equal(
            ["E", "E", "J", "D", "A", "R", "M"],
            rows.Select(Pills)
        );

        // What a row has to say that is not a state is written rather than coloured, and only a merge that
        // cannot go on has anything: a move that changed nothing says nothing beyond being a move, and a merge
        // that can go on says which variant it waits for on its pill and where its two files are in the two
        // columns.
        Assert.Equal(
            [string.Empty, string.Empty, string.Empty, string.Empty, string.Empty, string.Empty, string.Empty],
            rows.Select(Notes)
        );

        // A change of content has no kind of its own, so the row it makes is one pill saying what became of
        // it, and the two that are wrong carry the paths they are about.
        Assert.Equal(("theirs.psd", string.Empty), (rows[0].From, rows[0].To));
        Assert.Equal(("stale.psd", string.Empty), (rows[1].From, rows[1].To));

        // A merge is read the way its content would move: the variant file it is joining comes first and the
        // file the Layout names, which that content is to become part of, is what it goes to.
        Assert.Equal(("merge/file.1a2b3c4.txt", "merge/file.txt"), (rows[2].From, rows[2].To));

        // What the Layout names and the tree has lost comes from a path and goes nowhere, and what the tree
        // holds and the Layout does not name is read the same way round rather than the other: an untagged
        // entry names no path the Layout has, so which column carries it is the table's choice and not the
        // reading's, and it is written where what the Layout names goes. A move is read after both, since what
        // it says is where a file went rather than that one is missing.
        Assert.Equal(("gone.txt", string.Empty), (rows[3].From, rows[3].To));
        Assert.Equal(("new.txt", string.Empty), (rows[4].From, rows[4].To));
        Assert.Equal(("a.txt", "b.txt"), (rows[5].From, rows[5].To));

        Assert.Equal(("kept.txt", string.Empty), (rows[6].From, rows[6].To));
    }

    /// <summary>A path a sync would move is a row of the last group, with no button of its own.</summary>
    /// <remarks>
    /// What is in step with nothing wrong about it is still a file a sync would carry, and the Overview is where it
    /// is read which way the work moves: sent, fetched, created, or settled only by a forced sync. The path is a row
    /// of its own only where no other row already names it — a path that changed here and is behind is on the row of
    /// its change, wearing the one pill saying its base is wrong — and the row offers no button, because the run that
    /// settles it is the strip's sync rather than one entry of the table.
    /// </remarks>
    [Fact]
    public void WhatASyncWouldMoveIsReadLastAndOffersNoButton()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: [],
            Untagged: [],
            Renamed: [],
            Modified: [],
            Unowned: [],
            WrongBase: [],
            Merging: [],
            Broken: [],
            Sends: ["ahead.psd", "edited.psd"],
            Receives: ["behind.psd"],
            Apart: ["apart.psd"]
        );

        // The edited path is in the reading as a change as well, and the change is the row it is read on.
        var rows = OverviewView.Rows(facts with { Modified = ["edited.psd"] });

        // The change is one group above, and the three the reading says nothing else about are the last.
        Assert.Equal(
            ["M", "S↑", "S↓", "Sx"],
            rows.Select(Pills)
        );

        // Each is written where it comes from, with nothing on the other side, and none of the sync rows is a
        // button: what settles them is the strip's sync rather than a run of one entry. The change above keeps its.
        Assert.Equal(("edited.psd", string.Empty), (rows[0].From, rows[0].To));
        Assert.Equal(("ahead.psd", string.Empty), (rows[1].From, rows[1].To));
        Assert.Equal(("behind.psd", string.Empty), (rows[2].From, rows[2].To));
        Assert.Equal(("apart.psd", string.Empty), (rows[3].From, rows[3].To));
        Assert.All(rows.Skip(1), row => Assert.Empty(row.Operations));
    }

    /// <summary>A merge can be cancelled, and what is cancelled is read from the variant file.</summary>
    /// <remarks>
    /// A merge is the one row whose button acts on its <c>From</c> rather than on where its file is now: the
    /// command that ends it is run on the variant file, and the file the variant is for is left as it holds.
    /// What the pill is read by is the head of the variant's hash, which the row's columns do not carry.
    /// </remarks>
    [Fact]
    public void AMergeCanBeCancelledFromTheVariantFile()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: [],
            Untagged: [],
            Renamed: [],
            Modified: [],
            Unowned: [],
            WrongBase: [],
            Merging:
            [
                new Merge("merge/file.txt", "1a2b3c4d5e6f", "merge/file.txt.v", null),
                new Merge("merge/other.psd", "abcdef0123456789", "merge/other.psd.v", "merge/that.psd.v"),
            ],
            Broken: [],
            Sends: [],
            Receives: [],
            Apart: []
        );

        var rows = OverviewView.Rows(facts);

        Assert.Equal(2, rows.Count);

        // The first variant file is where the record puts it, and the second is where the tree holds it: what
        // the row reads is the file a reader can see rather than the place a record remembers.
        Assert.Equal(("merge/file.txt.v", "merge/file.txt"), (rows[0].From, rows[0].To));
        Assert.Equal(("merge/that.psd.v", "merge/other.psd"), (rows[1].From, rows[1].To));

        // What ends a merge is the variant file, which is the path the row comes from rather than the one it
        // goes to: the file the variant is for is left as it holds now.
        Assert.Equal(rows[0].From, rows[0].Act);
        Assert.Equal(rows[1].From, rows[1].Act);
        Assert.Equal([Operation.CancelMerge], rows[0].Operations);
        Assert.Equal([Operation.CancelMerge], rows[1].Operations);

        // The pill is the merge's own, and the whole of what it means is which variant is waiting: the head of
        // the hash, which is what a person recognises a variant by and what the row's columns do not carry.
        Assert.Equal("J", Assert.Single(rows[0].Pills).Letters);
        Assert.Equal(
            RolaI18N.Get("rorolala_vcs.variant.pending", VariantFiles.Head("1a2b3c4d5e6f")),
            Assert.Single(rows[0].Pills).Hint
        );
        Assert.Equal(
            RolaI18N.Get("rorolala_vcs.variant.pending", VariantFiles.Head("abcdef0123456789")),
            Assert.Single(rows[1].Pills).Hint
        );
        Assert.Empty(rows[0].Notes);
        Assert.Empty(rows[1].Notes);
    }

    /// <summary>A move whose content is wrong is read among the wrong changes rather than among the moves.</summary>
    /// <remarks>
    /// A row stands in one group, and it is the group of the state that has to be acted on first: the move is
    /// a move whichever group it is read in, and what the order says is which of the two a reader meets first.
    /// </remarks>
    [Fact]
    public void AMoveWithAWrongBaseIsReadWithTheWrongChanges()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: [],
            Untagged: [],
            Renamed:
            [
                new Rename("c.txt", "d.txt", Modified: false, Strong: true),
                new Rename("a.txt", "b.txt", Modified: true, Strong: true),
            ],
            Modified: ["b.txt"],
            Unowned: [],
            WrongBase: ["b.txt"],
            Merging: [],
            Broken: [],
            Sends: [],
            Receives: [],
            Apart: []
        );

        var rows = OverviewView.Rows(facts);

        Assert.Equal(2, rows.Count);
        Assert.Equal("b.txt", rows[0].To);
        Assert.Equal("d.txt", rows[1].To);
    }

    /// <summary>A move whose file was edited is one row, which says both things about it.</summary>
    /// <remarks>
    /// The destination of a move is among the changes of content as well — what it holds is not what the
    /// Layout agreed with at the path it came from — so a table that took both lists at their word would
    /// show one file twice, once as a move and once as a change, with nothing to say they are the same.
    /// </remarks>
    [Fact]
    public void AMoveThatWasEditedIsOneRow()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: [],
            Untagged: [],
            Renamed: [new Rename("a.txt", "b.txt", Modified: true, Strong: false)],
            Modified: ["b.txt"],
            Unowned: [],
            WrongBase: [],
            Merging: [],
            Broken: [],
            Sends: [],
            Receives: [],
            Apart: []
        );

        var row = Assert.Single(OverviewView.Rows(facts));

        // A move whose file was edited is two states on one row, because the move and what became of its
        // content are two things to know.
        Assert.Equal("R? + M", Pills(row));
        Assert.Equal(("a.txt", "b.txt"), (row.From, row.To));
    }

    /// <summary>A merge that cannot go on still names the file it is for.</summary>
    /// <remarks>
    /// The reason crosses as a kind and what it is about; the file the variant is for is the one thing the
    /// row's own columns want and no reason carries, so it is asked for beside them — and the one reason
    /// where the Layout names no such file is the one that answers with nothing.
    /// </remarks>
    [Fact]
    public void AMergeThatCannotGoOnNamesTheFileItIsFor()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: [],
            Untagged: [],
            Renamed: [],
            Modified: [],
            Unowned: [],
            WrongBase: [],
            Merging: [],
            Broken:
            [
                new BrokenReason(BrokenKind.FileGone, "merge/hero.txt", "merge/hero.psd"),
                new BrokenReason(BrokenKind.NoTarget, "1a2b3c4d5e6f", string.Empty),
            ],
            Sends: [],
            Receives: [],
            Apart: []
        );

        var rows = OverviewView.Rows(facts);

        Assert.Equal(2, rows.Count);

        // Read the same way round as a merge that can go on: the reason names the variant file, which comes
        // first, and the file it is for is what it goes to.
        Assert.Equal(("merge/hero.txt", "merge/hero.psd"), (rows[0].From, rows[0].To));

        // A merge is one state whatever became of it, so the row wears one pill: the colour is what says that
        // this merge is a thing gone wrong, and the reason is in the note, where there is room to say it.
        Assert.Single(rows[0].Pills);
        Assert.Equal("J", rows[0].Pills[0].Letters);
        Assert.StartsWith(
            RolaI18N.Get("rorolala_vcs.status.merge_broken"),
            Assert.Single(rows[0].Notes),
            StringComparison.Ordinal
        );

        // It can be cancelled like any other, from the variant file the reason named.
        Assert.Equal(rows[0].From, rows[0].Act);
        Assert.Equal([Operation.CancelMerge], rows[0].Operations);

        // The Layout naming no such file is a reason rather than a missing answer, so the row is there with its
        // pill and its reason, and the columns that want a path are the ones left empty — and a row with no
        // path is one nothing can be done from, so it is offered no button.
        Assert.Equal((string.Empty, string.Empty), (rows[1].From, rows[1].To));
        Assert.Null(rows[1].Act);
        Assert.Empty(rows[1].Operations);
        Assert.Single(rows[1].Pills);
        Assert.True(
            Assert.Single(rows[1].Notes).Length > RolaI18N.Get("rorolala_vcs.status.merge_broken").Length,
            Assert.Single(rows[1].Notes)
        );
    }

    /// <summary>Two readings that say the same thing are one drawing, and any difference makes two.</summary>
    /// <remarks>
    /// The dock reads the work again on a timer, and what it reads is what it draws: a reading that says
    /// what the last one said must leave the table alone, or the reader loses what they had scrolled to,
    /// chosen and were pointing at every few seconds. That rests on two readings built apart comparing equal
    /// — which a row holding a list of pills is where it is easy to get wrong, since two lists that say the
    /// same thing are not the same list.
    /// </remarks>
    [Fact]
    public void ReadingsThatSayTheSameThingAreTheSameDrawing()
    {
        _ = Host.Services();

        StatusFacts facts = new(
            Lost: ["gone.txt"],
            Untagged: ["new.txt"],
            Renamed: [new Rename("a.txt", "b.txt", Modified: true, Strong: true)],
            Modified: ["a-b.txt", "kept.txt"],
            Unowned: ["theirs.psd"],
            WrongBase: [],
            Merging: [new Merge("merge/file.txt", "1a2b3c4d5e6f", "merge/file.txt.v", null)],
            Broken: [],
            Sends: [],
            Receives: [],
            Apart: []
        );

        var once = OverviewView.Rows(facts);

        Assert.True(OverviewView.Same(once, OverviewView.Rows(facts)));

        // A path the reading no longer holds is a different table, and so is one it holds instead.
        Assert.False(OverviewView.Same(once, OverviewView.Rows(facts with { Lost = [] })));
        Assert.False(OverviewView.Same(once, OverviewView.Rows(facts with { Untagged = ["new.txt", "more.txt"] })));

        // A path that moved losing its edit turns two pills into one, and a version that is behind turns a
        // pill into another: what a row says is what it is drawn as.
        StatusFacts settled = facts with
        {
            Renamed = [new Rename("a.txt", "b.txt", Modified: false, Strong: true)],
        };

        Assert.False(OverviewView.Same(once, OverviewView.Rows(settled)));
        Assert.False(OverviewView.Same(once, OverviewView.Rows(facts with { WrongBase = ["kept.txt"] })));
    }

    /// <summary>What a row's pills are read by, in the order they are drawn.</summary>
    /// <param name="row">The row.</param>
    /// <returns>What the letters on it are.</returns>
    private static string Pills(TableRow row)
    {
        ArgumentNullException.ThrowIfNull(row);

        return string.Join(" + ", row.Pills.Select(pill => pill.Letters));
    }

    /// <summary>What a row's notes say, in the order they are drawn.</summary>
    /// <param name="row">The row.</param>
    /// <returns>What it says that is not a state.</returns>
    private static string Notes(TableRow row)
    {
        ArgumentNullException.ThrowIfNull(row);

        return string.Join(" + ", row.Notes);
    }
}

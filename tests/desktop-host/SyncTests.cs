using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The sync: what a run is told, what it is warned about, and what is read back from it.
/// </summary>
/// <remarks>
/// The exchange itself is the command line's and is not run here — a run needs a Vault and a network — so what
/// is checked is everything this plugin decides: which flags a direction and a scope come to, which warning a
/// scope and a direction are confirmed with, and what the records a run writes about itself come to.
/// </remarks>
public sealed class SyncTests
{
    /// <summary>The three parts are left out by the flag each is named for, and carried by none of them.</summary>
    [Fact]
    public void AScopeIsTheFlagsOfWhatItLeavesOut()
    {
        Assert.Empty(SyncScope.All.Flags);
        Assert.True(SyncScope.All.Everything);

        Assert.Equal(["--no-index"], new SyncScope(Index: false, Layout: true, Storage: true).Flags);
        Assert.Equal(["--no-layout"], new SyncScope(Index: true, Layout: false, Storage: true).Flags);
        Assert.Equal(["--no-storage"], new SyncScope(Index: true, Layout: true, Storage: false).Flags);

        // One flag per part left out, in the order the three are read in: what a run leaves behind is said once
        // each and in one order, whichever parts they are.
        Assert.Equal(
            ["--no-index", "--no-layout", "--no-storage"],
            new SyncScope(Index: false, Layout: false, Storage: false).Flags
        );
    }

    /// <summary>A direction is the flag the command line takes for it, and a forced run says so.</summary>
    /// <remarks>
    /// One direction and no other, since a run told to go both up and down is a run the command refuses: the
    /// three buttons are three runs, and what tells them apart is the one flag each adds.
    /// </remarks>
    [Fact]
    public void ADirectionIsTheFlagTheCommandLineTakes()
    {
        Assert.Contains("--up-only", Sync.Arguments(SyncWay.Up, SyncScope.All, force: false));
        Assert.Contains("--down-only", Sync.Arguments(SyncWay.Down, SyncScope.All, force: false));
        Assert.Contains("--both", Sync.Arguments(SyncWay.Both, SyncScope.All, force: false));

        foreach (var way in new[] { SyncWay.Up, SyncWay.Down, SyncWay.Both })
        {
            var arguments = Sync.Arguments(way, SyncScope.All, force: false);

            Assert.Single(
                arguments,
                argument => argument is "--up-only" or "--down-only" or "--both"
            );
        }

        // A chain that has moved apart is changed only when the key is held for it.
        Assert.Contains("--force", Sync.Arguments(SyncWay.Both, SyncScope.All, force: true));
        Assert.DoesNotContain("--force", Sync.Arguments(SyncWay.Both, SyncScope.All, force: false));

        // And what is left out is asked for as well: a run with everything on asks for nothing.
        Assert.Contains("--no-storage", Sync.Arguments(SyncWay.Up, new SyncScope(true, true, false), true));
        Assert.DoesNotContain("--no-storage", Sync.Arguments(SyncWay.Up, SyncScope.All, true));
    }

    /// <summary>A run that carries everything is confirmed with nothing, and one that does not says what.</summary>
    /// <remarks>
    /// Thirteen things may be said, which is the table a reader was given: seven ways of leaving something out,
    /// and the three of them that differ by which way the run goes. The three parts are not interchangeable —
    /// leaving the storage behind is not what leaving the index behind is — so what is checked is that no two
    /// cells of that table say the same thing, and that every one of them is stated in the catalogue.
    /// </remarks>
    [Fact]
    public void WhatIsLeftOutIsSaidBeforeARun()
    {
        _ = Host.Services();

        Assert.Null(SyncWarnings.Key(SyncScope.All, SyncWay.Up));
        Assert.Null(SyncWarnings.Key(SyncScope.All, SyncWay.Down));
        Assert.Null(SyncWarnings.Key(SyncScope.All, SyncWay.Both));

        var keys = new List<string>();

        foreach (var scope in Left())
        {
            foreach (var way in new[] { SyncWay.Up, SyncWay.Down, SyncWay.Both })
            {
                var key = SyncWarnings.Key(scope, way);

                Assert.NotNull(key);
                Assert.NotEmpty(RolaI18N.Get(key));

                keys.Add(key);
            }
        }

        // The table's thirteen cells, no two of them the same: a warning that said what another said would be a
        // table with a cell copied into it, and a reader would be told less than the omission costs.
        Assert.Equal(13, keys.Distinct(StringComparer.Ordinal).Count());

        // The ways that leave the Layout out say one thing whichever way the run goes, since nothing about the
        // Vault's Layout moves either way — which is what the difference between thirteen and twenty-one is.
        foreach (var scope in Left().Where(scope => !scope.Layout))
        {
            Assert.Equal(
                SyncWarnings.Key(scope, SyncWay.Up),
                SyncWarnings.Key(scope, SyncWay.Down)
            );
        }

        // And which part a scope leaves out is read the way round the table reads it: only the Layout carried
        // is the omission of the index and the storage, not the other way about.
        Assert.Equal(SyncLeft.LayoutOnly, SyncWarnings.Left(new(false, true, false)));
        Assert.Equal(SyncLeft.IndexOnly, SyncWarnings.Left(new(true, false, false)));
        Assert.Equal(SyncLeft.StorageOnly, SyncWarnings.Left(new(false, false, true)));

        // And the two omissions that do differ by direction: leaving the index out of an upload is not what
        // leaving it out of a download is, and the same for the storage.
        Assert.NotEqual(
            SyncWarnings.Key(new(false, true, true), SyncWay.Up),
            SyncWarnings.Key(new(false, true, true), SyncWay.Down)
        );
        Assert.NotEqual(
            SyncWarnings.Key(new(true, true, false), SyncWay.Up),
            SyncWarnings.Key(new(true, true, false), SyncWay.Both)
        );
    }

    /// <summary>The seven ways of leaving something out, which is what the table of warnings is about.</summary>
    /// <returns>One scope per way.</returns>
    private static IReadOnlyList<SyncScope> Left() =>
        [
            new(false, true, true),
            new(true, true, false),
            new(true, false, true),
            new(false, false, true),
            new(false, true, false),
            new(true, false, false),
            new(false, false, false),
        ];

    /// <summary>
    /// Whether the buttons may be pressed is what a plan says, and a plan is the command line's own answer.
    /// </summary>
    /// <remarks>
    /// The plan is asked for before anything is pressed, and what it comes to decides whether the three are drawn
    /// pressable: a plan with entries is work to carry, and one with none is a run that would write nothing. What
    /// is checked is that reading one is reading the entries — an answer that cannot be read is not taken as
    /// "nothing to do" quietly, but said.
    /// </remarks>
    [Fact]
    public void WhetherThereIsWorkToCarryIsWhatAPlanSays()
    {
        _ = Host.Services();

        // A plan with entries: the entries are what a run would move, and their kind is the command's own.
        Assert.True(
            Sync.Planned(
                """
                {"layout":"a","vault":"v","forced":false,"up_to_date":2,"entries":[
                  {"uuid":"u","local_path":"a.psd","remote_path":null,"planned_path":null,
                   "local_version":"1","remote_version":null,"owner":null,"deprecated":false,
                   "modified":false,"kind":"upload"}]}
                """
            ).Ready
        );

        // And one with none, which is the whole of what "there is nothing to carry" is.
        var none = Sync.Planned(
            """{"layout":"a","vault":"v","forced":false,"up_to_date":9,"entries":[]}"""
        );

        Assert.False(none.Ready);

        // An answer that cannot be read is said rather than swallowed: a reader looking at three grey buttons is
        // owed the reason.
        foreach (var unreadable in new[] { "", "not json at all", """{"layout":"a"}""", "[]" })
        {
            var plan = Sync.Planned(unreadable);

            Assert.False(plan.Ready);
            Assert.Equal(RolaI18N.Get("rorolala_vcs.sync.plan_unreadable"), plan.Said);
        }

        // Nothing to carry is not the same as an unreadable answer: it is the command saying so.
        Assert.Null(none.Said);
    }

    /// <summary>What a run says about itself is read into the bar and the line.</summary>
    /// <remarks>
    /// The records are the command line's, so what is checked is what this reads out of them: the run's own
    /// length and progress come from the one task that is part of nothing else, the name of what is moving from
    /// the tasks that are part of it, and anything that is not a record at all is ignored rather than drawn.
    /// </remarks>
    [Fact]
    public void WhatARunSaysAboutItselfIsReadIntoTheBar()
    {
        var records = new SyncProgress();

        Assert.Equal(0, records.Done);
        Assert.Null(records.Total);
        Assert.Null(records.Moving);

        // What is not a record is nothing to read: the error stream carries what the command said as well.
        Assert.False(records.Read("warning: something worth knowing"));
        Assert.False(records.Read(""));
        Assert.False(records.Read("{"));

        // The run itself, which is the task with no parent: it is the bar.
        Assert.True(
            records.Read(
                """{"signal":"begin","id":1,"parent":null,"what":"","direction":null,"total":3}"""
            )
        );
        Assert.Equal(3, records.Total);
        Assert.Null(records.Moving);

        // A piece of it, which is what the line under the bar names.
        Assert.True(
            records.Read(
                """{"signal":"begin","id":2,"parent":1,"what":"a key","direction":"up","total":1}"""
            )
        );
        Assert.Equal("a key", records.Moving);

        // The piece moving is not the bar moving: what is drawn of the run is the run's own count.
        Assert.False(records.Read("""{"signal":"advance","id":2,"done":1}"""));
        Assert.Equal(0, records.Done);

        Assert.True(records.Read("""{"signal":"advance","id":1,"done":2}"""));
        Assert.Equal(2, records.Done);

        // A piece that finished leaves what it said as the last thing moved, which is what the line shows.
        Assert.True(records.Read("""{"signal":"finish","id":2}"""));
        Assert.Equal("a key", records.Moving);

        // The run finishing puts the bar at its end, which is not the same as the last count it gave.
        Assert.True(records.Read("""{"signal":"finish","id":1}"""));
        Assert.Equal(3, records.Done);

        // And what a run said before is let go of, so that the next run's bar starts at its own first record.
        records.Reset();
        Assert.Equal(0, records.Done);
        Assert.Null(records.Total);
        Assert.Null(records.Moving);
    }

    /// <summary>A record this does not model is ignored rather than refused.</summary>
    /// <remarks>
    /// The stream is the command line's to add to, so a reader that stopped at what it did not know would stop
    /// drawing the day a record was added. What is checked is that the records it does know still arrive around
    /// it.
    /// </remarks>
    [Fact]
    public void ARecordThisDoesNotKnowIsIgnored()
    {
        var records = new SyncProgress();

        Assert.True(records.Read("""{"signal":"begin","id":7,"parent":null,"what":"","total":2}"""));
        Assert.False(records.Read("""{"signal":"something_new","id":7}"""));
        Assert.False(records.Read("""{"signal":"begin"}"""));

        Assert.True(records.Read("""{"signal":"advance","id":7,"done":1}"""));
        Assert.Equal(1, records.Done);
        Assert.Equal(2, records.Total);
    }
}

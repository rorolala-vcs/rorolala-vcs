using RolaSharp.Write;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Progress;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The sync: what a run is told, what it is warned about, and what is read back from it.
/// </summary>
/// <remarks>
/// The exchange itself needs a Vault and a network, so what is checked is everything this plugin decides: the
/// request a direction and a scope come to, which warning a scope and a direction are confirmed with, how a write
/// failure is said, and what the records a run writes about itself come to. The writer's own plan and apply are
/// checked by the bindings' suite, against a real Workspace.
/// </remarks>
public sealed class SyncTests
{
    /// <summary>The request is the panel's scope and direction, and what the panel does not offer stays unset.</summary>
    /// <remarks>
    /// The three sides are three switches of their own, the direction is the one the writer narrows the plan by,
    /// and a forced run says so. What the panel has no control for is left at what the writer means by saying
    /// nothing: the tracked Vault, a run that may reach it, and the account the work acts as.
    /// </remarks>
    [Fact]
    public void ARequestIsThePanelsScopeAndDirection()
    {
        _ = Host.Services();

        var all = Sync.Request("/work", SyncWay.Both, SyncScope.All, force: false, account: "alice");

        Assert.True(all.Index);
        Assert.True(all.Layout);
        Assert.True(all.Storage);
        Assert.True(SyncScope.All.Everything);

        Assert.False(
            Sync.Request("/work", SyncWay.Both, new SyncScope(Index: false, Layout: true, Storage: true), false, "alice")
                .Index
        );
        Assert.False(
            Sync.Request("/work", SyncWay.Both, new SyncScope(Index: true, Layout: false, Storage: true), false, "alice")
                .Layout
        );
        Assert.False(
            Sync.Request("/work", SyncWay.Both, new SyncScope(Index: true, Layout: true, Storage: false), false, "alice")
                .Storage
        );

        // Every side is a switch of its own: one turned off leaves the two others as they were.
        var none = Sync.Request("/work", SyncWay.Both, new SyncScope(false, false, false), false, "alice");

        Assert.False(none.Index);
        Assert.False(none.Layout);
        Assert.False(none.Storage);

        Assert.Equal(
            SyncDirection.Up,
            Sync.Request("/work", SyncWay.Up, SyncScope.All, force: false, account: "alice").Direction
        );
        Assert.Equal(
            SyncDirection.Down,
            Sync.Request("/work", SyncWay.Down, SyncScope.All, force: false, account: "alice").Direction
        );
        Assert.Equal(SyncDirection.Both, all.Direction);

        // A chain that has moved apart is changed only when the key is held for it.
        Assert.False(all.Forced);
        Assert.True(Sync.Request("/work", SyncWay.Both, SyncScope.All, force: true, account: "alice").Forced);

        Assert.Equal(WriteTargetKind.Tracked, all.Target.Kind);
        Assert.False(all.Offline);
        Assert.Equal("alice", all.Account);
        Assert.Equal("/work", all.Workspace);

        // The fetch is the one step a sync plan says by name, so it is said in the window's language rather than
        // in the writer's English.
        Assert.Equal(RolaI18N.Get("rorolala_vcs.progress.fetch"), all.Labels?.Fetching);
    }

    /// <summary>A plan reads the Layout even when the run leaves it out.</summary>
    /// <remarks>
    /// A plan is the comparison with the Vault's Layout, and a request that leaves the Layout out has none to make:
    /// the writer refuses it. A run that leaves the Layout itself behind still carries the index and the storage,
    /// so the plan is asked with the Layout read while the run says what it actually leaves out.
    /// </remarks>
    [Fact]
    public void APlanReadsTheLayoutEvenWhenARunLeavesItOut()
    {
        foreach (var scope in new[] { new SyncScope(true, false, true), new SyncScope(false, true, false), SyncScope.All })
        {
            var run = Sync.Request("/work", SyncWay.Both, scope, force: false, account: "alice");
            var planned = Sync.PlanRequest("/work", SyncWay.Both, scope, force: false, account: "alice");

            Assert.True(planned.Layout);

            // The other two sides are the run's, since a plan is not about them; only the Layout is forced.
            Assert.Equal(run.Index, planned.Index);
            Assert.Equal(run.Storage, planned.Storage);
        }
    }

    /// <summary>A plan is not asked about a place there is no work in, and not asked at all with every side off.</summary>
    /// <remarks>
    /// The plan is a read of the work, and there is no work where no Workspace holds the directory. What is said is
    /// the same sentence the docks say about the base directory, because it is the same absence. Every side left
    /// out is a run that changes nothing, which is nothing to plan and nothing to say against it.
    /// </remarks>
    [Fact]
    public void APlanSaysWhenThereIsNothingToAskAbout()
    {
        _ = Host.Services();

        var directory = Scratch.New("sync-plan");
        var outside = Sync.Plan(directory, SyncWay.Both, SyncScope.All, force: false);

        Assert.False(outside.Ready);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.not_a_workspace"), outside.Said);

        var none = Sync.Plan(
            "/a/directory/that/is/never/read",
            SyncWay.Both,
            new SyncScope(Index: false, Layout: false, Storage: false),
            force: false
        );

        Assert.False(none.Ready);
        Assert.Null(none.Said);
    }

    /// <summary>A write failure is said once, and a run with no plan says why it cannot go.</summary>
    /// <remarks>
    /// The kinds the panel already has words for are said in the reader's language; the others are the writer's own
    /// message, shown once rather than beside a kind that would say the same thing a second time. A plan that could
    /// not be made leaves no plan to apply, so the run says why and touches nothing.
    /// </remarks>
    [Fact]
    public void AFailureIsSaidOnceAndARunWithoutAPlanDoesNotStart()
    {
        _ = Host.Services();

        Assert.Equal(
            RolaI18N.Get("rorolala_vcs.not_a_workspace"),
            Sync.Told(new RolaWriteError(RolaWriteErrorKind.NoWorkspace))
        );
        Assert.Equal(
            RolaI18N.Get("rorolala_vcs.sync.plan_unreadable"),
            Sync.Told(new RolaWriteError(RolaWriteErrorKind.Json))
        );
        Assert.Equal(
            "the Vault's Layout was never fetched here",
            Sync.Told(
                new RolaWriteError(
                    RolaWriteErrorKind.NotCached,
                    reason: "the Vault's Layout was never fetched here"
                )
            )
        );

        var records = new ProgressRecords();
        var touched = 0;
        var said = RolaI18N.Get("rorolala_vcs.sync.plan_unreadable");

        var (done, told) = Sync.Run(
            new RorolalaVCSPlugin.SyncPlan(false, said),
            records,
            () => touched++
        );

        Assert.False(done);
        Assert.Equal(said, told);
        Assert.Empty(records.Tasks);
        Assert.Equal(0, touched);

        // A plan with every side left out says nothing against it, and the run that applies it changes nothing
        // without failing: it is the answer the run gave before the writer took it over.
        var (nothing, why) = Sync.Run(
            new RorolalaVCSPlugin.SyncPlan(false, null),
            records,
            () => touched++
        );

        Assert.True(nothing);
        Assert.Equal(string.Empty, why);
        Assert.Empty(records.Tasks);
        Assert.Equal(0, touched);
    }

    /// <summary>A reading that names work is enough to press for, whether or not a plan has answered.</summary>
    /// <remarks>
    /// The table and the buttons read the work two ways: the plan is the writer's whole answer, and the reading
    /// names where each recorded version stands. They can disagree while one is stale, and the table saying
    /// "Sync Down" beside three grey buttons is the disagreement a reader must not meet, so what the reading names
    /// is pressable on its own.
    /// </remarks>
    [Fact]
    public void AReadingThatNamesWorkLeavesTheButtonsPressable()
    {
        var host = Host.Services().For(new PluginId("rorolala.vcs"), 0);
        var hub = new ProgressHub();
        var sync = new SyncWatcher(host, host.Log, hub, new VaultFetch(host, hub));

        Assert.False(sync.Pressable);

        sync.Observe(true);

        Assert.True(sync.Pressable);

        sync.Observe(false);

        Assert.False(sync.Pressable);
    }

    /// <summary>A run that carries everything is confirmed with nothing, and one that does not says what.</summary>
    /// <remarks>
    /// Thirteen things may be said, which is the table a reader was given: seven ways of leaving something out,
    /// and the three of them that differ by which way the run goes. The three parts are not interchangeable,
    /// leaving the storage behind is not what leaving the index behind is, so what is checked is that no two
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
        // Vault's Layout moves either way, which is what the difference between thirteen and twenty-one is.
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

    /// <summary>What a run says about itself is read into the tasks it reports.</summary>
    /// <remarks>
    /// The records are the writer's, so what is checked is what this reads out of them: every task a `begin` opens
    /// is kept (a bar of the run's own, and a direction under it) while a task that only names what its parent is
    /// moving is that parent's line rather than a task of its own. Anything that is not a record at all is ignored
    /// rather than drawn.
    /// </remarks>
    [Fact]
    public void WhatARunSaysAboutItselfIsReadIntoTheTasks()
    {
        var records = new ProgressRecords();

        Assert.Empty(records.Tasks);

        // What is not a record is nothing to read: the error stream carries what the command said as well.
        Assert.False(records.Read("warning: something worth knowing"));
        Assert.False(records.Read(""));
        Assert.False(records.Read("{"));

        // A bar of the run's own, which is a task with no parent.
        Assert.True(
            records.Read(
                """{"signal":"begin","id":1,"parent":null,"what":"","direction":null,"total":3}"""
            )
        );

        var whole = Assert.Single(records.Tasks);
        Assert.Equal(3, whole.Total);
        Assert.Null(whole.Moving);

        // A direction under it, which is a bar of its own because it says which way it moves.
        Assert.True(
            records.Read(
                """{"signal":"begin","id":2,"parent":1,"what":"","direction":"up","total":10}"""
            )
        );

        Assert.Equal(2, records.Tasks.Count);
        Assert.Equal(MoveAim.Up, records.Tasks[1].Aim);

        // A piece of the direction, which is not a bar: it is what the direction says it is moving.
        Assert.True(
            records.Read(
                """{"signal":"begin","id":3,"parent":2,"what":"a key","direction":null,"total":null}"""
            )
        );

        Assert.Equal(2, records.Tasks.Count);
        Assert.Equal("a key", records.Tasks[1].Moving);

        // A piece's own advance is not a bar's: only a task this keeps is moved by one.
        Assert.False(records.Read("""{"signal":"advance","id":3,"done":1}"""));

        Assert.True(records.Read("""{"signal":"advance","id":2,"done":4}"""));
        Assert.Equal(4, records.Tasks[1].Done);

        // A task that finished is kept and marked full, what it was moving let go of: the panel's card outlives
        // its run, so a bar that vanished the moment its work ended would leave the card with nothing to show.
        Assert.True(records.Read("""{"signal":"finish","id":2}"""));

        Assert.Equal(2, records.Tasks.Count);
        Assert.Equal(10, records.Tasks[1].Total);
        Assert.Equal(10, records.Tasks[1].Done);
        Assert.Null(records.Tasks[1].Moving);

        // And what a run said before is let go of, so that the next run's rows start at its own first record.
        records.Reset();
        Assert.Empty(records.Tasks);
    }

    /// <summary>A record this does not model is ignored rather than refused.</summary>
    /// <remarks>
    /// The stream is the writer's to add to, so a reader that stopped at what it did not know would stop drawing
    /// the day a record was added. What is checked is that the records it does know still arrive around it.
    /// </remarks>
    [Fact]
    public void ARecordThisDoesNotKnowIsIgnored()
    {
        var records = new ProgressRecords();

        Assert.True(records.Read("""{"signal":"begin","id":7,"parent":null,"what":"","total":2}"""));

        var task = Assert.Single(records.Tasks);

        Assert.False(records.Read("""{"signal":"something_new","id":7}"""));
        Assert.False(records.Read("""{"signal":"begin"}"""));

        Assert.True(records.Read("""{"signal":"advance","id":7,"done":1}"""));

        Assert.Equal(1, task.Done);
        Assert.Equal(2, task.Total);
    }
}

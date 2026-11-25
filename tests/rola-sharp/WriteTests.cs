using System.Diagnostics;
using System.Text;
using System.Text.Json;

using RolaSharp;
using RolaSharp.Write;
using Xunit;

namespace RolaSharp.IntegrationTests;

public sealed class WriteTests
{
    [Fact]
    public void A_cancellation_is_set_and_read()
    {
        using var cancellation = Cancellation.New();

        Assert.False(cancellation.IsCancelled);
        cancellation.Cancel();
        Assert.True(cancellation.IsCancelled);
    }

    [Fact]
    public void A_progress_handle_starts_empty()
    {
        using var progress = Progress.New();

        Assert.Null(progress.NextJson());
    }

    [Fact]
    public void A_file_is_stored_and_extracted_again()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var source = Path.Combine(directory, "content.bin");
            File.WriteAllText(source, "stored content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var storage = workspace.Storage;
            storage.Read();
            var writer = new StorageWriter(storage);

            var stored = writer.Write(new FileInfo(source));
            Assert.StartsWith("blake3:", stored.Key, StringComparison.Ordinal);

            var destination = new FileInfo(Path.Combine(directory, "out.bin"));
            writer.Extract(stored.Key, destination);

            Assert.Equal("stored content", File.ReadAllText(destination.FullName));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_workspace_rebuilds_its_tree_cache()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "one.txt"), "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var report = writer.RebuildTreeCache();

            Assert.True(report.Paths >= 1);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_creator_and_a_message_are_written_to_the_index()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var index = workspace.VCSIndex;
            index.Read();
            var writer = new VCSIndexWriter(index);

            var creator = writer.WriteCreator("alice");
            var message = writer.WriteMessage("first");

            Assert.False(string.IsNullOrEmpty(creator));
            Assert.False(string.IsNullOrEmpty(message));
            Assert.Equal("alice", Assert.IsType<VcsIndexCreatorView>(index.ObjectView(creator)).Text);

            index.Read();
            Assert.Contains(
                index.Listing!.StringHashes,
                key => key == creator || key == message
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_layout_path_is_bound_moved_and_removed()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var layout = workspace.LocalLayout;
            layout.Read();
            var writer = new LayoutWriter(layout);
            var id = Guid.NewGuid();
            var version = $"blake3:{new string('0', 64)}";

            writer.InsertEntry(id, "a.psd", version, null, "first");
            layout.Read();
            Assert.NotNull(layout.Find("a.psd"));

            writer.MovePath("a.psd", "b.psd");
            layout.Read();
            Assert.Null(layout.Find("a.psd"));
            Assert.NotNull(layout.Find("b.psd"));

            writer.RemovePath("b.psd");
            layout.Read();
            Assert.Null(layout.Find("b.psd"));

            writer.SetEntry(id, version, null, "changed", update: true);
            layout.Read();
            Assert.Equal("changed", layout.Entries.Single(entry => entry.Uuid == id.ToString()).Description);

            writer.RemoveEntry(id);
            layout.Read();
            Assert.DoesNotContain(layout.Entries, entry => entry.Uuid == id.ToString());
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_layout_path_is_switched_to_a_new_uuid()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var tracked = writer.TrackFile(
                "a.psd",
                "alice",
                "first",
                owner: "alice",
                description: "note"
            );

            var layout = workspace.Layout;
            layout.Read();
            var id = Guid.NewGuid();

            // The switch is the fork the command line makes: the entry the path names goes, and one
            // under the new Uuid takes its place with the same version, holder and words.
            new LayoutWriter(layout).SwitchEntry("a.psd", id, tracked.Version, "alice", "note", null);

            layout.Read();
            var switched = layout.Find("a.psd");

            Assert.NotNull(switched);
            Assert.Equal(id.ToString(), switched.Uuid);
            Assert.NotEqual(tracked.Id.ToString(), switched.Uuid);
            Assert.Equal(tracked.Version["blake3:".Length..], switched.Version);
            Assert.Equal("alice", switched.Owner);
            Assert.Equal("note", switched.Description);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_fetched_layout_write_names_the_Vault_it_is_bound_to()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new LayoutWriter(new LayoutScope(directory, "truth@origin"));

            // The workspace bound no Vault, so the writer refuses before it reaches one. Both writes a
            // fetched copy needs answer with the same kind, which is what tells a caller to bind it.
            var move = Assert.Throws<RolaWriteError>(() => writer.MoveFetched("a.psd", "b.psd"));
            Assert.Equal(RolaWriteErrorKind.Precondition, move.Kind);

            var remove = Assert.Throws<RolaWriteError>(
                () => writer.RemoveFetchedEntry(Guid.NewGuid())
            );
            Assert.Equal(RolaWriteErrorKind.Precondition, remove.Kind);

            // A Layout of the Workspace's own is not a fetched copy, and is refused as an argument.
            var local = Assert.Throws<RolaWriteError>(
                () => new LayoutWriter(workspace.Layout).MoveFetched("a.psd", "b.psd")
            );
            Assert.Equal(RolaWriteErrorKind.InvalidPath, local.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_workspace_is_found_from_a_nested_directory()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var nested = Directory.CreateDirectory(Path.Combine(directory, "sub")).FullName;

            var workspace = WorkspaceScope.Open(nested);

            Assert.Equal(Path.GetFullPath(directory), workspace.Root.FullName.TrimEnd(Path.DirectorySeparatorChar));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_vault_is_found_from_a_nested_directory()
    {
        var directory = Scratch();
        try
        {
            var vault = Path.Combine(directory, "vault");
            RunRola(directory, $"create --vault \"{vault}\"");
            var nested = Directory.CreateDirectory(Path.Combine(vault, "sub")).FullName;

            var scope = VaultScope.Open(nested);

            Assert.Equal(
                Path.GetFullPath(vault).TrimEnd(Path.DirectorySeparatorChar),
                scope.Root.FullName.TrimEnd(Path.DirectorySeparatorChar)
            );
            Assert.True(scope.ConfigFile.Exists);
            Assert.True(scope.StorageDirectory.Exists);
            Assert.True(scope.IndexDirectory.Exists);
            Assert.True(scope.LayoutDirectory.Exists);
            Assert.Throws<RolaReadError>(() => scope.TreeAt(".."));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_vault_packs_its_store_and_index()
    {
        var directory = Scratch();
        try
        {
            var vaultPath = Path.Combine(directory, "vault");
            RunRola(directory, $"create --vault \"{vaultPath}\"");

            var vault = VaultScope.Open(vaultPath);
            vault.Read();
            var writer = new VaultWriter(vault);

            using (var guard = writer.Lock())
            {
                Assert.NotNull(guard);
            }

            var packed = writer.Pack();

            Assert.NotNull(packed);
            Assert.NotNull(writer.Pack(storage: true, index: false));
            Assert.NotNull(writer.Pack(storage: false, index: true));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_vault_path_is_untracked()
    {
        var directory = Scratch();
        try
        {
            var vaultPath = Path.Combine(directory, "vault");
            RunRola(directory, $"create --vault \"{vaultPath}\"");

            var vault = VaultScope.Open(vaultPath);
            vault.Read();
            var layout = vault.Layout;
            layout.Read();
            var id = Guid.NewGuid();
            var layoutWriter = new LayoutWriter(layout);
            layoutWriter.SetEntry(id, $"blake3:{new string('0', 64)}");
            layoutWriter.CreatePath("a.txt", id);

            new VaultWriter(vault).Untrack("a.txt");
            Assert.Throws<RolaWriteError>(() => new VaultWriter(vault).Untrack("missing"));

            layout.Read();
            Assert.Null(layout.Find("a.txt"));

            var pathOnly = Guid.NewGuid();
            layoutWriter.SetEntry(pathOnly, $"blake3:{new string('0', 64)}");
            layoutWriter.CreatePath("b.txt", pathOnly);
            new VaultWriter(vault).RemovePath("b.txt");
            layout.Read();
            Assert.Null(layout.Find("b.txt"));

            new VaultWriter(vault).RemoveEntry(pathOnly);
            layout.Read();
            Assert.DoesNotContain(layout.Entries, entry => entry.Uuid == pathOnly.ToString());
            Assert.Throws<RolaWriteError>(
                () => new VaultWriter(vault).RemoveEntry(Guid.NewGuid())
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_vault_file_is_tracked()
    {
        var directory = Scratch();
        try
        {
            var vaultPath = Path.Combine(directory, "vault");
            RunRola(directory, $"create --vault \"{vaultPath}\"");
            File.WriteAllText(Path.Combine(vaultPath, "a.txt"), "content");

            var vault = VaultScope.Open(vaultPath);
            vault.Read();
            var writer = new VaultWriter(vault);
            var first = writer.TrackFile("a.txt", "alice", "first");

            File.WriteAllText(Path.Combine(vaultPath, "a.txt"), "second");
            var second = writer.TrackFile("a.txt", "alice", "second");

            var retracked = writer.RetrackFile("a.txt");
            Assert.Equal("content", File.ReadAllText(Path.Combine(vaultPath, "a.txt")));
            Assert.Equal(first.Version, retracked.Version);

            var onlyIndex = writer.RetrackFile("a.txt", second.Version, onlyIndex: true);
            Assert.Equal("content", File.ReadAllText(Path.Combine(vaultPath, "a.txt")));
            Assert.Equal(second.Version, onlyIndex.Version);
            Assert.Throws<RolaWriteError>(() => writer.RetrackFile("missing"));
            Assert.Throws<RolaWriteError>(() => writer.RetrackFile("a.txt", "not-a-version"));

            writer.Align("a.txt", first.Version, "alice", "aligned", "share");
            Assert.Equal("content", File.ReadAllText(Path.Combine(vaultPath, "a.txt")));
            var alignedLayout = vault.Layout;
            alignedLayout.Read();
            Assert.Equal("aligned", alignedLayout.Find("a.txt")?.Description);
            Assert.Throws<RolaWriteError>(() => writer.Align("missing", first.Version));

            writer.Restore("a.txt", first.Version);
            Assert.Throws<RolaWriteError>(() => writer.Restore("missing", first.Version));
            Assert.Throws<RolaWriteError>(() => writer.Restore("a.txt", "not-a-version"));

            writer.SetAll(first.Id, first.Version, "bob", "set", null);
            var setLayout = vault.Layout;
            setLayout.Read();
            Assert.Equal("bob", setLayout.Find("a.txt")?.Owner);

            Assert.Equal("content", File.ReadAllText(Path.Combine(vaultPath, "a.txt")));

            var layout = vault.Layout;
            layout.Read();
            Assert.NotNull(layout.Find("a.txt"));
            Assert.NotEqual(Guid.Empty, first.Id);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void The_builtin_book_is_searchable()
    {
        var root = RepositoryRoot();
        var path = Path.Combine(root, "build", "docs", "builtin.book");
        if (!File.Exists(path))
        {
            return;
        }

        var book = BookScope.Open(path);
        book.Read();

        Assert.True(book.ArticleCount > 0);
        Assert.NotEmpty(book.Search("rola", 3));
    }

    [Fact]
    public void A_workspace_track_file_is_written_and_cleared()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            writer.SetTrack("vault");
            Assert.Equal(
                "vault",
                File.ReadAllText(RolaPaths.LayoutTrackFile(directory)).Trim()
            );

            writer.ClearTrack();
            Assert.False(File.Exists(RolaPaths.LayoutTrackFile(directory)));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_workspace_paths_come_from_the_library()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var layout = RolaPaths.WorkspaceLayout(directory);
            Assert.True(Directory.Exists(layout));
            Assert.StartsWith(
                layout,
                RolaPaths.LayoutTrackFile(directory),
                StringComparison.Ordinal
            );
            Assert.True(Directory.Exists(RolaPaths.WorkspaceIndex(directory)));

            var workspace = new WorkspaceScope(directory);
            Assert.True(workspace.Data.Exists);
            Assert.True(workspace.ConfigFile.Exists);
            Assert.True(workspace.LayoutDirectory.Exists);
            Assert.True(workspace.StorageDirectory.Exists);
            Assert.True(workspace.IndexDirectory.Exists);
            Assert.Throws<RolaReadError>(() => workspace.TreeAt(".."));
            Assert.Throws<RolaReadError>(() => workspace.TruthLayout("../vault"));
            Assert.Throws<RolaReadError>(() => workspace.TruthLayout(""));
            Assert.Throws<RolaReadError>(() => workspace.TruthLayout("a/b"));
            Assert.Throws<RolaReadError>(() => workspace.TruthLayout(" vault"));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_workspace_exposes_its_ignore_rules()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var ignore = workspace.Ignore;
            ignore.Read();

            Assert.False(ignore.IsIgnored("a.psd"));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_rule_ignores_a_path()
    {
        var directory = Scratch();
        try
        {
            var rules = Path.Combine(directory, "rules");
            Directory.CreateDirectory(rules);
            File.WriteAllText(Path.Combine(rules, "ignore"), "*.tmp\n");

            var ignore = new IgnoreScope(rules, directory);
            ignore.Read();

            Assert.True(ignore.IsIgnored("a.tmp"));
            Assert.False(ignore.IsIgnored("a.psd"));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_file_content_becomes_a_variant_and_a_version()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var storage = workspace.Storage;
            storage.Read();
            var content = new StorageWriter(storage).Write("content"u8.ToArray());
            var index = workspace.VCSIndex;
            index.Read();
            var writer = new VCSIndexWriter(index);
            var root = $"blake3:{new string('0', 64)}";

            var written = writer.WriteFileVersion(content.Key, root, null, "alice", "first");

            Assert.False(string.IsNullOrEmpty(written.Variant));
            Assert.False(string.IsNullOrEmpty(written.Version));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void An_inverse_index_is_rebuilt()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var inverse = workspace.InverseIndex;
            inverse.Read();
            var report = inverse.Rebuild();

            Assert.NotNull(report.Mode);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_layout_track_is_set_and_cleared()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var track = RolaPaths.LayoutTrackFile(directory);

            writer.SetTrack("origin");
            Assert.True(File.Exists(track));

            writer.ClearTrack();
            Assert.False(File.Exists(track));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_directory_is_made_a_tree()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            Directory.CreateDirectory(Path.Combine(directory, "tree"));

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new TreeWriter(workspace);

            var tree = writer.Init("tree");

            Assert.Equal("tree", tree.Path);
            Assert.True(Directory.Exists(Path.Combine(directory, "tree", ".rola-tree")));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_file_is_tracked_and_named()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var tracked = writer.TrackFile("a.psd", "alice", "first", owner: "alice", description: "note");

            Assert.NotEqual(Guid.Empty, tracked.Id);
            Assert.False(string.IsNullOrEmpty(tracked.Version));

            var layout = workspace.Layout;
            layout.Read();
            Assert.Equal("alice", layout.Find("a.psd")?.Owner);
            Assert.Equal("note", layout.Find("a.psd")?.Description);

            File.WriteAllText(Path.Combine(directory, "a.psd"), "content again");
            var again = writer.TrackFile("a.psd", "alice", "second");

            Assert.Equal(tracked.Id, again.Id);
            Assert.NotEqual(tracked.Version, again.Version);

            var back = writer.RetrackFile("a.psd");
            Assert.Equal(tracked.Version, back.Version);
            Assert.Equal("content", File.ReadAllText(Path.Combine(directory, "a.psd")));

            var restored = writer.Restore("a.psd", again.Version);
            Assert.Equal(tracked.Id, restored.Id);
            Assert.Equal("content again", File.ReadAllText(Path.Combine(directory, "a.psd")));

            writer.Remember("a.psd");
            writer.Forget("a.psd");
            layout.Read();
            Assert.Equal("alice", layout.Find("a.psd")?.Owner);

            new LayoutWriter(workspace.Layout).SetOwner(tracked.Id, "bob");
            layout.Read();
            Assert.Equal("bob", layout.Find("a.psd")?.Owner);

            new LayoutWriter(workspace.Layout).SetDescription(tracked.Id, "changed");
            layout.Read();
            Assert.Equal("changed", layout.Find("a.psd")?.Description);

            new LayoutWriter(workspace.Layout).SetPreference(tracked.Id, "psd-basic");
            layout.Read();
            Assert.Equal("psd-basic", layout.Find("a.psd")?.Preference);

            new LayoutWriter(workspace.Layout).SetAll(
                tracked.Id,
                tracked.Version,
                "carol",
                "all",
                "blend-basic"
            );
            layout.Read();
            Assert.Equal("carol", layout.Find("a.psd")?.Owner);
            Assert.Equal("all", layout.Find("a.psd")?.Description);
            Assert.Equal("blend-basic", layout.Find("a.psd")?.Preference);

            new LayoutWriter(workspace.Layout).SetVersion(tracked.Id, tracked.Version);
            layout.Read();
            Assert.Equal(
                tracked.Version.Replace("blake3:", string.Empty),
                layout.Find("a.psd")?.Version
            );

            var forked = writer.RetrackFile("a.psd", fork: true, onlyIndex: true);
            Assert.NotEqual(tracked.Id, forked.Id);

            var packed = writer.Pack();
            Assert.NotNull(packed);
            Assert.NotNull(writer.Pack(storage: true, index: false));
            Assert.NotNull(writer.Pack(storage: false, index: true));

            var storePacked = new StorageWriter(workspace.Storage).Pack();
            Assert.NotNull(storePacked);
            var indexPacked = new VCSIndexWriter(workspace.VCSIndex).Pack();
            Assert.NotNull(indexPacked);

            File.WriteAllText(Path.Combine(directory, "b.psd"), "other");
            var other = writer.TrackFile("b.psd", "alice", "other");
            Assert.Throws<RolaWriteError>(
                () =>
                    writer.RetrackFile(
                        "a.psd",
                        other.Version,
                        onlyIndex: true,
                        denyJump: true
                    )
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_vault_collects_nothing_it_keeps()
    {
        var directory = Scratch();
        try
        {
            var vaultPath = Path.Combine(directory, "vault");
            RunRola(directory, $"create --vault \"{vaultPath}\"");
            File.WriteAllText(Path.Combine(vaultPath, "a.psd"), "content");

            var vault = VaultScope.Open(vaultPath);
            vault.Read();
            var writer = new VaultWriter(vault);
            writer.TrackFile("a.psd", "alice", "first");

            var plan = writer.PlanCollect();
            Assert.NotNull(plan.Index);
            Assert.NotNull(plan.Storage);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Index);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Storage);
            Assert.Null(writer.PlanCollect(storage: true, index: false).Index);
            Assert.Null(writer.PlanCollect(storage: false, index: true).Storage);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Index);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Storage);

            var report = writer.Collect();
            Assert.NotNull(report.Index);
            Assert.NotNull(report.Storage);
            var nothing = writer.Collect(storage: false, index: false);
            Assert.Null(nothing.Index);
            Assert.Null(nothing.Storage);
            var storageOnly = writer.Collect(storage: true, index: false);
            Assert.Null(storageOnly.Index);
            Assert.NotNull(storageOnly.Storage);
            var indexOnly = writer.Collect(storage: false, index: true);
            Assert.Null(indexOnly.Storage);
            Assert.NotNull(indexOnly.Index);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_workspace_collects_nothing_it_keeps()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            writer.TrackFile("a.psd", "alice", "first");

            var plan = writer.PlanCollect();
            Assert.NotNull(plan.Index);
            Assert.NotNull(plan.Storage);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Index);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Storage);
            Assert.Null(writer.PlanCollect(storage: true, index: false).Index);
            Assert.Null(writer.PlanCollect(storage: false, index: true).Storage);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Index);
            Assert.Null(writer.PlanCollect(storage: false, index: false).Storage);

            var report = writer.Collect();
            Assert.NotNull(report.Index);
            Assert.NotNull(report.Storage);
            var nothing = writer.Collect(storage: false, index: false);
            Assert.Null(nothing.Index);
            Assert.Null(nothing.Storage);
            var storageOnly = writer.Collect(storage: true, index: false);
            Assert.Null(storageOnly.Index);
            Assert.NotNull(storageOnly.Storage);
            var indexOnly = writer.Collect(storage: false, index: true);
            Assert.Null(indexOnly.Storage);
            Assert.NotNull(indexOnly.Index);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void An_account_that_is_not_there_is_not_found()
    {
        Assert.Null(AccountScope.Find("definitely-not-an-account"));
    }

    [Fact]
    public void A_workspace_lock_is_held_and_released()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            using (var guard = writer.Lock())
            {
                Assert.NotNull(guard);
            }
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_sync_plan_reads_the_cached_Vault_Layout()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            RunRola(directory, "vault bind vault rola://127.0.0.1/");
            var keys = Path.Combine(directory, ".rola", "auth");
            Directory.CreateDirectory(keys);
            File.WriteAllText(
                Path.Combine(keys, "alice.pem"),
                "a key this test never signs with"
            );
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            writer.TrackFile("a.psd", "alice", "first");

            var truth = Path.Combine(
                RolaPaths.WorkspaceReadonlyLayouts(directory),
                "vault",
                RolaPaths.VaultLayoutName
            );
            CopyDirectory(RolaPaths.WorkspaceLayout(directory), truth);

            var request = new SyncRequest
            {
                Account = "alice",
                Target = WriteTarget.Named("vault"),
                Offline = true,
            };
            var plan = writer.PlanSync(request);

            Assert.Equal("vault", plan.Vault);
            Assert.True(plan.UpToDate >= 1);
            Assert.True(writer.PlanSync(request with { Direction = SyncDirection.Up }).UpToDate >= 1);
            Assert.True(writer.PlanSync(request with { Direction = SyncDirection.Down }).UpToDate >= 1);
            Assert.True(writer.PlanSync(request with { Forced = true }).Forced);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_sync_run_with_every_side_left_alone_answers_with_nothing()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var report = writer.SyncAt(
                "vault",
                "rola://127.0.0.1/",
                "alice",
                "missing.pem",
                storage: false,
                index: false,
                layout: false
            );

            Assert.Equal(0ul, report.Applied);
            Assert.Equal(0ul, report.Received);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_sync_run_refuses_an_address_that_does_not_read()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var error = Assert.Throws<RolaWriteError>(
                () =>
                    writer.SyncAt(
                        "vault",
                        "",
                        "alice",
                        "missing.pem",
                        storage: false,
                        index: false,
                        layout: false
                    )
            );

            Assert.Equal(RolaWriteErrorKind.InvalidPath, error.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_sync_run_needs_an_account()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            Assert.Throws<ArgumentNullException>(() => writer.Sync("vault", null!));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_sync_plan_needs_the_cached_Vault_Layout()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            RunRola(directory, "vault bind vault rola://127.0.0.1/");
            var keys = Path.Combine(directory, ".rola", "auth");
            Directory.CreateDirectory(keys);
            File.WriteAllText(
                Path.Combine(keys, "alice.pem"),
                "a key this test never signs with"
            );

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            Assert.Throws<RolaWriteError>(
                () =>
                    writer.PlanSync(
                        new SyncRequest
                        {
                            Account = "alice",
                            Target = WriteTarget.Named("vault"),
                            Offline = true,
                        }
                    )
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_path_is_aligned_to_a_version()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var first = writer.TrackFile("a.psd", "alice", "first");

            File.WriteAllText(Path.Combine(directory, "a.psd"), "two");
            writer.TrackFile("a.psd", "alice", "second");

            writer.AlignEntry("a.psd", first.Version, "alice", "aligned", "share");

            Assert.Equal("one", File.ReadAllText(Path.Combine(directory, "a.psd")));
            var layout = workspace.Layout;
            layout.Read();
            var entry = layout.Find("a.psd");
            Assert.NotNull(entry);
            Assert.Equal(first.Version["blake3:".Length..], entry.Version);
            Assert.Equal("aligned", entry.Description);

            writer.SetAll(first.Id, first.Version, "bob", "set", null);
            layout.Read();
            Assert.Equal("bob", layout.Find("a.psd")?.Owner);
            Assert.Throws<RolaWriteError>(() => writer.AlignEntry("missing", first.Version));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_tracked_entry_is_removed_by_id()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var tracked = writer.TrackFile("a.psd", "alice", "first");

            writer.RemoveEntry(tracked.Id);
            Assert.Throws<RolaWriteError>(() => writer.RemoveEntry(Guid.NewGuid()));

            var layout = workspace.Layout;
            layout.Read();
            Assert.Null(layout.Find("a.psd"));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_tracked_path_is_removed_but_its_entry_stays()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            writer.TrackFile("a.psd", "alice", "first");

            writer.RemovePath("a.psd");

            var layout = workspace.Layout;
            layout.Read();
            Assert.Null(layout.Find("a.psd"));
            Assert.Single(layout.Entries);
            Assert.True(File.Exists(Path.Combine(directory, "a.psd")));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_tracked_path_is_untracked()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            writer.TrackFile("a.psd", "alice", "first");

            writer.Untrack("a.psd");
            Assert.Throws<RolaWriteError>(() => writer.Untrack("missing"));

            var layout = workspace.Layout;
            layout.Read();
            Assert.Null(layout.Find("a.psd"));
            Assert.True(File.Exists(Path.Combine(directory, "a.psd")));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_tree_path_is_staged()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            Directory.CreateDirectory(Path.Combine(directory, "tree", ".rola-tree"));
            File.WriteAllText(Path.Combine(directory, "tree", "one.txt"), "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new TreeWriter(workspace);

            var changes = writer.Add(["one.txt"], "tree");

            Assert.Contains(changes, change => change.Path == "one.txt");

            var reset = writer.Reset(["one.txt"], "tree");
            Assert.Contains("one.txt", reset.Paths);

            writer.Add(["one.txt"], "tree");
            var removed = writer.Remove(["one.txt"], "tree");
            Assert.Contains("one.txt", removed.Paths);
            Assert.False(File.Exists(Path.Combine(directory, "tree", "one.txt")));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_tree_description_is_written()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            Directory.CreateDirectory(Path.Combine(directory, "tree", ".rola-tree"));

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new TreeWriter(workspace);

            Assert.Equal(5, writer.WriteDescription("first", "tree"));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void The_shared_paths_are_the_librarys_own()
    {
        var root = Path.Combine(Path.GetTempPath(), "rola-sharp-paths");
        var workspace = RolaPaths.WorkspaceStorage(root);
        var vault = RolaPaths.VaultStorage(root);

        Assert.StartsWith(Path.GetFullPath(root), workspace, StringComparison.Ordinal);
        Assert.StartsWith(Path.GetFullPath(root), vault, StringComparison.Ordinal);
        Assert.NotEqual(workspace, vault);
    }

    [Fact]
    public void A_workspace_status_reads_its_untagged_work()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "loose.txt"), "loose");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var status = workspace.Status;
            status.Read();

            Assert.Contains(status.Status.Untagged, path => path == "loose.txt");
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void An_align_delete_names_the_path_it_confirmed()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var file = Path.Combine(directory, "a.psd");
            File.WriteAllText(file, "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            writer.TrackFile("a.psd", "alice", "first");
            File.Delete(file);

            var result = writer.Align(
                new AlignRequest
                {
                    Workspace = directory,
                    Path = "a.psd",
                    Mode = AlignMode.Delete,
                }
            );

            Assert.Equal(AlignKind.Paths, result.Kind);
            Assert.Contains(
                result.Paths,
                change => change.Did == "deleted" && change.What == "a.psd"
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_retrack_moves_a_file_one_version_back()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var file = Path.Combine(directory, "a.psd");
            File.WriteAllText(file, "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var first = writer.TrackFile("a.psd", "alice", "first");

            File.WriteAllText(file, "two");
            writer.TrackFile("a.psd", "alice", "second");

            var result = writer.Retrack(
                new RetrackRequest { Workspace = directory, File = "a.psd" }
            );

            Assert.Equal(RetrackKind.Path, result.Kind);
            Assert.True(result.Changed);
            Assert.Equal(first.Version["blake3:".Length..], result.Version);
            Assert.Equal("one", File.ReadAllText(file));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_bring_and_an_ownership_plan_need_an_account()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var bring = Assert.Throws<RolaWriteError>(
                () =>
                    writer.Bring(
                        new BringRequest { Workspace = directory, Refs = ["a.psd"] }
                    )
            );
            Assert.Equal(RolaWriteErrorKind.NoAccount, bring.Kind);

            var plan = Assert.Throws<RolaWriteError>(
                () =>
                    writer.PlanOwnership(
                        new OwnershipPlanRequest
                        {
                            Workspace = directory,
                            Kind = OwnershipKind.Hold,
                            Paths = ["a.psd"],
                        }
                    )
            );
            Assert.Equal(RolaWriteErrorKind.NoAccount, plan.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_bring_request_writes_and_reads_its_optional_fields()
    {
        var request = new BringRequest
        {
            Workspace = "/work",
            Account = "alice",
            Refs = ["a.psd", "Assets/*"],
            To = "here",
            ByName = true,
            Offline = true,
            Force = true,
            Concurrency = 4,
            Labels = new WriteLabels { Fetching = "Bringing", Applying = "Moving" },
        };

        var json = JsonSerializer.Serialize(request);
        using (var document = JsonDocument.Parse(json))
        {
            var root = document.RootElement;
            Assert.Equal("/work", root.GetProperty("workspace").GetString());
            Assert.Equal("here", root.GetProperty("to").GetString());
            Assert.True(root.GetProperty("by_name").GetBoolean());
            Assert.True(root.GetProperty("offline").GetBoolean());
            Assert.True(root.GetProperty("force").GetBoolean());
            Assert.Equal(4, root.GetProperty("concurrency").GetInt32());
            Assert.Equal("tracked", root.GetProperty("target").GetProperty("kind").GetString());
            Assert.Equal("Bringing", root.GetProperty("labels").GetProperty("fetching").GetString());
        }

        var again = JsonSerializer.Deserialize<BringRequest>(json);

        Assert.NotNull(again);
        Assert.Equal("here", again.To);
        Assert.True(again.ByName);
        Assert.Equal("a.psd", again.Refs[0]);
        Assert.Equal("Assets/*", again.Refs[1]);
        Assert.Equal(WriteTargetKind.Tracked, again.Target.Kind);
        Assert.Equal("Bringing", again.Labels?.Fetching);
    }

    [Fact]
    public void A_bring_variant_request_writes_and_reads_its_fields()
    {
        // The native side dispatches on `join`, so the request names the variant twice: `variant` is
        // what the merge reads and `join` is what decides the document is a merge at all.
        var request = new BringVariantRequest
        {
            Workspace = "/work",
            Account = "alice",
            Variant = "blake3:abc",
            File = "a.psd",
            To = "beside",
            TreeMerge = true,
            Offline = true,
            Force = true,
            Concurrency = 2,
            Target = WriteTarget.Named("origin"),
            Labels = new WriteLabels { Fetching = "Bringing", Applying = "Joining" },
        };

        var json = JsonSerializer.Serialize(request);
        using (var document = JsonDocument.Parse(json))
        {
            var root = document.RootElement;
            Assert.Equal("/work", root.GetProperty("workspace").GetString());
            Assert.Equal("alice", root.GetProperty("account").GetString());
            Assert.Equal("blake3:abc", root.GetProperty("variant").GetString());
            Assert.Equal("a.psd", root.GetProperty("file").GetString());
            Assert.Equal("beside", root.GetProperty("to").GetString());
            Assert.True(root.GetProperty("tree_merge").GetBoolean());
            Assert.Equal(
                root.GetProperty("variant").GetString(),
                root.GetProperty("join").GetString()
            );
            Assert.True(root.GetProperty("offline").GetBoolean());
            Assert.True(root.GetProperty("force").GetBoolean());
            Assert.Equal(2, root.GetProperty("concurrency").GetInt32());
            Assert.Equal("named", root.GetProperty("target").GetProperty("kind").GetString());
            Assert.Equal("origin", root.GetProperty("target").GetProperty("name").GetString());
            Assert.Equal("Joining", root.GetProperty("labels").GetProperty("applying").GetString());
        }

        var again = JsonSerializer.Deserialize<BringVariantRequest>(json);

        Assert.NotNull(again);
        Assert.Equal("blake3:abc", again.Variant);
        Assert.Equal("a.psd", again.File);
        Assert.Equal("beside", again.To);
        Assert.True(again.TreeMerge);
        Assert.Equal("blake3:abc", again.Join);
        Assert.Equal("alice", again.Account);
        Assert.True(again.Offline);
        Assert.True(again.Force);
        Assert.Equal(2, again.Concurrency);
        Assert.Equal(WriteTargetKind.Named, again.Target.Kind);
        Assert.Equal("Bringing", again.Labels?.Fetching);
    }

    [Fact]
    public void A_bring_variant_of_a_word_that_names_no_variant_is_refused()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var variant = "not-a-variant";

            var error = Assert.Throws<RolaWriteError>(
                () =>
                    writer.BringVariant(
                        new BringVariantRequest
                        {
                            Workspace = directory,
                            Variant = variant,
                            File = "a.psd",
                        }
                    )
            );

            // The refusal is the merge's own reading of the word rather than a transport failure:
            // a word that is no hash is a miss, and the native side answers a miss with either of
            // these two kinds. The C ABI error names the word and has no field for the file it was
            // to be joined into, so the file is named only by the command line's own wrapper around
            // this failure, `error_bring_join`.
            Assert.True(
                error.Kind is RolaWriteErrorKind.NotFound or RolaWriteErrorKind.Precondition,
                error.Kind.ToString()
            );
            Assert.Contains(variant, error.Message, StringComparison.Ordinal);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    /// <summary>
    /// A variant merge the window asks for finishes from the Workspace's own store when the run is
    /// offline.
    /// </summary>
    /// <remarks>
    /// This is the writer the Desktop plugin calls, so the check is made at the door the window
    /// uses rather than through the command line. The variant, its content and the file it is joined
    /// into are all here, so nothing has to be reached for: a run that looked for a Vault anyway
    /// would fail with the window's own words instead of writing the variant file.
    /// </remarks>
    [Fact]
    public void A_bring_variant_finishes_offline_from_the_store()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            // The window's Workspaces are bound to a Vault, and that binding is what a run reaches
            // for when the content is not here. The address is never dialled: the run is offline, so
            // what this checks is that a bound Vault is not mistaken for a needed one.
            RunRola(directory, "vault bind origin 127.0.0.1:1/vault");

            // An account is located by the name of its key file alone, so the run acts as alice
            // without anything being signed.
            var keys = Path.Combine(directory, ".rola", "auth");
            Directory.CreateDirectory(keys);
            File.WriteAllText(
                Path.Combine(keys, "alice.pem"),
                "a key this test never signs with"
            );

            var file = Path.Combine(directory, "a.psd");
            File.WriteAllText(file, "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var first = writer.TrackFile("a.psd", "alice", "first");

            File.WriteAllText(file, "two");
            var second = writer.TrackFile("a.psd", "alice", "second");

            // The variant of the second version is based on the first, so the file's own chain
            // shares a version with it and the merge has a place to hang.
            Assert.NotEqual(first.Version, second.Version);

            var written = writer.BringVariant(
                new BringVariantRequest
                {
                    Workspace = directory,
                    Account = "alice",
                    Target = WriteTarget.Tracked,
                    Variant = second.Variant,
                    File = "a.psd",
                    Offline = true,
                }
            );

            Assert.True(
                File.Exists(Path.Combine(directory, written.Path)),
                $"the variant file was not written at {written.Path}"
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_track_plan_needs_an_account()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            File.WriteAllText(Path.Combine(directory, "a.psd"), "content");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var error = Assert.Throws<RolaWriteError>(
                () =>
                    writer.PlanTrack(
                        new TrackRequest { Workspace = directory, Files = ["a.psd"] }
                    )
            );

            Assert.Equal(RolaWriteErrorKind.NoAccount, error.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_track_plan_and_an_apply_record_a_file()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            // An account is located by the name of its key file alone, so a file this test never
            // signs with still lets the recording act as alice: what a recording names is the
            // creator, and no signature is made here.
            var keys = Path.Combine(directory, ".rola", "auth");
            Directory.CreateDirectory(keys);
            File.WriteAllText(
                Path.Combine(keys, "alice.pem"),
                "a key this test never signs with"
            );
            File.WriteAllText(Path.Combine(directory, "a.txt"), "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var request = new TrackRequest
            {
                Workspace = directory,
                Account = "alice",
                Files = ["a.txt"],
            };

            var plan = writer.PlanTrack(request);
            var subject = Assert.Single(plan.Subjects);
            Assert.Equal("a.txt", subject.Path);
            Assert.Equal(64, subject.Digest.Length);

            var result = writer.Track(
                request,
                plan,
                new Dictionary<string, string> { ["a.txt"] = "first" }
            );
            var item = Assert.Single(result.Items);
            Assert.Equal("a.txt", item.Path);
            Assert.Equal(TrackAction.Added, item.Action);

            var again = writer.PlanTrack(request);
            Assert.Empty(again.Subjects);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_cancelled_plan_is_refused()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var keys = Path.Combine(directory, ".rola", "auth");
            Directory.CreateDirectory(keys);
            File.WriteAllText(
                Path.Combine(keys, "alice.pem"),
                "a key this test never signs with"
            );
            File.WriteAllText(Path.Combine(directory, "a.txt"), "one");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            using var cancellation = Cancellation.New();
            cancellation.Cancel();

            var error = Assert.Throws<RolaWriteError>(
                () =>
                    writer.PlanTrack(
                        new TrackRequest
                        {
                            Workspace = directory,
                            Account = "alice",
                            Files = ["a.txt"],
                        },
                        cancellation: cancellation
                    )
            );

            Assert.Equal(RolaWriteErrorKind.Cancelled, error.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_cancelled_sync_request_plan_is_refused()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");
            var keys = Path.Combine(directory, ".rola", "auth");
            Directory.CreateDirectory(keys);
            File.WriteAllText(
                Path.Combine(keys, "alice.pem"),
                "a key this test never signs with"
            );

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            using var cancellation = Cancellation.New();
            cancellation.Cancel();

            var error = Assert.Throws<RolaWriteError>(
                () =>
                    writer.PlanSync(
                        new SyncRequest { Account = "alice", Offline = true },
                        cancellation: cancellation
                    )
            );

            Assert.Equal(RolaWriteErrorKind.Cancelled, error.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_track_request_writes_and_reads_its_optional_fields()
    {
        var request = new TrackRequest
        {
            Workspace = "/work",
            Account = "alice",
            Files = ["a.psd", "Assets"],
            Replace = true,
            Policy = "psd-basic",
            IncludeIgnored = true,
            Offline = true,
            Force = true,
            Concurrency = 3,
            Target = WriteTarget.Named("origin"),
            Labels = new WriteLabels { Checking = "Checking", Applying = "Recording" },
        };

        var json = JsonSerializer.Serialize(request);
        using (var document = JsonDocument.Parse(json))
        {
            var root = document.RootElement;
            Assert.Equal("/work", root.GetProperty("workspace").GetString());
            Assert.Equal("alice", root.GetProperty("account").GetString());
            Assert.Equal("a.psd", root.GetProperty("files")[0].GetString());
            Assert.Equal("Assets", root.GetProperty("files")[1].GetString());
            Assert.True(root.GetProperty("replace").GetBoolean());
            Assert.Equal("psd-basic", root.GetProperty("policy").GetString());
            Assert.True(root.GetProperty("include_ignored").GetBoolean());
            Assert.True(root.GetProperty("offline").GetBoolean());
            Assert.True(root.GetProperty("force").GetBoolean());
            Assert.Equal(3, root.GetProperty("concurrency").GetInt32());
            Assert.Equal("named", root.GetProperty("target").GetProperty("kind").GetString());
            Assert.Equal("origin", root.GetProperty("target").GetProperty("name").GetString());
            Assert.Equal("Checking", root.GetProperty("labels").GetProperty("checking").GetString());
        }

        var again = JsonSerializer.Deserialize<TrackRequest>(json);

        Assert.NotNull(again);
        Assert.Equal("a.psd", again.Files[0]);
        Assert.Equal("Assets", again.Files[1]);
        Assert.True(again.Replace);
        Assert.Equal("psd-basic", again.Policy);
        Assert.True(again.IncludeIgnored);
        Assert.Equal(WriteTargetKind.Named, again.Target.Kind);
        Assert.Equal("origin", again.Target.Name);
        Assert.Equal("Checking", again.Labels?.Checking);
    }

    [Fact]
    public void A_track_plan_that_was_not_made_by_plan_track_is_refused()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);
            var plan = new TrackPlan([], 0, [], null);

            var error = Assert.Throws<RolaWriteError>(
                () =>
                    writer.Track(
                        new TrackRequest { Workspace = directory, Files = ["a.psd"] },
                        plan,
                        new Dictionary<string, string>()
                    )
            );

            Assert.Equal(RolaWriteErrorKind.Precondition, error.Kind);
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_workspace_binds_a_store_and_writes_into_it()
    {
        var directory = Scratch();
        var store = Path.Combine(Scratch(), "shared");
        try
        {
            RunRola(directory, "init");
            RunRola(directory, $"storage create \"{store}\"");
            RunRola(directory, $"bind \"{store}\"");

            var workspace = new WorkspaceScope(directory);
            workspace.Read();

            var storages = workspace.Config!.Storages;
            Assert.Equal(2, storages.Count);
            Assert.False(storages[0].IsOwn);
            Assert.True(storages[0].Present);
            Assert.True(storages[0].Writable);
            Assert.True(storages[1].IsOwn);

            var source = Path.Combine(directory, "bound.bin");
            File.WriteAllText(source, "written into the bound store");

            var storage = workspace.Storage;
            storage.Read();
            var writer = new StorageWriter(storage);
            var stored = writer.Write(new FileInfo(source));

            Assert.NotEmpty(
                Directory.GetFiles(store, "*", SearchOption.AllDirectories)
            );

            var workspaceStore = Path.Combine(directory, ".rola", "storage", "obj");
            Assert.Empty(
                Directory.Exists(workspaceStore)
                    ? Directory.GetFiles(workspaceStore, "*", SearchOption.AllDirectories)
                    : Array.Empty<string>()
            );

            var destination = new FileInfo(Path.Combine(directory, "bound-out.bin"));
            writer.Extract(stored.Key, destination);
            Assert.Equal("written into the bound store", File.ReadAllText(destination.FullName));
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    [Fact]
    public void A_store_is_made_and_bound_through_the_writer()
    {
        var directory = Scratch();
        var store = Path.Combine(Scratch(), "shared");
        try
        {
            RunRola(directory, "init");
            StorageWriter.Create(new DirectoryInfo(store));
            Assert.True(File.Exists(Path.Combine(store, "rolast.toml")));

            var workspace = new WorkspaceScope(directory);
            workspace.Read();
            var bound = new WorkspaceWriter(workspace).BindStorage(new DirectoryInfo(store));

            Assert.True(bound.BoundWorkspace);
            Assert.False(bound.BoundVault);
            Assert.True(bound.ReverseRecorded);

            var reread = new WorkspaceScope(directory);
            reread.Read();
            Assert.Contains(
                reread.Config!.Storages,
                item => !item.IsOwn && item.Present && item.Writable
            );
        }
        finally
        {
            Directory.Delete(directory, true);
        }
    }

    private static string Scratch() =>
        Directory.CreateTempSubdirectory("rola-sharp-write-").FullName;

    private static string RepositoryRoot()
    {
        var at = new DirectoryInfo(AppContext.BaseDirectory);
        while (at is not null)
        {
            if (Directory.Exists(Path.Combine(at.FullName, ".git")))
            {
                return at.FullName;
            }

            at = at.Parent;
        }

        throw new InvalidOperationException("the repository root was not found");
    }

    private static void CopyDirectory(string source, string destination)    {
        Directory.CreateDirectory(destination);
        foreach (var file in Directory.GetFiles(source))
        {
            File.Copy(file, Path.Combine(destination, Path.GetFileName(file)), true);
        }

        foreach (var directory in Directory.GetDirectories(source))
        {
            CopyDirectory(directory, Path.Combine(destination, Path.GetFileName(directory)));
        }
    }

    private static void RunRola(string directory, string arguments)
    {
        var bin = Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
            ?? throw new InvalidOperationException("ROLA_BIN_DIR is not set");
        var program = Path.Combine(bin, OperatingSystem.IsWindows() ? "rola.exe" : "rola");
        var start = new ProcessStartInfo(program, arguments)
        {
            WorkingDirectory = directory,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
        };
        using var process = Process.Start(start)
            ?? throw new InvalidOperationException("the command line did not start");
        var output = new StringBuilder();
        output.Append(process.StandardOutput.ReadToEnd());
        output.Append(process.StandardError.ReadToEnd());
        process.WaitForExit();

        Assert.True(process.ExitCode == 0, output.ToString());
    }
}

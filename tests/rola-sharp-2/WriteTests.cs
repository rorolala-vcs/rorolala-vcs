using System.Diagnostics;
using System.Text;

using RolaSharp.V2;
using RolaSharp.V2.Write;
using Xunit;

namespace RolaSharp.V2.IntegrationTests;

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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
            workspace.Read();
            var index = workspace.VCSIndex;
            index.Read();
            var writer = new VCSIndexWriter(index);

            var creator = writer.WriteCreator("alice");
            var message = writer.WriteMessage("first");

            Assert.False(string.IsNullOrEmpty(creator));
            Assert.False(string.IsNullOrEmpty(message));
            Assert.Contains("alice", index.ObjectJson(creator), StringComparison.Ordinal);

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

            var workspace = new WorkspaceScoop(directory);
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
    public async Task An_async_aggregate_read_honours_cancellation()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            using var cancellation = new CancellationTokenSource();
            await cancellation.CancelAsync();

            var error = await Assert.ThrowsAsync<RolaReadError>(
                () =>
                    ReadApi.JsonAsync(
                        directory,
                        "{\"kind\":\"layouts\",\"path\":\"\"}",
                        cancellation.Token
                    )
            );

            Assert.Equal(RolaReadErrorKind.Cancelled, error.Kind);
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

            var workspace = WorkspaceScoop.Open(nested);

            Assert.Equal(Path.GetFullPath(directory), workspace.Root.FullName.TrimEnd(Path.DirectorySeparatorChar));
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
        var book = BookScoop.Open(Path.Combine(root, "build", "docs", "builtin.book"));
        book.Read();

        Assert.True(book.ArticleCount > 0);
        Assert.NotEmpty(book.Search("rola", 3));
    }

    [Fact]
    public void A_workspace_exposes_its_ignore_rules()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var workspace = new WorkspaceScoop(directory);
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

            var ignore = new IgnoreScoop(rules, directory);
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
    public void An_aggregate_read_answers_and_can_be_cancelled()
    {
        var directory = Scratch();
        try
        {
            RunRola(directory, "init");

            var json = ReadApi.Json(directory, "{\"kind\":\"layouts\",\"path\":\"\"}");
            Assert.Contains("main", json, StringComparison.Ordinal);

            using var cancellation = Cancellation.New();
            cancellation.Cancel();
            var error = Assert.Throws<RolaReadError>(
                () =>
                    ReadApi.Json(
                        directory,
                        "{\"kind\":\"layouts\",\"path\":\"\"}",
                        cancellation
                    )
            );
            Assert.Equal(RolaReadErrorKind.Cancelled, error.Kind);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
            workspace.Read();
            var writer = new WorkspaceWriter(workspace);

            var tracked = writer.TrackFile("a.psd", "alice", "first");

            Assert.NotEqual(Guid.Empty, tracked.Id);
            Assert.False(string.IsNullOrEmpty(tracked.Version));

            File.WriteAllText(Path.Combine(directory, "a.psd"), "content again");
            var again = writer.TrackFile("a.psd", "alice", "second");

            Assert.Equal(tracked.Id, again.Id);
            Assert.NotEqual(tracked.Version, again.Version);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
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

            var workspace = new WorkspaceScoop(directory);
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

    private static string Scratch() =>
        Directory.CreateTempSubdirectory("rola-sharp-write-").FullName;

    private static string RepositoryRoot()
    {
        var at = new DirectoryInfo(AppContext.BaseDirectory);
        while (at is not null)
        {
            if (Directory.Exists(Path.Combine(at.FullName, "build", "docs")))
            {
                return at.FullName;
            }

            at = at.Parent;
        }

        throw new InvalidOperationException("the repository root was not found");
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

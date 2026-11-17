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

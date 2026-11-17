using System.Diagnostics;
using System.Text;

using RolaSharp.V2;
using RolaSharp.V2.Write;
using Xunit;

namespace RolaSharp.V2.IntegrationTests;

public sealed class WriteTests
{
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

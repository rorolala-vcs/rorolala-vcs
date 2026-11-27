using System.Diagnostics;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The operations a real Workspace recorded, read through the C ABI as the Workspace dock reads them.
/// </summary>
/// <remarks>
/// The journal is the writer's, so the fixture is made by the command line rather than written here:
/// a Workspace, one account, one file recorded. What is checked is that a run's command crosses and
/// that the effects it recorded are there as changes, which is the part the command line's own
/// listing does not print and the dock opens a row to show.
/// </remarks>
public sealed class WorkspaceJournalTests
{
    /// <summary>A tracked file is one operation whose changes are the entry and the path it is bound at.</summary>
    [Fact]
    public void ARecordedRunIsReadWithWhatItChanged()
    {
        var parent = Scratch.New("workspace-journal");
        var workspace = Path.Combine(parent, "ws");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "one\n");
        Given(workspace, "track", "art.txt", "--message", "first", "--no-editor");

        var operation = Assert.Single(WorkspaceOpsReading.Read(workspace));

        Assert.Equal("track", operation.Command);

        // Recording a file is two changes: an entry that did not exist and now does, and the path
        // that is bound to it. What a row shows is the effects the run actually wrote.
        Assert.Contains(
            operation.Changes,
            change =>
                change.Kind == "entry"
                && change.State == "added"
                && change.From.EndsWith("art.txt", StringComparison.Ordinal)
        );
        Assert.Contains(
            operation.Changes,
            change =>
                change.Kind == "path"
                && change.State == "bound"
                && change.To.EndsWith("art.txt", StringComparison.Ordinal)
        );
    }

    /// <summary>The newest operation is taken off the stack by the Undo the dock's header offers.</summary>
    [Fact]
    public void UndoTakesTheNewestOperationOffTheStack()
    {
        var parent = Scratch.New("workspace-journal-undo");
        var workspace = Path.Combine(parent, "ws");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "one\n");
        Given(workspace, "track", "art.txt", "--message", "first", "--no-editor");

        var (done, said) = WorkspaceUndo.Run(workspace);

        Assert.True(done, said);
        Assert.Empty(WorkspaceOpsReading.Read(workspace));
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

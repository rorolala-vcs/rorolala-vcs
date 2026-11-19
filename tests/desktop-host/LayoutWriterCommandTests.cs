using System.Diagnostics;

using RolaSharp;

using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The three low-level Layout commands, after they stopped running <c>rola</c>: what a move, a removal
/// and a fork now do through the writer, and what they say when the writer refuses.
/// </summary>
/// <remarks>
/// None of these needs the program the window was opened by, which is the point of the move: a move
/// within one Layout, a fork of one entry, and a removal in a Vault's copy are writes the library
/// already knows how to make. The suite has no Vault to ask, so what the two that reach one are checked
/// for here is the refusal the writer answers before a Vault is dialled: a Workspace that bound no
/// Vault cannot name one.
/// </remarks>
public sealed class LayoutWriterCommandTests
{
    /// <summary>A fork gives the path a new Uuid, holding the data the old entry held.</summary>
    [Fact]
    public void AForkGivesThePathANewUuidHoldingWhatItHeld()
    {
        var directory = Scratch.New("fork-uuid");
        var id = Guid.NewGuid().ToString();

        Given(directory, "init");
        Given(
            directory,
            "layout",
            "entry",
            "create",
            id,
            $"blake3:{new string('0', 64)}",
            "--owner",
            "alice",
            "--description",
            "the hero"
        );
        Given(directory, "layout", "path", "create", "a.psd", id);

        var (done, said) = LayoutPathNewUuidCommand.Run(directory, "a.psd");

        Assert.True(done, said);

        using var workspace = new WorkspaceScope(directory);
        workspace.Read();
        var layout = workspace.Layout;
        layout.Read();
        var forked = layout.Find("a.psd");

        Assert.NotNull(forked);
        Assert.NotEqual(id, forked.Uuid);
        Assert.Equal("alice", forked.Owner);
        Assert.Equal("the hero", forked.Description);
        Assert.Equal(new string('0', 64), forked.Version);
    }

    /// <summary>A fork of a path no Layout names is refused rather than making an entry.</summary>
    [Fact]
    public void AForkOfAPathTheLayoutDoesNotNameIsRefused()
    {
        var directory = Scratch.New("fork-missing");

        Given(directory, "init");

        var (done, said) = LayoutPathNewUuidCommand.Run(directory, "missing.psd");

        Assert.False(done);
        Assert.Contains("no such path", said, StringComparison.Ordinal);
    }

    /// <summary>A move in a fetched copy reaches the writer, which names the Vault the Workspace never bound.</summary>
    [Fact]
    public void AFetchedMoveReachesTheWriter()
    {
        var directory = Scratch.New("fetched-move");

        Given(directory, "init");

        var (done, said) = LayoutPathMoveCommand.Move(
            directory,
            LayoutPathMoveCommand.Named("origin"),
            "a.psd",
            "b.psd"
        );

        Assert.False(done);
        Assert.Contains("origin", said, StringComparison.Ordinal);
    }

    /// <summary>A removal in a fetched copy reaches the writer, which names the Vault the Workspace never bound.</summary>
    [Fact]
    public void AFetchedRemovalReachesTheWriter()
    {
        var directory = Scratch.New("fetched-remove");

        Given(directory, "init");

        var (done, said) = LayoutEntryRemoveCommand.Run(
            directory,
            LayoutPathMoveCommand.Named("origin"),
            Guid.NewGuid().ToString()
        );

        Assert.False(done);
        Assert.Contains("origin", said, StringComparison.Ordinal);
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

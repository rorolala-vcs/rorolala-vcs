using System.Diagnostics;
using RolaSharp;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The variant graph read through the native library: what a graph of the work names, and what a
/// window of it holds.
/// </summary>
/// <remarks>
/// The graph is the one reading this suite cannot stand in for: it is walked over the inverse index,
/// which only a workspace with a recorded history has, and the entries, the merges and the texts are
/// read beside it by the module that owns them. So the workspace is made by running the command line
/// here — a file recorded twice, which is a chain of two variants — and what is checked is the reading
/// of it through the C ABI and the wrapper above it.
/// </remarks>
public sealed class VariantGraphTests
{
    /// <summary>A chain of two versions is drawn with its entries, its words and its versions.</summary>
    [Fact]
    public void AGraphOfTheWorkNamesItsChainAndItsEntries()
    {
        var workspace = Recorded("graph");

        var graph = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(graph);

        // The focus is the entry the Layout names, which records the second version and resolves to
        // the variant that version fixes.
        var focus = Assert.Single(graph.Focus);
        Assert.Equal("local", focus.Side);
        Assert.Equal("art.txt", focus.Path);
        Assert.NotNull(focus.Version);
        Assert.NotNull(focus.Variant);

        // The chain behind it: two variants, the three versions that fix them and the root, and one
        // base edge between the two variants.
        Assert.Equal(2, graph.Nodes.Count);
        Assert.Equal(3, graph.Versions.Count);
        var edge = Assert.Single(graph.Edges);
        Assert.Equal("base", edge.Kind);
        Assert.False(graph.Truncated);
        Assert.Empty(graph.Missing);

        // The first version is numbered from nothing and the second one past it; the root is named as
        // the root rather than numbered.
        Assert.Contains(graph.Versions, version => version is { Number: 0, IsRoot: false });
        Assert.Contains(graph.Versions, version => version is { Number: 1, IsRoot: false });
        Assert.Contains(
            graph.Versions,
            version => version is { Number: null, IsRoot: true }
        );

        // The entry is named beside the graph, held by the account the work was recorded as.
        var entry = Assert.Single(graph.Entries);
        Assert.Equal("local", entry.Side);
        Assert.Equal("art.txt", entry.Path);
        Assert.NotNull(entry.Version);
        Assert.Equal("alice", entry.Owner);
        Assert.False(entry.Deprecated);

        // The words of both variants are read from the index, once each.
        Assert.Contains("alice", graph.Texts.Creators.Values);
        Assert.Contains("first", graph.Texts.Messages.Values);
        Assert.Contains("second", graph.Texts.Messages.Values);
    }

    /// <summary>A window holds what the request asked for, and says where it stopped.</summary>
    [Fact]
    public void AWindowHoldsWhatTheRequestAskedFor()
    {
        var workspace = Recorded("window");

        // A second file is another chain, and the reading of the whole work names both its entries.
        File.WriteAllText(Path.Combine(workspace, "other.txt"), "other\n");
        Given(workspace, "track", "other.txt", "--message", "other", "--no-editor");

        var whole = RolaVariantGraph.Locate(workspace);

        Assert.NotNull(whole);
        Assert.Equal(2, whole.Focus.Count);
        Assert.Equal(2, whole.Entries.Count);

        var graph = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest { MaxNodes = 1 }
        );

        Assert.NotNull(graph);
        Assert.True(graph.Truncated);

        var node = Assert.Single(graph.Nodes);
        Assert.True(node.Frontier.Parents);

        // A window that stands on one file carries the entry that file is, and not the other chain's.
        var one = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "art.txt")],
            }
        );

        Assert.NotNull(one);
        Assert.Equal("art.txt", Assert.Single(one.Entries).Path);

        // A focus the reading does not hold is answered as an entry recording no version, and the
        // window is empty rather than centred somewhere else.
        var unknown = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("local", "nowhere.txt")],
            }
        );

        Assert.NotNull(unknown);
        var focus = Assert.Single(unknown.Focus);
        Assert.Null(focus.Version);
        Assert.Null(focus.Variant);
        Assert.Empty(unknown.Nodes);
        Assert.Empty(unknown.Entries);

        // A side this reading does not hold is the same answer: the work may have moved on since the
        // request was written, and it is not answered from somewhere else.
        var unknownSide = RolaVariantGraph.Locate(
            workspace,
            new VariantGraphRequest
            {
                Focus = [new VariantGraphFocus("vault", "art.txt")],
            }
        );

        Assert.NotNull(unknownSide);
        Assert.Null(unknownSide.Focus[0].Version);
        Assert.Empty(unknownSide.Nodes);
    }

    /// <summary>A directory no workspace holds has no graph, which is nothing rather than a failure.</summary>
    [Fact]
    public void ADirectoryWithoutAWorkspaceHasNoGraph()
    {
        var plain = Scratch.New("plain");

        Assert.Null(RolaVariantGraph.Locate(plain));
    }

    /// <summary>
    /// A workspace holding one file with two recorded versions, and the account the work is recorded
    /// as.
    /// </summary>
    /// <param name="label">What the workspace is for, for a person reading the path.</param>
    /// <returns>The workspace's directory.</returns>
    private static string Recorded(string label)
    {
        var parent = Scratch.New($"variant-graph-{label}");
        var workspace = Path.Combine(parent, "ws");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "one\n");
        Given(workspace, "track", "art.txt", "--message", "first", "--no-editor");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "two\n");
        Given(workspace, "track", "art.txt", "--message", "second", "--no-editor");

        return workspace;
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

using System.Diagnostics;

using RolaSharp;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The read the Desktop does through the C ABI rather than through the program beside it: the Layout
/// a place holds, the Vaults a Workspace knows, the mark a tree carries, and the identities a key
/// scope finds.
/// </summary>
/// <remarks>
/// The fixtures are made by the command line, so what is read here is what the commands left: a
/// Workspace working in a Layout that names one recorded file, and the account that recorded it. The
/// library is loaded from <c>ROLA_BIN_DIR</c> by <see cref="NativeRorolala"/>.
/// </remarks>
public sealed class LayoutReadTests
{
    /// <summary>A Workspace's Layout is listed under the label the command line uses.</summary>
    [Fact]
    public void AWorkspaceListsTheLayoutItWorksIn()
    {
        var workspace = Recorded("layouts");

        var layouts = RolaLayouts.Locate(workspace);

        var one = Assert.Single(layouts);
        Assert.Equal("main", one.Name);
        Assert.True(one.IsCurrent);
        Assert.Equal(string.Empty, one.Tracking);
    }

    /// <summary>The entries are the ones the Layout records, with what each says about itself.</summary>
    [Fact]
    public void TheEntriesCarryTheirPathOwnerAndVersion()
    {
        var workspace = Recorded("entries");

        var reading = RolaLayoutEntries.Locate(workspace);

        Assert.Equal(LayoutReadState.Ok, reading.State);
        Assert.Equal(string.Empty, reading.Reason);

        var entry = Assert.Single(reading.Entries, one => one.Path == "art.txt");
        Assert.Equal("alice", entry.Owner);
        Assert.False(string.IsNullOrEmpty(entry.Version));
        Assert.False(entry.Deprecated);
    }

    /// <summary>A copy that was never fetched is told apart from a name that is simply wrong.</summary>
    [Fact]
    public void AnUnfetchedCopyAndAWrongNameAreDifferentStates()
    {
        var workspace = Recorded("states");

        Assert.Equal(
            LayoutReadState.NotCached,
            RolaLayoutEntries.Locate(workspace, "truth@studio").State
        );
        Assert.Equal(
            LayoutReadState.Missing,
            RolaLayoutEntries.Locate(workspace, "nowhere").State
        );
    }

    /// <summary>A directory outside every place has no Layout to read.</summary>
    [Fact]
    public void ADirectoryOutsideEveryPlaceHasNoWorkspace()
    {
        var nowhere = Scratch.New("layout-nowhere");

        Assert.Equal(
            LayoutReadState.NoWorkspace,
            RolaLayoutEntries.Locate(nowhere, null).State
        );
        Assert.Empty(RolaLayouts.Locate(nowhere));
    }

    /// <summary>A Workspace's configuration is read, and its Vaults are the ones it knows.</summary>
    [Fact]
    public void TheConfigurationOfAWorkspaceIsRead()
    {
        var workspace = Recorded("config");

        var config = RolaWorkspaceConfig.Locate(workspace);

        Assert.NotNull(config);
        Assert.Empty(config.Vaults);
        Assert.Null(config.Default);
    }

    /// <summary>A tree's mark is the cheap question, and its id and draft are the data beside it.</summary>
    [Fact]
    public void ATreeMarkIsReadWithoutTheRestOfTheReading()
    {
        var root = Scratch.New("tree-mark");
        var mark = Path.Combine(root, ".rola-tree");

        Assert.False(RolaTreeMark.IsTree(root));
        Assert.Null(RolaTreeMark.Locate(root));

        Directory.CreateDirectory(mark);
        File.WriteAllText(Path.Combine(mark, "id"), "tree-id");
        File.WriteAllText(Path.Combine(mark, "description"), "what the next version says");

        Assert.True(RolaTreeMark.IsTree(root));

        var reading = RolaTreeMark.Locate(root);

        Assert.NotNull(reading);
        Assert.Equal("tree-id", reading.Id);
        Assert.True(reading.Described);
        Assert.Equal("what the next version says", reading.Description);
    }

    /// <summary>The names the two programs share are read from the library rather than written twice.</summary>
    [Fact]
    public void TheNamesAreReadFromTheLibrary()
    {
        var workspace = Recorded("paths");

        Assert.Equal("truth", RolaPaths.VaultLayoutName);
        Assert.Equal(
            Path.TrimEndingDirectorySeparator(
                Path.GetFullPath(Path.Combine(workspace, ".rola", "info", "ignore"))
            ),
            Path.TrimEndingDirectorySeparator(
                Path.GetFullPath(RolaPaths.WorkspaceIgnoreRules(workspace))
            )
        );
    }

    /// <summary>The accounts a key scope finds are read one at a time, and the rule is a rule.</summary>
    [Fact]
    public void TheAccountsOfAScopeAreReadThroughTheRule()
    {
        using var rule = RolaKeyLocateRule.New();

        Assert.True(rule.FindGlobal);
        Assert.True(rule.FindLocal);
        Assert.True(rule.FindUser);
        Assert.True(rule.FindEnv);

        rule.FindUser = false;
        Assert.False(rule.FindUser);
        rule.FindUser = true;

        using var accounts = RolaAccounts.Locate(null, rule);

        Assert.NotNull(accounts);

        // A private key is not shared, so an account is found beside a place and not under the
        // user's data directory; what is checked here is that the list and its positions agree.
        for (var index = 0; index < accounts.Count; index++)
        {
            using var account = accounts[index];

            Assert.NotNull(account);
            Assert.False(string.IsNullOrEmpty(account.Name));
            Assert.False(string.IsNullOrEmpty(account.KeyPath));
        }

        Assert.Null(accounts[accounts.Count]);
        Assert.Null(accounts[-1]);
    }

    /// <summary>The members a key scope finds include the public half of the account's pair.</summary>
    [Fact]
    public void TheMembersOfTheMachineAreReadThroughTheRule()
    {
        var workspace = Recorded("members");

        using var rule = RolaKeyLocateRule.New();
        using var members = RolaMembers.Locate(null, rule);

        Assert.NotNull(members);

        var found = new List<string>();

        for (var index = 0; index < members.Count; index++)
        {
            using var member = members[index];

            Assert.NotNull(member);
            found.Add(member.Name);
        }

        Assert.Contains("alice", found);
    }

    /// <summary>A workspace holding one recorded file, recorded as Alice.</summary>
    /// <param name="label">What the workspace is for, for a person reading the path.</param>
    /// <returns>The workspace's directory.</returns>
    private static string Recorded(string label)
    {
        var parent = Scratch.New($"layout-read-{label}");
        var workspace = Path.Combine(parent, "ws");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "one\n");
        Given(workspace, "track", "art.txt", "--message", "first", "--no-editor");

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

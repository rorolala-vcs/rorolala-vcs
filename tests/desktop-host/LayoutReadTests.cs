using System.Diagnostics;

using RolaSharp;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The read the Desktop does through the C ABI rather than through the program beside it: the Layout
/// a place holds, the configuration a Workspace knows, the mark a tree carries, and the identities a
/// key scope finds.
/// </summary>
/// <remarks>
/// The fixtures are made by the command line, so what is read here is what the commands left: a
/// Workspace working in a Layout that names one recorded file, and the account that recorded it. The
/// library is loaded from <c>ROLA_BIN_DIR</c> by <see cref="NativeRorolala"/>.
/// </remarks>
public sealed class LayoutReadTests
{
    /// <summary>A Workspace's Layout is read as the one it works in.</summary>
    [Fact]
    public void AWorkspaceReadsTheLayoutItWorksIn()
    {
        var workspace = Recorded("layouts");

        var layout = new LocalLayoutScope(workspace);

        // A scope that names no Layout is the one being worked in, which is the label the command
        // line calls `main`.
        Assert.Null(layout.Name);

        layout.Read();

        Assert.Equal(Path.GetFullPath(workspace), layout.Root.FullName);
        Assert.Contains(layout.Entries, entry => entry.Path == "art.txt");
    }

    /// <summary>The entries are the ones the Layout records, with what each says about itself.</summary>
    [Fact]
    public void TheEntriesCarryTheirPathOwnerAndVersion()
    {
        var workspace = Recorded("entries");

        var layout = new LocalLayoutScope(workspace);
        layout.Read();

        var entry = Assert.Single(layout.Entries, one => one.Path == "art.txt");
        Assert.Equal("alice", entry.Owner);
        Assert.False(string.IsNullOrEmpty(entry.Version));
        Assert.False(entry.Deprecated);
    }

    /// <summary>A copy that was never fetched is told apart from a name that is simply wrong.</summary>
    [Fact]
    public void AnUnfetchedCopyAndAWrongNameAreDifferentStates()
    {
        var workspace = Recorded("states");

        var notCached = Assert.Throws<RolaReadError>(
            () => new TruthLayoutScope(workspace, "studio").Read()
        );
        Assert.Equal(RolaReadErrorKind.NotCached, notCached.Kind);

        var missing = Assert.Throws<RolaReadError>(
            () => new LayoutScope(workspace, "nowhere").Read()
        );
        Assert.Equal(RolaReadErrorKind.Missing, missing.Kind);
    }

    /// <summary>A directory outside every place has no Layout to read.</summary>
    [Fact]
    public void ADirectoryOutsideEveryPlaceHasNoWorkspace()
    {
        var nowhere = Scratch.New("layout-nowhere");

        var noWorkspace = Assert.Throws<RolaReadError>(
            () => new LocalLayoutScope(nowhere).Read()
        );
        Assert.Equal(RolaReadErrorKind.NoWorkspace, noWorkspace.Kind);

        var unavailable = Assert.Throws<RolaReadError>(() => WorkspaceScope.Open(nowhere));
        Assert.Equal(RolaReadErrorKind.Unavailable, unavailable.Kind);
    }

    /// <summary>A Workspace's configuration is read, and its Vaults are the ones it knows.</summary>
    [Fact]
    public void TheConfigurationOfAWorkspaceIsRead()
    {
        var workspace = Recorded("config");

        using var scope = new WorkspaceScope(workspace);
        scope.Read();

        Assert.NotNull(scope.Config);
        Assert.Empty(scope.Config.Vaults);
        Assert.Null(scope.Config.Default);
    }

    /// <summary>A tree's mark is the cheap question, and its id and draft are the data beside it.</summary>
    [Fact]
    public void ATreeMarkIsReadWithoutTheRestOfTheReading()
    {
        var root = Scratch.New("tree-mark");
        var mark = Path.Combine(root, ".rola-tree");

        Assert.False(TreeScope.IsTree(root));
        Assert.Null(TreeScope.MarkOf(root));

        Directory.CreateDirectory(mark);
        File.WriteAllText(Path.Combine(mark, "id"), "tree-id");
        File.WriteAllText(Path.Combine(mark, "description"), "what the next version says");

        Assert.True(TreeScope.IsTree(root));

        var reading = TreeScope.MarkOf(root);

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

    /// <summary>The accounts a user scope holds are read with the key files behind them.</summary>
    [Fact]
    public void TheAccountsOfAUserScopeAreRead()
    {
        _ = Recorded("accounts");

        var user = UserScopeWithInstalledKey();
        user.Read();

        var account = Assert.Single(user.Accounts);
        Assert.Equal("alice", account.Name);
        Assert.False(string.IsNullOrEmpty(account.Private.File.FullName));
        Assert.NotNull(account.Public);
        Assert.Equal("alice", account.Public.Name);
        Assert.Contains(user.PrivateKeys, key => key.Name == "alice");
    }

    /// <summary>The members a user scope holds include the public half of the account's pair.</summary>
    [Fact]
    public void TheMembersOfAUserScopeAreRead()
    {
        _ = Recorded("members");

        var user = UserScopeWithInstalledKey();
        user.Read();

        Assert.Contains(user.PublicKeys, key => key.Name == "alice");
        Assert.All(
            user.PublicKeys,
            key => Assert.False(string.IsNullOrEmpty(key.File.FullName))
        );
    }

    /// <summary>An account beside a Vault is read one at a time through the account scope.</summary>
    [Fact]
    public void AnAccountBesideAVaultIsReadThroughTheAccountScope()
    {
        var parent = Scratch.New("account-scope");
        var vault = Path.Combine(parent, "vault");

        Given(parent, "key", "generate", "--install", "alice");
        Given(parent, "create", "--vault", vault);

        CopyInstalledKeys(Path.Combine(vault, "keys"));

        using var scope = new VaultScope(vault);
        scope.Read();

        using var account = AccountScope.Locate(scope);

        Assert.NotNull(account);
        Assert.Equal("alice", account.Name);
        Assert.False(string.IsNullOrEmpty(account.KeyPath));
    }

    /// <summary>A user scope over a scratch home, holding a copy of the account the fixture installed.</summary>
    /// <returns>The scope, which has not been read.</returns>
    private static UserScope UserScopeWithInstalledKey()
    {
        var user = new UserScope(Scratch.New("user-home"));
        CopyInstalledKeys(user.KeysDirectory.FullName);

        return user;
    }

    /// <summary>Copies the account the fixture installed into a key directory.</summary>
    /// <param name="into">The directory the key files are copied into.</param>
    private static void CopyInstalledKeys(string into)
    {
        Directory.CreateDirectory(into);

        foreach (var name in new[] { "alice.pem", "alice.pub" })
        {
            File.Copy(
                Path.Combine(InstalledKeys(), name),
                Path.Combine(into, name),
                overwrite: true
            );
        }
    }

    /// <summary>Where the command line installed the fixture's key pair.</summary>
    /// <returns>The user key directory of the run's data home.</returns>
    private static string InstalledKeys()
    {
        var home =
            Environment.GetEnvironmentVariable("ROLA_DATA_HOME")
            ?? throw new InvalidOperationException("the suites are run with ROLA_DATA_HOME");

        return Path.Combine(home, "rola", "keys");
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

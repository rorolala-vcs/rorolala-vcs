using System.Diagnostics;
using Avalonia.Media;
using FileSystemPlugin;
using GitVCSPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The git-rola place's reading of a project, and the prefix it is offered under.
/// </summary>
/// <remarks>
/// What the place does with a Vault is a run, which this suite has no daemon for; what it does with
/// the project's own files is a reading, and that is what is checked here. The prefix is the one
/// the contract's grammar had to be widened for, so it is checked with the rest.
/// </remarks>
public sealed class GitRolaTests
{
    /// <summary>The prefix the place claims is one the contract allows.</summary>
    [Fact]
    public void ThePrefixIsOneTheContractAllows()
    {
        Assert.True(LogicalAddress.IsScheme(GitRolaPlace.Scheme));
        Assert.True(LogicalAddress.TryRead("git-rola@vault", out var address));
        Assert.Equal("git-rola", address!.Scheme);
        Assert.Equal("vault", address.Payload);

        Assert.False(LogicalAddress.IsScheme("git-"));
        Assert.False(LogicalAddress.IsScheme("-git"));
        Assert.True(LogicalAddress.IsScheme("git-rola.x"));
    }

    /// <summary>The project file names its Vaults, and the default one it states.</summary>
    [Fact]
    public void TheProjectFileNamesItsVaultsAndItsDefault()
    {
        const string text = """
            # where the work comes from
            [vaults]
            origin = "rola://127.0.0.1/rainbow"
            mirror = "rola://127.0.0.1/mirror"

            [defaults]
            vault = "mirror"
            """;

        var project = GitRolaToml.Read(text);

        Assert.Equal(new[] { "origin", "mirror" }, project.Vaults);
        Assert.Equal("mirror", project.DefaultVault);
    }

    /// <summary>A project that states no default goes through origin, then the first it names.</summary>
    [Fact]
    public void AProjectWithoutADefaultGoesThroughOrigin()
    {
        Assert.Equal(
            "mirror",
            GitRolaToml.Read("[vaults]\nmirror = \"rola://127.0.0.1/mirror\"\n").DefaultVault
        );
        Assert.Equal(
            "origin",
            GitRolaToml.Read("[vaults]\norigin = \"rola://127.0.0.1/a\"\nmirror = \"rola://127.0.0.1/b\"\n").DefaultVault
        );
    }

    /// <summary>The lock names the paths it places, and nothing else.</summary>
    [Fact]
    public void TheLockNamesThePathsItPlaces()
    {
        var materials = GitRolaLock.Materials(
            "? FORMAT 2\n? VAULT origin rola://127.0.0.1/rainbow\n/Assets/hero.psd blake3:0000 file\n/Source/c.fbx blake3:1111 tree\n"
        );

        Assert.Contains("Assets/hero.psd", materials);
        Assert.Contains("Source/c.fbx", materials);
        Assert.DoesNotContain("? FORMAT 2", materials);
        Assert.DoesNotContain("/Assets/hero.psd", materials);
    }

    /// <summary>The check command runs where the project is, and writes the short status.</summary>
    [Fact]
    public void TheCheckRunsWhereTheProjectIs()
    {
        var bin =
            Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
            ?? throw new InvalidOperationException("the suites are run with ROLA_BIN_DIR");
        var directory = Scratch.New("git-rola-check");

        Init(directory);
        File.WriteAllText(
            Path.Combine(directory, "rola.toml"),
            "[vaults]\norigin = \"rola://127.0.0.1/rainbow\"\n\n[features]\ngit_support = true\n"
        );
        File.WriteAllText(
            Path.Combine(directory, "rola.lock"),
            "? FORMAT 2\n/hero.psd blake3:0000000000000000000000000000000000000000000000000000000000000000 file\n"
        );
        File.WriteAllText(Path.Combine(directory, "hero.psd"), "not the lock's\n");

        var (code, output, said) = GitRolaCommandLine.Run(
            Path.Combine(bin, OperatingSystem.IsWindows() ? "git-rola.exe" : "git-rola"),
            ["check"],
            directory
        );

        Assert.Equal(1, code);
        Assert.Contains("hero.psd", output, StringComparison.Ordinal);
        Assert.True(said.Length == 0, said);
    }

    /// <summary>A material whose content is not the lock's is red, and wears the change badge.</summary>
    [Fact]
    public void AChangedMaterialIsRedAndWearsTheBadge()
    {
        var bin =
            Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
            ?? throw new InvalidOperationException("the suites are run with ROLA_BIN_DIR");
        var directory = Scratch.New("git-rola-wrong");

        Init(directory);

        File.WriteAllText(
            Path.Combine(directory, "rola.toml"),
            "[vaults]\norigin = \"rola://127.0.0.1/rainbow\"\n\n[features]\ngit_support = true\n"
        );
        File.WriteAllText(
            Path.Combine(directory, "rola.lock"),
            "? FORMAT 2\n/hero.psd blake3:0000000000000000000000000000000000000000000000000000000000000000 file\n"
        );
        File.WriteAllText(Path.Combine(directory, "hero.psd"), "not the lock's\n");

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystemPlugin.FileSystemPlugin.Identity), 1);
        var previous = BaseDirectory.Current;

        _ = new Shared(directory, new HideRegistry(host.Config), host.Config, new FileChanges());

        var program = Environment.GetEnvironmentVariable(GitRolaPrograms.GitVariable);

        Environment.SetEnvironmentVariable(
            GitRolaPrograms.GitVariable,
            Path.Combine(bin, OperatingSystem.IsWindows() ? "git-rola.exe" : "git-rola")
        );

        try
        {
            var check = new GitRolaCheck(host);
            var colour = new GitRolaColour(host.Palette, check);
            var badge = new GitRolaBadge(check, host.Palette);
            var entry = new Entry(Path.Combine(directory, "hero.psd"), EntryKind.File);
            Color? marked = null;

            for (var attempt = 0; attempt < 200; attempt++)
            {
                marked = colour.Colour(entry);

                if (marked is not null && marked.Value == host.Palette.Semantic.Error)
                {
                    break;
                }

                Thread.Sleep(50);
            }

            var root = GitRolaToml.ProjectOf(entry.Path);

            Assert.True(
                marked is not null && marked.Value == host.Palette.Semantic.Error,
                $"wrong={check.Wrong(entry.Path)} root={root} answer={string.Join('|', check.AnswerFor(root ?? string.Empty) ?? ["<none>"])} lock={File.Exists(Path.Combine(directory, "rola.lock"))} exe={GitRolaPrograms.Git} project={GitRolaToml.IsProject(directory)}"
            );
            Assert.Equal(GitVCSPlugin.Icons.NotePencil, badge.Mark(entry).IconKey);
        }
        finally
        {
            Environment.SetEnvironmentVariable(GitRolaPrograms.GitVariable, program);

            if (previous is not null)
            {
                BaseDirectory.Publish(previous);
            }
        }
    }

    /// <summary>The directory as a git repository the check can read.</summary>
    /// <param name="directory">The directory.</param>
    private static void Init(string directory)
    {
        var start = new ProcessStartInfo
        {
            FileName = "git",
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };

        start.ArgumentList.Add("-C");
        start.ArgumentList.Add(directory);
        start.ArgumentList.Add("init");
        start.ArgumentList.Add("-b");
        start.ArgumentList.Add("main");

        using var process = Process.Start(start) ?? throw new InvalidOperationException("git was not started");

        process.WaitForExit();

        if (process.ExitCode != 0)
        {
            throw new InvalidOperationException(process.StandardError.ReadToEnd());
        }
    }

    /// <summary>The Git logomark and the Open picture travel in the plugin's assembly.</summary>
    [Fact]
    public void TheGitPicturesAreContributed()
    {
        var library = new IconLibrary();

        GitVCSPlugin.Icons.Declare(library);

        Assert.NotNull(library.Find(GitRolaActions.Category));
        Assert.NotNull(library.Find(GitVCSPlugin.Icons.ArrowDown));
    }

    /// <summary>The lock names which of its materials are trees.</summary>
    [Fact]
    public void TheLockNamesItsTrees()
    {
        var trees = GitRolaLock.Trees(
            "? FORMAT 2\n? VAULT origin rola://127.0.0.1/rainbow\n/Tree blake3:1111 tree\n/hero.psd blake3:0000 file\n"
        );

        Assert.Contains("Tree", trees);
        Assert.DoesNotContain("hero.psd", trees);
    }

    /// <summary>A tree material is offered the restore, and a directory that is not one is not.</summary>
    [Fact]
    public void ATreeMaterialIsOfferedTheRestore()
    {
        var directory = Scratch.New("tree-menu");

        Directory.CreateDirectory(Path.Combine(directory, ".git"));
        File.WriteAllText(
            Path.Combine(directory, "rola.toml"),
            "[vaults]\norigin = \"rola://127.0.0.1/rainbow\"\n"
        );
        File.WriteAllText(
            Path.Combine(directory, "rola.lock"),
            "? FORMAT 2\n/Tree blake3:1111111111111111111111111111111111111111111111111111111111111111 tree\n"
        );

        Assert.True(
            GitRolaActions.TreeInProject(new ContextTarget(Path.Combine(directory, "Tree"), []))
        );
        Assert.True(
            GitRolaActions.TreeInProject(
                new ContextTarget(directory, [new Entry(Path.Combine(directory, "Tree"), EntryKind.Directory)])
            )
        );
        Assert.False(
            GitRolaActions.TreeInProject(new ContextTarget(Path.Combine(directory, "Other"), []))
        );
    }

    /// <summary>A check's short status becomes the paths it named, quoted or not.</summary>
    [Fact]
    public void TheCheckLinesBecomePaths()
    {
        var changed = GitRolaCheck.Parse(
            " M Assets/hero.psd\nM Assets/second.psd\n D \"\\346\\226\\260\\346\\240\\221/a.psd\"\n"
        );

        Assert.Contains("Assets/hero.psd", changed);
        Assert.Contains("Assets/second.psd", changed);
        Assert.Contains("新树/a.psd", changed);
    }

    /// <summary>A choice travels as one object per row, which is what a drop reads.</summary>
    [Fact]
    public void AChoiceTravelsAsOneObjectPerRow()
    {
        var first = new GitRolaCarry("origin", "Assets/a.psd");
        var second = new GitRolaCarry("origin", "Assets/b.psd");

        Assert.Single(GitRolaDrops.Carried(first));
        Assert.Equal(new[] { first, second }, GitRolaDrops.Carried(new object[] { first, second }));
        Assert.Empty(GitRolaDrops.Carried("something else"));
    }

    /// <summary>A level is the next name under a prefix, folders first and then by name.</summary>
    [Fact]
    public void ALevelIsFoldersFirstAndThenNames()
    {
        var entries = new[]
        {
            new GitRolaUpstreamEntry("Assets/hero.psd", false),
            new GitRolaUpstreamEntry("Source/c.fbx", false),
            new GitRolaUpstreamEntry("Assets/Textures", true),
        };

        var top = GitRolaUpstream.Level(entries, "");
        var assets = GitRolaUpstream.Level(entries, "Assets");

        Assert.Equal(new[] { "Assets", "Source" }, top.Select(row => row.Name));
        Assert.True(top[0].Folder);

        // A tree is one entry of the Layout, so it is drawn as a file the way rola@ draws one: a
        // thing to bring in rather than a level to open.
        Assert.Equal(new[] { "hero.psd", "Textures" }, assets.Select(row => row.Name));
        Assert.False(assets[0].Folder);
        Assert.False(assets[1].Folder);
        Assert.Equal("Assets/Textures", assets[1].Path);
    }
}

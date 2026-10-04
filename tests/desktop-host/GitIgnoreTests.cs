using System.Diagnostics;
using FileSystemPlugin;
using GitVCSPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Git plugin's answers about what a repository ignores.
/// </summary>
/// <remarks>
/// A real repository is made in a scratch directory and the real <c>git</c> program is asked, so what is
/// checked is the answer a listing gets rather than a stand-in for it. The program is the whole point of
/// the provider — the rules are Git's and are not reimplemented — so a test that did not run it would test
/// nothing that matters.
/// </remarks>
public sealed class GitIgnoreTests
{
    /// <summary>Begins each test from no configuration at all, and from no provider registered.</summary>
    public GitIgnoreTests()
    {
        DataHome.Clean();
        HideRegistry.Clear();
    }

    /// <summary>
    /// What <c>.gitignore</c> leaves out is ignored, what it does not is not, and a directory no repository
    /// holds is asked of no program — so nothing is ignored there.
    /// </summary>
    [Fact]
    public void GitIgnoresWhatARepositoryIgnores()
    {
        var repository = Scratch.New("git");

        Given(repository, "git", ["init", "--quiet"]);
        File.WriteAllText(Path.Combine(repository, ".gitignore"), "*.tmp\nbuild/\n");
        File.WriteAllText(Path.Combine(repository, "kept.txt"), "x");
        File.WriteAllText(Path.Combine(repository, "left.tmp"), "x");
        Directory.CreateDirectory(Path.Combine(repository, "build"));

        Assert.True(GitIgnores.Hides(Path.Combine(repository, "left.tmp")));
        Assert.True(GitIgnores.Hides(Path.Combine(repository, "build")));
        Assert.False(GitIgnores.Hides(Path.Combine(repository, "kept.txt")));
        Assert.False(GitIgnores.Hides(Path.Combine(repository, ".gitignore")));

        // A directory outside any repository has no rules to meet, which is what the walk up answers before
        // the program is ever run.
        var outside = Scratch.New("git-outside");
        File.WriteAllText(Path.Combine(outside, "left.tmp"), "x");

        Assert.False(GitIgnores.Hides(Path.Combine(outside, "left.tmp")));
    }

    /// <summary>
    /// A directory Git ignores is hidden whether or not it holds anything, and so is what it holds.
    /// </summary>
    /// <remarks>
    /// The answer about a directory is its parent's list to give, and reading the directory itself to find
    /// it out is what once made an ignored directory visible as soon as something was put in it.
    /// </remarks>
    [Fact]
    public void AnIgnoredDirectoryIsHiddenWithWhatItHolds()
    {
        var repository = Scratch.New("git-directory");
        Given(repository, "git", ["init", "--quiet"]);
        File.WriteAllText(Path.Combine(repository, ".gitignore"), "build/\n");

        var build = Path.Combine(repository, "build");
        Directory.CreateDirectory(build);
        File.WriteAllText(Path.Combine(build, "kept.txt"), "x");

        Assert.True(GitIgnores.Hides(build));
        Assert.True(GitIgnores.Hides(Path.Combine(build, "kept.txt")));
    }

    /// <summary>
    /// The rules are read against the root a view is rooted at, and a root no repository holds has none.
    /// </summary>
    /// <remarks>
    /// A tree is rooted at a base: a base outside every repository is one under which nothing is ignored,
    /// whatever repositories the directories under it happen to be inside.
    /// </remarks>
    [Fact]
    public void NothingIsIgnoredUnderARootNoRepositoryHolds()
    {
        var repository = Scratch.New("git-rooted");
        Given(repository, "git", ["init", "--quiet"]);
        File.WriteAllText(Path.Combine(repository, ".gitignore"), "*.tmp\n");
        File.WriteAllText(Path.Combine(repository, "left.tmp"), "x");

        var outside = Scratch.New("git-rooted-outside");

        Assert.True(GitIgnores.Hides(Path.Combine(repository, "left.tmp"), repository));
        Assert.False(GitIgnores.Hides(Path.Combine(repository, "left.tmp"), outside));
    }

    /// <summary>
    /// A repository inside the root is read by the root's rules rather than answering for itself.
    /// </summary>
    [Fact]
    public void ARepositoryInsideTheRootDoesNotAnswerForItself()
    {
        var outer = Scratch.New("git-outer");
        Given(outer, "git", ["init", "--quiet"]);
        File.WriteAllText(Path.Combine(outer, ".gitignore"), "inner/\n");

        var inner = Path.Combine(outer, "inner");
        Directory.CreateDirectory(inner);
        Given(inner, "git", ["init", "--quiet"]);

        // Rooted at the outer, the nested repository is an entry the outer ignores; rooted at itself, it is
        // a repository with no rules of its own and nothing is hidden.
        Assert.True(GitIgnores.Hides(inner, outer));
        Assert.False(GitIgnores.Hides(inner, inner));
    }

    /// <summary>
    /// A rule written after an answer was asked for is not seen until what was asked is let go of, which is
    /// what a listing does when the files may have changed.
    /// </summary>
    /// <remarks>
    /// The rules are Git's and are asked of the program once per directory and kept, so the keeping is what
    /// makes asking worthwhile and also what goes stale: a <c>.gitignore</c> is a file, and editing one is a
    /// change no answer read before it can know about. What is checked is the whole of the way back — the
    /// listing's own `Shared` is what says the files may have changed, and the provider in force is what has
    /// to let go of its answer.
    /// </remarks>
    [Fact]
    public void AChangedIgnoreFileIsSeenOnceTheFilesMayHaveChanged()
    {
        var repository = Scratch.New("git-changed");
        Given(repository, "git", ["init", "--quiet"]);
        File.WriteAllText(Path.Combine(repository, ".gitignore"), "*.tmp\n");

        var kept = Path.Combine(repository, "kept.txt");
        File.WriteAllText(kept, "x");

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);

        // The real provider, registered the way the Git plugin registers it. Registering one already in the
        // catalogue adds nothing, so a test that ran before is no reason for this one to fail.
        HideRegistry.Register(new GitIgnored());

        var shared = new Shared(repository, new HideRegistry(host.Config), host.Config);
        var entry = new Entry(kept, EntryKind.File);

        Assert.False(shared.Hides(entry));

        // The rule now covers the file: what was asked is kept, so the listing goes on saying it is not
        // ignored — which is the staleness, stated.
        File.WriteAllText(Path.Combine(repository, ".gitignore"), "*.txt\n");
        Assert.False(shared.Hides(entry));

        // Saying the files may have changed is what a file operation and coming back to the window both say.
        shared.FilesChanged();

        Assert.True(shared.Hides(entry));
    }

    /// <summary>Runs one command in a directory, failing the test when it does.</summary>
    /// <param name="directory">Where to run it.</param>
    /// <param name="program">What to run.</param>
    /// <param name="arguments">What to run it with.</param>
    private static void Given(string directory, string program, string[] arguments)
    {
        var start = new ProcessStartInfo(program)
        {
            WorkingDirectory = directory,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        using var command = Process.Start(start)!;
        command.WaitForExit();

        Assert.Equal(0, command.ExitCode);
    }
}

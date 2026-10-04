using System.Diagnostics;
using GitVCSPlugin;

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
    /// <summary>Begins each test from no configuration at all.</summary>
    public GitIgnoreTests() => DataHome.Clean();

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

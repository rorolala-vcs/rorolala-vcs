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

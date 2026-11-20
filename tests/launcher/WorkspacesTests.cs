using RorolalaLauncher;

namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// What a directory on the launcher's list is: a Workspace, a git-rola project, or neither.
/// </summary>
/// <remarks>
/// The test is what the import and the opening both ask, so a project being importable and unable to
/// open would be one answer given twice rather than two. A git-rola project is not a Workspace: what
/// it holds is `rola.toml` beside a `.git`, and the Desktop's git-rola view is what reads it.
/// </remarks>
public sealed class WorkspacesTests
{
    /// <summary>A directory holding the data directory is a Workspace.</summary>
    [Fact]
    public void ADirectoryWithADataDirectoryIsAWorkspace()
    {
        var directory = Scratch.New("workspace");
        Directory.CreateDirectory(Path.Combine(directory, Workspaces.DataDirectory));

        Assert.Equal(WorkspaceState.Workspace, Workspaces.Of(directory));
    }

    /// <summary>A git repository holding a project file is a git-rola project.</summary>
    [Fact]
    public void AGitRepositoryWithAProjectFileIsAGitProject()
    {
        var directory = Scratch.New("git-project");
        Directory.CreateDirectory(Path.Combine(directory, ".git"));
        File.WriteAllText(Path.Combine(directory, Workspaces.ProjectFile), string.Empty);

        Assert.Equal(WorkspaceState.GitProject, Workspaces.Of(directory));
    }

    /// <summary>A directory that is both a Workspace and a git-rola project still wears the Git mark.</summary>
    [Fact]
    public void AWorkspaceThatIsAlsoAGitProjectIsStillGit()
    {
        var directory = Scratch.New("git-workspace");
        Directory.CreateDirectory(Path.Combine(directory, Workspaces.DataDirectory));
        Directory.CreateDirectory(Path.Combine(directory, ".git"));
        File.WriteAllText(Path.Combine(directory, Workspaces.ProjectFile), string.Empty);

        Assert.Equal(WorkspaceState.Workspace, Workspaces.Of(directory));
        Assert.True(Workspaces.IsGit(directory));
    }

    /// <summary>A repository with no project file has nothing for the view to read.</summary>
    [Fact]
    public void ARepositoryWithoutAProjectFileIsNotOne()
    {
        var directory = Scratch.New("no-project");
        Directory.CreateDirectory(Path.Combine(directory, ".git"));

        Assert.Equal(WorkspaceState.NotWorkspace, Workspaces.Of(directory));
    }

    /// <summary>A project file outside a repository places nothing, so it is not one either.</summary>
    [Fact]
    public void AProjectFileOutsideARepositoryIsNotOne()
    {
        var directory = Scratch.New("no-git");
        File.WriteAllText(Path.Combine(directory, Workspaces.ProjectFile), string.Empty);

        Assert.Equal(WorkspaceState.NotWorkspace, Workspaces.Of(directory));
    }

    /// <summary>A directory that is not there is missing rather than something else.</summary>
    [Fact]
    public void ADirectoryThatIsNotThereIsMissing()
    {
        var directory = Path.Combine(Scratch.New("missing"), "gone");

        Assert.Equal(WorkspaceState.Missing, Workspaces.Of(directory));
    }
}

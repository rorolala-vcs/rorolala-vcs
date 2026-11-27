using RolaSharp;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The ignore rules a Workspace and its Vaults keep, as the Workspace dock reads and writes them.
/// </summary>
/// <remarks>
/// A rule file is a plain text file at a path the library names, and the reading is the lines rather
/// than a parse of them: what a line means is the loader's. What is checked here is that the file the
/// dock writes is the file the loader will read, and that the Vault copies are found where a fetch
/// leaves them, with a fetch's staging directory left out.
/// </remarks>
public sealed class WorkspaceRulesTests
{
    /// <summary>The Workspace's own rules are read from its file and written back to it.</summary>
    [Fact]
    public void AWorkspacesRulesAreReadAndWritten()
    {
        var root = Scratch.New("workspace-rules");
        var file = RolaPaths.WorkspaceIgnoreRuleFile(root);

        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
        File.WriteAllText(file, "# note\n*.tmp\n");

        var rules = WorkspaceRules.Read(root);

        Assert.Equal(["# note", "*.tmp"], rules.Lines);
        Assert.Empty(rules.Vaults);
        Assert.EndsWith("default", rules.Shown, StringComparison.Ordinal);

        rules.Write(["*.tmp", "!keep.tmp"]);

        Assert.Equal("*.tmp\n!keep.tmp\n", File.ReadAllText(file));
        Assert.Equal(["*.tmp", "!keep.tmp"], WorkspaceRules.Read(root).Lines);
    }

    /// <summary>A Workspace with no rule file reads as stating none, which is not a failure.</summary>
    [Fact]
    public void AWorkspaceThatNeverWroteARuleFileStatesNoRules()
    {
        var root = Scratch.New("workspace-rules-none");

        Assert.Empty(WorkspaceRules.Read(root).Lines);
    }

    /// <summary>A Vault's rules are read from the copy a fetch leaves, and a staging copy is left out.</summary>
    [Fact]
    public void AVaultsRulesAreReadFromTheCopyAFetchLeaves()
    {
        var root = Scratch.New("workspace-rules-vaults");
        var vaults = RolaPaths.WorkspaceIgnoreRuleVaults(root);
        var maya = Path.Combine(vaults, "maya");
        var staging = Path.Combine(vaults, ".maya.staging");

        Directory.CreateDirectory(maya);
        Directory.CreateDirectory(staging);
        File.WriteAllText(Path.Combine(maya, "default-ignore-rule"), "# note\n*.mb\n!keep.mb\n");
        File.WriteAllText(Path.Combine(staging, "default-ignore-rule"), "*.stale\n");

        var rules = WorkspaceRules.Read(root);

        var vault = Assert.Single(rules.Vaults);

        Assert.Equal("maya", vault.Name);
        Assert.Equal(["*.mb", "!keep.mb"], vault.Patterns);
    }
}
